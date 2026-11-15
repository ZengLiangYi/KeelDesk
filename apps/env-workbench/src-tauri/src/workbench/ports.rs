//! Windows 11 native TCP port inspection (`list_active_ports`) and guarded process
//! termination (`kill_port_process`) built on Win32 IP Helper and Threading APIs.

use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    net::{Ipv4Addr, Ipv6Addr},
    path::Path,
};

use chrono::Utc;
use rmcp::{
    handler::server::router::tool::{AsyncTool, ToolBase},
    ErrorData as McpError,
};
use serde::{Deserialize, Serialize};

use keeldesk_core::KeelMcpServer;

pub const LIST_ACTIVE_PORTS_TOOL_NAME: &str = "list_active_ports";
pub const LIST_ACTIVE_PORTS_TOOL_DESCRIPTION: &str =
    "Enumerate active Windows 11 listening TCPv4/TCPv6 ports and their owning processes via native Win32 IP Helper APIs.";

pub const KILL_PORT_PROCESS_TOOL_NAME: &str = "kill_port_process";
pub const KILL_PORT_PROCESS_TOOL_DESCRIPTION: &str =
    "Terminate the process owning a listening TCP port on Windows 11 (protected by KeelDesk interactive desktop approval).";

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
pub struct ListActivePortsParams {
    /// Optional exact TCP port number filter (1..=65535).
    #[serde(default)]
    pub port: Option<u16>,
    /// Optional case-insensitive substring filter matched against `process_name` or `exe_path`.
    #[serde(default)]
    pub process_filter: Option<String>,
    /// When `true`, returns only loopback-bound listeners (`127.0.0.0/8` or `::1`).
    #[serde(default)]
    pub loopback_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
pub struct ActivePortEntry {
    pub port: u16,
    pub protocol: String,
    pub local_addr: String,
    pub pid: u32,
    pub process_name: String,
    pub exe_path: Option<String>,
    pub is_loopback: bool,
    pub is_keeldesk_self: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
pub struct ActivePortReport {
    pub captured_at_iso: String,
    pub total_listening: u32,
    pub entries: Vec<ActivePortEntry>,
}

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
pub struct KillPortProcessParams {
    /// Listening TCP port whose owning process should be terminated (1..=65535).
    pub port: u16,
    /// Optional expected owning PID; aborts if any listener on `port` has a different PID (TOCTOU guard).
    #[serde(default)]
    pub expected_pid: Option<u32>,
    /// When `true`, validates and reports the target process(es) without calling `TerminateProcess`.
    #[serde(default)]
    pub dry_run: Option<bool>,
    /// Optional operator or agent audit reason recorded in the result message.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
pub struct KillPortProcessResult {
    pub port: u16,
    pub dry_run: bool,
    pub target_pids: Vec<u32>,
    pub terminated_pids: Vec<u32>,
    pub process_names: Vec<String>,
    pub message: String,
}

pub struct ListActivePortsTool;

impl ToolBase for ListActivePortsTool {
    type Parameter = ListActivePortsParams;
    type Output = ActivePortReport;
    type Error = McpError;

    fn name() -> Cow<'static, str> {
        LIST_ACTIVE_PORTS_TOOL_NAME.into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some(LIST_ACTIVE_PORTS_TOOL_DESCRIPTION.into())
    }
}

impl AsyncTool<KeelMcpServer> for ListActivePortsTool {
    async fn invoke(
        _service: &KeelMcpServer,
        param: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        tokio::task::spawn_blocking(move || collect_active_ports(&param))
            .await
            .map_err(|err| {
                McpError::internal_error(format!("Port scan task panicked: {err}"), None)
            })?
            .map_err(|msg| McpError::internal_error(msg, None))
    }
}

pub struct KillPortProcessTool;

impl ToolBase for KillPortProcessTool {
    type Parameter = KillPortProcessParams;
    type Output = KillPortProcessResult;
    type Error = McpError;

