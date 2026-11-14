use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{routing::get, Json as AxumJson, Router};
use rmcp::{
    model::{CallToolRequestParams, CallToolResult, JsonObject},
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig,
        streamable_http_server::{
            session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
        },
        StreamableHttpClientTransport,
    },
    ServiceExt,
};
use serde_json::json;
use tauri_specta::Event;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use super::{
    config::KeelConfig,
    mcp_server::{
        ApprovalNotifier, KeelMcpServer, KeelSharedState, KeelToolRegistry, SnapshotNotifier,
    },
    types::{
        KeelPingPayload, KeelPingSelfTestResult, KeelRuntimeSnapshot, KeelRuntimeUpdatedEvent,
        KeelServerStatus, PendingToolApproval, ToolApprovalDecision, ToolApprovalRequestedEvent,
    },
};

#[derive(Clone)]
pub struct KeelRuntime {
    shared: Arc<KeelSharedState>,
    shutdown_token: CancellationToken,
    ready_notify: Arc<Notify>,
}

impl KeelRuntime {
    pub fn start_with_tauri(
        app_handle: tauri::AppHandle,
        config: KeelConfig,
        registry: KeelToolRegistry,
    ) -> Self {
        let snapshot_app = app_handle.clone();
        let notifier: SnapshotNotifier = Arc::new(move |snapshot| {
            let _ = KeelRuntimeUpdatedEvent(snapshot).emit(&snapshot_app);
        });

        let approval_app = app_handle;
        let approval_notifier: ApprovalNotifier = Arc::new(move |pending| {
            let _ = ToolApprovalRequestedEvent(pending).emit(&approval_app);
            if let Some(window) = tauri::Manager::get_webview_window(&approval_app, "main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
                let _ = window.request_user_attention(Some(tauri::UserAttentionType::Critical));
            }
        });

        Self::start_internal(
            config,
            registry,
            Some(notifier),
            Some(approval_notifier),
            true,
        )
    }

    #[cfg(test)]
    pub fn start_headless(config: KeelConfig, registry: KeelToolRegistry) -> Self {
        Self::start_internal(config, registry, None, None, false)
    }

    #[cfg(test)]
    pub fn start_headless_with_approval_notifier(
        config: KeelConfig,
        registry: KeelToolRegistry,
        approval_notifier: ApprovalNotifier,
    ) -> Self {
        Self::start_internal(config, registry, None, Some(approval_notifier), false)
    }

