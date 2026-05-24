//! BossKey IPC (#23-d / #42) — 3 命令：toggle / rebind / is_hidden。
//!
//! - `bosskey_toggle`：快捷键 / 托盘 / 前端按钮三路统一入口
//! - `bosskey_rebind { accelerator }`：改快捷键（unregister 旧 + register 新 + 落 KV）
//! - `bosskey_is_hidden`：前端 query 当前态（同步托盘图标 / UI 状态用）
//!
//! 详细实现见 [services::bosskey](crate::services::bosskey)。

use tauri::{AppHandle, State};

use crate::services::bosskey::{rebind, toggle, BossKeyState};

/// 切换隐藏 / 显示。onboarding 期静默忽略不切换（flows §12.4 Updated 2026-05-24）。
/// 返回操作后的 `hidden` 状态（前端可据此更新 UI）。
#[tauri::command]
pub async fn bosskey_toggle(app: AppHandle) -> Result<bool, String> {
    toggle(&app).await
}

/// 改 BossKey 快捷键。`accelerator` 例："Ctrl+Shift+B" / "Alt+F12" 等 tauri::Shortcut 可解析的串。
///
/// 失败原因：accelerator 字符串解析失败 / 系统占用 / unregister 旧失败。
/// 失败时旧已 unregister，前端可调 rebind 回旧值兜底（M1 不做事务）。
#[tauri::command]
pub async fn bosskey_rebind(app: AppHandle, accelerator: String) -> Result<(), String> {
    rebind(&app, &accelerator).await
}

/// 当前是否处于隐藏（摸鱼）态。前端 mount / 托盘 listener 用于同步 UI 图标 / tooltip。
#[tauri::command]
pub fn bosskey_is_hidden(state: State<'_, BossKeyState>) -> bool {
    state.is_hidden()
}
