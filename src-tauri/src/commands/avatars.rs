// 头像 IPC commands（#25 用户上传 + #26 VRM 导出）。
//
// 6 个 commands：
// - user_avatar_set(src_path) → 不裁剪直接复制（向后兼容 / 简单路径）
// - user_avatar_clear() → 删盘上文件；前端 memory_delete KV
// - avatar_read_to_data_url(src_path) → 读源文件返 base64 dataURL（cropper 喂图）
// - user_avatar_save_data_url(data_url) → 裁剪后 PNG dataURL 落盘
// - persona_avatar_save(persona_id, data_url) → VRM 截图 dataURL 落盘
// - persona_avatar_clear(persona_id) → 删 persona-<id>.png
//
// 设计：thin wrapper，业务逻辑下沉到 services::avatars。
// 不写 DB（路径由前端通过 memory_set 写到偏好表），避免与现有 KV 偏好层重复一套存储。

use crate::services::avatars;
use tauri::AppHandle;

#[tauri::command]
pub fn user_avatar_set(app: AppHandle, src_path: String) -> Result<String, String> {
    let path = avatars::copy_user_avatar(&app, &src_path).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn user_avatar_clear(app: AppHandle) -> Result<u32, String> {
    avatars::clear_user_avatar(&app).map_err(|e| e.to_string())
}

/// 读源文件 → base64 dataURL，给前端 cropper 喂图（#25 裁剪流）。
#[tauri::command]
pub fn avatar_read_to_data_url(app: AppHandle, src_path: String) -> Result<String, String> {
    avatars::read_image_to_data_url(&app, &src_path).map_err(|e| e.to_string())
}

/// 裁剪后 PNG dataURL 落盘（#25 裁剪流）。
#[tauri::command]
pub fn user_avatar_save_data_url(app: AppHandle, data_url: String) -> Result<String, String> {
    let path =
        avatars::save_user_avatar_from_data_url(&app, &data_url).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn persona_avatar_save(
    app: AppHandle,
    persona_id: String,
    data_url: String,
) -> Result<String, String> {
    let path =
        avatars::save_persona_avatar(&app, &persona_id, &data_url).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn persona_avatar_clear(app: AppHandle, persona_id: String) -> Result<bool, String> {
    avatars::clear_persona_avatar(&app, &persona_id).map_err(|e| e.to_string())
}
