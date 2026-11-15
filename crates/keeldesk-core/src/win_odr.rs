//! Windows 11 On-Device Agent Registry (`odr.exe`) discovery, enrollment, and MCPB `"0.3"` manifest generation.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use chrono::Utc;
use rmcp::model::Tool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::process::Command;

use super::{
    mcp_server::{KeelSharedState, KEEL_MCP_INSTRUCTIONS},
    win32_env::{
        build_merged_windows_path, decode_cli_bytes, new_hidden_std_command,
        resolve_executable_in_path, Win11OdrHostProfile,
    },
};

const ODR_PROBE_TIMEOUT: Duration = Duration::from_secs(4);
const ODR_ACTION_TIMEOUT: Duration = Duration::from_secs(8);
const MCPB_MANIFEST_VERSION: &str = "0.3";
const MCP_PROTOCOL_VERSION: &str = "2025-03-26";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum OdrReadinessState {
    Ready,
    MissingOdrCli,
    UnsupportedOs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum OdrRegisterMode {
    Uri,
    Manifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct OdrEnrolledServer {
    pub identifier: String,
    pub display_line: String,
    pub matches_keel: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct OdrStatusReport {
    pub probed_at_iso: String,
    pub readiness: OdrReadinessState,
    pub is_windows_11: bool,
    pub os_build: u32,
    pub os_ubr: u32,
    pub display_version: String,
    pub meets_odr_build_hint: bool,
    pub developer_mode_enabled: bool,
    pub odr_installed: bool,
    pub odr_binary_path: Option<String>,
    pub odr_version: Option<String>,
    pub keel_enrolled: bool,
    pub enrolled_servers: Vec<OdrEnrolledServer>,
    pub raw_list_output: Option<String>,
    pub uri_register_command: String,
    pub manifest_register_command: String,
    pub unregister_command: String,
    pub list_command: String,
    pub default_manifest_path: String,
    pub manifest_json: String,
    pub guidance_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct OdrActionOutcome {
    pub executed_cli: bool,
    pub succeeded: bool,
    pub mode: String,
    pub command_line: String,
    pub exported_manifest_path: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub message: String,
    pub status: OdrStatusReport,
}

pub(crate) fn default_manifest_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .filter(|val| !val.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);

    base.join("KeelDesk")
        .join("odr")
        .join("keeldesk.mcpb.manifest.json")
}

pub(crate) fn build_keel_odr_manifest_value(
    server_name: &str,
    server_version: &str,
    endpoint_url: &str,
    mcp_tools: &[Tool],
) -> Value {
    let current_exe_path = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "keeldesk.exe".to_string());

    let entry_point = Path::new(&current_exe_path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("keeldesk.exe")
        .to_string();

    let tools_json = serde_json::to_value(mcp_tools).unwrap_or_else(|_| json!([]));

    json!({
        "manifest_version": MCPB_MANIFEST_VERSION,
        "name": server_name,
        "version": server_version,
        "display_name": "KeelDesk",
        "description": "KeelDesk Windows 11 local desktop MCP runtime spine",
        "author": {
            "name": "Leon Zeng"
        },
        "server": {
            "type": "binary",
            "entry_point": entry_point,
            "mcp_config": {
                "command": current_exe_path,
                "args": [],
                "endpoint": endpoint_url
            }
        },
        "_meta": {
            "com.microsoft.windows": {
                "endpoint_url": endpoint_url,
                "static_responses": {
                    "initialize": {
                        "protocolVersion": MCP_PROTOCOL_VERSION,
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": server_name,
                            "version": server_version
                        },
                        "instructions": KEEL_MCP_INSTRUCTIONS
                    },
                    "tools/list": {
                        "tools": tools_json
                    }
                }
            }
        }
    })
}

fn find_odr_binary() -> Option<PathBuf> {
    let system_root = std::env::var_os("SystemRoot")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));

    let system32_candidate = system_root.join("System32").join("odr.exe");
    if system32_candidate.is_file() {
        return Some(system32_candidate);
    }

    let (merged_path, _) = build_merged_windows_path();
    resolve_executable_in_path("odr.exe", &merged_path)
}

