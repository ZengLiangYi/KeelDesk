use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

mod tray;

const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts");
pub(crate) const MAIN_WINDOW_LABEL: &str = "main";

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
#[specta::specta]
fn greet(name: String) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
#[specta::specta]
fn is_primary_mouse_button_down() -> bool {
    platform_is_primary_mouse_button_down()
}

#[cfg(target_os = "windows")]
fn platform_is_primary_mouse_button_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};

    const KEY_PRESSED_MASK: i16 = 0x8000u16 as i16;

    unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) & KEY_PRESSED_MASK != 0 }
}

#[cfg(not(target_os = "windows"))]
fn platform_is_primary_mouse_button_down() -> bool {
    false
}

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![greet, is_primary_mouse_button_down])
}

pub fn export_typescript_bindings() {
    specta_builder()
        .export(Typescript::default(), BINDINGS_PATH)
        .expect("failed to export typescript bindings");
}

#[cfg(target_os = "windows")]
fn apply_windows_mystic_border(
    window: &tauri::WebviewWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_BORDER_COLOR};

    let hwnd = window.hwnd()?;
    // DWM expects COLORREF: 0x00BBGGRR. This is RGB #8B5CF6.
    let border_color = 0x00F65C8B_u32;

    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border_color as *const _ as _,
            std::mem::size_of_val(&border_color) as u32,
        )?;
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = specta_builder();

    #[cfg(debug_assertions)]
    export_typescript_bindings();

    tauri::Builder::default()
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
            tray::setup_tray(app)?;

            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                apply_windows_mystic_border(&window)?;
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
