// 窗口动作 helper（被 tray.rs 与后续 shortcuts.rs 共用）
//
// 抽离动机：tray 的右键菜单与 shortcuts 的全局快捷键都需要 show / hide / toggle
// 桌宠主窗口的能力。把这些函数放在中立模块，避免任一方依赖另一方的私有实现。
//
// #21 锁死边界：所有 `show_*` / `toggle_*`（让主体窗口可见的路径）前置 ConsentGate
// 检查。gate=false（onboarding 未完成）时改为 `show_onboarding`，把焦点引导回宣誓页。
// `hide_*` 不查 gate（降级操作）；`show_onboarding` 自己不查（gate 关时它正是出口）。

use tauri::{AppHandle, Manager};

use crate::services::consent_gate::ConsentGate;

pub const PET_WINDOW_LABEL: &str = "pet";
/// 设置窗口 label（与 tauri.conf.json 静态注册一致；issue #9）。
pub const SETTINGS_WINDOW_LABEL: &str = "settings";
/// 对话窗口 label（与 tauri.conf.json 静态注册一致；issue #14）。
pub const CHAT_WINDOW_LABEL: &str = "chat";
/// 灵魂宣誓窗口 label（与 tauri.conf.json 静态注册一致；issue #16）。
/// 启动期 visible:false；setup hook 调 consent::check_version 后决定是否 show。
pub const ONBOARDING_WINDOW_LABEL: &str = "onboarding";

/// 进程级闸门读取。state 未 manage（setup 早期 / 极端 dev 路径）时保守返 false。
fn is_gate_open(app: &AppHandle) -> bool {
    app.try_state::<ConsentGate>()
        .map(|g| g.is_open())
        .unwrap_or(false)
}

pub(crate) fn show_pet(app: &AppHandle) {
    if !is_gate_open(app) {
        // 用户尚未完成 onboarding → 把焦点引导回宣誓页，而不是悄悄 show pet
        show_onboarding(app);
        return;
    }
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
    if !is_gate_open(app) {
        show_onboarding(app);
        return;
    }
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
    if !is_gate_open(app) {
        show_onboarding(app);
        return;
    }
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
    if !is_gate_open(app) {
        show_onboarding(app);
        return;
    }
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
    if !is_gate_open(app) {
        show_onboarding(app);
        return;
    }
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

/// 显示灵魂宣誓窗口（issue #16）。窗口启动期 `visible:false` 静态注册；
/// 由 setup hook 在 consent NotGranted / NeedReconsent 路径上唤起。
///
/// #21 锁死边界：本函数**不查 gate**——它正是 gate 关时的唯一引导出口。
pub(crate) fn show_onboarding(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(ONBOARDING_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 隐藏灵魂宣誓窗口（issue #16）。"我懂了"路径用——不 destroy 是为了 #17 接 6 步状态机时
/// 仍可复用同一 webview 渲染后续 Step 2-6（避免每步建窗销窗的闪烁开销）。
pub(crate) fn hide_onboarding(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(ONBOARDING_WINDOW_LABEL) {
        let _ = window.hide();
    }
}