async fn run_odr_cli(
    odr_path: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<(bool, String, String), String> {
    let mut std_cmd = new_hidden_std_command(odr_path);
    std_cmd.args(args);
    std_cmd.stdin(Stdio::null());
    std_cmd.stdout(Stdio::piped());
    std_cmd.stderr(Stdio::piped());

    let mut cmd = Command::from(std_cmd);
    cmd.kill_on_drop(true);

    let output = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| format!("Timed out running `odr.exe {}`", args.join(" ")))?
        .map_err(|err| format!("Failed to spawn `odr.exe {}`: {err}", args.join(" ")))?;

    let stdout = decode_cli_bytes(&output.stdout);
    let stderr = decode_cli_bytes(&output.stderr);
    Ok((output.status.success(), stdout, stderr))
}

fn first_non_empty_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(160).collect())
}

fn parse_enrolled_servers(
    list_output: &str,
    server_name: &str,
    endpoint_url: &str,
) -> Vec<OdrEnrolledServer> {
    let name_needle = server_name.to_ascii_lowercase();
    let endpoint_needle = endpoint_url.to_ascii_lowercase();

    list_output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.chars().all(|ch| ch == '-' || ch == '='))
        .map(|line| {
            let identifier = line
                .split_whitespace()
                .next()
                .unwrap_or(line)
                .trim_matches(|ch: char| matches!(ch, ',' | ';' | ':'))
                .to_string();
            let lower = line.to_ascii_lowercase();
            let matches_keel = lower.contains(&name_needle) || lower.contains(&endpoint_needle);
            OdrEnrolledServer {
                identifier,
                display_line: line.to_string(),
                matches_keel,
            }
        })
        .collect()
}

