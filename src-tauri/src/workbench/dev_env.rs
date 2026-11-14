//! Windows 11 developer toolchain discovery (`scan_dev_env`).
//!
//! Probes common CLI toolchains concurrently with `CREATE_NO_WINDOW` (`0x0800_0000`)
//! so GUI workbench refreshes and background AI agent checks never flash console windows.

use std::{
    borrow::Cow,
    collections::HashSet,
    process::Stdio,
    time::{Duration, Instant},
};

use chrono::Utc;
use rmcp::{
    handler::server::router::tool::{AsyncTool, ToolBase},
    ErrorData as McpError,
};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::keel_core::KeelMcpServer;

use super::{get_win11_host_summary, Win11HostSummary};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_VERSION_CHARS: usize = 160;

pub const SCAN_DEV_ENV_TOOL_NAME: &str = "scan_dev_env";
pub const SCAN_DEV_ENV_TOOL_DESCRIPTION: &str =
    "Probe installed Windows 11 developer toolchains (versions, executable paths, and latency) without flashing console windows.";

#[derive(Debug, Clone, Copy)]
struct DevToolSpec {
    id: &'static str,
    display_name: &'static str,
    binary: &'static str,
}

const DEFAULT_DEV_TOOLS: &[DevToolSpec] = &[
    DevToolSpec {
        id: "git",
        display_name: "Git",
        binary: "git",
    },
    DevToolSpec {
        id: "node",
        display_name: "Node.js",
        binary: "node",
    },
    DevToolSpec {
        id: "pnpm",
        display_name: "pnpm",
        binary: "pnpm",
    },
    DevToolSpec {
        id: "rustc",
        display_name: "Rust (rustc)",
        binary: "rustc",
    },
    DevToolSpec {
        id: "cargo",
        display_name: "Cargo",
        binary: "cargo",
    },
    DevToolSpec {
        id: "python",
        display_name: "Python",
        binary: "python",
    },
    DevToolSpec {
        id: "uv",
        display_name: "uv",
        binary: "uv",
    },
    DevToolSpec {
        id: "pwsh",
        display_name: "PowerShell 7",
        binary: "pwsh",
    },
    DevToolSpec {
        id: "winget",
        display_name: "WinGet",
        binary: "winget",
    },
    DevToolSpec {
        id: "wsl",
        display_name: "WSL",
        binary: "wsl",
    },
];

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct ScanDevEnvParams {
    /// Optional filter of tool IDs (e.g. `["git", "cargo", "pnpm"]`). When omitted or empty, scans all default tools.
    #[serde(default)]
    pub tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
pub struct DevToolProbe {
    pub id: String,
    pub display_name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub executable_path: Option<String>,
    pub probe_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
pub struct DevEnvScanReport {
    pub scanned_at_iso: String,
    pub host: Win11HostSummary,
    pub path_entry_count: u32,
    pub installed_count: u32,
    pub probes: Vec<DevToolProbe>,
}

pub struct ScanDevEnvTool;

impl ToolBase for ScanDevEnvTool {
    type Parameter = ScanDevEnvParams;
    type Output = DevEnvScanReport;
    type Error = McpError;

    fn name() -> Cow<'static, str> {
        SCAN_DEV_ENV_TOOL_NAME.into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some(SCAN_DEV_ENV_TOOL_DESCRIPTION.into())
    }
}

impl AsyncTool<KeelMcpServer> for ScanDevEnvTool {
    async fn invoke(
        _service: &KeelMcpServer,
        param: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        run_dev_env_scan(param.tools)
            .await
            .map_err(|msg| McpError::invalid_params(msg, None))
    }
}

#[derive(Debug, Clone)]
struct ResolvedProbeTarget {
    id: String,
    display_name: String,
    binary: String,
}

fn is_safe_tool_identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn resolve_probe_targets(filter: Option<Vec<String>>) -> Result<Vec<ResolvedProbeTarget>, String> {
    let normalized_requests: Vec<String> = filter
        .unwrap_or_default()
        .into_iter()
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect();

    if normalized_requests.is_empty() {
        return Ok(DEFAULT_DEV_TOOLS
            .iter()
            .map(|spec| ResolvedProbeTarget {
                id: spec.id.to_string(),
                display_name: spec.display_name.to_string(),
                binary: spec.binary.to_string(),
            })
            .collect());
    }

    let mut seen = HashSet::new();
    let mut targets = Vec::with_capacity(normalized_requests.len());

    for req_id in normalized_requests {
        if !is_safe_tool_identifier(&req_id) {
            return Err(format!(
                "Invalid tool identifier '{req_id}': only ASCII letters, digits, '-', '_', and '.' are permitted."
            ));
        }
        if !seen.insert(req_id.clone()) {
            continue;
        }

        if let Some(known) = DEFAULT_DEV_TOOLS.iter().find(|spec| spec.id == req_id) {
            targets.push(ResolvedProbeTarget {
                id: known.id.to_string(),
                display_name: known.display_name.to_string(),
                binary: known.binary.to_string(),
            });
        } else {
            targets.push(ResolvedProbeTarget {
                id: req_id.clone(),
                display_name: req_id.clone(),
                binary: req_id,
            });
        }
    }

    Ok(targets)
}

fn count_path_entries() -> u32 {
    std::env::var_os("PATH")
        .map(|val| {
            std::env::split_paths(&val)
                .filter(|entry| !entry.as_os_str().is_empty())
                .count()
                .min(u32::MAX as usize) as u32
        })
        .unwrap_or(0)
}

/// Decode raw CLI output bytes, transparently handling UTF-16LE (with or without BOM,
/// such as `wsl.exe --version` on Windows 11) as well as UTF-8.
pub(crate) fn decode_cli_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    if let Some(stripped) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16le_slice(stripped);
    }

    if let Some(stripped) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(stripped)
            .replace('\0', "")
            .trim_start_matches('\u{feff}')
            .to_string();
    }

    if looks_like_utf16le(bytes) {
        return decode_utf16le_slice(bytes);
    }

    String::from_utf8_lossy(bytes)
        .replace('\0', "")
        .trim_start_matches('\u{feff}')
        .to_string()
}

