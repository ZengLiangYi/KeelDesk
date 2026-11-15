//! Self-contained Win32 environment, Registry, executable resolution, and CLI decoding helpers.

use std::{
    collections::{HashMap, HashSet},
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command as StdCommand,
};

#[cfg(target_os = "windows")]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Win11OdrHostProfile {
    pub is_windows_11: bool,
    pub os_build: u32,
    pub os_ubr: u32,
    pub display_version: String,
    pub developer_mode_enabled: bool,
}

impl Win11OdrHostProfile {
    pub(crate) fn probe() -> Self {
        #[cfg(target_os = "windows")]
        {
            const NT_CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
            const APP_MODEL_UNLOCK: &str =
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock";

            let display_version = read_hklm_string(NT_CURRENT_VERSION, "DisplayVersion")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Unknown".to_string());

            let os_build = read_hklm_string(NT_CURRENT_VERSION, "CurrentBuildNumber")
                .and_then(|s| s.trim().parse::<u32>().ok())
                .unwrap_or(0);

            let os_ubr = read_hklm_dword(NT_CURRENT_VERSION, "UBR").unwrap_or(0);

            let developer_mode_enabled =
                read_hklm_dword(APP_MODEL_UNLOCK, "AllowDevelopmentWithoutDevLicense") == Some(1);

            Self {
                is_windows_11: os_build >= 22000,
                os_build,
                os_ubr,
                display_version,
                developer_mode_enabled,
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            Self {
                is_windows_11: false,
                os_build: 0,
                os_ubr: 0,
                display_version: "N/A".to_string(),
                developer_mode_enabled: false,
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn new_hidden_std_command(program: impl AsRef<OsStr>) -> StdCommand {
    use std::os::windows::process::CommandExt;

    let mut cmd = StdCommand::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn new_hidden_std_command(program: impl AsRef<OsStr>) -> StdCommand {
    StdCommand::new(program)
}

#[cfg(target_os = "windows")]
fn wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn read_reg_string(
    root: windows::Win32::System::Registry::HKEY,
    subkey: &str,
    value_name: &str,
) -> Option<String> {
    use windows::{
        core::PCWSTR,
        Win32::System::Registry::{
            RegGetValueW, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
        },
    };

    let subkey_w = wide_null(subkey);
    let value_w = wide_null(value_name);
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND;
    let mut byte_len: u32 = 0;

    // SAFETY: `subkey_w` and `value_w` are NUL-terminated UTF-16 strings; `None` buffer queries length.
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(subkey_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            flags,
            None,
            None,
            Some(&mut byte_len),
        )
    };
    if status.is_err() || byte_len < 2 {
        return None;
    }

    let mut buffer = vec![0u16; (byte_len as usize).div_ceil(2)];
    // SAFETY: `buffer` holds at least `byte_len` writable bytes for `RegGetValueW`.
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(subkey_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            flags,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut byte_len),
        )
    };
    if status.is_err() {
        return None;
    }

    let nul_pos = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..nul_pos]))
}

#[cfg(target_os = "windows")]
pub(crate) fn read_hklm_string(subkey: &str, value_name: &str) -> Option<String> {
    read_reg_string(
        windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
        subkey,
        value_name,
    )
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub(crate) fn read_hklm_string(_subkey: &str, _value_name: &str) -> Option<String> {
    None
}

#[cfg(target_os = "windows")]
pub(crate) fn read_hkcu_string(subkey: &str, value_name: &str) -> Option<String> {
    read_reg_string(
        windows::Win32::System::Registry::HKEY_CURRENT_USER,
        subkey,
        value_name,
    )
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub(crate) fn read_hkcu_string(_subkey: &str, _value_name: &str) -> Option<String> {
    None
}

#[cfg(target_os = "windows")]
pub(crate) fn read_hklm_dword(subkey: &str, value_name: &str) -> Option<u32> {
    use windows::{
        core::PCWSTR,
        Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD},
    };

    let subkey_w = wide_null(subkey);
    let value_w = wide_null(value_name);
    let mut value: u32 = 0;
    let mut byte_len = std::mem::size_of::<u32>() as u32;

    // SAFETY: `value` is a 4-byte `u32` matching `RRF_RT_REG_DWORD`.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut byte_len),
        )
    };

    status.is_ok().then_some(value)
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub(crate) fn read_hklm_dword(_subkey: &str, _value_name: &str) -> Option<u32> {
    None
}

pub(crate) fn expand_windows_env_vars(input: &str) -> String {
    if !input.contains('%') {
        return input.to_string();
    }

    let env_map: HashMap<String, String> = std::env::vars()
        .map(|(k, v)| (k.to_ascii_uppercase(), v))
        .collect();

    expand_windows_env_vars_with(input, |name| {
        env_map.get(&name.to_ascii_uppercase()).cloned()
    })
}