pub(crate) async fn probe_odr_status(shared: &KeelSharedState) -> OdrStatusReport {
    let profile = Win11OdrHostProfile::probe();
    let meets_odr_build_hint =
        profile.os_build > 26220 || (profile.os_build == 26220 && profile.os_ubr >= 7262);

    let server_name = shared.config.server_name.clone();
    let server_version = shared.config.server_version.clone();
    let endpoint_url = shared.endpoint_url();
    let manifest_path = default_manifest_path();
    let default_manifest_path_str = manifest_path.to_string_lossy().into_owned();

    let manifest_val = build_keel_odr_manifest_value(
        &server_name,
        &server_version,
        &endpoint_url,
        shared.mcp_tools(),
    );
    let manifest_json =
        serde_json::to_string_pretty(&manifest_val).unwrap_or_else(|_| "{}".to_string());

    let uri_register_command = format!("odr.exe mcp add --uri {endpoint_url}");
    let manifest_register_command = format!("odr.exe mcp add \"{default_manifest_path_str}\"");
    let unregister_command = format!("odr.exe mcp remove {server_name}");
    let list_command = "odr.exe mcp list".to_string();

    let odr_path = if profile.is_windows_11 {
        find_odr_binary()
    } else {
        None
    };

    let mut odr_version = None;
    let mut raw_list_output = None;
    let mut enrolled_servers = Vec::new();

    if let Some(ref bin_path) = odr_path {
        if let Ok((ok, stdout, stderr)) =
            run_odr_cli(bin_path, &["--version"], ODR_PROBE_TIMEOUT).await
        {
            odr_version = if ok {
                first_non_empty_line(&stdout).or_else(|| first_non_empty_line(&stderr))
            } else {
                None
            };
        }
        if odr_version.is_none() {
            if let Ok((_, stdout, stderr)) =
                run_odr_cli(bin_path, &["--help"], ODR_PROBE_TIMEOUT).await
            {
                odr_version =
                    first_non_empty_line(&stdout).or_else(|| first_non_empty_line(&stderr));
            }
        }

        let list_outcome = match run_odr_cli(bin_path, &["mcp", "list"], ODR_PROBE_TIMEOUT).await {
            Ok((true, stdout, _)) => Some(stdout),
            _ => match run_odr_cli(bin_path, &["list"], ODR_PROBE_TIMEOUT).await {
                Ok((true, stdout, _)) => Some(stdout),
                Ok((false, stdout, stderr)) => {
                    let combined = if stdout.trim().is_empty() {
                        stderr
                    } else {
                        stdout
                    };
                    (!combined.trim().is_empty()).then_some(combined)
                }
                Err(_) => None,
            },
        };

        if let Some(list_text) = list_outcome {
            enrolled_servers = parse_enrolled_servers(&list_text, &server_name, &endpoint_url);
            if !list_text.trim().is_empty() {
                raw_list_output = Some(list_text);
            }
        }
    }

    let odr_installed = odr_path.is_some();
    let keel_enrolled = enrolled_servers.iter().any(|item| item.matches_keel);

    let readiness = if !profile.is_windows_11 {
        OdrReadinessState::UnsupportedOs
    } else if odr_installed {
        OdrReadinessState::Ready
    } else {
        OdrReadinessState::MissingOdrCli
    };

    let guidance_message = match readiness {
        OdrReadinessState::Ready => {
            if keel_enrolled {
                format!(
                    "Windows 11 ODR CLI (`odr.exe`) is available and KeelDesk ({endpoint_url}) appears in the registry list."
                )
            } else if profile.developer_mode_enabled {
                "Windows 11 ODR CLI (`odr.exe`) is ready and Developer Mode is enabled. Register KeelDesk via URI or MCPB 0.3 manifest.".to_string()
            } else {
                "Windows 11 ODR CLI (`odr.exe`) is available, but Windows Developer Mode is disabled; enable Developer Mode in Settings > System > For developers if ODR enrollment requires local dev privileges.".to_string()
            }
        }
        OdrReadinessState::MissingOdrCli => {
            if meets_odr_build_hint {
                "odr.exe was not found on this Windows 11 host. Fallback mode is active: export or inspect the generated MCPB 0.3 manifest and register manually once odr.exe is enabled.".to_string()
            } else {
                format!(
                    "odr.exe is not installed on Windows 11 build {}.{} (Microsoft ODR previews target Insider build 26220.7262+ with Developer Mode). Fallback mode is active: you can export the MCPB 0.3 manifest now.",
                    profile.os_build, profile.os_ubr
                )
            }
        }
        OdrReadinessState::UnsupportedOs => {
            "KeelDesk Windows 11 ODR integration requires Windows 11 (Build 22000+).".to_string()
        }
    };

    OdrStatusReport {
        probed_at_iso: Utc::now().to_rfc3339(),
        readiness,
        is_windows_11: profile.is_windows_11,
        os_build: profile.os_build,
        os_ubr: profile.os_ubr,
        display_version: profile.display_version,
        meets_odr_build_hint,
        developer_mode_enabled: profile.developer_mode_enabled,
        odr_installed,
        odr_binary_path: odr_path.map(|p| p.to_string_lossy().into_owned()),
        odr_version,
        keel_enrolled,
        enrolled_servers,
        raw_list_output,
        uri_register_command,
        manifest_register_command,
        unregister_command,
        list_command,
        default_manifest_path: default_manifest_path_str,
        manifest_json,
        guidance_message,
    }
}

async fn write_manifest_file(path: &Path, manifest_json: &str) -> Result<String, String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|err| {
            format!(
                "Failed to create ODR manifest directory '{}': {err}",
                parent.display()
            )
        })?;
    }

    tokio::fs::write(path, manifest_json.as_bytes())
        .await
        .map_err(|err| {
            format!(
                "Failed to write MCPB 0.3 manifest to '{}': {err}",
                path.display()
            )
        })?;

    Ok(path.to_string_lossy().into_owned())
}

