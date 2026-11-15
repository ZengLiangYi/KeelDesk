use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use chrono::Utc;
use rmcp::{
    handler::server::{
        router::tool::{AsyncTool, IntoToolRoute, SyncTool, ToolRouter},
        tool::ToolCallContext,
        wrapper::Parameters,
    },
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
        InitializeRequestParams, InitializeResult, JsonObject, ListToolsResult,
        PaginatedRequestParams, RequestId, ServerCapabilities, ServerConfig, ServerResult, Tool,
    },
    service::RequestContext,
    tool, tool_router, ErrorData as McpError, Json, RoleServer, ServerHandler,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::oneshot;

use super::{
    config::KeelConfig,
    devtools::{
        McpLatencyBreakdown, McpProtocolTraceStore, McpTraceApprovalOutcome, McpTraceDirection,
        McpTraceStatus, McpTraceTransport, NewProtocolTrace,
    },
    types::{
        KeelPingPayload, KeelRuntimeSnapshot, KeelServerStatus, KeelToolDescriptor,
        McpSessionSummary, PendingToolApproval, SessionAllowlistEntry, ToolApprovalDecision,
        ToolCallLogEntry, ToolSafetyTier,
    },
};

const MAX_RECENT_CALL_LOGS: usize = 32;
const MAX_ACTIVE_SESSIONS: usize = 64;
const MAX_PENDING_INIT_META: usize = 32;
const PENDING_INIT_META_TTL: Duration = Duration::from_secs(10);
pub(crate) const KEEL_MCP_INSTRUCTIONS: &str =
    "KeelDesk Windows 11 local desktop MCP spine. Use `keel_ping` to verify connectivity.";

pub type SnapshotNotifier = Arc<dyn Fn(KeelRuntimeSnapshot) + Send + Sync>;
pub type ApprovalNotifier = Arc<dyn Fn(PendingToolApproval) + Send + Sync>;

#[derive(Debug)]
struct PendingApprovalSlot {
    pending: PendingToolApproval,
    responder: oneshot::Sender<ToolApprovalDecision>,
}

#[derive(Debug, Clone)]
struct InitializeClientMeta {
    user_agent: Option<String>,
    client_name: String,
    client_version: String,
    protocol_version: String,
    recorded_at: Instant,
}

#[derive(Debug)]
struct MutableRuntimeMetrics {
    status: KeelServerStatus,
    bound_port: u16,
    started_at_iso: Option<String>,
    error_message: Option<String>,
    total_calls: u32,
    next_call_id: u32,
    next_approval_id: u64,
    recent_calls: VecDeque<ToolCallLogEntry>,
    pending_approvals: Vec<PendingApprovalSlot>,
    session_allowlist: Vec<SessionAllowlistEntry>,
    active_sessions: VecDeque<McpSessionSummary>,
    pending_init_meta: VecDeque<InitializeClientMeta>,
}

pub struct KeelSharedState {
    pub config: KeelConfig,
    tools_meta: Vec<KeelToolDescriptor>,
    mcp_tools: Vec<Tool>,
    metrics: RwLock<MutableRuntimeMetrics>,
    trace_store: Arc<McpProtocolTraceStore>,
    notifier: Option<SnapshotNotifier>,
    approval_notifier: Option<ApprovalNotifier>,
}

impl KeelSharedState {
    pub fn new(
        config: KeelConfig,
        tools_meta: Vec<KeelToolDescriptor>,
        mcp_tools: Vec<Tool>,
        trace_store: Arc<McpProtocolTraceStore>,
        notifier: Option<SnapshotNotifier>,
        approval_notifier: Option<ApprovalNotifier>,
    ) -> Self {
        let initial_port = config.port;
        Self {
            config,
            tools_meta,
            mcp_tools,
            metrics: RwLock::new(MutableRuntimeMetrics {
                status: KeelServerStatus::Starting,
                bound_port: initial_port,
                started_at_iso: None,
                error_message: None,
                total_calls: 0,
                next_call_id: 1,
                next_approval_id: 1,
                recent_calls: VecDeque::with_capacity(MAX_RECENT_CALL_LOGS),
                pending_approvals: Vec::new(),
                session_allowlist: Vec::new(),
                active_sessions: VecDeque::with_capacity(MAX_ACTIVE_SESSIONS),
                pending_init_meta: VecDeque::new(),
            }),
            trace_store,
            notifier,
            approval_notifier,
        }
    }

    pub fn trace_store(&self) -> &Arc<McpProtocolTraceStore> {
        &self.trace_store
    }

    pub fn mcp_tools(&self) -> &[Tool] {
        &self.mcp_tools
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
        let drained = if let Ok(mut guard) = self.metrics.write() {
            guard.status = KeelServerStatus::Stopped;
            guard.active_sessions.clear();
            guard.pending_init_meta.clear();
            std::mem::take(&mut guard.pending_approvals)
        } else {
            Vec::new()
        };
        drop(drained);
        self.emit_snapshot();
    }

    pub fn touch_http_session(&self, session_id: &str, user_agent: Option<&str>) {
        self.touch_http_session_with_init(session_id, user_agent, false);
    }