    fn name() -> Cow<'static, str> {
        KILL_PORT_PROCESS_TOOL_NAME.into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some(KILL_PORT_PROCESS_TOOL_DESCRIPTION.into())
    }
}

impl AsyncTool<KeelMcpServer> for KillPortProcessTool {
    async fn invoke(
        _service: &KeelMcpServer,
        param: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        tokio::task::spawn_blocking(move || execute_kill_port_process(param))
            .await
            .map_err(|err| {
                McpError::internal_error(format!("Kill port task panicked: {err}"), None)
            })?
    }
}

#[derive(Debug)]
struct RawSocketListener {
    port: u16,
    protocol: &'static str,
    local_addr: String,
    pid: u32,
    is_loopback: bool,
}

#[cfg(target_os = "windows")]
struct OwnedHandle(windows::Win32::Foundation::HANDLE);

#[cfg(target_os = "windows")]
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: `self.0` was returned by `OpenProcess` and is closed exactly once on drop.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(target_os = "windows")]
fn fetch_extended_tcp_table_words(af: u32) -> Result<Vec<u32>, String> {
    use windows::Win32::{
        Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_NO_DATA, NO_ERROR},
        NetworkManagement::IpHelper::{GetExtendedTcpTable, TCP_TABLE_OWNER_PID_LISTENER},
    };

    let mut dw_size: u32 = 0;
    // SAFETY: Passing `None` queries the required byte size into `dw_size`.
    let first_status = unsafe {
        GetExtendedTcpTable(
            None,
            &mut dw_size,
            false,
            af,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };

    if first_status == ERROR_NO_DATA.0 || (first_status == NO_ERROR.0 && dw_size == 0) {
        return Ok(Vec::new());
    }
    if first_status != ERROR_INSUFFICIENT_BUFFER.0 && first_status != NO_ERROR.0 {
        return Err(format!(
            "GetExtendedTcpTable(af={af}) size query failed with Win32 error {first_status}"
        ));
    }

    for _ in 0..6 {
        let alloc_bytes = (dw_size as usize).saturating_add(4096).max(4096);
        let word_len = alloc_bytes.div_ceil(std::mem::size_of::<u32>());
        let mut buffer = vec![0u32; word_len];
        dw_size = (buffer.len() * std::mem::size_of::<u32>()) as u32;

        // SAFETY: `buffer` is 4-byte aligned and holds at least `dw_size` writable bytes.
        let status = unsafe {
            GetExtendedTcpTable(
                Some(buffer.as_mut_ptr().cast()),
                &mut dw_size,
                false,
                af,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };

        if status == NO_ERROR.0 {
            return Ok(buffer);
        }
        if status == ERROR_NO_DATA.0 {
            return Ok(Vec::new());
        }
        if status != ERROR_INSUFFICIENT_BUFFER.0 {
            return Err(format!(
                "GetExtendedTcpTable(af={af}) failed with Win32 error {status}"
            ));
        }
    }

    Err(format!(
        "GetExtendedTcpTable(af={af}) exceeded buffer resize retries"
    ))
}

#[cfg(target_os = "windows")]
fn enumerate_ipv4_listeners() -> Result<Vec<RawSocketListener>, String> {
    use windows::Win32::{
        NetworkManagement::IpHelper::{MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID},
        Networking::WinSock::AF_INET,
    };

    let buffer = fetch_extended_tcp_table_words(u32::from(AF_INET.0))?;
    let byte_len = buffer.len() * std::mem::size_of::<u32>();
    if byte_len < std::mem::size_of::<u32>() {
        return Ok(Vec::new());
    }

    let table_ptr = buffer.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
    // SAFETY: `buffer` is 4-byte aligned and contains at least `size_of::<u32>()` bytes.
    let num_entries = unsafe { std::ptr::addr_of!((*table_ptr).dwNumEntries).read() } as usize;
    if num_entries == 0 {
        return Ok(Vec::new());
    }

    // SAFETY: Compute raw pointer to flexible array member without narrowing provenance to `[T; 1]`.
    let rows_ptr = unsafe { std::ptr::addr_of!((*table_ptr).table).cast::<MIB_TCPROW_OWNER_PID>() };
    let header_offset = (rows_ptr as usize).saturating_sub(buffer.as_ptr() as usize);
    let max_rows =
        byte_len.saturating_sub(header_offset) / std::mem::size_of::<MIB_TCPROW_OWNER_PID>();
    let safe_count = num_entries.min(max_rows);
    // SAFETY: `rows_ptr` is properly aligned and `safe_count` rows fit within `buffer`.
    let rows = unsafe { std::slice::from_raw_parts(rows_ptr, safe_count) };

    let mut out = Vec::with_capacity(safe_count);
    for row in rows {
        let port = u16::from_be((row.dwLocalPort & 0xFFFF) as u16);
        let ipv4 = Ipv4Addr::from(u32::from_be(row.dwLocalAddr));
        out.push(RawSocketListener {
            port,
            protocol: "tcp4",
            local_addr: ipv4.to_string(),
            pid: row.dwOwningPid,
            is_loopback: ipv4.is_loopback(),
        });
    }

    Ok(out)
}

#[cfg(target_os = "windows")]
fn enumerate_ipv6_listeners() -> Result<Vec<RawSocketListener>, String> {
    use windows::Win32::{
        NetworkManagement::IpHelper::{MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID},
        Networking::WinSock::AF_INET6,
    };

    let buffer = fetch_extended_tcp_table_words(u32::from(AF_INET6.0))?;
    let byte_len = buffer.len() * std::mem::size_of::<u32>();
    if byte_len < std::mem::size_of::<u32>() {
        return Ok(Vec::new());
    }

    let table_ptr = buffer.as_ptr().cast::<MIB_TCP6TABLE_OWNER_PID>();
    // SAFETY: `buffer` is 4-byte aligned and contains at least `size_of::<u32>()` bytes.
    let num_entries = unsafe { std::ptr::addr_of!((*table_ptr).dwNumEntries).read() } as usize;
    if num_entries == 0 {
        return Ok(Vec::new());
    }

    // SAFETY: Compute raw pointer to flexible array member without narrowing provenance to `[T; 1]`.
    let rows_ptr =
        unsafe { std::ptr::addr_of!((*table_ptr).table).cast::<MIB_TCP6ROW_OWNER_PID>() };
    let header_offset = (rows_ptr as usize).saturating_sub(buffer.as_ptr() as usize);
    let max_rows =
        byte_len.saturating_sub(header_offset) / std::mem::size_of::<MIB_TCP6ROW_OWNER_PID>();
    let safe_count = num_entries.min(max_rows);
    // SAFETY: `rows_ptr` is properly aligned and `safe_count` rows fit within `buffer`.
    let rows = unsafe { std::slice::from_raw_parts(rows_ptr, safe_count) };

    let mut out = Vec::with_capacity(safe_count);
    for row in rows {
        let port = u16::from_be((row.dwLocalPort & 0xFFFF) as u16);
        let ipv6 = Ipv6Addr::from(row.ucLocalAddr);
        let is_loopback =
            ipv6.is_loopback() || ipv6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback());
        out.push(RawSocketListener {
            port,
            protocol: "tcp6",
            local_addr: ipv6.to_string(),
            pid: row.dwOwningPid,
            is_loopback,
        });
    }

    Ok(out)
}

