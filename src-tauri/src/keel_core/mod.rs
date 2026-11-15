pub mod commands;
pub mod config;
pub mod devtools;
pub mod mcp_server;
pub mod process_host;
pub mod runtime;
pub mod types;
pub(crate) mod win32_env;
pub mod win_odr;

pub use config::KeelConfig;
pub use devtools::{
    KeelProtocolTraceClearedEvent, KeelProtocolTraceEvent, McpLatencyBreakdown,
    McpProtocolTraceEntry, McpProtocolTraceStore, McpTraceApprovalOutcome, McpTraceDirection,
    McpTraceStatus, McpTraceTransport, NewProtocolTrace, ProtocolTraceClearedNotifier,
    ProtocolTraceNotifier, MAX_PROTOCOL_TRACES, MAX_TRACE_PAYLOAD_BYTES,
};
pub use mcp_server::{ApprovalNotifier, KeelMcpServer, KeelToolRegistry};
pub use process_host::{
    ChildEnvVar, ChildMcpToolSummary, ChildServerRecord, ChildServerStatus, ChildToolCallResult,
    KeelProcessHost, ProcessHostNotifier, ProcessHostSnapshot, ProcessHostUpdatedEvent,
    SpawnChildServerSpec,
};
pub use runtime::KeelRuntime;
pub use types::{
    KeelHealthCheckResult, KeelPingPayload, KeelPingSelfTestResult, KeelRuntimeSnapshot,
    KeelRuntimeUpdatedEvent, KeelServerStatus, KeelToolDescriptor, McpSessionSummary,
    PendingToolApproval, SessionAllowlistEntry, ToolApprovalDecision, ToolApprovalRequestedEvent,
    ToolCallLogEntry, ToolSafetyTier,
};
pub use win_odr::{
    OdrActionOutcome, OdrEnrolledServer, OdrReadinessState, OdrRegisterMode, OdrStatusReport,
};
