use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ToolSafetyTier {
    Safe,
    ConfirmRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ToolApprovalDecision {
    ApproveOnce,
    ApproveForSession,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct PendingToolApproval {
    pub request_id: String,
    pub tool_name: String,
    pub tool_source: String,
    pub description: String,
    pub arguments_json: String,
    pub caller_hint: Option<String>,
    pub session_scope: String,
    pub requested_at_iso: String,
    pub expires_at_iso: String,
    pub timeout_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SessionAllowlistEntry {
    pub session_scope: String,
    pub tool_name: String,
    pub granted_at_iso: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum KeelServerStatus {
    Starting,
    Running,
    Error,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct KeelToolDescriptor {
    pub name: String,
    pub description: String,
    pub source: String,
    pub safety: ToolSafetyTier,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ToolCallLogEntry {
    pub id: u32,
    pub tool_name: String,
    pub timestamp_iso: String,
    pub duration_ms: u32,
    pub succeeded: bool,
    pub summary: String,
    pub caller_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct McpSessionSummary {
    pub session_id: String,
    pub client_name: Option<String>,
    pub client_version: Option<String>,
    pub protocol_version: Option<String>,
    pub user_agent: Option<String>,
    pub connected_at_iso: String,
    pub last_active_at_iso: String,
    pub request_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct KeelRuntimeSnapshot {
    pub status: KeelServerStatus,
    pub endpoint_url: String,
    pub bind_addr: String,
    pub bound_port: u16,
    pub server_name: String,
    pub server_version: String,
    pub started_at_iso: Option<String>,
    pub error_message: Option<String>,
    pub total_calls: u32,
    pub tools: Vec<KeelToolDescriptor>,
    pub recent_calls: Vec<ToolCallLogEntry>,
    pub pending_approvals: Vec<PendingToolApproval>,
    pub session_allowlist: Vec<SessionAllowlistEntry>,
    pub active_sessions: Vec<McpSessionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct KeelHealthCheckResult {
    pub ok: bool,
    pub status_code: u16,
    pub latency_ms: u32,
    pub endpoint_url: String,
    pub healthz_url: String,
    pub bound_port: u16,
    pub active_session_count: u32,
    pub total_calls: u32,
    pub checked_at_iso: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
pub struct KeelPingPayload {
    pub ok: bool,
    pub server: String,
    pub version: String,
    pub transport: String,
    pub endpoint: String,
    pub echo: Option<String>,
    pub timestamp_iso: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct KeelPingSelfTestResult {
    pub payload: KeelPingPayload,
    pub roundtrip_ms: u32,
    pub advertised_tools: Vec<String>,
}
