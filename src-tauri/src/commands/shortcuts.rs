// 全局快捷键 IPC commands（#11）
//
// - probe_global_shortcut(shortcut)：探测某快捷键是否可用（给 #17 Onboarding Step 3 冲突检测）
// - set_shortcut_chat(shortcut)：改 chat 快捷键 + 持久化到 config（M1 W2 stub）
// - get_chat_shortcut()：读当前已注册的 chat 快捷键（#21 Step 3 onboarding 同步显示）

use crate::services::shortcuts::{self, ProbeResult, ShortcutRegisterFailedPayload};
use tauri::AppHandle;

#[tauri::command]
pub async fn probe_global_shortcut(
    app: AppHandle,
    shortcut: String,
) -> Result<ProbeResult, String> {
    Ok(shortcuts::probe(&app, &shortcut))
}

#[tauri::command]
pub async fn set_shortcut_chat(app: AppHandle, shortcut: String) -> Result<(), String> {
    shortcuts::set_chat_shortcut(&app, &shortcut).await
}

/// 返回当前 chat 快捷键状态。
/// `Some(s)` 启动期 register 成功；`None` 当前无 chat 快捷键（启动期失败）。
#[tauri::command]
pub async fn get_chat_shortcut(app: AppHandle) -> Result<Option<String>, String> {
    Ok(shortcuts::current_chat_shortcut(&app))
}

/// 返回启动期 register chat 快捷键的留痕（#21 收尾 #2 失败兜底）。
/// `Some(p)` = 启动期 register 失败且尚未通过 set_chat_shortcut 恢复 → 前端 toast 提示改键；
/// `None` = 启动期 OK / 用户已成功改键。
///
/// 设计动机：emit `shortcut:register-failed` 单走会丢（setup 内 emit 早于 webview JS 完成
/// 初始化和 listener 挂载的 race）；前端 mount 时查询此 IPC 兜底拿到状态。
#[tauri::command]
pub async fn get_chat_register_status(
    app: AppHandle,
) -> Result<Option<ShortcutRegisterFailedPayload>, String> {
    Ok(shortcuts::last_chat_register_error(&app))
}
