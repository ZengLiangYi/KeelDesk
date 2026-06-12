use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

use crate::MAIN_WINDOW_LABEL;

const TRAY_ID: &str = "main-tray";
const TRAY_SHOW_ID: &str = "tray_show";
const TRAY_HIDE_ID: &str = "tray_hide";
const TRAY_QUIT_ID: &str = "tray_quit";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrayMenuAction {
    Show,
    Hide,
    Quit,
}

fn tray_menu_action(menu_id: &str) -> Option<TrayMenuAction> {
    match menu_id {
        TRAY_SHOW_ID => Some(TrayMenuAction::Show),
        TRAY_HIDE_ID => Some(TrayMenuAction::Hide),
        TRAY_QUIT_ID => Some(TrayMenuAction::Quit),
        _ => None,
    }
}

fn should_restore_window_from_tray_click(
    button: MouseButton,
    button_state: MouseButtonState,
) -> bool {
    button == MouseButton::Left && button_state == MouseButtonState::Up
}

pub(crate) fn should_hide_window_on_close(window_label: &str) -> bool {
    window_label == MAIN_WINDOW_LABEL
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub(crate) fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.hide();
    }
}

pub(crate) fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text(TRAY_SHOW_ID, "Show KeelDesk")
        .text(TRAY_HIDE_ID, "Hide KeelDesk")
        .separator()
        .text(TRAY_QUIT_ID, "Quit")
        .build()?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon should be available");

    TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("KeelDesk")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match tray_menu_action(event.id().as_ref()) {
            Some(TrayMenuAction::Show) => show_main_window(app),
            Some(TrayMenuAction::Hide) => hide_main_window(app),
            Some(TrayMenuAction::Quit) => app.exit(0),
            None => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            {
                if should_restore_window_from_tray_click(button, button_state) {
                    show_main_window(tray.app_handle());
                }
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_tray_menu_ids_to_window_actions() {
        assert_eq!(tray_menu_action(TRAY_SHOW_ID), Some(TrayMenuAction::Show));
        assert_eq!(tray_menu_action(TRAY_HIDE_ID), Some(TrayMenuAction::Hide));
        assert_eq!(tray_menu_action(TRAY_QUIT_ID), Some(TrayMenuAction::Quit));
    }

    #[test]
    fn ignores_unknown_tray_menu_ids() {
        assert_eq!(tray_menu_action("tray_unknown"), None);
    }

    #[test]
    fn restores_window_from_left_mouse_release_only() {
        assert!(should_restore_window_from_tray_click(
            MouseButton::Left,
            MouseButtonState::Up
        ));
        assert!(!should_restore_window_from_tray_click(
            MouseButton::Left,
            MouseButtonState::Down
        ));
        assert!(!should_restore_window_from_tray_click(
            MouseButton::Right,
            MouseButtonState::Up
        ));
    }

    #[test]
    fn hides_only_main_window_on_close() {
        assert!(should_hide_window_on_close(MAIN_WINDOW_LABEL));
        assert!(!should_hide_window_on_close("settings"));
    }
}
