use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::Request,
    middleware::{self, Next},
    response::Response,
    routing::get,
    Json as AxumJson, Router,
};
use chrono::Utc;
use http::{HeaderMap, Method, StatusCode};
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
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use super::{
    config::KeelConfig,
    devtools::{
        McpLatencyBreakdown, McpProtocolTraceEntry, McpProtocolTraceStore, McpTraceApprovalOutcome,
        McpTraceDirection, McpTraceStatus, McpTraceTransport, NewProtocolTrace,
        ProtocolTraceClearedNotifier, ProtocolTraceNotifier,
    },
    mcp_server::{
        ApprovalNotifier, KeelMcpServer, KeelSharedState, KeelToolRegistry, SnapshotNotifier,
    },
    process_host::{
        ChildServerRecord, ChildToolCallResult, KeelProcessHost, ProcessHostNotifier,
        ProcessHostSnapshot, SpawnChildServerSpec,
    },
    types::{
        KeelHealthCheckResult, KeelPingPayload, KeelPingSelfTestResult, KeelRuntimeSnapshot,
        KeelServerStatus, PendingToolApproval, ToolApprovalDecision,
    },
    win_odr::{
        export_manifest, probe_odr_status, register_with_odr, unregister_from_odr,
        OdrActionOutcome, OdrRegisterMode, OdrStatusReport,
    },
};

pub type RuntimeTaskSpawner = Arc<
    dyn Fn(std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>) + Send + Sync,
>;

#[derive(Clone, Default)]
pub struct KeelRuntimeOptions {
    pub snapshot_notifier: Option<SnapshotNotifier>,
    pub approval_notifier: Option<ApprovalNotifier>,
    pub process_host_notifier: Option<ProcessHostNotifier>,
    pub trace_notifier: Option<ProtocolTraceNotifier>,
    pub trace_cleared_notifier: Option<ProtocolTraceClearedNotifier>,
    pub task_spawner: Option<RuntimeTaskSpawner>,
}

#[derive(Clone)]
pub struct KeelRuntime {
    shared: Arc<KeelSharedState>,
    process_host: Arc<KeelProcessHost>,
    shutdown_token: CancellationToken,
    ready_notify: Arc<Notify>,
}

