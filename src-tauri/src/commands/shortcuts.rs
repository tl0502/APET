// 全局快捷键 IPC commands（#11）
//
// - probe_global_shortcut(shortcut)：探测某快捷键是否可用（给 #17 Onboarding Step 3 冲突检测）
// - set_shortcut_chat(shortcut)：改 chat 快捷键 + 持久化到 config（M1 W2 stub）
// - get_chat_shortcut()：读当前已注册的 chat 快捷键（#21 Step 3 onboarding 同步显示）

use crate::services::shortcuts::{self, ProbeResult};
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
    shortcuts::set_chat_shortcut(&app, &shortcut)
}

/// 返回当前 chat 快捷键状态。
/// `Some(s)` 启动期 register 成功；`None` 当前无 chat 快捷键（启动期失败）。
#[tauri::command]
pub async fn get_chat_shortcut(app: AppHandle) -> Result<Option<String>, String> {
    Ok(shortcuts::current_chat_shortcut(&app))
}