fn decode_utf16le_slice(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|&unit| unit != 0)
        .collect();
    String::from_utf16_lossy(&units)
        .trim_start_matches('\u{feff}')
        .to_string()
}

fn looks_like_utf16le(bytes: &[u8]) -> bool {
    if bytes.len() < 2 || !bytes.contains(&0) {
        return false;
    }

    let sample = &bytes[..bytes.len().min(128)];
    let pair_count = sample.len() / 2;
    if pair_count == 0 {
        return false;
    }

    let mut even_zeros = 0usize;
    let mut odd_zeros = 0usize;
    for chunk in sample.chunks_exact(2) {
        if chunk[0] == 0 {
            even_zeros += 1;
        }
        if chunk[1] == 0 {
            odd_zeros += 1;
        }
    }

    odd_zeros > even_zeros && odd_zeros.saturating_mul(4) >= pair_count
}

/// Extract the concise first non-empty version line from stdout (falling back to stderr).
pub(crate) fn extract_version_line(stdout: &[u8], stderr: &[u8]) -> Option<String> {
    let primary = decode_cli_output(stdout);
    if let Some(line) = first_concise_line(&primary) {
        return Some(line);
    }

    let fallback = decode_cli_output(stderr);
    first_concise_line(&fallback)
}

fn first_concise_line(decoded: &str) -> Option<String> {
    let raw_line = decoded
        .lines()
        .map(|line| line.trim().trim_start_matches('\u{feff}'))
        .find(|line| !line.is_empty())?;

    let mut chars = raw_line.chars();
    let head: String = chars.by_ref().take(MAX_VERSION_CHARS).collect();
    if chars.next().is_some() {
        Some(format!(
            "{}…",
            head.chars()
                .take(MAX_VERSION_CHARS.saturating_sub(1))
                .collect::<String>()
        ))
    } else {
        Some(head)
    }
}