pub(crate) fn expand_windows_env_vars_with<F>(input: &str, mut lookup: F) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    if !input.contains('%') {
        return input.to_string();
    }

    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(open_idx) = rest.find('%') {
        out.push_str(&rest[..open_idx]);
        let after_open = &rest[open_idx + 1..];

        let Some(close_rel) = after_open.find('%') else {
            out.push('%');
            out.push_str(after_open);
            return out;
        };

        let token = &after_open[..close_rel];
        if !token.is_empty() {
            if let Some(val) = lookup(token) {
                out.push_str(&val);
                rest = &after_open[close_rel + 1..];
                continue;
            }
        }

        // Preserve unknown `%TOKEN%` or `%%` and continue scanning from the second `%`
        // so strings like `100% of %TEMP%` still expand `%TEMP%`.
        out.push('%');
        out.push_str(token);
        rest = &after_open[close_rel..];
    }

    out.push_str(rest);
    out
}

fn push_unique_path_entries(
    raw_path: &OsStr,
    expand_vars: bool,
    seen: &mut HashSet<String>,
    out: &mut Vec<PathBuf>,
) {
    for entry in std::env::split_paths(raw_path) {
        let raw_text = entry.to_string_lossy();
        let trimmed = raw_text.trim().trim_matches('"');
        if trimmed.is_empty() {
            continue;
        }

        let expanded = if expand_vars {
            expand_windows_env_vars(trimmed)
        } else {
            trimmed.to_string()
        };
        let cleaned = expanded.trim().trim_matches('"');
        if cleaned.is_empty() {
            continue;
        }

        let dedup_key = if cfg!(target_os = "windows") {
            cleaned.to_ascii_lowercase()
        } else {
            cleaned.to_string()
        };

        if seen.insert(dedup_key) {
            out.push(PathBuf::from(cleaned));
        }
    }
}

pub(crate) fn build_merged_windows_path() -> (OsString, u32) {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();

    if let Some(process_path) = std::env::var_os("PATH") {
        push_unique_path_entries(&process_path, true, &mut seen, &mut entries);
    }

    if let Some(machine_path) = read_hklm_string(
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        "Path",
    ) {
        push_unique_path_entries(OsStr::new(&machine_path), true, &mut seen, &mut entries);
    }

    if let Some(user_path) = read_hkcu_string("Environment", "Path") {
        push_unique_path_entries(OsStr::new(&user_path), true, &mut seen, &mut entries);
    }

    let count = entries.len().min(u32::MAX as usize) as u32;
    let joined = std::env::join_paths(&entries).unwrap_or_default();
    (joined, count)
}

fn is_candidate_file(path: &Path) -> bool {
    path.is_file() || std::fs::symlink_metadata(path).is_ok_and(|meta| !meta.is_dir())
}

fn select_preferred_candidate(candidates: &[PathBuf]) -> Option<PathBuf> {
    if candidates.is_empty() {
        return None;
    }

    let is_windows_apps = |p: &Path| {
        let lower = p.to_string_lossy().to_ascii_lowercase();
        lower.contains(r"\microsoft\windowsapps\") || lower.contains("/microsoft/windowsapps/")
    };
    let has_ext = |p: &Path, exts: &[&str]| {
        let lower = p.to_string_lossy().to_ascii_lowercase();
        exts.iter().any(|ext| lower.ends_with(ext))
    };

    if let Some(non_stub_exe) = candidates
        .iter()
        .find(|p| has_ext(p, &[".exe"]) && !is_windows_apps(p))
    {
        return Some(non_stub_exe.clone());
    }

    if let Some(any_exe) = candidates.iter().find(|p| has_ext(p, &[".exe", ".com"])) {
        return Some(any_exe.clone());
    }

    if let Some(cmd_shim) = candidates.iter().find(|p| has_ext(p, &[".cmd", ".bat"])) {
        return Some(cmd_shim.clone());
    }

    Some(candidates[0].clone())
}

pub(crate) fn resolve_executable_in_path(command: &str, search_path: &OsStr) -> Option<PathBuf> {
    let trimmed = command.trim();
    if trimmed.is_empty()
        || trimmed.contains('\0')
        || trimmed.contains('"')
        || trimmed.starts_with('\'')
        || trimmed.ends_with('\'')
    {
        return None;
    }

    let has_explicit_path =
        trimmed.contains('\\') || trimmed.contains('/') || trimmed.contains(':');

    if has_explicit_path {
        let direct = PathBuf::from(trimmed);
        if is_candidate_file(&direct) {
            return Some(direct);
        }

        if direct.extension().is_none() {
            for ext in [".exe", ".cmd", ".bat", ".com"] {
                let mut candidate_os = direct.as_os_str().to_os_string();
                candidate_os.push(ext);
                let candidate = PathBuf::from(candidate_os);
                if is_candidate_file(&candidate) {
                    return Some(candidate);
                }
            }
        }
        return None;
    }

    let has_ext = Path::new(trimmed).extension().is_some();
    let mut candidates = Vec::new();

    for dir in std::env::split_paths(search_path) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        if has_ext {
            let candidate = dir.join(trimmed);
            if is_candidate_file(&candidate) {
                candidates.push(candidate);
            }
        } else {
            for ext in [".exe", ".com", ".cmd", ".bat"] {
                let candidate = dir.join(format!("{trimmed}{ext}"));
                if is_candidate_file(&candidate) {
                    candidates.push(candidate);
                }
            }
            let bare = dir.join(trimmed);
            if is_candidate_file(&bare) {
                candidates.push(bare);
            }
        }
    }

    select_preferred_candidate(&candidates)
}