pub(crate) async fn export_manifest(
    shared: &KeelSharedState,
    target_path: Option<String>,
) -> Result<OdrActionOutcome, String> {
    let dest = match target_path {
        Some(raw) if !raw.trim().is_empty() => {
            let trimmed = raw.trim();
            if trimmed.contains('\0') {
                return Err("Manifest export path cannot contain NUL bytes.".to_string());
            }
            PathBuf::from(trimmed)
        }
        _ => default_manifest_path(),
    };

    let endpoint_url = shared.endpoint_url();
    let manifest_val = build_keel_odr_manifest_value(
        &shared.config.server_name,
        &shared.config.server_version,
        &endpoint_url,
        shared.mcp_tools(),
    );
    let manifest_json = serde_json::to_string_pretty(&manifest_val)
        .map_err(|err| format!("Failed to serialize MCPB 0.3 manifest: {err}"))?;

    let exported_path = write_manifest_file(&dest, &manifest_json).await?;
    let status = probe_odr_status(shared).await;

    Ok(OdrActionOutcome {
        executed_cli: false,
        succeeded: true,
        mode: "export_manifest".to_string(),
        command_line: format!("odr.exe mcp add \"{exported_path}\""),
        exported_manifest_path: Some(exported_path.clone()),
        stdout: None,
        stderr: None,
        message: format!("Exported KeelDesk MCPB 0.3 manifest to '{exported_path}'."),
        status,
    })
}

pub(crate) async fn register_with_odr(
    shared: &KeelSharedState,
    mode: OdrRegisterMode,
) -> OdrActionOutcome {
    let endpoint_url = shared.endpoint_url();
    let manifest_path = default_manifest_path();
    let manifest_path_str = manifest_path.to_string_lossy().into_owned();
    let mode_label = match mode {
        OdrRegisterMode::Uri => "uri",
        OdrRegisterMode::Manifest => "manifest",
    };
    let command_line = match mode {
        OdrRegisterMode::Uri => format!("odr.exe mcp add --uri {endpoint_url}"),
        OdrRegisterMode::Manifest => format!("odr.exe mcp add \"{manifest_path_str}\""),
    };

    let manifest_val = build_keel_odr_manifest_value(
        &shared.config.server_name,
        &shared.config.server_version,
        &endpoint_url,
        shared.mcp_tools(),
    );
    let manifest_json =
        serde_json::to_string_pretty(&manifest_val).unwrap_or_else(|_| "{}".to_string());

    let odr_bin = if Win11OdrHostProfile::probe().is_windows_11 {
        find_odr_binary()
    } else {
        None
    };

    // Write manifest before invoking `Manifest` registration, or as a fallback artifact when `odr.exe` is missing.
    let mut exported_manifest_path = None;
    if mode == OdrRegisterMode::Manifest || odr_bin.is_none() {
        match write_manifest_file(&manifest_path, &manifest_json).await {
            Ok(written) => exported_manifest_path = Some(written),
            Err(err) => {
                let status = probe_odr_status(shared).await;
                return OdrActionOutcome {
                    executed_cli: false,
                    succeeded: false,
                    mode: mode_label.to_string(),
                    command_line,
                    exported_manifest_path: None,
                    stdout: None,
                    stderr: Some(err.clone()),
                    message: err,
                    status,
                };
            }
        }
    }

    let Some(odr_path) = odr_bin else {
        let status = probe_odr_status(shared).await;
        let saved_note = exported_manifest_path
            .as_deref()
            .unwrap_or(&manifest_path_str);
        let message = format!(
            "odr.exe is not installed on this Windows 11 host. Wrote fallback MCPB 0.3 manifest to '{saved_note}'. Once odr.exe is available, run `{command_line}`."
        );
        return OdrActionOutcome {
            executed_cli: false,
            succeeded: false,
            mode: mode_label.to_string(),
            command_line,
            exported_manifest_path,
            stdout: None,
            stderr: None,
            message,
            status,
        };
    };

    let cli_result = match mode {
        OdrRegisterMode::Uri => {
            run_odr_cli(
                &odr_path,
                &["mcp", "add", "--uri", &endpoint_url],
                ODR_ACTION_TIMEOUT,
            )
            .await
        }
        OdrRegisterMode::Manifest => {
            run_odr_cli(
                &odr_path,
                &["mcp", "add", &manifest_path_str],
                ODR_ACTION_TIMEOUT,
            )
            .await
        }
    };

    let status = probe_odr_status(shared).await;
    match cli_result {
        Ok((succeeded, stdout, stderr)) => {
            let message = if succeeded {
                format!(
                    "Successfully registered KeelDesk with Windows 11 ODR via `{command_line}`."
                )
            } else {
                let detail = first_non_empty_line(&stderr)
                    .or_else(|| first_non_empty_line(&stdout))
                    .unwrap_or_else(|| "odr.exe returned a non-zero exit status".to_string());
                format!("`{command_line}` failed: {detail}")
            };

            OdrActionOutcome {
                executed_cli: true,
                succeeded,
                mode: mode_label.to_string(),
                command_line,
                exported_manifest_path,
                stdout: (!stdout.trim().is_empty()).then_some(stdout),
                stderr: (!stderr.trim().is_empty()).then_some(stderr),
                message,
                status,
            }
        }
        Err(err) => OdrActionOutcome {
            executed_cli: true,
            succeeded: false,
            mode: mode_label.to_string(),
            command_line,
            exported_manifest_path,
            stdout: None,
            stderr: Some(err.clone()),
            message: err,
            status,
        },
    }
}

