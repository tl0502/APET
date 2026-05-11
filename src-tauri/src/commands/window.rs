// 窗口控制 IPC commands
//
// 当前窗口语义：
// - settings show/hide（#9）
// - pet 位置 get/save（#10；与后端 Moved 自动保存路径独立，前端可主动覆写）
// - chat show/hide/toggle（#14；接 #11 全局快捷键 + 关闭按钮 / ESC）

use crate::services::window_actions::{
    hide_chat, hide_settings, show_chat, show_settings, toggle_chat,
};
use crate::services::window_state::{self, LastPosition};
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

#[tauri::command]
pub async fn get_pet_position(app: AppHandle) -> Result<Option<LastPosition>, String> {
    window_state::load_pet_position(&app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_pet_position(app: AppHandle, pos: LastPosition) -> Result<(), String> {
    window_state::set_pet_position(&app, &pos)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn chat_show(app: AppHandle) -> Result<(), String> {
    show_chat(&app);
    Ok(())
}

#[tauri::command]
pub async fn chat_hide(app: AppHandle) -> Result<(), String> {
    hide_chat(&app);
    Ok(())
}

#[tauri::command]
pub async fn chat_toggle(app: AppHandle) -> Result<(), String> {
    toggle_chat(&app);
    Ok(())
}
