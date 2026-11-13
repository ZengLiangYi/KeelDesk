use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
    time::Instant,
};

use chrono::Utc;
use rmcp::{
    handler::server::{
        router::tool::{IntoToolRoute, ToolRouter},
        tool::ToolCallContext,
        wrapper::Parameters,
    },
    model::{
        CallToolRequestParams, CallToolResponse, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
    },
    service::RequestContext,
    tool, tool_router, ErrorData as McpError, Json, RoleServer, ServerHandler,
};
use serde::Deserialize;

use super::{
    config::KeelConfig,
    types::{
        KeelPingPayload, KeelRuntimeSnapshot, KeelServerStatus, KeelToolDescriptor,
        ToolCallLogEntry, ToolSafetyTier,
    },
};

const MAX_RECENT_CALL_LOGS: usize = 32;

pub type SnapshotNotifier = Arc<dyn Fn(KeelRuntimeSnapshot) + Send + Sync>;

#[derive(Debug)]
struct MutableRuntimeMetrics {
    status: KeelServerStatus,
    bound_port: u16,
    started_at_iso: Option<String>,
    error_message: Option<String>,
    total_calls: u32,
    next_call_id: u32,
    recent_calls: VecDeque<ToolCallLogEntry>,
}

pub struct KeelSharedState {
    pub config: KeelConfig,
    tools_meta: Vec<KeelToolDescriptor>,
    metrics: RwLock<MutableRuntimeMetrics>,
    notifier: Option<SnapshotNotifier>,
}

impl KeelSharedState {
    pub fn new(
        config: KeelConfig,
        tools_meta: Vec<KeelToolDescriptor>,
        notifier: Option<SnapshotNotifier>,
    ) -> Self {
        let initial_port = config.port;
        Self {
            config,
            tools_meta,
            metrics: RwLock::new(MutableRuntimeMetrics {
                status: KeelServerStatus::Starting,
                bound_port: initial_port,
                started_at_iso: None,
                error_message: None,
                total_calls: 0,
                next_call_id: 1,
                recent_calls: VecDeque::with_capacity(MAX_RECENT_CALL_LOGS),
            }),
            notifier,
        }
    }

    pub fn mark_running(&self, bound_port: u16) {
        if let Ok(mut guard) = self.metrics.write() {
            guard.status = KeelServerStatus::Running;
            guard.bound_port = bound_port;
            guard.started_at_iso = Some(Utc::now().to_rfc3339());
            guard.error_message = None;
        }
        self.emit_snapshot();
    }

    pub fn mark_error(&self, message: impl Into<String>) {
        if let Ok(mut guard) = self.metrics.write() {
            guard.status = KeelServerStatus::Error;
            guard.error_message = Some(message.into());
        }
        self.emit_snapshot();
    }

    pub fn mark_stopped(&self) {
        if let Ok(mut guard) = self.metrics.write() {
            guard.status = KeelServerStatus::Stopped;
        }
        self.emit_snapshot();
    }

    pub fn endpoint_url(&self) -> String {
        let port = self
            .metrics
            .read()
            .map(|g| g.bound_port)
            .unwrap_or(self.config.port);
        self.config.format_endpoint(port)
    }

    pub fn record_tool_call(
        &self,
        tool_name: String,
        duration_ms: u32,
        succeeded: bool,
        summary: String,
        caller_hint: Option<String>,
    ) {
        if let Ok(mut guard) = self.metrics.write() {
            let id = guard.next_call_id;
            guard.next_call_id = guard.next_call_id.saturating_add(1);
            guard.total_calls = guard.total_calls.saturating_add(1);

            if guard.recent_calls.len() >= MAX_RECENT_CALL_LOGS {
                guard.recent_calls.pop_back();
            }
            guard.recent_calls.push_front(ToolCallLogEntry {
                id,
                tool_name,
                timestamp_iso: Utc::now().to_rfc3339(),
                duration_ms,
                succeeded,
                summary,
                caller_hint,
            });
        }
        self.emit_snapshot();
    }

    pub fn snapshot(&self) -> KeelRuntimeSnapshot {
        let (status, bound_port, started_at_iso, error_message, total_calls, recent_calls) =
            match self.metrics.read() {
                Ok(guard) => (
                    guard.status,
                    guard.bound_port,
                    guard.started_at_iso.clone(),
                    guard.error_message.clone(),
                    guard.total_calls,
                    guard.recent_calls.iter().cloned().collect(),
                ),
                Err(_) => (
                    KeelServerStatus::Error,
                    self.config.port,
                    None,
                    Some("Runtime state lock poisoned".to_string()),
                    0,
                    Vec::new(),
                ),
            };

        KeelRuntimeSnapshot {
            status,
            endpoint_url: self.config.format_endpoint(bound_port),
            bind_addr: format!("{}:{}", self.config.bind_ip, bound_port),
            server_name: self.config.server_name.clone(),
            server_version: self.config.server_version.clone(),
            started_at_iso,
            error_message,
            total_calls,
            tools: self.tools_meta.clone(),
            recent_calls,
        }
    }

    fn emit_snapshot(&self) {
        if let Some(notifier) = &self.notifier {
            notifier(self.snapshot());
        }
    }
}