#[cfg(not(target_os = "windows"))]
fn enumerate_ipv4_listeners() -> Result<Vec<RawSocketListener>, String> {
    let _ = Ipv4Addr::LOCALHOST;
    Ok(Vec::new())
}

#[cfg(not(target_os = "windows"))]
fn enumerate_ipv6_listeners() -> Result<Vec<RawSocketListener>, String> {
    let _ = Ipv6Addr::LOCALHOST;
    Ok(Vec::new())
}

#[cfg(target_os = "windows")]
fn query_process_identity_uncached(pid: u32) -> (String, Option<String>) {
    use windows::{
        core::PWSTR,
        Win32::System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    };

    if pid == 0 {
        return ("Idle".to_string(), None);
    }
    if pid == 4 {
        return ("System".to_string(), None);
    }

    // SAFETY: Queries read-only limited process metadata without handle inheritance.
    let raw_handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(handle) => handle,
        Err(_) => return ("System / Protected".to_string(), None),
    };
    let handle = OwnedHandle(raw_handle);

    let mut buf = vec![0u16; 1024];
    let mut size = buf.len() as u32;
    // SAFETY: `buf` is valid writable UTF-16 storage of `size` code units.
    let ok = unsafe {
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
    };

    if ok.is_err() || size == 0 {
        return ("System / Protected".to_string(), None);
    }

    let full_path = String::from_utf16_lossy(&buf[..size as usize]);
    let process_name = Path::new(&full_path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(&full_path)
        .to_string();

    (process_name, Some(full_path))
}

#[cfg(not(target_os = "windows"))]
fn query_process_identity_uncached(pid: u32) -> (String, Option<String>) {
    let _ = Path::new("");
    match pid {
        0 => ("Idle".to_string(), None),
        4 => ("System".to_string(), None),
        _ => ("Unknown".to_string(), None),
    }
}

