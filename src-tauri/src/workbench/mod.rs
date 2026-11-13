//! Business layer: Windows 11 local environment management workbench.
//!
//! Consumes `keel_core` without `keel_core` depending back on `workbench`.

use serde::{Deserialize, Serialize};

use crate::{keel_core::KeelToolRegistry, win11};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct Win11HostSummary {
    pub edition: String,
    pub display_version: String,
    pub build_label: String,
    pub hostname: String,
    pub arch: String,
    pub is_windows_11: bool,
}

pub fn register_tools(_registry: &mut KeelToolRegistry) {
    // Host environment inspection and port management tools attach to `KeelToolRegistry` here.
}

#[tauri::command]
#[specta::specta]
pub fn get_win11_host_summary() -> Win11HostSummary {
    let profile = win11::read_win11_os_profile();
    let build_label = if profile.build_number > 0 {
        format!("{}.{}", profile.build_number, profile.ubr)
    } else {
        "Unknown".to_string()
    };

    Win11HostSummary {
        edition: profile.edition,
        display_version: profile.display_version,
        build_label,
        hostname: profile.hostname,
        arch: profile.arch,
        is_windows_11: profile.is_windows_11,
    }
}