    pub fn touch_http_session_with_init(
        &self,
        session_id: &str,
        user_agent: Option<&str>,
        consume_pending_init: bool,
    ) {
        let trimmed_id = session_id.trim();
        if trimmed_id.is_empty() {
            return;
        }
        let normalized_ua = normalize_user_agent(user_agent);
        let now_iso = Utc::now().to_rfc3339();

        let updated = if let Ok(mut guard) = self.metrics.write() {
            prune_stale_pending_init_meta(&mut guard.pending_init_meta);

            let init_meta = if consume_pending_init {
                consume_pending_init_meta(&mut guard.pending_init_meta, normalized_ua.as_deref())
            } else {
                None
            };

            if let Some(pos) = guard
                .active_sessions
                .iter()
                .position(|s| s.session_id == trimmed_id)
            {
                if let Some(mut existing) = guard.active_sessions.remove(pos) {
                    existing.request_count = existing.request_count.saturating_add(1);
                    existing.last_active_at_iso = now_iso;
                    if let Some(ua) = normalized_ua {
                        existing.user_agent = Some(ua);
                    }
                    if let Some(meta) = init_meta {
                        existing.client_name = Some(meta.client_name);
                        existing.client_version = Some(meta.client_version);
                        existing.protocol_version = Some(meta.protocol_version);
                    }
                    guard.active_sessions.push_front(existing);
                }
            } else {
                if guard.active_sessions.len() >= MAX_ACTIVE_SESSIONS {
                    guard.active_sessions.pop_back();
                }
                let (client_name, client_version, protocol_version) = match init_meta {
                    Some(meta) => (
                        Some(meta.client_name),
                        Some(meta.client_version),
                        Some(meta.protocol_version),
                    ),
                    None => (None, None, None),
                };
                guard.active_sessions.push_front(McpSessionSummary {
                    session_id: trimmed_id.to_string(),
                    client_name,
                    client_version,
                    protocol_version,
                    user_agent: normalized_ua,
                    connected_at_iso: now_iso.clone(),
                    last_active_at_iso: now_iso,
                    request_count: 1,
                });
            }
            true
        } else {
            false
        };

        if updated {
            self.emit_snapshot();
        }
    }

    pub fn enrich_http_session_client(
        &self,
        session_id: Option<&str>,
        user_agent: Option<&str>,
        client_name: String,
        client_version: String,
        protocol_version: String,
    ) {
        let normalized_ua = normalize_user_agent(user_agent);
        let mut updated_existing = false;

        if let Ok(mut guard) = self.metrics.write() {
            prune_stale_pending_init_meta(&mut guard.pending_init_meta);

            if let Some(sid) = session_id.map(str::trim).filter(|s| !s.is_empty()) {
                if let Some(existing) = guard
                    .active_sessions
                    .iter_mut()
                    .find(|s| s.session_id == sid)
                {
                    existing.client_name = Some(client_name.clone());
                    existing.client_version = Some(client_version.clone());
                    existing.protocol_version = Some(protocol_version.clone());
                    if existing.user_agent.is_none() {
                        existing.user_agent = normalized_ua.clone();
                    }
                    updated_existing = true;
                }
            }

            if !updated_existing {
                if guard.pending_init_meta.len() >= MAX_PENDING_INIT_META {
                    guard.pending_init_meta.pop_front();
                }
                guard.pending_init_meta.push_back(InitializeClientMeta {
                    user_agent: normalized_ua,
                    client_name,
                    client_version,
                    protocol_version,
                    recorded_at: Instant::now(),
                });
            }
        }

        if updated_existing {
            self.emit_snapshot();
        }
    }

    pub fn remove_http_session(&self, session_id: &str) {
        let trimmed_id = session_id.trim();
        if trimmed_id.is_empty() {
            return;
        }
        let removed = if let Ok(mut guard) = self.metrics.write() {
            let before = guard.active_sessions.len();
            guard.active_sessions.retain(|s| s.session_id != trimmed_id);
            guard.active_sessions.len() != before
        } else {
            false
        };

        if removed {
            self.emit_snapshot();
        }
    }

    pub fn endpoint_url(&self) -> String {
        let port = self
            .metrics
            .read()
            .map(|g| g.bound_port)
            .unwrap_or(self.config.port);
        self.config.format_endpoint(port)
    }

    pub fn tool_descriptor(&self, tool_name: &str) -> Option<&KeelToolDescriptor> {
        self.tools_meta.iter().find(|d| d.name == tool_name)
    }

    pub fn is_session_allowlisted(&self, session_scope: &str, tool_name: &str) -> bool {
        self.metrics
            .read()
            .map(|guard| {
                guard.session_allowlist.iter().any(|entry| {
                    entry.session_scope == session_scope && entry.tool_name == tool_name
                })
            })
            .unwrap_or(false)
    }

