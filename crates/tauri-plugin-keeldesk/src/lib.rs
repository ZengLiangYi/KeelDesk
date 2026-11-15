use std::{path::Path, sync::Arc};

use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, collect_events, Event};

pub use keeldesk_core::*;

pub mod commands;
pub mod events;

pub use events::{
    KeelProtocolTraceClearedEvent, KeelProtocolTraceEvent, KeelRuntimeUpdatedEvent,
    ProcessHostUpdatedEvent, ToolApprovalRequestedEvent,
};

pub const PLUGIN_NAME: &str = "keeldesk";
pub const DEFAULT_MAIN_WINDOW_LABEL: &str = "main";
const DEFAULT_SDK_BINDINGS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/react/src/bindings.ts"
);

pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(collect_commands![
            commands::get_keel_runtime_snapshot,
            commands::self_test_keel_ping,
            commands::resolve_keel_tool_approval,
            commands::clear_keel_session_allowlist,
            commands::get_keel_odr_status,
            commands::register_keel_with_odr,
            commands::unregister_keel_from_odr,
            commands::export_keel_odr_manifest,
            commands::get_keel_process_host_snapshot,
            commands::start_keel_child_server,
            commands::stop_keel_child_server,
            commands::remove_keel_child_server,
            commands::check_keel_healthz,
            commands::get_keel_protocol_traces,
            commands::clear_keel_protocol_traces,
            commands::call_keel_child_server_tool,
        ])
        .events(collect_events![
            events::KeelRuntimeUpdatedEvent,
            events::ToolApprovalRequestedEvent,
            events::ProcessHostUpdatedEvent,
            events::KeelProtocolTraceEvent,
            events::KeelProtocolTraceClearedEvent,
        ])
}

pub fn export_typescript_bindings_to(output_path: impl AsRef<Path>) {
    let path = output_path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("failed to create bindings parent directory");
    }
    specta_builder()
        .export(Typescript::default(), path)
        .expect("failed to export keeldesk plugin typescript bindings");
}

pub fn export_default_sdk_bindings() {
    export_typescript_bindings_to(DEFAULT_SDK_BINDINGS_PATH);
}

pub fn start_runtime_with_tauri(
    app_handle: tauri::AppHandle,
    config: KeelConfig,
    registry: KeelToolRegistry,
    focus_window_label: String,
) -> KeelRuntime {
    let snapshot_app = app_handle.clone();
    let snapshot_notifier: SnapshotNotifier = Arc::new(move |snapshot| {
        let _ = KeelRuntimeUpdatedEvent(snapshot).emit(&snapshot_app);
    });

    let process_host_app = app_handle.clone();
    let process_host_notifier: ProcessHostNotifier = Arc::new(move |snapshot| {
        let _ = ProcessHostUpdatedEvent(snapshot).emit(&process_host_app);
    });

    let trace_app = app_handle.clone();
    let trace_notifier: ProtocolTraceNotifier = Arc::new(move |entry| {
        let _ = KeelProtocolTraceEvent(entry).emit(&trace_app);
    });

    let trace_cleared_app = app_handle.clone();
    let trace_cleared_notifier: ProtocolTraceClearedNotifier = Arc::new(move |cleared| {
        let _ = KeelProtocolTraceClearedEvent(cleared).emit(&trace_cleared_app);
    });

    let approval_app = app_handle;
    let approval_notifier: ApprovalNotifier = Arc::new(move |pending| {
        let _ = ToolApprovalRequestedEvent(pending).emit(&approval_app);
        if let Some(window) = approval_app.get_webview_window(&focus_window_label) {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
            let _ = window.request_user_attention(Some(tauri::UserAttentionType::Critical));
        }
    });

    let task_spawner: RuntimeTaskSpawner = Arc::new(|fut| {
        tauri::async_runtime::spawn(fut);
    });

    KeelRuntime::start(
        config,
        registry,
        KeelRuntimeOptions {
            snapshot_notifier: Some(snapshot_notifier),
            approval_notifier: Some(approval_notifier),
            process_host_notifier: Some(process_host_notifier),
            trace_notifier: Some(trace_notifier),
            trace_cleared_notifier: Some(trace_cleared_notifier),
            task_spawner: Some(task_spawner),
        },
    )
}

pub struct Builder {
    config: KeelConfig,
    tool_registry: KeelToolRegistry,
    focus_window_label: String,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub fn new() -> Self {
        Self {
            config: KeelConfig::default(),
            tool_registry: KeelToolRegistry::new(),
            focus_window_label: DEFAULT_MAIN_WINDOW_LABEL.to_string(),
        }
    }

    pub fn config(mut self, config: KeelConfig) -> Self {
        self.config = config;
        self
    }

    pub fn tool_registry(mut self, registry: KeelToolRegistry) -> Self {
        self.tool_registry = registry;
        self
    }

    pub fn focus_window_label(mut self, label: impl Into<String>) -> Self {
        self.focus_window_label = label.into();
        self
    }

    pub fn build(self) -> tauri::plugin::TauriPlugin<tauri::Wry> {
        let specta_builder = specta_builder();
        let config = self.config;
        let tool_registry = self.tool_registry;
        let focus_window_label = self.focus_window_label;

        tauri::plugin::Builder::<tauri::Wry>::new(PLUGIN_NAME)
            .invoke_handler(specta_builder.invoke_handler())
            .setup(move |app, _api| {
                specta_builder.mount_events(app);
                let runtime = start_runtime_with_tauri(
                    app.clone(),
                    config,
                    tool_registry,
                    focus_window_label,
                );
                app.manage(runtime);
                Ok(())
            })
            .on_event(|app, event| {
                if matches!(event, tauri::RunEvent::Exit) {
                    if let Some(runtime) = app.try_state::<KeelRuntime>() {
                        runtime.shutdown();
                    }
                }
            })
            .build()
    }
}

pub fn init(
    config: KeelConfig,
    tool_registry: KeelToolRegistry,
) -> tauri::plugin::TauriPlugin<tauri::Wry> {
    Builder::new()
        .config(config)
        .tool_registry(tool_registry)
        .build()
}

pub trait KeelDeskExt<R: tauri::Runtime> {
    fn keel_runtime(&self) -> tauri::State<'_, KeelRuntime>;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> KeelDeskExt<R> for T {
    fn keel_runtime(&self) -> tauri::State<'_, KeelRuntime> {
        self.state::<KeelRuntime>()
    }
}