impl KeelRuntime {
    pub fn start(
        config: KeelConfig,
        registry: KeelToolRegistry,
        options: KeelRuntimeOptions,
    ) -> Self {
        let KeelRuntimeOptions {
            snapshot_notifier,
            approval_notifier,
            process_host_notifier,
            trace_notifier,
            trace_cleared_notifier,
            task_spawner,
        } = options;

        let (extra_router, extra_descriptors) = registry.into_parts();
        let mut all_descriptors = KeelMcpServer::builtin_tool_descriptors();
        all_descriptors.extend(extra_descriptors);

        let full_router = KeelMcpServer::merged_tool_router(extra_router);
        let mcp_tools = full_router.list_all();

        let trace_store = Arc::new(McpProtocolTraceStore::new(
            trace_notifier,
            trace_cleared_notifier,
        ));
        let shared = Arc::new(KeelSharedState::new(
            config.clone(),
            all_descriptors,
            mcp_tools,
            Arc::clone(&trace_store),
            snapshot_notifier,
            approval_notifier,
        ));
        let process_host = Arc::new(KeelProcessHost::new(
            process_host_notifier,
            Some(trace_store),
        ));
        let shutdown_token = CancellationToken::new();
        let ready_notify = Arc::new(Notify::new());

        let server_prototype = KeelMcpServer::from_router(shared.clone(), full_router);
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
            let mw_shared = task_shared.clone();
            let mcp_path = if config.mcp_path.starts_with('/') {
                config.mcp_path.clone()
            } else {
                format!("/{}", config.mcp_path)
            };

            let mcp_router =
                Router::new()
                    .nest_service(&mcp_path, mcp_service)
                    .layer(middleware::from_fn(move |req: Request, next: Next| {
                        let shared = mw_shared.clone();
                        mcp_http_trace_middleware(shared, req, next)
                    }));

            let health_router = Router::new().route(
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

            let app = mcp_router.merge(health_router);

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

        if let Some(spawner) = task_spawner {
            spawner(Box::pin(server_future));
        } else {
            tokio::spawn(server_future);
        }

        Self {
            shared,
            process_host,
            shutdown_token,
            ready_notify,
        }
    }

    pub fn start_headless(config: KeelConfig, registry: KeelToolRegistry) -> Self {
        Self::start(config, registry, KeelRuntimeOptions::default())
    }

    pub fn start_headless_with_approval_notifier(
        config: KeelConfig,
        registry: KeelToolRegistry,
        approval_notifier: ApprovalNotifier,
    ) -> Self {
        Self::start(
            config,
            registry,
            KeelRuntimeOptions {
                approval_notifier: Some(approval_notifier),
                ..Default::default()
            },
        )
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

    pub async fn odr_status(&self) -> OdrStatusReport {
        probe_odr_status(&self.shared).await
    }

    pub async fn register_with_odr(&self, mode: OdrRegisterMode) -> OdrActionOutcome {
        register_with_odr(&self.shared, mode).await
    }

    pub async fn unregister_from_odr(&self, server_id: Option<String>) -> OdrActionOutcome {
        unregister_from_odr(&self.shared, server_id).await
    }

    pub async fn export_odr_manifest(
        &self,
        target_path: Option<String>,
    ) -> Result<OdrActionOutcome, String> {
        export_manifest(&self.shared, target_path).await
    }

    pub fn process_host_snapshot(&self) -> ProcessHostSnapshot {
        self.process_host.snapshot()
    }

    pub async fn start_child_server(
        &self,
        spec: SpawnChildServerSpec,
    ) -> Result<ChildServerRecord, String> {
        self.process_host.start_server(spec).await
    }

    pub async fn stop_child_server(&self, server_id: &str) -> Result<ChildServerRecord, String> {
        self.process_host.stop_server(server_id).await
    }

    pub async fn remove_child_server(
        &self,
        server_id: &str,
    ) -> Result<ProcessHostSnapshot, String> {
        self.process_host.remove_server(server_id).await
    }

    pub async fn call_child_server_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments_json: Option<String>,
    ) -> Result<ChildToolCallResult, String> {
        self.process_host
            .call_server_tool(server_id, tool_name, arguments_json)
            .await
    }

    pub fn protocol_traces(&self) -> Vec<McpProtocolTraceEntry> {
        self.shared.trace_store().list()
    }

    pub fn clear_protocol_traces(&self) -> u32 {
        self.shared.trace_store().clear()
    }

    pub async fn check_healthz(&self) -> Result<KeelHealthCheckResult, String> {
        let snap = self.snapshot();
        let healthz_url = format!(
            "http://{}:{}/healthz",
            self.shared.config.bind_ip, snap.bound_port
        );
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| format!("Failed to construct loopback healthz client: {e}"))?;

        let started = Instant::now();
        let resp = client
            .get(&healthz_url)
            .send()
            .await
            .map_err(|e| format!("Loopback health check GET {} failed: {}", healthz_url, e))?;
        let latency_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
        let status = resp.status();
        let status_code = status.as_u16();
        let body_ok = resp
            .bytes()
            .await
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|v| v.get("ok").and_then(|b| b.as_bool()))
            .unwrap_or(false);

        let after = self.snapshot();
        Ok(KeelHealthCheckResult {
            ok: status.is_success() && body_ok,
            status_code,
            latency_ms,
            endpoint_url: after.endpoint_url,
            healthz_url,
            bound_port: after.bound_port,
            active_session_count: after.active_sessions.len().min(u32::MAX as usize) as u32,
            total_calls: after.total_calls,
            checked_at_iso: Utc::now().to_rfc3339(),
        })
    }

    pub fn shutdown(&self) {
        self.process_host.shutdown_all();
        self.shared.mark_stopped();
        self.shutdown_token.cancel();
    }

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

