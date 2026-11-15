use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

mod tray;
mod win11;
pub mod workbench;

const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts");
pub(crate) const MAIN_WINDOW_LABEL: &str = "main";

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        workbench::get_win11_host_summary,
        workbench::scan_win11_dev_env,
        workbench::list_win11_active_ports,
        workbench::drill_kill_port_approval,
    ])
}

pub fn export_typescript_bindings() {
    if let Some(parent) = std::path::Path::new(BINDINGS_PATH).parent() {
        std::fs::create_dir_all(parent).expect("failed to create env-workbench bindings directory");
    }
    tauri_plugin_keeldesk::export_default_sdk_bindings();
    specta_builder()
        .export(Typescript::default(), BINDINGS_PATH)
        .expect("failed to export workbench typescript bindings");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = specta_builder();

    #[cfg(debug_assertions)]
    export_typescript_bindings();

    let mut tool_registry = keeldesk_core::KeelToolRegistry::new();
    workbench::register_tools(&mut tool_registry);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_keeldesk::Builder::new()
                .config(keeldesk_core::KeelConfig::default())
                .tool_registry(tool_registry)
                .focus_window_label(MAIN_WINDOW_LABEL)
                .build(),
        )
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
