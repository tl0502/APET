// #5 NicknameService IPC commands（F.2 facade 入口）
//
// 5 个 commands 委托 services::nickname 模块；仅 IPC 层错误转换（NicknameError → String）。
// `nickname:changed` event 在 service 层 emit，IPC 不重复 emit。
//
// 2026-05-06 code-review #5：set_pet / set_user 加输入校验（trim + empty + length cap）。

use crate::services::nickname;
use tauri::AppHandle;

const NICKNAME_MAX_LEN: usize = 50;

fn validate_nickname(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("昵称不能为空或仅含空白".to_string());
    }
    let chars = trimmed.chars().count();
    if chars > NICKNAME_MAX_LEN {
        return Err(format!("昵称长度超限（≤{NICKNAME_MAX_LEN} 字符）"));
    }
    Ok(trimmed.to_string())
}

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
    let name = validate_nickname(&name)?;
    nickname::set_pet_nickname(&app, name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn nickname_set_user(app: AppHandle, name: String) -> Result<(), String> {
    let name = validate_nickname(&name)?;
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
