// 窗口动作 helper（被 tray.rs 与后续 shortcuts.rs 共用）
//
// 抽离动机：tray 的右键菜单与 shortcuts 的全局快捷键都需要 show / hide / toggle
// 桌宠主窗口的能力。把这些函数放在中立模块，避免任一方依赖另一方的私有实现。

use tauri::{AppHandle, Manager};

pub const PET_WINDOW_LABEL: &str = "pet";
/// 设置窗口 label（与 tauri.conf.json 静态注册一致；issue #9）。
pub const SETTINGS_WINDOW_LABEL: &str = "settings";
/// 对话窗口 label（与 tauri.conf.json 静态注册一致；issue #14）。
pub const CHAT_WINDOW_LABEL: &str = "chat";

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

/// 显示设置窗口（issue #9）。窗口启动期 `visible:false` 静态注册，由托盘菜单 / IPC 唤起。
pub(crate) fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 隐藏设置窗口（不销毁，保留 tab 状态供下次唤起；issue #9 验收）。
pub(crate) fn hide_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
        let _ = window.hide();
    }
}

/// 显示对话窗口（issue #14）。窗口启动期 `visible:false` 静态注册，由全局快捷键 / IPC 唤起。
pub(crate) fn show_chat(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(CHAT_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 隐藏对话窗口（不销毁，保留 messages state 供下次唤起；issue #14 验收）。
pub(crate) fn hide_chat(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(CHAT_WINDOW_LABEL) {
        let _ = window.hide();
    }
}

/// 切换对话窗口可见性（issue #14；接 #11 全局快捷键 `shortcut:chat` 主路径）。
pub(crate) fn toggle_chat(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(CHAT_WINDOW_LABEL) {
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
