use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ToolSafetyTier {
    Safe,
    ConfirmRequired,
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
pub struct KeelRuntimeSnapshot {
    pub status: KeelServerStatus,
    pub endpoint_url: String,
    pub bind_addr: String,
    pub server_name: String,
    pub server_version: String,
    pub started_at_iso: Option<String>,
    pub error_message: Option<String>,
    pub total_calls: u32,
    pub tools: Vec<KeelToolDescriptor>,
    pub recent_calls: Vec<ToolCallLogEntry>,
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

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct KeelRuntimeUpdatedEvent(pub KeelRuntimeSnapshot);
