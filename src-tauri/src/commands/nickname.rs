// #5 NicknameService IPC commands（F.2 facade 入口）
//
// 5 个 commands 委托 services::nickname 模块；仅 IPC 层错误转换（NicknameError → String）。
// `nickname:changed` event 在 service 层 emit，IPC 不重复 emit。

use crate::services::nickname;
use tauri::AppHandle;

#[tauri::command]
pub async fn nickname_get_pet(app: AppHandle) -> Result<String, String> {
    nickname::get_pet_nickname(&app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn nickname_get_user(app: AppHandle) -> Result<Option<String>, String> {
    nickname::get_user_nickname(&app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn nickname_set_pet(app: AppHandle, name: String) -> Result<(), String> {
    nickname::set_pet_nickname(&app, name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn nickname_set_user(app: AppHandle, name: String) -> Result<(), String> {
    nickname::set_user_nickname(&app, name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn nickname_restore_pet(app: AppHandle) -> Result<Option<String>, String> {
    nickname::restore_pet_nickname(&app)
        .await
        .map_err(|e| e.to_string())
}
