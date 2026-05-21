// 窗口动作 helper（被 tray.rs 与后续 shortcuts.rs 共用）
//
// 抽离动机：tray 的右键菜单与 shortcuts 的全局快捷键都需要 show / hide / toggle
// 桌宠主窗口的能力。把这些函数放在中立模块，避免任一方依赖另一方的私有实现。
//
// #21 锁死边界：所有 `show_*` / `toggle_*`（让主体窗口可见的路径）前置 ConsentGate
// 检查。gate=false（onboarding 未完成）时改为 `show_onboarding`，把焦点引导回宣誓页。
// `hide_*` 不查 gate（降级操作）；`show_onboarding` 自己不查（gate 关时它正是出口）。
//
// #30 follow-up G：所有 show_* / hide_* / toggle_* 在 window.show()/hide() 后必须调
// emit_visibility_changed —— WebView2 不在 hide 时触发 DOM visibilitychange（已知
// Tauri/WebView2 bug，参 #6864 #9524 #10592），所以靠后端事件让前端 useSnapWindow
// 同步 windowRegistry visible 标志，让 candidates / solver / occupancy 跳过隐形窗。

use tauri::{AppHandle, Emitter, Manager};

use crate::services::consent_gate::ConsentGate;

pub const PET_WINDOW_LABEL: &str = "pet";
/// 对话窗口 label（与 tauri.conf.json 静态注册一致；issue #14）。
pub const CHAT_WINDOW_LABEL: &str = "chat";
/// 灵魂宣誓窗口 label（与 tauri.conf.json 静态注册一致；issue #16）。
/// 启动期 visible:false；setup hook 调 consent::check_version 后决定是否 show。
pub const ONBOARDING_WINDOW_LABEL: &str = "onboarding";
/// 番茄独立窗口 label（与 tauri.conf.json 静态注册一致；#28 follow-up）。
/// 启动期 visible:false；由托盘菜单"番茄..."/ TasksPomodoroPanel "独立窗口 ↗" 按钮唤起。
/// #33 phase E：删 pomodoro_start 自动 show，浮窗仅手动入口。
/// alwaysOnTop 由前端 PomodoroApp.vue 按 phase 动态切换（FOCUS/PAUSED_F 置顶；其余不置顶）。
pub const POMODORO_WINDOW_LABEL: &str = "pomodoro";
/// Workspace 主窗 label（与 tauri.conf.json 静态注册一致；#35 ADR-021 P1）。
/// 启动期 visible:false；由托盘菜单"工作台..." / 左键双击 / 全局快捷键 Ctrl+Alt+W 三路唤起。
/// 关闭语义同 chat：拦截 CloseRequested → hide（保留 webview + 三栏布局 + master 宽度）。
/// #33 phase E：settings/tasks 独立窗已删（panel 迁入 workspace），剩 pet/chat/workspace/pomodoro 四窗。
pub const WORKSPACE_WINDOW_LABEL: &str = "workspace";

/// #30 follow-up G：跨 webview 广播窗口 visibility 变化的事件名。
pub const VISIBILITY_CHANGED_EVENT: &str = "window:visibility-changed";

/// hide / show 后调一次：通知所有 webview 同步 visible 字段。
/// 失败仅 eprintln，不阻塞主路径（visibility 不同步只是 snap 失效，不影响用户基本操作）。
pub(crate) fn emit_visibility_changed(app: &AppHandle, label: &str, visible: bool) {
    if let Err(e) = app.emit(
        VISIBILITY_CHANGED_EVENT,
        serde_json::json!({ "label": label, "visible": visible }),
    ) {
        eprintln!(
            "[window_actions] emit {VISIBILITY_CHANGED_EVENT} for {label}={visible} failed: {e}"
        );
    }
}

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
        emit_visibility_changed(app, PET_WINDOW_LABEL, true);
    }
}

pub(crate) fn hide_pet(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        let _ = window.hide();
        emit_visibility_changed(app, PET_WINDOW_LABEL, false);
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
                emit_visibility_changed(app, PET_WINDOW_LABEL, false);
            }
            _ => {
                let _ = window.show();
                let _ = window.set_focus();
                emit_visibility_changed(app, PET_WINDOW_LABEL, true);
            }
        }
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
        emit_visibility_changed(app, CHAT_WINDOW_LABEL, true);
    }
}