pub(crate) fn decode_cli_bytes(bytes: &[u8]) -> String {
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
    let (pairs, _) = bytes.as_chunks::<2>();
    let units: Vec<u16> = pairs
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
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
    let (pairs, _) = sample.as_chunks::<2>();
    if pairs.is_empty() {
        return false;
    }

    let mut even_zeros = 0usize;
    let mut odd_zeros = 0usize;
    for &[lo, hi] in pairs {
        if lo == 0 {
            even_zeros += 1;
        }
        if hi == 0 {
            odd_zeros += 1;
        }
    }

    odd_zeros > even_zeros && odd_zeros.saturating_mul(4) >= pairs.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_windows_env_vars_case_insensitively_and_preserves_unknown_tokens() {
        let lookup = |name: &str| match name.to_ascii_uppercase().as_str() {
            "SYSTEMROOT" => Some(r"C:\Windows".to_string()),
            "USERPROFILE" => Some(r"C:\Users\dev".to_string()),
            _ => None,
        };

        assert_eq!(
            expand_windows_env_vars_with(r"%systemroot%\System32;%UserProfile%\.cargo\bin", lookup),
            r"C:\Windows\System32;C:\Users\dev\.cargo\bin"
        );
        assert_eq!(
            expand_windows_env_vars_with(r"%UNKNOWN_VAR%\bin", lookup),
            r"%UNKNOWN_VAR%\bin"
        );
        assert_eq!(
            expand_windows_env_vars_with(r"100% of %SystemRoot% %% %UNCLOSED", lookup),
            r"100% of C:\Windows %% %UNCLOSED"
        );
    }

    #[test]
    fn merged_windows_path_returns_non_empty_entries() {
        let (joined, count) = build_merged_windows_path();
        assert!(count > 0, "merged PATH must contain at least one entry");
        assert!(!joined.is_empty(), "joined PATH must not be empty");
    }

    #[test]
    fn resolves_executables_preferring_non_windows_apps_exe_then_exe_then_cmd() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("keel_win32_env_test_{nonce}"));
        let windows_apps = root.join("Microsoft").join("WindowsApps");
        let real_bin = root.join("RealBin");
        let npm_bin = root.join("NpmBin");

        std::fs::create_dir_all(&windows_apps).unwrap();
        std::fs::create_dir_all(&real_bin).unwrap();
        std::fs::create_dir_all(&npm_bin).unwrap();

        let stub_py = windows_apps.join("python.exe");
        let real_py = real_bin.join("python.exe");
        let bare_pnpm = npm_bin.join("pnpm");
        let cmd_pnpm = npm_bin.join("pnpm.cmd");

        std::fs::write(&stub_py, b"").unwrap();
        std::fs::write(&real_py, b"").unwrap();
        std::fs::write(&bare_pnpm, b"").unwrap();
        std::fs::write(&cmd_pnpm, b"").unwrap();

        let search_path = std::env::join_paths([&windows_apps, &real_bin, &npm_bin]).unwrap();

        assert_eq!(
            resolve_executable_in_path("python", &search_path),
            Some(real_py.clone())
        );
        assert_eq!(
            resolve_executable_in_path("pnpm", &search_path),
            Some(cmd_pnpm.clone())
        );
        assert_eq!(resolve_executable_in_path("", &search_path), None);
        assert_eq!(resolve_executable_in_path("\"python\"", &search_path), None);
        assert_eq!(resolve_executable_in_path("py\0thon", &search_path), None);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn decodes_utf8_bom_and_utf16le_cli_bytes() {
        assert_eq!(decode_cli_bytes(b"\xEF\xBB\xBFhello\r\n"), "hello\r\n");

        let mut utf16_bom = vec![0xFF, 0xFE];
        for unit in "odr 0.1.0".encode_utf16() {
            utf16_bom.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(decode_cli_bytes(&utf16_bom), "odr 0.1.0");

        let mut utf16_bomless = Vec::new();
        for unit in "WSL version".encode_utf16() {
            utf16_bomless.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(decode_cli_bytes(&utf16_bomless), "WSL version");
    }
}