fn extract_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

async fn mcp_http_trace_middleware(
    shared: Arc<KeelSharedState>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let uri = req.uri().to_string();
    let req_session_id = extract_header(req.headers(), "mcp-session-id");
    let user_agent = extract_header(req.headers(), http::header::USER_AGENT.as_str());
    let origin = extract_header(req.headers(), http::header::ORIGIN.as_str());
    let host = extract_header(req.headers(), http::header::HOST.as_str());

    let started = Instant::now();
    let res = next.run(req).await;
    let elapsed_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;

    let status = res.status();
    let res_session_id = extract_header(res.headers(), "mcp-session-id");

    if method == Method::DELETE {
        if status.is_success() {
            if let Some(sid) = req_session_id {
                shared.remove_http_session(&sid);
            }
        } else if status.is_client_error() || status.is_server_error() {
            record_http_rejection_trace(
                &shared,
                &method,
                &uri,
                status,
                elapsed_ms,
                req_session_id,
                user_agent,
                origin,
                host,
            );
        }
    } else if status.is_success() {
        let newly_minted = res_session_id.is_some() && req_session_id.is_none();
        if let Some(sid) = res_session_id.or(req_session_id) {
            shared.touch_http_session_with_init(&sid, user_agent.as_deref(), newly_minted);
        }
    } else if status.is_client_error() || status.is_server_error() {
        record_http_rejection_trace(
            &shared,
            &method,
            &uri,
            status,
            elapsed_ms,
            req_session_id,
            user_agent,
            origin,
            host,
        );
    }

    res
}

