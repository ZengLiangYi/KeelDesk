use keeldesk_core::{
    KeelRuntimeSnapshot, McpProtocolTraceEntry, PendingToolApproval, ProcessHostSnapshot,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct KeelRuntimeUpdatedEvent(pub KeelRuntimeSnapshot);

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct ToolApprovalRequestedEvent(pub PendingToolApproval);

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct ProcessHostUpdatedEvent(pub ProcessHostSnapshot);

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct KeelProtocolTraceEvent(pub McpProtocolTraceEntry);

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct KeelProtocolTraceClearedEvent(pub u32);
