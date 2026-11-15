pub mod commands;
pub mod config;
pub mod mcp_server;
pub mod process_host;
pub mod runtime;
pub mod types;
pub(crate) mod win32_env;
pub mod win_odr;

pub use config::KeelConfig;
pub use mcp_server::{ApprovalNotifier, KeelMcpServer, KeelToolRegistry};
pub use process_host::{
    ChildEnvVar, ChildMcpToolSummary, ChildServerRecord, ChildServerStatus, KeelProcessHost,
    ProcessHostNotifier, ProcessHostSnapshot, ProcessHostUpdatedEvent, SpawnChildServerSpec,
};
pub use runtime::KeelRuntime;
pub use types::{
    KeelPingPayload, KeelPingSelfTestResult, KeelRuntimeSnapshot, KeelRuntimeUpdatedEvent,
    KeelServerStatus, KeelToolDescriptor, PendingToolApproval, SessionAllowlistEntry,
    ToolApprovalDecision, ToolApprovalRequestedEvent, ToolCallLogEntry, ToolSafetyTier,
};
pub use win_odr::{
    OdrActionOutcome, OdrEnrolledServer, OdrReadinessState, OdrRegisterMode, OdrStatusReport,
};
