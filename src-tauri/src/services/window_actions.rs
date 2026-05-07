// 窗口动作 helper（被 tray.rs 与后续 shortcuts.rs 共用）
//
// 抽离动机：tray 的右键菜单与 shortcuts 的全局快捷键都需要 show / hide / toggle
// 桌宠主窗口的能力。把这些函数放在中立模块，避免任一方依赖另一方的私有实现。

use tauri::{AppHandle, Manager};

pub const PET_WINDOW_LABEL: &str = "pet";

pub(crate) fn show_pet(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub(crate) fn hide_pet(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        let _ = window.hide();
    }
}

pub(crate) fn toggle_pet(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
            }
            _ => {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }
}