pub fn collect_active_ports(params: &ListActivePortsParams) -> Result<ActivePortReport, String> {
    let mut raw = enumerate_ipv4_listeners()?;
    raw.extend(enumerate_ipv6_listeners()?);

    let port_filter = params.port;
    let loopback_only = params.loopback_only.unwrap_or(false);
    let needle = params
        .process_filter
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase);

    let self_pid = std::process::id();
    let mut pid_cache: HashMap<u32, (String, Option<String>)> = HashMap::new();
    let mut entries = Vec::with_capacity(raw.len());

    for sock in raw {
        if let Some(target_port) = port_filter {
            if sock.port != target_port {
                continue;
            }
        }
        if loopback_only && !sock.is_loopback {
            continue;
        }

        let (process_name, exe_path) = pid_cache
            .entry(sock.pid)
            .or_insert_with(|| query_process_identity_uncached(sock.pid))
            .clone();

        if let Some(ref filter_text) = needle {
            let name_matches = process_name.to_lowercase().contains(filter_text);
            let path_matches = exe_path
                .as_deref()
                .is_some_and(|p| p.to_lowercase().contains(filter_text));
            if !name_matches && !path_matches {
                continue;
            }
        }

        entries.push(ActivePortEntry {
            port: sock.port,
            protocol: sock.protocol.to_string(),
            local_addr: sock.local_addr,
            pid: sock.pid,
            process_name,
            exe_path,
            is_loopback: sock.is_loopback,
            is_keeldesk_self: sock.pid == self_pid,
        });
    }

    entries.sort_by(|a, b| {
        (&a.port, &a.protocol, &a.local_addr, &a.pid).cmp(&(
            &b.port,
            &b.protocol,
            &b.local_addr,
            &b.pid,
        ))
    });
    entries.dedup_by(|a, b| {
        a.port == b.port
            && a.protocol == b.protocol
            && a.local_addr == b.local_addr
            && a.pid == b.pid
    });

    let total_listening = entries.len().min(u32::MAX as usize) as u32;

    Ok(ActivePortReport {
        captured_at_iso: Utc::now().to_rfc3339(),
        total_listening,
        entries,
    })
}

#[cfg(target_os = "windows")]
fn terminate_win32_process(pid: u32, port: u16) -> Result<(), String> {
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    // SAFETY: Opens `pid` strictly with `PROCESS_TERMINATE` and closes handle via `OwnedHandle`.
    let raw_handle = unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) }.map_err(|err| {
        format!("Failed to open process PID {pid} on port {port} for termination: {err}")
    })?;
    let handle = OwnedHandle(raw_handle);

    // SAFETY: `handle.0` is a valid process handle with `PROCESS_TERMINATE` access.
    unsafe { TerminateProcess(handle.0, 1) }
        .map_err(|err| format!("TerminateProcess failed for PID {pid} on port {port}: {err}"))
}

#[cfg(not(target_os = "windows"))]
fn terminate_win32_process(pid: u32, port: u16) -> Result<(), String> {
    Err(format!(
        "Process termination for PID {pid} on port {port} is only supported on Windows 11"
    ))
}

