use super::{
    runtime::KeelRuntime,
    types::{
        KeelPingSelfTestResult, KeelRuntimeSnapshot, PendingToolApproval, ToolApprovalDecision,
    },
};

#[tauri::command]
#[specta::specta]
pub async fn get_keel_runtime_snapshot(
    runtime: tauri::State<'_, KeelRuntime>,
) -> Result<KeelRuntimeSnapshot, String> {
    Ok(runtime.snapshot())
}

#[tauri::command]
#[specta::specta]
pub async fn self_test_keel_ping(
    runtime: tauri::State<'_, KeelRuntime>,
    message: Option<String>,
) -> Result<KeelPingSelfTestResult, String> {
    runtime.self_test_ping(message).await
}

#[tauri::command]
#[specta::specta]
pub async fn resolve_keel_tool_approval(
    runtime: tauri::State<'_, KeelRuntime>,
    request_id: String,
    decision: ToolApprovalDecision,
) -> Result<PendingToolApproval, String> {
    runtime.resolve_tool_approval(&request_id, decision)
}

#[tauri::command]
#[specta::specta]
pub async fn clear_keel_session_allowlist(
    runtime: tauri::State<'_, KeelRuntime>,
) -> Result<KeelRuntimeSnapshot, String> {
    Ok(runtime.clear_session_allowlist())
}