/// Pick the most directly executable Windows binary path from `where.exe` output,
/// preferring `.exe` (non-WindowsApps stub first when multiple `.exe` candidates exist),
/// then `.cmd`/`.bat`, then `.com`, and finally any remaining candidate.
pub(crate) fn select_preferred_executable_path(where_stdout: &str) -> Option<String> {
    let candidates: Vec<&str> = where_stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.contains('"'))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    let is_windows_apps = |p: &str| p.to_ascii_lowercase().contains(r"\microsoft\windowsapps\");
    let has_ext = |p: &str, exts: &[&str]| {
        let lower = p.to_ascii_lowercase();
        exts.iter().any(|ext| lower.ends_with(ext))
    };

    if let Some(&non_stub_exe) = candidates
        .iter()
        .find(|&&p| has_ext(p, &[".exe"]) && !is_windows_apps(p))
    {
        return Some(non_stub_exe.to_string());
    }

    if let Some(&any_exe) = candidates.iter().find(|&&p| has_ext(p, &[".exe", ".com"])) {
        return Some(any_exe.to_string());
    }

    if let Some(&cmd_shim) = candidates.iter().find(|&&p| has_ext(p, &[".cmd", ".bat"])) {
        return Some(cmd_shim.to_string());
    }

    Some(candidates[0].to_string())
}

#[cfg(target_os = "windows")]
fn new_hidden_std_command(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;

    let mut cmd = std::process::Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd
}

#[cfg(not(target_os = "windows"))]
fn new_hidden_std_command(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd
}

async fn resolve_executable_via_where(binary: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    let locator = "where.exe";
    #[cfg(not(target_os = "windows"))]
    let locator = "which";

    let mut std_cmd = new_hidden_std_command(locator);
    std_cmd.arg(binary);

    let mut cmd = Command::from(std_cmd);
    cmd.kill_on_drop(true);

    let output = cmd.output().await.ok()?;
    if !output.status.success() {
        return None;
    }

    let decoded = decode_cli_output(&output.stdout);
    select_preferred_executable_path(&decoded)
}

async fn run_version_command(exe_path: &str) -> Option<std::process::Output> {
    let lower = exe_path.to_ascii_lowercase();
    let is_batch_script = lower.ends_with(".cmd") || lower.ends_with(".bat");

    #[cfg(target_os = "windows")]
    let std_cmd = if is_batch_script {
        use std::os::windows::process::CommandExt;

        let mut cmd = new_hidden_std_command("cmd.exe");
        cmd.arg("/d").arg("/s").arg("/c");
        // `cmd.exe /s /c` strips the outermost quote pair and preserves inner quotes verbatim.
        cmd.raw_arg(format!("\"\"{exe_path}\" --version\""));
        cmd
    } else {
        let mut cmd = new_hidden_std_command(exe_path);
        cmd.arg("--version");
        cmd
    };

    #[cfg(not(target_os = "windows"))]
    let std_cmd = {
        let _ = is_batch_script;
        let mut cmd = new_hidden_std_command(exe_path);
        cmd.arg("--version");
        cmd
    };

    let mut cmd = Command::from(std_cmd);
    cmd.kill_on_drop(true);
    cmd.output().await.ok()
}

async fn probe_single_tool(target: ResolvedProbeTarget) -> DevToolProbe {
    let started = Instant::now();

    let probe_outcome = tokio::time::timeout(PROBE_TIMEOUT, async {
        let exe_path = resolve_executable_via_where(&target.binary).await?;
        let output = run_version_command(&exe_path).await?;
        if !output.status.success() {
            return None;
        }
        let version = extract_version_line(&output.stdout, &output.stderr);
        Some((exe_path, version))
    })
    .await;

    let probe_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;

    match probe_outcome {
        Ok(Some((exe_path, version))) => DevToolProbe {
            id: target.id,
            display_name: target.display_name,
            installed: true,
            version,
            executable_path: Some(exe_path),
            probe_ms,
        },
        Ok(None) | Err(_) => DevToolProbe {
            id: target.id,
            display_name: target.display_name,
            installed: false,
            version: None,
            executable_path: None,
            probe_ms,
        },
    }
}