    fn start_internal(
        config: KeelConfig,
        registry: KeelToolRegistry,
        notifier: Option<SnapshotNotifier>,
        approval_notifier: Option<ApprovalNotifier>,
        use_tauri_runtime: bool,
    ) -> Self {
        let (extra_router, extra_descriptors) = registry.into_parts();
        let mut all_descriptors = KeelMcpServer::builtin_tool_descriptors();
        all_descriptors.extend(extra_descriptors);

        let shared = Arc::new(KeelSharedState::new(
            config.clone(),
            all_descriptors,
            notifier,
            approval_notifier,
        ));
        let shutdown_token = CancellationToken::new();
        let ready_notify = Arc::new(Notify::new());

        let server_prototype = KeelMcpServer::new(shared.clone(), extra_router);
        let task_shared = shared.clone();
        let task_token = shutdown_token.clone();
        let task_ready = ready_notify.clone();

        let server_future = async move {
            let bind_target = config.socket_addr();
            let listener = match tokio::net::TcpListener::bind(bind_target).await {
                Ok(listener) => listener,
                Err(err) => {
                    task_shared.mark_error(format!(
                        "Failed to bind MCP HTTP listener on {}: {}",
                        bind_target, err
                    ));
                    task_ready.notify_waiters();
                    return;
                }
            };

            let actual_port = listener
                .local_addr()
                .map(|addr| addr.port())
                .unwrap_or(config.port);

            let mcp_http_config = StreamableHttpServerConfig::default()
                .with_allowed_hosts(config.allowed_hosts.clone())
                .with_allowed_origins(config.allowed_origins.clone())
                .with_cancellation_token(task_token.child_token());

            let mcp_service: StreamableHttpService<KeelMcpServer, LocalSessionManager> =
                StreamableHttpService::new(
                    move || Ok(server_prototype.clone()),
                    Default::default(),
                    mcp_http_config,
                );

            let health_shared = task_shared.clone();
            let mcp_path = if config.mcp_path.starts_with('/') {
                config.mcp_path.clone()
            } else {
                format!("/{}", config.mcp_path)
            };

            let app = Router::new()
                .nest_service(&mcp_path, mcp_service)
                .route(
                    "/healthz",
                    get(move || {
                        let snap = health_shared.snapshot();
                        async move {
                            AxumJson(json!({
                                "ok": snap.status == KeelServerStatus::Running,
                                "server": snap.server_name,
                                "version": snap.server_version,
                                "endpoint": snap.endpoint_url,
                                "total_calls": snap.total_calls,
                            }))
                        }
                    }),
                );

            task_shared.mark_running(actual_port);
            task_ready.notify_waiters();

            if let Err(err) = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    task_token.cancelled().await;
                })
                .await
            {
                task_shared.mark_error(format!("MCP HTTP server exited with error: {}", err));
            } else {
                task_shared.mark_stopped();
            }
        };

        if use_tauri_runtime {
            tauri::async_runtime::spawn(server_future);
        } else {
            tokio::spawn(server_future);
        }

        Self {
            shared,
            shutdown_token,
            ready_notify,
        }
    }

    pub fn snapshot(&self) -> KeelRuntimeSnapshot {
        self.shared.snapshot()
    }

    pub fn resolve_tool_approval(
        &self,
        request_id: &str,
        decision: ToolApprovalDecision,
    ) -> Result<PendingToolApproval, String> {
        self.shared.resolve_approval(request_id, decision)
    }

    pub fn clear_session_allowlist(&self) -> KeelRuntimeSnapshot {
        self.shared.clear_session_allowlist()
    }

    pub fn shutdown(&self) {
        self.shared.mark_stopped();
        self.shutdown_token.cancel();
    }

    #[cfg(test)]
    pub async fn wait_until_ready(&self, timeout: Duration) -> KeelRuntimeSnapshot {
        for _ in 0..50 {
            let snap = self.snapshot();
            if snap.status != KeelServerStatus::Starting {
                return snap;
            }
            let _ = tokio::time::timeout(timeout / 50, self.ready_notify.notified()).await;
        }
        self.snapshot()
    }

    pub async fn call_loopback_tool(
        &self,
        tool_name: &str,
        args: JsonObject,
        timeout: Duration,
    ) -> Result<CallToolResult, String> {
        let _ = &self.ready_notify;
        let snap = self.snapshot();
        if snap.status != KeelServerStatus::Running {
            return Err(match snap.error_message {
                Some(err) => format!("Keel MCP server is not running: {}", err),
                None => format!("Keel MCP server is currently {:?}", snap.status),
            });
        }

        let http_client = reqwest::Client::builder()
            .no_proxy()
            .user_agent(format!("keeldesk-workbench/{}", snap.server_version))
            .timeout(timeout)
            .build()
            .map_err(|e| format!("Failed to construct loopback HTTP client: {}", e))?;

        let transport = StreamableHttpClientTransport::with_client(
            http_client,
            StreamableHttpClientTransportConfig::with_uri(snap.endpoint_url),
        );

        let client = ()
            .serve(transport)
            .await
            .map_err(|e| format!("MCP initialize handshake failed: {}", e))?;

        let call_outcome = client
            .call_tool(CallToolRequestParams::new(tool_name.to_string()).with_arguments(args))
            .await
            .map_err(|e| format!("MCP tools/call {} failed: {}", tool_name, e));

        let _ = client.cancel().await;

        call_outcome
    }

    pub async fn self_test_ping(
        &self,
        message: Option<String>,
    ) -> Result<KeelPingSelfTestResult, String> {
        let _ = &self.ready_notify;
        let snap = self.snapshot();
        if snap.status != KeelServerStatus::Running {
            return Err(match snap.error_message {
                Some(err) => format!("Keel MCP server is not running: {}", err),
                None => format!("Keel MCP server is currently {:?}", snap.status),
            });
        }

        let started = Instant::now();
        let http_client = reqwest::Client::builder()
            .no_proxy()
            .user_agent(format!("keeldesk-selftest/{}", snap.server_version))
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| format!("Failed to construct loopback HTTP client: {}", e))?;

        let transport = StreamableHttpClientTransport::with_client(
            http_client,
            StreamableHttpClientTransportConfig::with_uri(snap.endpoint_url.clone()),
        );

        let client = ()
            .serve(transport)
            .await
            .map_err(|e| format!("MCP initialize handshake failed: {}", e))?;

        let tools_res = client
            .list_all_tools()
            .await
            .map_err(|e| format!("MCP tools/list failed: {}", e))?;
        let advertised_tools: Vec<String> =
            tools_res.into_iter().map(|t| t.name.to_string()).collect();

        let mut args = JsonObject::new();
        if let Some(msg) = message {
            args.insert("message".to_string(), serde_json::Value::String(msg));
        }

        let call_res = client
            .call_tool(CallToolRequestParams::new("keel_ping").with_arguments(args))
            .await
            .map_err(|e| format!("MCP tools/call keel_ping failed: {}", e))?;

        let _ = client.cancel().await;

        let roundtrip_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;

        let structured_value = call_res.structured_content.ok_or_else(|| {
            "MCP keel_ping response did not include structuredContent".to_string()
        })?;
        let payload: KeelPingPayload = serde_json::from_value(structured_value)
            .map_err(|e| format!("Invalid keel_ping payload schema: {}", e))?;

        Ok(KeelPingSelfTestResult {
            payload,
            roundtrip_ms,
            advertised_tools,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn embedded_streamable_http_server_completes_keel_ping_roundtrip() {
        let runtime =
            KeelRuntime::start_headless(KeelConfig::ephemeral_for_test(), KeelToolRegistry::new());
        let initial = runtime.wait_until_ready(Duration::from_secs(3)).await;
        assert_eq!(initial.status, KeelServerStatus::Running);
        assert!(initial.endpoint_url.starts_with("http://127.0.0.1:"));
        assert!(initial.endpoint_url.ends_with("/mcp"));

        let res = runtime
            .self_test_ping(Some("hello-win11".to_string()))
            .await
            .expect("loopback MCP self-test should succeed");

        assert!(res.payload.ok);
        assert_eq!(res.payload.server, "keeldesk");
        assert_eq!(res.payload.transport, "streamable-http");
        assert_eq!(res.payload.echo.as_deref(), Some("hello-win11"));
        assert!(res.advertised_tools.iter().any(|t| t == "keel_ping"));

        let after = runtime.snapshot();
        assert_eq!(after.total_calls, 1);
        assert_eq!(after.recent_calls.len(), 1);
        assert_eq!(after.recent_calls[0].tool_name, "keel_ping");
        assert!(after.recent_calls[0].succeeded);

        runtime.shutdown();
    }

    #[tokio::test]
    async fn rejects_disallowed_host_header_against_dns_rebinding() {
        let runtime =
            KeelRuntime::start_headless(KeelConfig::ephemeral_for_test(), KeelToolRegistry::new());
        let snap = runtime.wait_until_ready(Duration::from_secs(3)).await;
        assert_eq!(snap.status, KeelServerStatus::Running);

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let response = client
            .post(&snap.endpoint_url)
            .header("Host", "evil-rebind.example.com")
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
            .send()
            .await
            .expect("HTTP request should complete");

        assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
        runtime.shutdown();
    }

    #[derive(Debug, Default, serde::Deserialize, schemars::JsonSchema)]
    struct GuardedActionParams {
        #[serde(default)]
        target: String,
    }

    #[derive(
        Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema, PartialEq, Eq,
    )]
    struct GuardedActionOutput {
        executed: bool,
        target: String,
    }

    struct GuardedActionTool;

    impl rmcp::handler::server::router::tool::ToolBase for GuardedActionTool {
        type Parameter = GuardedActionParams;
        type Output = GuardedActionOutput;
        type Error = rmcp::ErrorData;

        fn name() -> std::borrow::Cow<'static, str> {
            "guarded_test_action".into()
        }

        fn description() -> Option<std::borrow::Cow<'static, str>> {
            Some("Execute a privileged host action gated by desktop approval.".into())
        }
    }

    impl rmcp::handler::server::router::tool::AsyncTool<KeelMcpServer> for GuardedActionTool {
        async fn invoke(
            _service: &KeelMcpServer,
            param: Self::Parameter,
        ) -> Result<Self::Output, Self::Error> {
            Ok(GuardedActionOutput {
                executed: true,
                target: param.target,
            })
        }
    }

    struct SafeSyncProbeTool;

    impl rmcp::handler::server::router::tool::ToolBase for SafeSyncProbeTool {
        type Parameter = GuardedActionParams;
        type Output = GuardedActionOutput;
        type Error = rmcp::ErrorData;

        fn name() -> std::borrow::Cow<'static, str> {
            "safe_sync_probe".into()
        }

        fn description() -> Option<std::borrow::Cow<'static, str>> {
            Some("Synchronous safe probe tool for registry verification.".into())
        }
    }

    impl rmcp::handler::server::router::tool::SyncTool<KeelMcpServer> for SafeSyncProbeTool {
        fn invoke(
            _service: &KeelMcpServer,
            param: Self::Parameter,
        ) -> Result<Self::Output, Self::Error> {
            Ok(GuardedActionOutput {
                executed: true,
                target: param.target,
            })
        }
    }

    fn registry_with_guarded_tool() -> KeelToolRegistry {
        let mut registry = KeelToolRegistry::new();
        registry.register_async_tool::<GuardedActionTool>(
            "test_workbench",
             super::super::types::ToolSafetyTier::ConfirmRequired,
        );
        registry.register_sync_tool::<SafeSyncProbeTool>(
            "test_workbench",
            super::super::types::ToolSafetyTier::Safe,
        );
        registry
    }

    async fn connect_test_mcp_client(
        endpoint_url: &str,
    ) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
        let http_client = reqwest::Client::builder()
            .no_proxy()
            .user_agent("keeldesk-integration-test/0.1.0")
            .timeout(Duration::from_secs(5))
            .build()
            .expect("loopback reqwest client should build");

        let transport = StreamableHttpClientTransport::with_client(
            http_client,
            StreamableHttpClientTransportConfig::with_uri(endpoint_url.to_string()),
        );

        ()
            .serve(transport)
            .await
            .expect("MCP initialize handshake should succeed")
    }

    async fn wait_for_pending_approval(runtime: &KeelRuntime) -> PendingToolApproval {
        for _ in 0..100 {
            let snap = runtime.snapshot();
            if let Some(first) = snap.pending_approvals.first() {
                return first.clone();
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
        panic!("Timed out waiting for pending_approvals entry to appear");
    }

    fn target_args(target: &str) -> JsonObject {
        let mut args = JsonObject::new();
        args.insert(
            "target".to_string(),
            serde_json::Value::String(target.to_string()),
        );
        args
    }

    #[tokio::test]
    async fn confirm_required_tool_suspends_until_approved_once() {
        let notified: Arc<std::sync::Mutex<Vec<PendingToolApproval>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let notified_sink = notified.clone();
        let approval_notifier: ApprovalNotifier = Arc::new(move |pending| {
            notified_sink.lock().unwrap().push(pending);
        });

        let runtime = KeelRuntime::start_headless_with_approval_notifier(
            KeelConfig::ephemeral_for_test(),
            registry_with_guarded_tool(),
            approval_notifier,
        );
        let snap = runtime.wait_until_ready(Duration::from_secs(3)).await;
        assert_eq!(snap.status, KeelServerStatus::Running);

        let client = connect_test_mcp_client(&snap.endpoint_url).await;
        let call_client = client.peer().clone();
        let call_handle = tokio::spawn(async move {
            call_client
                .call_tool(
                    CallToolRequestParams::new("guarded_test_action")
                        .with_arguments(target_args("pid:4242")),
                )
                .await
        });

        let pending = wait_for_pending_approval(&runtime).await;
        assert_eq!(pending.request_id, "apr-1");
        assert_eq!(pending.tool_name, "guarded_test_action");
        assert_eq!(pending.tool_source, "test_workbench");
        assert!(pending.arguments_json.contains("pid:4242"));
        assert!(!pending.session_scope.is_empty());
        assert!(!call_handle.is_finished());
        assert_eq!(notified.lock().unwrap().len(), 1);

        let resolved = runtime
            .resolve_tool_approval(&pending.request_id, ToolApprovalDecision::ApproveOnce)
            .expect("resolving active approval should succeed");
        assert_eq!(resolved.request_id, pending.request_id);

        let call_res = call_handle
            .await
            .expect("call task should join")
            .expect("approved tool call should succeed");
        assert_ne!(call_res.is_error, Some(true));
        let output: GuardedActionOutput = serde_json::from_value(
            call_res
                .structured_content
                .expect("structured output expected"),
        )
        .expect("output schema should match");
        assert_eq!(
            output,
            GuardedActionOutput {
                executed: true,
                target: "pid:4242".to_string(),
            }
        );

        let after = runtime.snapshot();
        assert!(after.pending_approvals.is_empty());
        assert!(after.session_allowlist.is_empty());
        assert_eq!(after.recent_calls.len(), 1);
        assert!(after.recent_calls[0].succeeded);
        assert_eq!(after.recent_calls[0].summary, "OK · approved once");

        let _ = client.cancel().await;
        runtime.shutdown();
    }

    #[tokio::test]
    async fn confirm_required_tool_denied_returns_error_and_logs_failure() {
        let runtime = KeelRuntime::start_headless(
            KeelConfig::ephemeral_for_test(),
            registry_with_guarded_tool(),
        );
        let snap = runtime.wait_until_ready(Duration::from_secs(3)).await;
        let client = connect_test_mcp_client(&snap.endpoint_url).await;

        let call_client = client.peer().clone();
        let call_handle = tokio::spawn(async move {
            call_client
                .call_tool(
                    CallToolRequestParams::new("guarded_test_action")
                        .with_arguments(target_args("pid:9999")),
                )
                .await
        });

        let pending = wait_for_pending_approval(&runtime).await;
        runtime
            .resolve_tool_approval(&pending.request_id, ToolApprovalDecision::Deny)
            .expect("denying active approval should succeed");

        let call_res = call_handle
            .await
            .expect("call task should join")
            .expect("denied tool call returns MCP CallToolResult error");
        assert_eq!(call_res.is_error, Some(true));
        let text = call_res
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or_default();
        assert!(text.contains("User denied execution"));

        let after = runtime.snapshot();
        assert!(after.pending_approvals.is_empty());
        assert_eq!(after.recent_calls.len(), 1);
        assert!(!after.recent_calls[0].succeeded);
        assert_eq!(
            after.recent_calls[0].summary,
            "Blocked · denied by desktop approval gate"
        );

        let _ = client.cancel().await;
        runtime.shutdown();
    }

    #[tokio::test]
    async fn approve_for_session_allows_subsequent_calls_until_allowlist_cleared() {
        let runtime = KeelRuntime::start_headless(
            KeelConfig::ephemeral_for_test(),
            registry_with_guarded_tool(),
        );
        let snap = runtime.wait_until_ready(Duration::from_secs(3)).await;
        let client = connect_test_mcp_client(&snap.endpoint_url).await;

        let call_client = client.peer().clone();
        let first_call = tokio::spawn(async move {
            call_client
                .call_tool(
                    CallToolRequestParams::new("guarded_test_action")
                        .with_arguments(target_args("first-op")),
                )
                .await
        });

        let pending = wait_for_pending_approval(&runtime).await;
        runtime
            .resolve_tool_approval(&pending.request_id, ToolApprovalDecision::ApproveForSession)
            .expect("ApproveForSession should resolve");

        let first_res = first_call
            .await
            .expect("first call task should join")
            .expect("first call should succeed");
        assert_ne!(first_res.is_error, Some(true));

        let mid_snap = runtime.snapshot();
        assert!(mid_snap.pending_approvals.is_empty());
        assert_eq!(mid_snap.session_allowlist.len(), 1);
        assert_eq!(mid_snap.session_allowlist[0].tool_name, "guarded_test_action");

        // Second call on the same MCP client session must succeed immediately without prompting.
        let second_res = client
            .call_tool(
                CallToolRequestParams::new("guarded_test_action")
                    .with_arguments(target_args("second-op")),
            )
            .await
            .expect("allowlisted second call should succeed immediately");
        assert_ne!(second_res.is_error, Some(true));
        let after_second = runtime.snapshot();
        assert!(after_second.pending_approvals.is_empty());
        assert_eq!(after_second.recent_calls[0].summary, "OK · session allowlisted");

        // Clearing the allowlist revokes session bypass so the next call requires approval again.
        let cleared = runtime.clear_session_allowlist();
        assert!(cleared.session_allowlist.is_empty());

        let third_client = client.peer().clone();
        let third_call = tokio::spawn(async move {
            third_client
                .call_tool(
                    CallToolRequestParams::new("guarded_test_action")
                        .with_arguments(target_args("third-op")),
                )
                .await
        });

        let rearmed_pending = wait_for_pending_approval(&runtime).await;
        assert_eq!(rearmed_pending.tool_name, "guarded_test_action");
        runtime
            .resolve_tool_approval(&rearmed_pending.request_id, ToolApprovalDecision::ApproveOnce)
            .expect("re-armed approval should resolve");

        let third_res = third_call
            .await
            .expect("third call should join")
            .expect("third call should succeed");
        assert_ne!(third_res.is_error, Some(true));

        // Verify safe sync tool registration & compact argument preview as well.
        let sync_res = client
            .call_tool(
                CallToolRequestParams::new("safe_sync_probe")
                    .with_arguments(target_args("probe-1")),
            )
            .await
            .expect("safe sync probe should succeed");
        assert_ne!(sync_res.is_error, Some(true));
        assert!(runtime.snapshot().recent_calls[0].succeeded);
        assert!(runtime.snapshot().recent_calls[0]
            .summary
            .starts_with("OK · "));

        let _ = client.cancel().await;
        runtime.shutdown();
    }

    #[tokio::test]
    async fn unapproved_confirm_required_tool_times_out_and_cleans_up_pending() {
        let config = KeelConfig::ephemeral_for_test().with_approval_timeout(Duration::from_millis(120));
        let runtime = KeelRuntime::start_headless(config, registry_with_guarded_tool());
        let snap = runtime.wait_until_ready(Duration::from_secs(3)).await;
        let client = connect_test_mcp_client(&snap.endpoint_url).await;

        let call_res = client
            .call_tool(
                CallToolRequestParams::new("guarded_test_action")
                    .with_arguments(target_args("slow-op")),
            )
            .await
            .expect("timed out approval should return MCP CallToolResult error");

        assert_eq!(call_res.is_error, Some(true));
        let text = call_res
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or_default();
        assert!(text.contains("Timed out waiting for desktop user approval"));

        let after = runtime.snapshot();
        assert!(after.pending_approvals.is_empty());
        assert_eq!(after.recent_calls.len(), 1);
        assert!(!after.recent_calls[0].succeeded);
        assert_eq!(after.recent_calls[0].summary, "Blocked · approval timed out");

        assert!(runtime
            .resolve_tool_approval("apr-1", ToolApprovalDecision::ApproveOnce)
            .is_err());

        let _ = client.cancel().await;
        runtime.shutdown();
    }

    #[tokio::test]
    async fn app_tool_error_telemetry_and_shutdown_drains_pending_approvals() {
        async fn app_level_failure_handler() -> Result<rmcp::model::CallToolResult, rmcp::ErrorData>
        {
            Ok(rmcp::model::CallToolResult::error(vec![
                rmcp::model::ContentBlock::text("simulated host tool failure"),
            ]))
        }

        let mut registry = registry_with_guarded_tool();
        registry.register_route(
            "test_workbench",
            super::super::types::ToolSafetyTier::Safe,
            (
                rmcp::model::Tool::new(
                    "app_level_failure_probe",
                    "Returns an application-level CallToolResult::error payload.",
                    Arc::new(JsonObject::new()),
                ),
                app_level_failure_handler,
            ),
        );

        let runtime = KeelRuntime::start_headless(KeelConfig::ephemeral_for_test(), registry);
        let snap = runtime.wait_until_ready(Duration::from_secs(3)).await;
        let client = connect_test_mcp_client(&snap.endpoint_url).await;

        let err_res = client
            .call_tool(CallToolRequestParams::new("app_level_failure_probe"))
            .await
            .expect("application-level error still completes JSON-RPC call");
        assert_eq!(err_res.is_error, Some(true));
        let after_err = runtime.snapshot();
        assert!(!after_err.recent_calls[0].succeeded);
        assert!(after_err.recent_calls[0]
            .summary
            .contains("simulated host tool failure"));

        // Verify mark_stopped() drains pending approvals immediately rather than hanging for 60s.
        let call_client = client.peer().clone();
        let suspended_call = tokio::spawn(async move {
            call_client
                .call_tool(
                    CallToolRequestParams::new("guarded_test_action")
                        .with_arguments(target_args("shutdown-drain")),
                )
                .await
        });

        let _pending = wait_for_pending_approval(&runtime).await;
        runtime.shared.mark_stopped();

        let drained_res = tokio::time::timeout(Duration::from_secs(2), suspended_call)
            .await
            .expect("suspended tool call must unblock promptly when mark_stopped drains approvals")
            .expect("task should join")
            .expect("drained approval should return CallToolResult error");
        assert_eq!(drained_res.is_error, Some(true));
        assert!(runtime.snapshot().pending_approvals.is_empty());

        let _ = client.cancel().await;
        runtime.shutdown();
    }
}
