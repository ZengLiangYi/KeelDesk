use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, collect_events, Builder};

pub mod keel_core;
mod tray;
mod win11;
pub mod workbench;

const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts");
pub(crate) const MAIN_WINDOW_LABEL: &str = "main";

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            keel_core::commands::get_keel_runtime_snapshot,
            keel_core::commands::self_test_keel_ping,
            keel_core::commands::resolve_keel_tool_approval,
            keel_core::commands::clear_keel_session_allowlist,
            keel_core::commands::get_keel_odr_status,
            keel_core::commands::register_keel_with_odr,
            keel_core::commands::unregister_keel_from_odr,
            keel_core::commands::export_keel_odr_manifest,
            keel_core::commands::get_keel_process_host_snapshot,
            keel_core::commands::start_keel_child_server,
            keel_core::commands::stop_keel_child_server,
            keel_core::commands::remove_keel_child_server,
            workbench::get_win11_host_summary,
            workbench::scan_win11_dev_env,
            workbench::list_win11_active_ports,
            workbench::drill_kill_port_approval,
        ])
        .events(collect_events![
            keel_core::KeelRuntimeUpdatedEvent,
            keel_core::ToolApprovalRequestedEvent,
            keel_core::ProcessHostUpdatedEvent,
        ])
}

pub fn export_typescript_bindings() {
    specta_builder()
        .export(Typescript::default(), BINDINGS_PATH)
        .expect("failed to export typescript bindings");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = specta_builder();

    #[cfg(debug_assertions)]
    export_typescript_bindings();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(specta_builder.invoke_handler())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if tray::should_hide_window_on_close(window.label()) {
                    api.prevent_close();
                    tray::hide_main_window(window.app_handle());
                }
            }
        })
        .setup(move |app| {
            specta_builder.mount_events(app);

            let mut tool_registry = keel_core::KeelToolRegistry::new();
            workbench::register_tools(&mut tool_registry);

            let keel_runtime = keel_core::KeelRuntime::start_with_tauri(
                app.handle().clone(),
                keel_core::KeelConfig::default(),
                tool_registry,
            );
            app.manage(keel_runtime);

            tray::setup_tray(app)?;

            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                win11::apply_win11_window_chrome(&window)?;
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