pub async fn run_dev_env_scan(filter: Option<Vec<String>>) -> Result<DevEnvScanReport, String> {
    let targets = resolve_probe_targets(filter)?;
    let mut handles = Vec::with_capacity(targets.len());

    for target in targets {
        let fallback_id = target.id.clone();
        let fallback_name = target.display_name.clone();
        let handle = tokio::spawn(probe_single_tool(target));
        handles.push((fallback_id, fallback_name, handle));
    }

    let mut probes = Vec::with_capacity(handles.len());
    for (fallback_id, fallback_name, handle) in handles {
        match handle.await {
            Ok(probe) => probes.push(probe),
            Err(_) => probes.push(DevToolProbe {
                id: fallback_id,
                display_name: fallback_name,
                installed: false,
                version: None,
                executable_path: None,
                probe_ms: 0,
            }),
        }
    }

    let installed_count = probes.iter().filter(|p| p.installed).count() as u32;

    Ok(DevEnvScanReport {
        scanned_at_iso: Utc::now().to_rfc3339(),
        host: get_win11_host_summary(),
        path_entry_count: count_path_entries(),
        installed_count,
        probes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_utf16le(text: &str, with_bom: bool) -> Vec<u8> {
        let mut out = Vec::new();
        if with_bom {
            out.extend_from_slice(&[0xFF, 0xFE]);
        }
        for unit in text.encode_utf16() {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        out
    }

    #[test]
    fn parses_wsl_utf16le_output_with_and_without_bom() {
        let raw_wsl =
            "WSL version: 2.4.11.0\r\nKernel version: 5.15.167.4-1\r\nWSLg version: 1.0.65\r\n";

        let bomless_bytes = encode_utf16le(raw_wsl, false);
        assert_eq!(
            extract_version_line(&bomless_bytes, &[]).as_deref(),
            Some("WSL version: 2.4.11.0")
        );

        let bom_bytes = encode_utf16le(
            "\r\n  WSL 版本: 2.4.11.0  \r\n内核版本: 5.15.167.4-1\r\n",
            true,
        );
        assert_eq!(
            extract_version_line(&bom_bytes, &[]).as_deref(),
            Some("WSL 版本: 2.4.11.0")
        );
    }

    #[test]
    fn parses_utf8_multiline_cli_banners_and_stderr_fallback() {
        let multiline_banner =
            b"\n\r\n  git version 2.48.1.windows.1  \r\nextra diagnostic line\r\n";
        assert_eq!(
            extract_version_line(multiline_banner, &[]).as_deref(),
            Some("git version 2.48.1.windows.1")
        );

        let stderr_only = b"Python 3.13.2\r\n";
        assert_eq!(
            extract_version_line(&[], stderr_only).as_deref(),
            Some("Python 3.13.2")
        );
    }

    #[test]
    fn prefers_cmd_or_exe_over_extensionless_npm_shim_and_store_stub() {
        let npm_where = "C:\\Users\\dev\\AppData\\Roaming\\npm\\pnpm\r\nC:\\Users\\dev\\AppData\\Roaming\\npm\\pnpm.cmd\r\n";
        assert_eq!(
            select_preferred_executable_path(npm_where).as_deref(),
            Some("C:\\Users\\dev\\AppData\\Roaming\\npm\\pnpm.cmd")
        );

        let python_where = "C:\\Users\\dev\\AppData\\Local\\Microsoft\\WindowsApps\\python.exe\r\nC:\\Python313\\python.exe\r\n";
        assert_eq!(
            select_preferred_executable_path(python_where).as_deref(),
            Some("C:\\Python313\\python.exe")
        );
    }

    #[tokio::test]
    async fn live_scan_dev_env_filters_cargo_and_git_without_console_flash() {
        let report = run_dev_env_scan(Some(vec![
            "cargo".to_string(),
            "git".to_string(),
            "pnpm".to_string(),
        ]))
        .await
        .expect("filtered dev-env scan should succeed");

        assert_eq!(report.probes.len(), 3);
        assert_eq!(report.probes[0].id, "cargo");
        assert_eq!(report.probes[1].id, "git");
        assert_eq!(report.probes[2].id, "pnpm");
        assert!(report.path_entry_count > 0);

        let cargo_probe = &report.probes[0];
        assert!(
            cargo_probe.installed,
            "cargo must be installed in test runner"
        );
        assert!(
            cargo_probe
                .version
                .as_deref()
                .is_some_and(|v| v.to_ascii_lowercase().contains("cargo")),
            "unexpected cargo version: {:?}",
            cargo_probe.version
        );
        assert!(cargo_probe.executable_path.is_some());
        assert!(report.installed_count >= 1);

        // Rejects unsafe shell characters in custom tool filter IDs.
        let bad_err = run_dev_env_scan(Some(vec!["git && calc".to_string()]))
            .await
            .expect_err("unsafe tool identifier must fail validation");
        assert!(bad_err.contains("Invalid tool identifier"));
    }
}
