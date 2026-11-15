use keeldesk_core::{
    ChildServerRecord, ChildToolCallResult, KeelHealthCheckResult, KeelPingSelfTestResult,
    KeelRuntime, KeelRuntimeSnapshot, McpProtocolTraceEntry, OdrActionOutcome, OdrRegisterMode,
    OdrStatusReport, PendingToolApproval, ProcessHostSnapshot, SpawnChildServerSpec,
    ToolApprovalDecision,
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

#[tauri::command]
#[specta::specta]
pub async fn get_keel_odr_status(
    runtime: tauri::State<'_, KeelRuntime>,
) -> Result<OdrStatusReport, String> {
    Ok(runtime.odr_status().await)
}

#[tauri::command]
#[specta::specta]
pub async fn register_keel_with_odr(
    runtime: tauri::State<'_, KeelRuntime>,
    mode: OdrRegisterMode,
) -> Result<OdrActionOutcome, String> {
    Ok(runtime.register_with_odr(mode).await)
}

#[tauri::command]
#[specta::specta]
pub async fn unregister_keel_from_odr(
    runtime: tauri::State<'_, KeelRuntime>,
    server_id: Option<String>,
) -> Result<OdrActionOutcome, String> {
    Ok(runtime.unregister_from_odr(server_id).await)
}

#[tauri::command]
#[specta::specta]
pub async fn export_keel_odr_manifest(
    runtime: tauri::State<'_, KeelRuntime>,
    target_path: Option<String>,
) -> Result<OdrActionOutcome, String> {
    runtime.export_odr_manifest(target_path).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_keel_process_host_snapshot(
    runtime: tauri::State<'_, KeelRuntime>,
) -> Result<ProcessHostSnapshot, String> {
    Ok(runtime.process_host_snapshot())
}

#[tauri::command]
#[specta::specta]
pub async fn start_keel_child_server(
    runtime: tauri::State<'_, KeelRuntime>,
    spec: SpawnChildServerSpec,
) -> Result<ChildServerRecord, String> {
    runtime.start_child_server(spec).await
}

#[tauri::command]
#[specta::specta]
pub async fn stop_keel_child_server(
    runtime: tauri::State<'_, KeelRuntime>,
    server_id: String,
) -> Result<ChildServerRecord, String> {
    runtime.stop_child_server(&server_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_keel_child_server(
    runtime: tauri::State<'_, KeelRuntime>,
    server_id: String,
) -> Result<ProcessHostSnapshot, String> {
    runtime.remove_child_server(&server_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn check_keel_healthz(
    runtime: tauri::State<'_, KeelRuntime>,
) -> Result<KeelHealthCheckResult, String> {
    runtime.check_healthz().await
}

#[tauri::command]
#[specta::specta]
pub async fn get_keel_protocol_traces(
    runtime: tauri::State<'_, KeelRuntime>,
) -> Result<Vec<McpProtocolTraceEntry>, String> {
    Ok(runtime.protocol_traces())
}

#[tauri::command]
#[specta::specta]
pub async fn clear_keel_protocol_traces(
    runtime: tauri::State<'_, KeelRuntime>,
) -> Result<u32, String> {
    Ok(runtime.clear_protocol_traces())
}

#[tauri::command]
#[specta::specta]
pub async fn call_keel_child_server_tool(
    runtime: tauri::State<'_, KeelRuntime>,
    server_id: String,
    tool_name: String,
    arguments_json: Option<String>,
) -> Result<ChildToolCallResult, String> {
    runtime
        .call_child_server_tool(&server_id, &tool_name, arguments_json)
        .await
}
