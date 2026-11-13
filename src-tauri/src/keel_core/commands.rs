use super::{
    runtime::KeelRuntime,
    types::{KeelPingSelfTestResult, KeelRuntimeSnapshot},
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