    pub fn enqueue_tool_approval(
        &self,
        tool_name: &str,
        tool_source: &str,
        description: &str,
        arguments_json: String,
        caller_hint: Option<String>,
        session_scope: String,
    ) -> (PendingToolApproval, oneshot::Receiver<ToolApprovalDecision>) {
        let now = Utc::now();
        let approval_timeout = self.config.approval_timeout;
        let chrono_timeout = chrono::Duration::from_std(approval_timeout)
            .unwrap_or_else(|_| chrono::Duration::seconds(60));
        let expires_at = now + chrono_timeout;
        let timeout_secs = approval_timeout
            .as_secs_f64()
            .ceil()
            .clamp(0.0, u32::MAX as f64) as u32;

        let (tx, rx) = oneshot::channel();

        let pending = {
            let mut guard = self
                .metrics
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let seq = guard.next_approval_id;
            guard.next_approval_id = guard.next_approval_id.saturating_add(1);

            let pending = PendingToolApproval {
                request_id: format!("apr-{seq}"),
                tool_name: tool_name.to_string(),
                tool_source: tool_source.to_string(),
                description: description.to_string(),
                arguments_json,
                caller_hint,
                session_scope,
                requested_at_iso: now.to_rfc3339(),
                expires_at_iso: expires_at.to_rfc3339(),
                timeout_secs,
            };

            guard.pending_approvals.push(PendingApprovalSlot {
                pending: pending.clone(),
                responder: tx,
            });

            pending
        };

        self.emit_snapshot();
        if let Some(approval_notifier) = &self.approval_notifier {
            approval_notifier(pending.clone());
        }

        (pending, rx)
    }

    pub fn remove_pending_approval(&self, request_id: &str) -> bool {
        let removed = if let Ok(mut guard) = self.metrics.write() {
            if let Some(pos) = guard
                .pending_approvals
                .iter()
                .position(|slot| slot.pending.request_id == request_id)
            {
                guard.pending_approvals.remove(pos);
                true
            } else {
                false
            }
        } else {
            false
        };

        if removed {
            self.emit_snapshot();
        }
        removed
    }

    pub fn resolve_approval(
        &self,
        request_id: &str,
        decision: ToolApprovalDecision,
    ) -> Result<PendingToolApproval, String> {
        let trimmed_id = request_id.trim();
        let slot = {
            let mut guard = self
                .metrics
                .write()
                .map_err(|_| "Runtime state lock poisoned".to_string())?;
            let pos = guard
                .pending_approvals
                .iter()
                .position(|entry| entry.pending.request_id == trimmed_id)
                .ok_or_else(|| {
                    format!(
                        "Approval request '{}' was not found or has already expired.",
                        trimmed_id
                    )
                })?;

            let removed = guard.pending_approvals.remove(pos);

            if decision == ToolApprovalDecision::ApproveForSession {
                let already_present = guard.session_allowlist.iter().any(|entry| {
                    entry.session_scope == removed.pending.session_scope
                        && entry.tool_name == removed.pending.tool_name
                });
                if !already_present {
                    guard.session_allowlist.push(SessionAllowlistEntry {
                        session_scope: removed.pending.session_scope.clone(),
                        tool_name: removed.pending.tool_name.clone(),
                        granted_at_iso: Utc::now().to_rfc3339(),
                    });
                }
            }

            removed
        };

        let pending = slot.pending;
        let _ = slot.responder.send(decision);
        self.emit_snapshot();
        Ok(pending)
    }

    pub fn clear_session_allowlist(&self) -> KeelRuntimeSnapshot {
        if let Ok(mut guard) = self.metrics.write() {
            guard.session_allowlist.clear();
        }
        let snap = self.snapshot();
        if let Some(notifier) = &self.notifier {
            notifier(snap.clone());
        }
        snap
    }

    pub fn record_tool_call(
        &self,
        tool_name: String,
        duration_ms: u32,
        succeeded: bool,
        summary: String,
        caller_hint: Option<String>,
    ) -> u32 {
        let assigned_id = if let Ok(mut guard) = self.metrics.write() {
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
            id
        } else {
            0
        };
        self.emit_snapshot();
        assigned_id
    }

    pub fn snapshot(&self) -> KeelRuntimeSnapshot {
        let (
            status,
            bound_port,
            started_at_iso,
            error_message,
            total_calls,
            recent_calls,
            pending_approvals,
            session_allowlist,
            active_sessions,
        ) = match self.metrics.read() {
            Ok(guard) => (
                guard.status,
                guard.bound_port,
                guard.started_at_iso.clone(),
                guard.error_message.clone(),
                guard.total_calls,
                guard.recent_calls.iter().cloned().collect(),
                guard
                    .pending_approvals
                    .iter()
                    .map(|slot| slot.pending.clone())
                    .collect(),
                guard.session_allowlist.clone(),
                guard.active_sessions.iter().cloned().collect(),
            ),
            Err(_) => (
                KeelServerStatus::Error,
                self.config.port,
                None,
                Some("Runtime state lock poisoned".to_string()),
                0,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
        };

        KeelRuntimeSnapshot {
            status,
            endpoint_url: self.config.format_endpoint(bound_port),
            bind_addr: format!("{}:{}", self.config.bind_ip, bound_port),
            bound_port,
            server_name: self.config.server_name.clone(),
            server_version: self.config.server_version.clone(),
            started_at_iso,
            error_message,
            total_calls,
            tools: self.tools_meta.clone(),
            recent_calls,
            pending_approvals,
            session_allowlist,
            active_sessions,
        }
    }

    fn emit_snapshot(&self) {
        if let Some(notifier) = &self.notifier {
            notifier(self.snapshot());
        }
    }
}

fn normalize_user_agent(user_agent: Option<&str>) -> Option<String> {
    user_agent
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(128).collect())
}

