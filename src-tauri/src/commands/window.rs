// 窗口控制 IPC commands
//
// 当前窗口语义：
// - settings show/hide（#9）
// - pet 位置 get/save（#10；与后端 Moved 自动保存路径独立，前端可主动覆写）

use crate::services::window_actions::{hide_settings, show_settings};
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