pub fn execute_kill_port_process(
    param: KillPortProcessParams,
) -> Result<KillPortProcessResult, McpError> {
    let port = param.port;
    if port == 0 {
        return Err(McpError::invalid_params(
            "port must be between 1 and 65535",
            None,
        ));
    }

    let report = collect_active_ports(&ListActivePortsParams {
        port: Some(port),
        process_filter: None,
        loopback_only: None,
    })
    .map_err(|msg| McpError::internal_error(msg, None))?;

    if report.entries.is_empty() {
        return Err(McpError::invalid_params(
            format!("No listening TCP process found on port {port}"),
            None,
        ));
    }

    let mut unique_targets: BTreeMap<u32, String> = BTreeMap::new();
    for entry in &report.entries {
        unique_targets
            .entry(entry.pid)
            .or_insert_with(|| entry.process_name.clone());
    }

    let target_pids: Vec<u32> = unique_targets.keys().copied().collect();
    let process_names: Vec<String> = unique_targets.values().cloned().collect();

    if let Some(expected_pid) = param.expected_pid {
        if !target_pids.iter().all(|&pid| pid == expected_pid) {
            return Err(McpError::invalid_params(
                format!(
                    "Refusing termination on port {port}: expected PID {expected_pid}, found active listener PID(s) {target_pids:?}"
                ),
                None,
            ));
        }
    }

    let dry_run = param.dry_run.unwrap_or(false);
    let reason_note = param
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|r| format!(" (reason: {r})"))
        .unwrap_or_default();

    if dry_run {
        return Ok(KillPortProcessResult {
            port,
            dry_run: true,
            target_pids: target_pids.clone(),
            terminated_pids: Vec::new(),
            process_names: process_names.clone(),
            message: format!(
                "Dry-run verified listener PID(s) {:?} ({}) on TCP port {} without terminating any process{}.",
                target_pids,
                process_names.join(", "),
                port,
                reason_note
            ),
        });
    }

    let self_pid = std::process::id();
    if target_pids
        .iter()
        .any(|&pid| pid == 0 || pid == 4 || pid == self_pid)
    {
        return Err(McpError::invalid_params(
            format!(
                "Refusing to terminate protected or KeelDesk host process (PID {target_pids:?}) on port {port}"
            ),
            None,
        ));
    }

    let mut terminated_pids = Vec::with_capacity(target_pids.len());
    for &pid in &target_pids {
        terminate_win32_process(pid, port).map_err(|msg| McpError::internal_error(msg, None))?;
        terminated_pids.push(pid);
    }

    Ok(KillPortProcessResult {
        port,
        dry_run: false,
        target_pids,
        terminated_pids: terminated_pids.clone(),
        process_names: process_names.clone(),
        message: format!(
            "Terminated PID(s) {:?} ({}) owning TCP port {}{}.",
            terminated_pids,
            process_names.join(", "),
            port,
            reason_note
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn live_win32_list_active_ports_identifies_ephemeral_loopback_listener() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral loopback listener should bind");
        let bound_port = listener.local_addr().unwrap().port();
        let self_pid = std::process::id();

        let report = collect_active_ports(&ListActivePortsParams {
            port: Some(bound_port),
            process_filter: None,
            loopback_only: Some(true),
        })
        .expect("Win32 GetExtendedTcpTable port scan should succeed");

        assert!(
            !report.entries.is_empty(),
            "expected at least one active listener on port {bound_port}"
        );
        let entry = report
            .entries
            .iter()
            .find(|e| e.pid == self_pid)
            .expect("listener must resolve to current test process PID");

        assert_eq!(entry.port, bound_port);
        assert_eq!(entry.protocol, "tcp4");
        assert_eq!(entry.local_addr, "127.0.0.1");
        assert!(entry.is_loopback);
        assert!(entry.is_keeldesk_self);
        assert!(!entry.process_name.is_empty());
        assert!(entry.exe_path.is_some());

        // Verify case-insensitive `process_filter` matches our own executable name.
        let filtered = collect_active_ports(&ListActivePortsParams {
            port: Some(bound_port),
            process_filter: Some(entry.process_name.to_uppercase()),
            loopback_only: Some(true),
        })
        .expect("filtered port scan should succeed");
        assert!(filtered.entries.iter().any(|e| e.pid == self_pid));

        // Verify IPv6 loopback (`[::1]:0`) listener enumeration via `MIB_TCP6TABLE_OWNER_PID`.
        if let Ok(v6_listener) = tokio::net::TcpListener::bind("[::1]:0").await {
            let v6_port = v6_listener.local_addr().unwrap().port();
            let v6_report = collect_active_ports(&ListActivePortsParams {
                port: Some(v6_port),
                process_filter: None,
                loopback_only: Some(true),
            })
            .expect("IPv6 port scan should succeed");
            let v6_entry = v6_report
                .entries
                .iter()
                .find(|e| e.pid == self_pid && e.protocol == "tcp6")
                .expect("IPv6 loopback listener must be enumerated");
            assert_eq!(v6_entry.local_addr, "::1");
            assert!(v6_entry.is_loopback);
            assert!(v6_entry.is_keeldesk_self);
        }

        drop(listener);
    }

    #[tokio::test]
    async fn kill_port_process_validates_port_zero_pid_mismatch_and_self_protection() {
        let zero_err = execute_kill_port_process(KillPortProcessParams {
            port: 0,
            expected_pid: None,
            dry_run: Some(true),
            reason: None,
        })
        .expect_err("port 0 must be rejected");
        assert!(zero_err.message.contains("between 1 and 65535"));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral listener should bind");
        let bound_port = listener.local_addr().unwrap().port();

        let mismatch_err = execute_kill_port_process(KillPortProcessParams {
            port: bound_port,
            expected_pid: Some(u32::MAX - 1),
            dry_run: Some(true),
            reason: None,
        })
        .expect_err("mismatched expected_pid must be rejected");
        assert!(mismatch_err.message.contains("expected PID"));

        let self_guard_err = execute_kill_port_process(KillPortProcessParams {
            port: bound_port,
            expected_pid: Some(std::process::id()),
            dry_run: Some(false),
            reason: Some("unit-test".to_string()),
        })
        .expect_err("non-dry-run against self PID must be rejected");
        assert!(self_guard_err
            .message
            .contains("Refusing to terminate protected or KeelDesk host process"));

        drop(listener);
    }
}