fn prune_stale_pending_init_meta(queue: &mut VecDeque<InitializeClientMeta>) {
    while let Some(front) = queue.front() {
        if front.recorded_at.elapsed() > PENDING_INIT_META_TTL {
            queue.pop_front();
        } else {
            break;
        }
    }
}

fn consume_pending_init_meta(
    queue: &mut VecDeque<InitializeClientMeta>,
    user_agent: Option<&str>,
) -> Option<InitializeClientMeta> {
    if queue.is_empty() {
        return None;
    }
    if let Some(ua) = user_agent {
        if let Some(pos) = queue
            .iter()
            .position(|item| item.user_agent.as_deref() == Some(ua))
        {
            return queue.remove(pos);
        }
    } else if let Some(pos) = queue.iter().position(|item| item.user_agent.is_none()) {
        return queue.remove(pos);
    }
    queue.pop_front()
}

struct PendingApprovalDropGuard {
    shared: Arc<KeelSharedState>,
    request_id: String,
}

impl Drop for PendingApprovalDropGuard {
    fn drop(&mut self) {
        self.shared.remove_pending_approval(&self.request_id);
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
        let name = tool_route.attr.name.to_string();
        let descriptor = KeelToolDescriptor {
            name: name.clone(),
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
        self.descriptors.retain(|d| d.name != name);
        self.descriptors.push(descriptor);
    }

    pub fn register_async_tool<T>(&mut self, source: &str, safety: ToolSafetyTier)
    where
        T: AsyncTool<KeelMcpServer> + 'static,
    {
        self.merge_router(source, safety, ToolRouter::new().with_async_tool::<T>());
    }

    pub fn register_sync_tool<T>(&mut self, source: &str, safety: ToolSafetyTier)
    where
        T: SyncTool<KeelMcpServer> + 'static,
    {
        self.merge_router(source, safety, ToolRouter::new().with_sync_tool::<T>());
    }

    pub fn merge_router(
        &mut self,
        source: &str,
        default_safety: ToolSafetyTier,
        router: ToolRouter<KeelMcpServer>,
    ) {
        for tool in router.list_all() {
            let name = tool.name.to_string();
            self.descriptors.retain(|d| d.name != name);
            self.descriptors.push(KeelToolDescriptor {
                name,
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
        let tool_router = Self::merged_tool_router(extra_router);
        Self {
            shared,
            tool_router,
        }
    }

    pub(crate) fn from_router(shared: Arc<KeelSharedState>, tool_router: ToolRouter<Self>) -> Self {
        Self {
            shared,
            tool_router,
        }
    }

    pub(crate) fn merged_tool_router(extra_router: ToolRouter<Self>) -> ToolRouter<Self> {
        let mut router = Self::tool_router();
        router.merge(extra_router);
        router
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

fn extract_raw_session_id(context: &RequestContext<RoleServer>) -> Option<String> {
    context
        .extensions
        .get::<http::request::Parts>()
        .and_then(|parts| parts.headers.get("mcp-session-id"))
        .and_then(|val| val.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_user_agent(context: &RequestContext<RoleServer>) -> Option<String> {
    context
        .extensions
        .get::<http::request::Parts>()
        .and_then(|parts| parts.headers.get(http::header::USER_AGENT))
        .and_then(|val| val.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn format_request_id(id: &RequestId) -> String {
    match id {
        RequestId::Number(n) => n.to_string(),
        RequestId::String(s) => s.to_string(),
    }
}

fn extract_caller_hint(context: &RequestContext<RoleServer>) -> Option<String> {
    if let Some(parts) = context.extensions.get::<http::request::Parts>() {
        if let Some(ua) = parts
            .headers
            .get(http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(ua.chars().take(48).collect());
        }
        if let Some(sid) = parts
            .headers
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(format!(
                "session:{}",
                sid.chars().take(12).collect::<String>()
            ));
        }
    }
    context
        .client_info()
        .map(|info| format!("{}/{}", info.name, info.version))
}

fn serialize_arguments_json(arguments: Option<&JsonObject>) -> String {
    match arguments {
        Some(map) => serde_json::to_string(map).unwrap_or_else(|_| "{}".to_string()),
        None => "{}".to_string(),
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let mut chars = input.chars();
    let taken: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!(
            "{}…",
            taken
                .chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        taken
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApprovalTrace {
    NotRequired,
    ApprovedOnce,
    ApprovedForSession,
    SessionAllowlisted,
}

impl From<ApprovalTrace> for McpTraceApprovalOutcome {
    fn from(value: ApprovalTrace) -> Self {
        match value {
            ApprovalTrace::NotRequired => McpTraceApprovalOutcome::NotRequired,
            ApprovalTrace::ApprovedOnce => McpTraceApprovalOutcome::ApprovedOnce,
            ApprovalTrace::ApprovedForSession => McpTraceApprovalOutcome::ApprovedForSession,
            ApprovalTrace::SessionAllowlisted => McpTraceApprovalOutcome::SessionAllowlisted,
        }
    }
}

#[derive(Debug)]
enum ApprovalWaitOutcome {
    Decided(Result<ToolApprovalDecision, oneshot::error::RecvError>),
    Cancelled,
    TimedOut,
}

impl ServerHandler for KeelMcpServer {
    fn get_info(&self) -> ServerConfig {
        let server_info = Implementation::new(
            self.shared.config.server_name.clone(),
            self.shared.config.server_version.clone(),
        );

        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(server_info)
            .with_instructions(KEEL_MCP_INSTRUCTIONS)
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        let started = Instant::now();
        let req_id_json = context.id.clone().into_json_value();
        let req_id_str = format_request_id(&context.id);
        let session_id = extract_raw_session_id(&context);
        let user_agent = extract_user_agent(&context);
        let caller_hint = extract_caller_hint(&context).or_else(|| {
            Some(format!(
                "{}/{}",
                request.client_info.name, request.client_info.version
            ))
        });

        if context.peer.peer_info().is_none() {
            context.peer.set_peer_info(request.clone());
        }
        let result = self.negotiate_initialize(&request);
        let elapsed_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;

        let req_envelope = json!({
            "jsonrpc": "2.0",
            "id": &req_id_json,
            "method": "initialize",
            "params": &request,
        });

        match &result {
            Ok(init_res) => {
                self.shared.enrich_http_session_client(
                    session_id.as_deref(),
                    user_agent.as_deref(),
                    request.client_info.name.clone(),
                    request.client_info.version.clone(),
                    init_res.protocol_version.to_string(),
                );
                self.shared.trace_store().record(NewProtocolTrace {
                    transport: McpTraceTransport::StreamableHttp,
                    direction: McpTraceDirection::Inbound,
                    target: "keeldesk".to_string(),
                    session_id,
                    caller_hint,
                    jsonrpc_id: Some(req_id_str),
                    method: "initialize".to_string(),
                    tool_name: None,
                    safety_tier: None,
                    approval_outcome: McpTraceApprovalOutcome::NotRequired,
                    status: McpTraceStatus::Succeeded,
                    status_code: Some(200),
                    error_message: None,
                    latency: McpLatencyBreakdown {
                        total_ms: elapsed_ms,
                        approval_wait_ms: 0,
                        execution_ms: elapsed_ms,
                    },
                    request_value: req_envelope,
                    response_value: json!({
                        "jsonrpc": "2.0",
                        "id": &req_id_json,
                        "result": init_res,
                    }),
                    linked_call_id: None,
                });
            }
            Err(err) => {
                self.shared.trace_store().record(NewProtocolTrace {
                    transport: McpTraceTransport::StreamableHttp,
                    direction: McpTraceDirection::Inbound,
                    target: "keeldesk".to_string(),
                    session_id,
                    caller_hint,
                    jsonrpc_id: Some(req_id_str),
                    method: "initialize".to_string(),
                    tool_name: None,
                    safety_tier: None,
                    approval_outcome: McpTraceApprovalOutcome::NotRequired,
                    status: McpTraceStatus::ProtocolError,
                    status_code: Some(err.code.0),
                    error_message: Some(err.message.to_string()),
                    latency: McpLatencyBreakdown {
                        total_ms: elapsed_ms,
                        approval_wait_ms: 0,
                        execution_ms: elapsed_ms,
                    },
                    request_value: req_envelope,
                    response_value: json!({
                        "jsonrpc": "2.0",
                        "id": &req_id_json,
                        "error": err,
                    }),
                    linked_call_id: None,
                });
            }
        }

        result
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let started = Instant::now();
        let req_id_json = context.id.clone().into_json_value();
        let req_id_str = format_request_id(&context.id);
        let session_id = extract_raw_session_id(&context);
        let caller_hint = extract_caller_hint(&context);
        let params_val = match &request {
            Some(p) => serde_json::to_value(p).unwrap_or_else(|_| json!({})),
            None => json!({}),
        };

        let result = ListToolsResult::with_all_items(self.tool_router.list_all());
        let elapsed_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;

        self.shared.trace_store().record(NewProtocolTrace {
            transport: McpTraceTransport::StreamableHttp,
            direction: McpTraceDirection::Inbound,
            target: "keeldesk".to_string(),
            session_id,
            caller_hint,
            jsonrpc_id: Some(req_id_str),
            method: "tools/list".to_string(),
            tool_name: None,
            safety_tier: None,
            approval_outcome: McpTraceApprovalOutcome::NotRequired,
            status: McpTraceStatus::Succeeded,
            status_code: Some(200),
            error_message: None,
            latency: McpLatencyBreakdown {
                total_ms: elapsed_ms,
                approval_wait_ms: 0,
                execution_ms: elapsed_ms,
            },
            request_value: json!({
                "jsonrpc": "2.0",
                "id": &req_id_json,
                "method": "tools/list",
                "params": params_val,
            }),
            response_value: json!({
                "jsonrpc": "2.0",
                "id": &req_id_json,
                "result": &result,
            }),
            linked_call_id: None,
        });

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
        let arguments_json = serialize_arguments_json(request.arguments.as_ref());
        let arg_echo = request
            .arguments
            .as_ref()
            .and_then(|args| args.get("message"))
            .and_then(|val| val.as_str())
            .map(|s| s.to_string());
        let caller_hint = extract_caller_hint(&context);
        let session_id = extract_raw_session_id(&context);
        let session_scope = session_id.clone().unwrap_or_else(|| "global".to_string());
        let safety_tier = self.shared.tool_descriptor(&tool_name).map(|d| d.safety);
        let req_id_json = context.id.clone().into_json_value();
        let req_id_str = format_request_id(&context.id);
        let req_envelope = json!({
            "jsonrpc": "2.0",
            "id": &req_id_json,
            "method": "tools/call",
            "params": &request,
        });
        let started = Instant::now();

        if context.ct.is_cancelled() {
            let duration_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
            let call_id = self.shared.record_tool_call(
                tool_name.clone(),
                duration_ms,
                false,
                "Blocked · cancelled by caller".to_string(),
                caller_hint.clone(),
            );
            let err_msg = format!(
                "Request for tool '{}' was cancelled before execution.",
                tool_name
            );
            let err_result = CallToolResult::error(vec![ContentBlock::text(err_msg.clone())]);
            self.shared.trace_store().record(NewProtocolTrace {
                transport: McpTraceTransport::StreamableHttp,
                direction: McpTraceDirection::Inbound,
                target: "keeldesk".to_string(),
                session_id,
                caller_hint,
                jsonrpc_id: Some(req_id_str),
                method: "tools/call".to_string(),
                tool_name: Some(tool_name),
                safety_tier,
                approval_outcome: McpTraceApprovalOutcome::Cancelled,
                status: McpTraceStatus::Blocked,
                status_code: Some(200),
                error_message: Some(err_msg),
                latency: McpLatencyBreakdown {
                    total_ms: duration_ms,
                    approval_wait_ms: 0,
                    execution_ms: 0,
                },
                request_value: req_envelope,
                response_value: json!({
                    "jsonrpc": "2.0",
                    "id": &req_id_json,
                    "result": &err_result,
                }),
                linked_call_id: Some(call_id),
            });
            return Ok(err_result.into());
        }

        let mut approval_trace = ApprovalTrace::NotRequired;
        let mut approval_wait_ms: u32 = 0;

        if let Some(descriptor) = self.shared.tool_descriptor(&tool_name).cloned() {
            if descriptor.safety == ToolSafetyTier::ConfirmRequired {
                if self
                    .shared
                    .is_session_allowlisted(&session_scope, &tool_name)
                {
                    approval_trace = ApprovalTrace::SessionAllowlisted;
                } else {
                    let cancel_token = context.ct.clone();
                    let wait_started = Instant::now();
                    let wait_outcome = {
                        let (pending, rx) = self.shared.enqueue_tool_approval(
                            &tool_name,
                            &descriptor.source,
                            &descriptor.description,
                            arguments_json.clone(),
                            caller_hint.clone(),
                            session_scope,
                        );
                        let _cleanup_guard = PendingApprovalDropGuard {
                            shared: Arc::clone(&self.shared),
                            request_id: pending.request_id,
                        };
                        tokio::select! {
                            biased;
                            _ = cancel_token.cancelled() => ApprovalWaitOutcome::Cancelled,
                            timed = tokio::time::timeout(self.shared.config.approval_timeout, rx) => {
                                match timed {
                                    Ok(decision) => ApprovalWaitOutcome::Decided(decision),
                                    Err(_) => ApprovalWaitOutcome::TimedOut,
                                }
                            }
                        }
                    };
                    approval_wait_ms =
                        wait_started.elapsed().as_millis().min(u32::MAX as u128) as u32;

                    match wait_outcome {
                        ApprovalWaitOutcome::Decided(Ok(ToolApprovalDecision::ApproveOnce)) => {
                            approval_trace = ApprovalTrace::ApprovedOnce;
                        }
                        ApprovalWaitOutcome::Decided(Ok(
                            ToolApprovalDecision::ApproveForSession,
                        )) => {
                            approval_trace = ApprovalTrace::ApprovedForSession;
                        }
                        ApprovalWaitOutcome::Decided(Ok(ToolApprovalDecision::Deny)) => {
                            let duration_ms =
                                started.elapsed().as_millis().min(u32::MAX as u128) as u32;
                            let call_id = self.shared.record_tool_call(
                                tool_name.clone(),
                                duration_ms,
                                false,
                                "Blocked · denied by desktop approval gate".to_string(),
                                caller_hint.clone(),
                            );
                            let err_msg = format!(
                                "User denied execution of ConfirmRequired tool '{}' in KeelDesk.",
                                tool_name
                            );
                            let err_result =
                                CallToolResult::error(vec![ContentBlock::text(err_msg.clone())]);
                            self.shared.trace_store().record(NewProtocolTrace {
                                transport: McpTraceTransport::StreamableHttp,
                                direction: McpTraceDirection::Inbound,
                                target: "keeldesk".to_string(),
                                session_id,
                                caller_hint,
                                jsonrpc_id: Some(req_id_str),
                                method: "tools/call".to_string(),
                                tool_name: Some(tool_name),
                                safety_tier,
                                approval_outcome: McpTraceApprovalOutcome::Denied,
                                status: McpTraceStatus::Blocked,
                                status_code: Some(200),
                                error_message: Some(err_msg),
                                latency: McpLatencyBreakdown {
                                    total_ms: duration_ms,
                                    approval_wait_ms,
                                    execution_ms: 0,
                                },
                                request_value: req_envelope,
                                response_value: json!({
                                    "jsonrpc": "2.0",
                                    "id": &req_id_json,
                                    "result": &err_result,
                                }),
                                linked_call_id: Some(call_id),
                            });
                            return Ok(err_result.into());
                        }
                        ApprovalWaitOutcome::Cancelled => {
                            let duration_ms =
                                started.elapsed().as_millis().min(u32::MAX as u128) as u32;
                            let call_id = self.shared.record_tool_call(
                                tool_name.clone(),
                                duration_ms,
                                false,
                                "Blocked · cancelled by caller".to_string(),
                                caller_hint.clone(),
                            );
                            let err_msg = format!(
                                "Cancelled while waiting for desktop user approval for tool '{}'.",
                                tool_name
                            );
                            let err_result =
                                CallToolResult::error(vec![ContentBlock::text(err_msg.clone())]);
                            self.shared.trace_store().record(NewProtocolTrace {
                                transport: McpTraceTransport::StreamableHttp,
                                direction: McpTraceDirection::Inbound,
                                target: "keeldesk".to_string(),
                                session_id,
                                caller_hint,
                                jsonrpc_id: Some(req_id_str),
                                method: "tools/call".to_string(),
                                tool_name: Some(tool_name),
                                safety_tier,
                                approval_outcome: McpTraceApprovalOutcome::Cancelled,
                                status: McpTraceStatus::Blocked,
                                status_code: Some(200),
                                error_message: Some(err_msg),
                                latency: McpLatencyBreakdown {
                                    total_ms: duration_ms,
                                    approval_wait_ms,
                                    execution_ms: 0,
                                },
                                request_value: req_envelope,
                                response_value: json!({
                                    "jsonrpc": "2.0",
                                    "id": &req_id_json,
                                    "result": &err_result,
                                }),
                                linked_call_id: Some(call_id),
                            });
                            return Ok(err_result.into());
                        }
                        ApprovalWaitOutcome::Decided(Err(_)) | ApprovalWaitOutcome::TimedOut => {
                            let duration_ms =
                                started.elapsed().as_millis().min(u32::MAX as u128) as u32;
                            let call_id = self.shared.record_tool_call(
                                tool_name.clone(),
                                duration_ms,
                                false,
                                "Blocked · approval timed out".to_string(),
                                caller_hint.clone(),
                            );
                            let err_msg = format!(
                                "Timed out waiting for desktop user approval for tool '{}'.",
                                tool_name
                            );
                            let err_result =
                                CallToolResult::error(vec![ContentBlock::text(err_msg.clone())]);
                            self.shared.trace_store().record(NewProtocolTrace {
                                transport: McpTraceTransport::StreamableHttp,
                                direction: McpTraceDirection::Inbound,
                                target: "keeldesk".to_string(),
                                session_id,
                                caller_hint,
                                jsonrpc_id: Some(req_id_str),
                                method: "tools/call".to_string(),
                                tool_name: Some(tool_name),
                                safety_tier,
                                approval_outcome: McpTraceApprovalOutcome::TimedOut,
                                status: McpTraceStatus::Blocked,
                                status_code: Some(200),
                                error_message: Some(err_msg),
                                latency: McpLatencyBreakdown {
                                    total_ms: duration_ms,
                                    approval_wait_ms,
                                    execution_ms: 0,
                                },
                                request_value: req_envelope,
                                response_value: json!({
                                    "jsonrpc": "2.0",
                                    "id": &req_id_json,
                                    "result": &err_result,
                                }),
                                linked_call_id: Some(call_id),
                            });
                            return Ok(err_result.into());
                        }
                    }
                }
            }
        }

        if context.ct.is_cancelled() {
            let duration_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
            let call_id = self.shared.record_tool_call(
                tool_name.clone(),
                duration_ms,
                false,
                "Blocked · cancelled by caller".to_string(),
                caller_hint.clone(),
            );
            let err_msg = format!(
                "Request for tool '{}' was cancelled before execution.",
                tool_name
            );
            let err_result = CallToolResult::error(vec![ContentBlock::text(err_msg.clone())]);
            self.shared.trace_store().record(NewProtocolTrace {
                transport: McpTraceTransport::StreamableHttp,
                direction: McpTraceDirection::Inbound,
                target: "keeldesk".to_string(),
                session_id,
                caller_hint,
                jsonrpc_id: Some(req_id_str),
                method: "tools/call".to_string(),
                tool_name: Some(tool_name),
                safety_tier,
                approval_outcome: McpTraceApprovalOutcome::Cancelled,
                status: McpTraceStatus::Blocked,
                status_code: Some(200),
                error_message: Some(err_msg),
                latency: McpLatencyBreakdown {
                    total_ms: duration_ms,
                    approval_wait_ms,
                    execution_ms: 0,
                },
                request_value: req_envelope,
                response_value: json!({
                    "jsonrpc": "2.0",
                    "id": &req_id_json,
                    "result": &err_result,
                }),
                linked_call_id: Some(call_id),
            });
            return Ok(err_result.into());
        }

        let exec_started = Instant::now();
        let outcome = self
            .tool_router
            .call(ToolCallContext::new(self, request, context))
            .await;
        let execution_ms = exec_started.elapsed().as_millis().min(u32::MAX as u128) as u32;
        let duration_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
        let approval_outcome: McpTraceApprovalOutcome = approval_trace.into();

        match &outcome {
            Ok(response) => {
                let server_result = ServerResult::from(response.clone());
                if let CallToolResponse::Complete(res) = response {
                    if res.is_error == Some(true) {
                        let raw_detail = res
                            .content
                            .iter()
                            .find_map(|block| block.as_text().map(|t| t.text.trim()))
                            .filter(|text| !text.is_empty())
                            .map(ToOwned::to_owned);
                        let summary = match raw_detail.as_deref() {
                            Some(msg) => format!("Tool error · {}", truncate_chars(msg, 48)),
                            None => "Tool error".to_string(),
                        };
                        let call_id = self.shared.record_tool_call(
                            tool_name.clone(),
                            duration_ms,
                            false,
                            summary,
                            caller_hint.clone(),
                        );
                        self.shared.trace_store().record(NewProtocolTrace {
                            transport: McpTraceTransport::StreamableHttp,
                            direction: McpTraceDirection::Inbound,
                            target: "keeldesk".to_string(),
                            session_id,
                            caller_hint,
                            jsonrpc_id: Some(req_id_str),
                            method: "tools/call".to_string(),
                            tool_name: Some(tool_name),
                            safety_tier,
                            approval_outcome,
                            status: McpTraceStatus::ToolError,
                            status_code: Some(200),
                            error_message: raw_detail,
                            latency: McpLatencyBreakdown {
                                total_ms: duration_ms,
                                approval_wait_ms,
                                execution_ms,
                            },
                            request_value: req_envelope,
                            response_value: json!({
                                "jsonrpc": "2.0",
                                "id": &req_id_json,
                                "result": &server_result,
                            }),
                            linked_call_id: Some(call_id),
                        });
                        return outcome;
                    }
                }

                let summary = if tool_name == "keel_ping" {
                    match arg_echo {
                        Some(msg) if !msg.trim().is_empty() => {
                            format!("OK · echo=\"{}\"", msg.trim())
                        }
                        _ => "OK · loopback pong".to_string(),
                    }
                } else {
                    match approval_trace {
                        ApprovalTrace::ApprovedOnce => "OK · approved once".to_string(),
                        ApprovalTrace::ApprovedForSession | ApprovalTrace::SessionAllowlisted => {
                            "OK · session allowlisted".to_string()
                        }
                        ApprovalTrace::NotRequired => {
                            if arguments_json == "{}" {
                                "OK".to_string()
                            } else {
                                format!("OK · {}", truncate_chars(&arguments_json, 48))
                            }
                        }
                    }
                };
                let call_id = self.shared.record_tool_call(
                    tool_name.clone(),
                    duration_ms,
                    true,
                    summary,
                    caller_hint.clone(),
                );
                self.shared.trace_store().record(NewProtocolTrace {
                    transport: McpTraceTransport::StreamableHttp,
                    direction: McpTraceDirection::Inbound,
                    target: "keeldesk".to_string(),
                    session_id,
                    caller_hint,
                    jsonrpc_id: Some(req_id_str),
                    method: "tools/call".to_string(),
                    tool_name: Some(tool_name),
                    safety_tier,
                    approval_outcome,
                    status: McpTraceStatus::Succeeded,
                    status_code: Some(200),
                    error_message: None,
                    latency: McpLatencyBreakdown {
                        total_ms: duration_ms,
                        approval_wait_ms,
                        execution_ms,
                    },
                    request_value: req_envelope,
                    response_value: json!({
                        "jsonrpc": "2.0",
                        "id": &req_id_json,
                        "result": &server_result,
                    }),
                    linked_call_id: Some(call_id),
                });
            }
            Err(err) => {
                let err_msg = err.message.to_string();
                let call_id = self.shared.record_tool_call(
                    tool_name.clone(),
                    duration_ms,
                    false,
                    format!("Error {}: {}", err.code.0, err.message),
                    caller_hint.clone(),
                );
                self.shared.trace_store().record(NewProtocolTrace {
                    transport: McpTraceTransport::StreamableHttp,
                    direction: McpTraceDirection::Inbound,
                    target: "keeldesk".to_string(),
                    session_id,
                    caller_hint,
                    jsonrpc_id: Some(req_id_str),
                    method: "tools/call".to_string(),
                    tool_name: Some(tool_name),
                    safety_tier,
                    approval_outcome,
                    status: McpTraceStatus::ProtocolError,
                    status_code: Some(err.code.0),
                    error_message: Some(err_msg),
                    latency: McpLatencyBreakdown {
                        total_ms: duration_ms,
                        approval_wait_ms,
                        execution_ms,
                    },
                    request_value: req_envelope,
                    response_value: json!({
                        "jsonrpc": "2.0",
                        "id": &req_id_json,
                        "error": err,
                    }),
                    linked_call_id: Some(call_id),
                });
            }
        }

        outcome
    }
}