pub(crate) async fn unregister_from_odr(
    shared: &KeelSharedState,
    server_id: Option<String>,
) -> OdrActionOutcome {
    let target_id = server_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&shared.config.server_name)
        .to_string();

    if target_id.contains('\0')
        || target_id.chars().any(char::is_control)
        || target_id.chars().any(char::is_whitespace)
    {
        let status = probe_odr_status(shared).await;
        return OdrActionOutcome {
            executed_cli: false,
            succeeded: false,
            mode: "unregister".to_string(),
            command_line: format!("odr.exe mcp remove {target_id}"),
            exported_manifest_path: None,
            stdout: None,
            stderr: None,
            message: format!("Invalid ODR server identifier '{target_id}'."),
            status,
        };
    }

    let command_line = format!("odr.exe mcp remove {target_id}");
    let odr_bin = if Win11OdrHostProfile::probe().is_windows_11 {
        find_odr_binary()
    } else {
        None
    };

    let Some(odr_path) = odr_bin else {
        let status = probe_odr_status(shared).await;
        return OdrActionOutcome {
            executed_cli: false,
            succeeded: false,
            mode: "unregister".to_string(),
            command_line: command_line.clone(),
            exported_manifest_path: None,
            stdout: None,
            stderr: None,
            message: format!(
                "odr.exe is not installed on this Windows 11 host; skipped `{command_line}`."
            ),
            status,
        };
    };

    let cli_result = run_odr_cli(
        &odr_path,
        &["mcp", "remove", &target_id],
        ODR_ACTION_TIMEOUT,
    )
    .await;
    let status = probe_odr_status(shared).await;

    match cli_result {
        Ok((succeeded, stdout, stderr)) => {
            let message = if succeeded {
                format!("Removed '{target_id}' from Windows 11 ODR via `{command_line}`.")
            } else {
                let detail = first_non_empty_line(&stderr)
                    .or_else(|| first_non_empty_line(&stdout))
                    .unwrap_or_else(|| "odr.exe returned a non-zero exit status".to_string());
                format!("`{command_line}` failed: {detail}")
            };

            OdrActionOutcome {
                executed_cli: true,
                succeeded,
                mode: "unregister".to_string(),
                command_line,
                exported_manifest_path: None,
                stdout: (!stdout.trim().is_empty()).then_some(stdout),
                stderr: (!stderr.trim().is_empty()).then_some(stderr),
                message,
                status,
            }
        }
        Err(err) => OdrActionOutcome {
            executed_cli: true,
            succeeded: false,
            mode: "unregister".to_string(),
            command_line,
            exported_manifest_path: None,
            stdout: None,
            stderr: Some(err.clone()),
            message: err,
            status,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::super::{KeelConfig, KeelRuntime, KeelServerStatus, KeelToolRegistry};
    use super::*;

    #[tokio::test]
    async fn generates_valid_mcpb_0_3_manifest_and_probes_status_without_panic() {
        let runtime =
            KeelRuntime::start_headless(KeelConfig::ephemeral_for_test(), KeelToolRegistry::new());
        let snap = runtime.wait_until_ready(Duration::from_secs(3)).await;
        assert_eq!(snap.status, KeelServerStatus::Running);

        let status = runtime.odr_status().await;
        assert!(status.uri_register_command.contains(&snap.endpoint_url));
        assert!(status.manifest_register_command.contains("odr.exe mcp add"));

        let parsed: Value =
            serde_json::from_str(&status.manifest_json).expect("manifest_json must be valid JSON");
        assert_eq!(parsed["manifest_version"], "0.3");
        assert_eq!(parsed["name"], snap.server_name);
        assert_eq!(parsed["version"], snap.server_version);

        let ms_meta = &parsed["_meta"]["com.microsoft.windows"];
        assert_eq!(ms_meta["endpoint_url"], snap.endpoint_url);
        assert_eq!(
            ms_meta["static_responses"]["initialize"]["serverInfo"]["name"],
            parsed["name"]
        );
        assert_eq!(
            ms_meta["static_responses"]["initialize"]["serverInfo"]["version"],
            parsed["version"]
        );

        let tools = ms_meta["static_responses"]["tools/list"]["tools"]
            .as_array()
            .expect("tools/list static response must contain tools array");
        let ping_tool = tools
            .iter()
            .find(|tool| tool["name"] == "keel_ping")
            .expect("keel_ping must appear in static tools/list");
        assert!(
            ping_tool.get("inputSchema").is_some(),
            "keel_ping must include JSON inputSchema: {ping_tool}"
        );

        // Verify manifest export writes valid JSON to an explicit temp path.
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("keel_odr_test_{nonce}"));
        let export_path = temp_dir.join("keeldesk.mcpb.manifest.json");

        let outcome = runtime
            .export_odr_manifest(Some(export_path.to_string_lossy().into_owned()))
            .await
            .expect("export_odr_manifest should succeed");
        assert!(outcome.succeeded);
        assert!(!outcome.executed_cli);
        assert_eq!(
            outcome.exported_manifest_path.as_deref(),
            Some(export_path.to_string_lossy().as_ref())
        );

        let disk_text = std::fs::read_to_string(&export_path).expect("manifest file should exist");
        let disk_json: Value = serde_json::from_str(&disk_text).expect("disk manifest valid JSON");
        assert_eq!(disk_json["manifest_version"], "0.3");
        assert_eq!(disk_json["name"], snap.server_name);

        if !status.odr_installed {
            let reg_outcome = runtime.register_with_odr(OdrRegisterMode::Uri).await;
            assert!(!reg_outcome.executed_cli);
            assert!(!reg_outcome.succeeded);
            assert!(reg_outcome.exported_manifest_path.is_some());
            if let Some(fallback_path) = reg_outcome.exported_manifest_path {
                assert!(Path::new(&fallback_path).is_file());
            }
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        runtime.shutdown();
    }
}
