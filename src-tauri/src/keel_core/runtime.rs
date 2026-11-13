use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{routing::get, Json as AxumJson, Router};
use rmcp::{
    model::{CallToolRequestParams, JsonObject},
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
    mcp_server::{KeelMcpServer, KeelSharedState, KeelToolRegistry, SnapshotNotifier},
    types::{
        KeelPingPayload, KeelPingSelfTestResult, KeelRuntimeSnapshot, KeelRuntimeUpdatedEvent,
        KeelServerStatus,
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
        let notifier: SnapshotNotifier = Arc::new(move |snapshot| {
            let _ = KeelRuntimeUpdatedEvent(snapshot).emit(&app_handle);
        });
        Self::start_internal(config, registry, Some(notifier), true)
    }

    #[cfg(test)]
    pub fn start_headless(config: KeelConfig, registry: KeelToolRegistry) -> Self {
        Self::start_internal(config, registry, None, false)
    }

    fn start_internal(
        config: KeelConfig,
        registry: KeelToolRegistry,
        notifier: Option<SnapshotNotifier>,
        use_tauri_runtime: bool,
    ) -> Self {
        let (extra_router, extra_descriptors) = registry.into_parts();
        let mut all_descriptors = KeelMcpServer::builtin_tool_descriptors();
        all_descriptors.extend(extra_descriptors);

        let shared = Arc::new(KeelSharedState::new(config.clone(), all_descriptors, notifier));
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

    pub fn shutdown(&self) {
        self.shutdown_token.cancel();
        self.shared.mark_stopped();
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
}