/// 隐藏对话窗口（不销毁，保留 messages state 供下次唤起；issue #14 验收）。
pub(crate) fn hide_chat(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(CHAT_WINDOW_LABEL) {
        let _ = window.hide();
        emit_visibility_changed(app, CHAT_WINDOW_LABEL, false);
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
                emit_visibility_changed(app, CHAT_WINDOW_LABEL, false);
            }
            _ => {
                let _ = window.show();
                let _ = window.set_focus();
                emit_visibility_changed(app, CHAT_WINDOW_LABEL, true);
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
        emit_visibility_changed(app, ONBOARDING_WINDOW_LABEL, true);
    }
}

/// 隐藏灵魂宣誓窗口（issue #16）。"我懂了"路径用——不 destroy 是为了 #17 接 6 步状态机时
/// 仍可复用同一 webview 渲染后续 Step 2-6（避免每步建窗销窗的闪烁开销）。
pub(crate) fn hide_onboarding(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(ONBOARDING_WINDOW_LABEL) {
        let _ = window.hide();
        emit_visibility_changed(app, ONBOARDING_WINDOW_LABEL, false);
    }
}

/// 显示番茄独立窗口（#28 follow-up）。`visible:false` 静态注册；位置由 setup 阶段
/// apply_initial_pomodoro_position 预设到 KV 记忆的上次位置（不在此处 apply 防闪动）。
/// alwaysOnTop 由前端 PomodoroApp.vue 按 phase 动态切换，本函数不参与。
pub(crate) fn show_pomodoro(app: &AppHandle) {
    if !is_gate_open(app) {
        show_onboarding(app);
        return;
    }
    if let Some(window) = app.get_webview_window(POMODORO_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        emit_visibility_changed(app, POMODORO_WINDOW_LABEL, true);
    }
}

/// 隐藏番茄窗口（不销毁，保留 webview 状态供下次唤起；同 tasks 模式）。
/// CloseRequested 首次拦截会 emit `pomodoro:hide_hint` 提示用户后台仍在计时（lib.rs 实现）。
pub(crate) fn hide_pomodoro(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(POMODORO_WINDOW_LABEL) {
        let _ = window.hide();
        emit_visibility_changed(app, POMODORO_WINDOW_LABEL, false);
    }
}

/// 切换番茄窗口可见性。
pub(crate) fn toggle_pomodoro(app: &AppHandle) {
    if !is_gate_open(app) {
        show_onboarding(app);
        return;
    }
    if let Some(window) = app.get_webview_window(POMODORO_WINDOW_LABEL) {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
                emit_visibility_changed(app, POMODORO_WINDOW_LABEL, false);
            }
            _ => {
                let _ = window.show();
                let _ = window.set_focus();
                emit_visibility_changed(app, POMODORO_WINDOW_LABEL, true);
            }
        }
    }
}

/// 显示 workspace 主窗（#35 ADR-021 P1）。`visible:false` 静态注册；
/// 由托盘菜单"工作台..."/ 左键双击 / 全局快捷键 Ctrl+Alt+W 三路唤起。
pub(crate) fn show_workspace(app: &AppHandle) {
    if !is_gate_open(app) {
        show_onboarding(app);
        return;
    }
    if let Some(window) = app.get_webview_window(WORKSPACE_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        emit_visibility_changed(app, WORKSPACE_WINDOW_LABEL, true);
    }
}

/// 隐藏 workspace 主窗（不销毁，保留 dockview layout + panel 实例；
/// 同 settings/tasks 模式；KV `workspace:layout` 在前端 onBeforeUnmount 持久化）。
pub(crate) fn hide_workspace(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WORKSPACE_WINDOW_LABEL) {
        let _ = window.hide();
        emit_visibility_changed(app, WORKSPACE_WINDOW_LABEL, false);
    }
}

/// 切换 workspace 可见性（托盘左键双击 + Ctrl+Alt+W + 菜单"工作台..."三入口都走这里）。
pub(crate) fn toggle_workspace(app: &AppHandle) {
    if !is_gate_open(app) {
        show_onboarding(app);
        return;
    }
    if let Some(window) = app.get_webview_window(WORKSPACE_WINDOW_LABEL) {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
                emit_visibility_changed(app, WORKSPACE_WINDOW_LABEL, false);
            }
            _ => {
                let _ = window.show();
                let _ = window.set_focus();
                emit_visibility_changed(app, WORKSPACE_WINDOW_LABEL, true);
            }
        }
    }
}