#[allow(clippy::too_many_arguments)]
fn record_http_rejection_trace(
    shared: &KeelSharedState,
    method: &Method,
    uri: &str,
    status: StatusCode,
    elapsed_ms: u32,
    req_session_id: Option<String>,
    user_agent: Option<String>,
    origin: Option<String>,
    host: Option<String>,
) {
    let trace_status = if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        McpTraceStatus::Blocked
    } else {
        McpTraceStatus::ProtocolError
    };
    let caller_hint = user_agent
        .as_deref()
        .map(|ua| ua.chars().take(48).collect::<String>())
        .or_else(|| {
            req_session_id
                .as_ref()
                .map(|sid| format!("session:{}", sid.chars().take(12).collect::<String>()))
        });
    let err_msg = format!(
        "HTTP {} {} rejected with status {}",
        method.as_str(),
        uri,
        status
    );

    shared.trace_store().record(NewProtocolTrace {
        transport: McpTraceTransport::StreamableHttp,
        direction: McpTraceDirection::Inbound,
        target: "keeldesk".to_string(),
        session_id: req_session_id.clone(),
        caller_hint,
        jsonrpc_id: None,
        method: format!("http/{}", method.as_str()),
        tool_name: None,
        safety_tier: None,
        approval_outcome: McpTraceApprovalOutcome::NotRequired,
        status: trace_status,
        status_code: Some(status.as_u16() as i32),
        error_message: Some(err_msg),
        latency: McpLatencyBreakdown {
            total_ms: elapsed_ms,
            approval_wait_ms: 0,
            execution_ms: elapsed_ms,
        },
        request_value: json!({
            "http_method": method.as_str(),
            "uri": uri,
            "host": host,
            "origin": origin,
            "user_agent": user_agent,
            "mcp_session_id": req_session_id,
        }),
        response_value: json!({
            "http_status": status.as_u16(),
            "reason": status.canonical_reason().unwrap_or("Unknown"),
        }),
        linked_call_id: None,
    });
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

    #[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema, PartialEq, Eq)]
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

        ().serve(transport)
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
        assert_eq!(
            mid_snap.session_allowlist[0].tool_name,
            "guarded_test_action"
        );

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
        assert_eq!(
            after_second.recent_calls[0].summary,
            "OK · session allowlisted"
        );

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
            .resolve_tool_approval(
                &rearmed_pending.request_id,
                ToolApprovalDecision::ApproveOnce,
            )
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
        let config =
            KeelConfig::ephemeral_for_test().with_approval_timeout(Duration::from_millis(120));
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
        assert_eq!(
            after.recent_calls[0].summary,
            "Blocked · approval timed out"
        );

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

    #[tokio::test]
    async fn client_cancelled_confirm_required_tool_cleans_up_pending_approval_promptly() {
        let runtime = KeelRuntime::start_headless(
            KeelConfig::ephemeral_for_test().with_approval_timeout(Duration::from_secs(60)),
            registry_with_guarded_tool(),
        );
        let snap = runtime.wait_until_ready(Duration::from_secs(3)).await;
        assert_eq!(snap.status, KeelServerStatus::Running);

        let http_client = reqwest::Client::builder()
            .no_proxy()
            .user_agent("coder-cancel-test/1.0")
            .build()
            .expect("reqwest client should build");

        let init_res = http_client
            .post(&snap.endpoint_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"cancel-test","version":"1.0.0"}}}"#)
            .send()
            .await
            .expect("initialize request should succeed");
        let session_id = init_res
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .expect("mcp-session-id header must be present")
            .to_string();
        let _ = init_res.text().await;

        let _ = http_client
            .post(&snap.endpoint_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("Mcp-Session-Id", &session_id)
            .body(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .send()
            .await
            .expect("notifications/initialized should succeed");

        let call_http = http_client.clone();
        let call_url = snap.endpoint_url.clone();
        let call_sid = session_id.clone();
        let in_flight_post = tokio::spawn(async move {
            call_http
                .post(&call_url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("Mcp-Session-Id", &call_sid)
                .body(r#"{"jsonrpc":"2.0","id":42,"method":"tools/call","params":{"name":"guarded_test_action","arguments":{"target":"cancel-me"}}}"#)
                .timeout(Duration::from_millis(250))
                .send()
                .await
        });

        let pending = wait_for_pending_approval(&runtime).await;
        assert_eq!(pending.tool_name, "guarded_test_action");
        assert_eq!(runtime.snapshot().pending_approvals.len(), 1);

        // Simulate external agent aborting its HTTP call and emitting MCP notifications/cancelled.
        in_flight_post.abort();
        let cancel_res = http_client
            .post(&snap.endpoint_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("Mcp-Session-Id", &session_id)
            .body(r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":42,"reason":"external agent timeout"}}"#)
            .send()
            .await
            .expect("notifications/cancelled should succeed");
        assert!(cancel_res.status().is_success());

        // PendingApprovalDropGuard must immediately remove apr-1 without waiting for the 60s timeout.
        let mut cleaned_up = false;
        for _ in 0..80 {
            if runtime.snapshot().pending_approvals.is_empty() {
                cleaned_up = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
        assert!(
            cleaned_up,
            "PendingApprovalDropGuard must clean up pending approval promptly on cancellation"
        );

        // Resolving a cancelled approval request afterwards must fail safely rather than executing.
        assert!(runtime
            .resolve_tool_approval(&pending.request_id, ToolApprovalDecision::ApproveOnce)
            .is_err());

        let after = runtime.snapshot();
        assert_eq!(after.total_calls, 1);
        assert_eq!(after.recent_calls.len(), 1);
        assert!(!after.recent_calls[0].succeeded);
        assert_eq!(
            after.recent_calls[0].summary,
            "Blocked · cancelled by caller"
        );

        runtime.shutdown();
    }

    #[tokio::test]
    async fn protocol_traces_capture_http_session_lifecycle_healthz_and_dns_rebinding_rejection() {
        let runtime =
            KeelRuntime::start_headless(KeelConfig::ephemeral_for_test(), KeelToolRegistry::new());
        let snap = runtime.wait_until_ready(Duration::from_secs(3)).await;
        assert_eq!(snap.status, KeelServerStatus::Running);
        assert!(snap.bound_port > 0);
        assert!(snap.active_sessions.is_empty());

        let health = runtime
            .check_healthz()
            .await
            .expect("loopback /healthz check should succeed");
        assert!(health.ok);
        assert_eq!(health.status_code, 200);
        assert_eq!(health.bound_port, snap.bound_port);
        assert_eq!(health.active_session_count, 0);

        let http_client = reqwest::Client::builder()
            .no_proxy()
            .user_agent("devtools-probe-client/1.0")
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let transport = StreamableHttpClientTransport::with_client(
            http_client,
            StreamableHttpClientTransportConfig::with_uri(snap.endpoint_url.clone()),
        );
        let client = ().serve(transport).await.expect("client should initialize");

        let tools = client.list_all_tools().await.expect("tools/list succeeds");
        assert!(tools.iter().any(|t| t.name == "keel_ping"));

        let mut args = JsonObject::new();
        args.insert(
            "message".to_string(),
            serde_json::Value::String("trace-check".to_string()),
        );
        let ping_res = client
            .call_tool(CallToolRequestParams::new("keel_ping").with_arguments(args))
            .await
            .expect("keel_ping should succeed");
        assert_ne!(ping_res.is_error, Some(true));

        let mid_snap = runtime.snapshot();
        assert_eq!(mid_snap.active_sessions.len(), 1);
        let session = &mid_snap.active_sessions[0];
        assert!(session.client_name.is_some());
        assert!(session.client_version.is_some());
        assert!(session.protocol_version.is_some());
        assert_eq!(
            session.user_agent.as_deref(),
            Some("devtools-probe-client/1.0")
        );
        assert!(session.request_count >= 3);

        let traces = runtime.protocol_traces();
        assert!(traces.iter().any(|t| t.method == "initialize"
            && t.transport == McpTraceTransport::StreamableHttp
            && t.direction == McpTraceDirection::Inbound
            && t.status == McpTraceStatus::Succeeded));
        assert!(traces
            .iter()
            .any(|t| t.method == "tools/list" && t.status == McpTraceStatus::Succeeded));
        let ping_trace = traces
            .iter()
            .find(|t| t.method == "tools/call" && t.tool_name.as_deref() == Some("keel_ping"))
            .expect("keel_ping trace should be recorded");
        assert_eq!(
            ping_trace.safety_tier,
            Some(super::super::types::ToolSafetyTier::Safe)
        );
        assert_eq!(
            ping_trace.approval_outcome,
            McpTraceApprovalOutcome::NotRequired
        );
        assert_eq!(ping_trace.status, McpTraceStatus::Succeeded);
        assert_eq!(ping_trace.linked_call_id, Some(1));
        assert!(ping_trace.request_json.contains("trace-check"));

        let _ = client.cancel().await;
        // Wait briefly for DELETE session cleanup if needed.
        let mut sessions_empty = false;
        for _ in 0..40 {
            if runtime.snapshot().active_sessions.is_empty() {
                sessions_empty = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
        assert!(
            sessions_empty,
            "active_sessions must be cleared when client cancels its HTTP session"
        );

        // Verify 403 DNS-rebinding rejection produces a Blocked http/POST trace.
        let raw_client = reqwest::Client::builder().no_proxy().build().unwrap();
        let rejected = raw_client
            .post(&snap.endpoint_url)
            .header("Host", "evil-rebind.example.com")
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
            .send()
            .await
            .expect("HTTP request completes");
        assert_eq!(rejected.status(), reqwest::StatusCode::FORBIDDEN);

        let after_reject = runtime.protocol_traces();
        let reject_trace = after_reject
            .iter()
            .find(|t| t.method == "http/POST" && t.status_code == Some(403))
            .expect("403 rejection should be recorded in protocol traces");
        assert_eq!(reject_trace.status, McpTraceStatus::Blocked);

        let cleared_count = runtime.clear_protocol_traces();
        assert!(cleared_count >= 4);
        assert!(runtime.protocol_traces().is_empty());

        runtime.shutdown();
    }

    #[tokio::test]
    async fn approval_gate_traces_record_wait_breakdown_and_denied_status() {
        let runtime = KeelRuntime::start_headless(
            KeelConfig::ephemeral_for_test(),
            registry_with_guarded_tool(),
        );
        let snap = runtime.wait_until_ready(Duration::from_secs(3)).await;
        let client = connect_test_mcp_client(&snap.endpoint_url).await;

        // 1. ApprovedOnce with measurable approval_wait_ms
        let call_client = client.peer().clone();
        let call_handle = tokio::spawn(async move {
            call_client
                .call_tool(
                    CallToolRequestParams::new("guarded_test_action")
                        .with_arguments(target_args("latency-probe")),
                )
                .await
        });

        let pending = wait_for_pending_approval(&runtime).await;
        tokio::time::sleep(Duration::from_millis(45)).await;
        runtime
            .resolve_tool_approval(&pending.request_id, ToolApprovalDecision::ApproveOnce)
            .expect("ApproveOnce should resolve");
        let call_res = call_handle
            .await
            .expect("join")
            .expect("approved call succeeds");
        assert_ne!(call_res.is_error, Some(true));

        let traces_after_approve = runtime.protocol_traces();
        let approved_trace = traces_after_approve
            .iter()
            .find(|t| {
                t.method == "tools/call" && t.tool_name.as_deref() == Some("guarded_test_action")
            })
            .expect("approved trace recorded");
        assert_eq!(
            approved_trace.safety_tier,
            Some(super::super::types::ToolSafetyTier::ConfirmRequired)
        );
        assert_eq!(
            approved_trace.approval_outcome,
            McpTraceApprovalOutcome::ApprovedOnce
        );
        assert_eq!(approved_trace.status, McpTraceStatus::Succeeded);
        assert_eq!(approved_trace.linked_call_id, Some(1));
        assert!(
            approved_trace.latency.approval_wait_ms >= 30,
            "expected approval_wait_ms >= 30, got {}",
            approved_trace.latency.approval_wait_ms
        );
        assert!(approved_trace.latency.total_ms >= approved_trace.latency.approval_wait_ms);
        assert!(approved_trace.latency.execution_ms <= approved_trace.latency.total_ms);

        // 2. Denied call produces McpTraceStatus::Blocked & McpTraceApprovalOutcome::Denied
        let deny_client = client.peer().clone();
        let deny_handle = tokio::spawn(async move {
            deny_client
                .call_tool(
                    CallToolRequestParams::new("guarded_test_action")
                        .with_arguments(target_args("deny-probe")),
                )
                .await
        });
        let deny_pending = wait_for_pending_approval(&runtime).await;
        runtime
            .resolve_tool_approval(&deny_pending.request_id, ToolApprovalDecision::Deny)
            .expect("Deny should resolve");
        let deny_res = deny_handle
            .await
            .expect("join")
            .expect("denied call returns tool error");
        assert_eq!(deny_res.is_error, Some(true));

        let traces_after_deny = runtime.protocol_traces();
        let denied_trace = traces_after_deny
            .iter()
            .find(|t| {
                t.method == "tools/call"
                    && t.tool_name.as_deref() == Some("guarded_test_action")
                    && t.linked_call_id == Some(2)
            })
            .expect("denied trace recorded");
        assert_eq!(
            denied_trace.approval_outcome,
            McpTraceApprovalOutcome::Denied
        );
        assert_eq!(denied_trace.status, McpTraceStatus::Blocked);
        assert_eq!(denied_trace.latency.execution_ms, 0);

        let _ = client.cancel().await;
        runtime.shutdown();
    }
}
