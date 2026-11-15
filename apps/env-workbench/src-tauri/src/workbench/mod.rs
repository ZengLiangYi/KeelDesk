//! Business layer: Windows 11 local environment management workbench.
//!
//! Consumes `keeldesk_core` without `keeldesk_core` depending back on `workbench`.

pub mod dev_env;
pub mod ports;

use std::{net::SocketAddr, time::Duration};

use rmcp::model::JsonObject;
use serde::{Deserialize, Serialize};
use serde_json::json;

use keeldesk_core::{KeelRuntime, KeelToolRegistry, ToolSafetyTier};

use crate::win11;

pub use dev_env::{DevEnvScanReport, DevToolProbe, ScanDevEnvParams, ScanDevEnvTool};
pub use ports::{
    ActivePortEntry, ActivePortReport, KillPortProcessParams, KillPortProcessResult,
    KillPortProcessTool, ListActivePortsParams, ListActivePortsTool,
};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
pub struct Win11HostSummary {
    pub edition: String,
    pub display_version: String,
    pub build_label: String,
    pub hostname: String,
    pub arch: String,
    pub is_windows_11: bool,
}

pub fn register_tools(registry: &mut KeelToolRegistry) {
    registry.register_async_tool::<ScanDevEnvTool>("workbench", ToolSafetyTier::Safe);
    registry.register_async_tool::<ListActivePortsTool>("workbench", ToolSafetyTier::Safe);
    registry
        .register_async_tool::<KillPortProcessTool>("workbench", ToolSafetyTier::ConfirmRequired);
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

#[tauri::command]
#[specta::specta]
pub async fn scan_win11_dev_env() -> Result<DevEnvScanReport, String> {
    dev_env::run_dev_env_scan(None).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_win11_active_ports(
    port: Option<u16>,
    process_filter: Option<String>,
) -> Result<ActivePortReport, String> {
    tokio::task::spawn_blocking(move || {
        ports::collect_active_ports(&ListActivePortsParams {
            port,
            process_filter,
            loopback_only: None,
        })
    })
    .await
    .map_err(|err| format!("Active port scan task failed: {err}"))?
}

fn parse_bound_port_from_addr(bind_addr: &str) -> Option<u16> {
    if let Ok(sock) = bind_addr.parse::<SocketAddr>() {
        if sock.port() > 0 {
            return Some(sock.port());
        }
    }
    bind_addr
        .rsplit_once(':')
        .and_then(|(_, raw_port)| raw_port.trim().parse::<u16>().ok())
        .filter(|&p| p > 0)
}

pub async fn run_kill_port_approval_drill(
    runtime: &KeelRuntime,
    port: Option<u16>,
    reason: Option<String>,
) -> Result<KillPortProcessResult, String> {
    let snap = runtime.snapshot();
    let target_port = port
        .filter(|&p| p > 0)
        .or_else(|| parse_bound_port_from_addr(&snap.bind_addr))
        .ok_or_else(|| {
            "Unable to determine target TCP port for approval drill; Keel MCP server may not be bound yet.".to_string()
        })?;

    let normalized_reason = reason
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
        .unwrap_or_else(|| "KeelDesk workbench interactive approval drill (dry-run)".to_string());

    let mut args = JsonObject::new();
    args.insert("port".to_string(), json!(target_port));
    args.insert("dry_run".to_string(), json!(true));
    args.insert("reason".to_string(), json!(normalized_reason));

    let call_res = runtime
        .call_loopback_tool("kill_port_process", args, Duration::from_secs(65))
        .await?;

    if call_res.is_error == Some(true) {
        let err_text = call_res
            .content
            .iter()
            .filter_map(|block| block.as_text().map(|t| t.text.trim()))
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        return Err(if err_text.is_empty() {
            "Tool execution was rejected by KeelDesk approval gate.".to_string()
        } else {
            err_text
        });
    }

    let structured = call_res.structured_content.ok_or_else(|| {
        "MCP kill_port_process response did not include structuredContent.".to_string()
    })?;

    serde_json::from_value(structured)
        .map_err(|err| format!("Invalid kill_port_process payload schema: {err}"))
}

#[tauri::command]
#[specta::specta]
pub async fn drill_kill_port_approval(
    runtime: tauri::State<'_, KeelRuntime>,
    port: Option<u16>,
    reason: Option<String>,
) -> Result<KillPortProcessResult, String> {
    run_kill_port_approval_drill(&runtime, port, reason).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use keeldesk_core::{KeelConfig, KeelServerStatus, PendingToolApproval, ToolApprovalDecision};

    async fn wait_for_pending_approval(runtime: &KeelRuntime) -> PendingToolApproval {
        for _ in 0..150 {
            let snap = runtime.snapshot();
            if let Some(first) = snap.pending_approvals.first() {
                return first.clone();
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
        panic!("Timed out waiting for pending tool approval in workbench test");
    }

    #[tokio::test]
    async fn workbench_mcp_tools_e2e_advertises_four_tools_and_enforces_approval_and_self_guard() {
        let mut registry = KeelToolRegistry::new();
        register_tools(&mut registry);

        let runtime = KeelRuntime::start_headless(KeelConfig::ephemeral_for_test(), registry);
        let snap = runtime.wait_until_ready(Duration::from_secs(3)).await;
        assert_eq!(snap.status, KeelServerStatus::Running);
        let bound_port =
            parse_bound_port_from_addr(&snap.bind_addr).expect("bound MCP port must be > 0");

        // 1. Verify all 4 tools are advertised via `tools/list` and carry accurate safety tiers.
        let ping = runtime
            .self_test_ping(Some("workbench-e2e".to_string()))
            .await
            .expect("self_test_ping should succeed");
        for expected_tool in [
            "keel_ping",
            "scan_dev_env",
            "list_active_ports",
            "kill_port_process",
        ] {
            assert!(
                ping.advertised_tools.iter().any(|t| t == expected_tool),
                "expected tool '{expected_tool}' in advertised tools: {:?}",
                ping.advertised_tools
            );
        }

        let kill_desc = snap
            .tools
            .iter()
            .find(|d| d.name == "kill_port_process")
            .expect("kill_port_process descriptor must exist");
        assert_eq!(kill_desc.source, "workbench");
        assert_eq!(kill_desc.safety, ToolSafetyTier::ConfirmRequired);

        // 2. Safe `list_active_ports` executes over HTTP MCP without approval prompt.
        let mut list_args = JsonObject::new();
        list_args.insert("port".to_string(), json!(bound_port));
        list_args.insert("loopback_only".to_string(), json!(true));
        let list_res = runtime
            .call_loopback_tool("list_active_ports", list_args, Duration::from_secs(5))
            .await
            .expect("list_active_ports loopback call should succeed");
        assert_ne!(list_res.is_error, Some(true));
        let port_report: ActivePortReport =
            serde_json::from_value(list_res.structured_content.expect("structured port report"))
                .expect("valid ActivePortReport");
        assert!(port_report.entries.iter().any(|e| e.is_keeldesk_self));

        // 3. `kill_port_process` with `dry_run: true` suspends on `ConfirmRequired` and succeeds after `ApproveOnce`.
        let drill_runtime = runtime.clone();
        let drill_task = tokio::spawn(async move {
            run_kill_port_approval_drill(
                &drill_runtime,
                Some(bound_port),
                Some("e2e-dry-run".to_string()),
            )
            .await
        });

        let pending = wait_for_pending_approval(&runtime).await;
        assert_eq!(runtime.snapshot().pending_approvals.len(), 1);
        assert_eq!(pending.tool_name, "kill_port_process");
        assert_eq!(pending.tool_source, "workbench");
        assert!(!drill_task.is_finished());

        runtime
            .resolve_tool_approval(&pending.request_id, ToolApprovalDecision::ApproveOnce)
            .expect("ApproveOnce should resolve pending drill");

        let drill_result = drill_task
            .await
            .expect("drill task should join")
            .expect("approved dry-run drill should succeed");
        assert_eq!(drill_result.port, bound_port);
        assert!(drill_result.dry_run);
        assert!(drill_result.target_pids.contains(&std::process::id()));
        assert!(drill_result.terminated_pids.is_empty());

        // 4. `kill_port_process` with `dry_run: false` against KeelDesk's own port still requires
        //    approval AND is refused by self-protection even after `ApproveOnce`!
        let live_runtime = runtime.clone();
        let live_self_kill_task = tokio::spawn(async move {
            let mut args = JsonObject::new();
            args.insert("port".to_string(), json!(bound_port));
            args.insert("dry_run".to_string(), json!(false));
            args.insert("reason".to_string(), json!("e2e-self-guard-check"));
            live_runtime
                .call_loopback_tool("kill_port_process", args, Duration::from_secs(10))
                .await
        });

        let pending_live = wait_for_pending_approval(&runtime).await;
        assert_eq!(runtime.snapshot().pending_approvals.len(), 1);
        assert_eq!(pending_live.tool_name, "kill_port_process");

        runtime
            .resolve_tool_approval(&pending_live.request_id, ToolApprovalDecision::ApproveOnce)
            .expect("ApproveOnce should resolve");

        let self_kill_outcome = live_self_kill_task
            .await
            .expect("live self-kill task should join");
        let err_msg = self_kill_outcome
            .expect_err("self-termination against KeelDesk bound port must be refused");
        assert!(
            err_msg.contains("Refusing to terminate protected or KeelDesk host process"),
            "unexpected self-protection error message: {err_msg}"
        );

        // 5. Denying the approval drill returns a clean user-facing `User denied` error string.
        let denied_runtime = runtime.clone();
        let denied_task = tokio::spawn(async move {
            run_kill_port_approval_drill(&denied_runtime, Some(bound_port), None).await
        });
        let pending_deny = wait_for_pending_approval(&runtime).await;
        runtime
            .resolve_tool_approval(&pending_deny.request_id, ToolApprovalDecision::Deny)
            .expect("Deny should resolve");
        let deny_err = denied_task
            .await
            .expect("denied task should join")
            .expect_err("denied drill must return Err");
        assert!(
            deny_err.contains("User denied"),
            "unexpected deny error: {deny_err}"
        );

        runtime.shutdown();
    }
}
