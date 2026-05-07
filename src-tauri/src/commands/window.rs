// 窗口控制 IPC commands（#9）
//
// 当前仅 settings 窗口的 show/hide。后续 hub / chat 窗口加入时在此扩展。
// 窗口实例由 tauri.conf.json 静态注册（visible:false 默认隐藏），
// 这里仅做 show/hide 的 thin wrapper —— 业务逻辑在 services::window_actions。

use crate::services::window_actions::{hide_settings, show_settings};
use tauri::AppHandle;

#[tauri::command]
pub async fn settings_show(app: AppHandle) -> Result<(), String> {
    show_settings(&app);
    Ok(())
}

#[tauri::command]
pub async fn settings_hide(app: AppHandle) -> Result<(), String> {
    hide_settings(&app);
    Ok(())
}
