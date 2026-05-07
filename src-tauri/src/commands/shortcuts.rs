// 全局快捷键 IPC commands（#11）
//
// - probe_global_shortcut(shortcut)：探测某快捷键是否可用（给 #17 Onboarding Step 3 冲突检测）
// - set_shortcut_chat(shortcut)：改 chat 快捷键 + 持久化到 config（M1 W2 stub）

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