/// Registry enabling external business modules (such as `workbench`) to attach
/// MCP tools with explicit safety metadata without `keel_core` depending on them.
#[derive(Default, Clone)]
pub struct KeelToolRegistry {
    router: ToolRouter<KeelMcpServer>,
    descriptors: Vec<KeelToolDescriptor>,
}

impl KeelToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_route<R, A>(&mut self, source: &str, safety: ToolSafetyTier, route: R)
    where
        R: IntoToolRoute<KeelMcpServer, A>,
    {
        let tool_route = route.into_tool_route();
        let descriptor = KeelToolDescriptor {
            name: tool_route.attr.name.to_string(),
            description: tool_route
                .attr
                .description
                .as_deref()
                .unwrap_or_default()
                .to_string(),
            source: source.to_string(),
            safety,
        };
        self.router.add_route(tool_route);
        self.descriptors.push(descriptor);
    }

    pub fn merge_router(
        &mut self,
        source: &str,
        default_safety: ToolSafetyTier,
        router: ToolRouter<KeelMcpServer>,
    ) {
        for tool in router.list_all() {
            self.descriptors.push(KeelToolDescriptor {
                name: tool.name.to_string(),
                description: tool.description.as_deref().unwrap_or_default().to_string(),
                source: source.to_string(),
                safety: default_safety,
            });
        }
        self.router.merge(router);
    }

    pub(crate) fn into_parts(self) -> (ToolRouter<KeelMcpServer>, Vec<KeelToolDescriptor>) {
        (self.router, self.descriptors)
    }
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct KeelPingParams {
    /// Optional message echoed back by KeelDesk to verify round-trip serialization.
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Clone)]
pub struct KeelMcpServer {
    shared: Arc<KeelSharedState>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl KeelMcpServer {
    #[tool(
        name = "keel_ping",
        description = "Verify connectivity with the local KeelDesk Windows 11 MCP spine and return runtime status."
    )]
    async fn keel_ping(
        &self,
        Parameters(params): Parameters<KeelPingParams>,
    ) -> Json<KeelPingPayload> {
        let echo = params
            .message
            .map(|msg| msg.trim().to_string())
            .filter(|msg| !msg.is_empty());

        Json(KeelPingPayload {
            ok: true,
            server: self.shared.config.server_name.clone(),
            version: self.shared.config.server_version.clone(),
            transport: "streamable-http".to_string(),
            endpoint: self.shared.endpoint_url(),
            echo,
            timestamp_iso: Utc::now().to_rfc3339(),
        })
    }
}

impl KeelMcpServer {
    pub fn new(shared: Arc<KeelSharedState>, extra_router: ToolRouter<Self>) -> Self {
        let mut router = Self::tool_router();
        router.merge(extra_router);
        Self {
            shared,
            tool_router: router,
        }
    }

    pub fn builtin_tool_descriptors() -> Vec<KeelToolDescriptor> {
        Self::tool_router()
            .list_all()
            .into_iter()
            .map(|tool| KeelToolDescriptor {
                name: tool.name.to_string(),
                description: tool.description.as_deref().unwrap_or_default().to_string(),
                source: "keel_core".to_string(),
                safety: ToolSafetyTier::Safe,
            })
            .collect()
    }
}

fn extract_caller_hint(context: &RequestContext<RoleServer>) -> Option<String> {
    let parts = context.extensions.get::<http::request::Parts>()?;
    if let Some(ua) = parts
        .headers
        .get(http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Some(ua.chars().take(48).collect());
    }
    parts
        .headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|sid| format!("session:{}", sid.chars().take(12).collect::<String>()))
}

impl ServerHandler for KeelMcpServer {
    fn get_info(&self) -> ServerConfig {
        let mut server_info = Implementation::default();
        server_info.name = self.shared.config.server_name.clone();
        server_info.version = self.shared.config.server_version.clone();

        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(server_info)
            .with_instructions(
                "KeelDesk Windows 11 local desktop MCP spine. Use `keel_ping` to verify connectivity.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let mut result = ListToolsResult::default();
        result.tools = self.tool_router.list_all();
        Ok(result)
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tool_router.get(name).cloned()
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let tool_name = request.name.to_string();
        let arg_echo = request
            .arguments
            .as_ref()
            .and_then(|args| args.get("message"))
            .and_then(|val| val.as_str())
            .map(|s| s.to_string());
        let caller_hint = extract_caller_hint(&context);
        let started = Instant::now();

        // Safety-tier enforcement hook (`ConfirmRequired` interactive approval gating).
        let outcome = self
            .tool_router
            .call(ToolCallContext::new(self, request, context))
            .await;

        let duration_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
        match &outcome {
            Ok(_) => {
                let summary = match arg_echo {
                    Some(msg) if !msg.trim().is_empty() => format!("OK · echo=\"{}\"", msg.trim()),
                    _ => "OK · loopback pong".to_string(),
                };
                self.shared
                    .record_tool_call(tool_name, duration_ms, true, summary, caller_hint);
            }
            Err(err) => {
                self.shared.record_tool_call(
                    tool_name,
                    duration_ms,
                    false,
                    format!("Error {}: {}", err.code.0, err.message),
                    caller_hint,
                );
            }
        }

        outcome
    }
}
