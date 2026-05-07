// #5 KV 偏好 IPC commands — 架构 §339 memory 表
//
// 2026-05-06 code-review #3+#5 重写：
// - 业务逻辑下沉到 services/preferences.rs（清晰区分 messages 表的 services/memory.rs）
// - IPC command 名保持 memory_get/set/list/delete 不变（前端契约）
// - 此处只做 thin wrapper：输入校验 + 调 service + 错误转字符串

use crate::services::preferences::{self, PreferenceItem};
use tauri::AppHandle;

const KEY_MAX_LEN: usize = 256;
const VALUE_MAX_LEN: usize = 8192;

fn validate_key(key: &str) -> Result<&str, String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("key 不能为空".to_string());
    }
    if trimmed.len() > KEY_MAX_LEN {
        return Err(format!("key 长度超限（≤{KEY_MAX_LEN} 字符）"));
    }
    if trimmed.contains('\0') {
        return Err("key 不能包含 NUL 字符".to_string());
    }
    Ok(trimmed)
}

fn validate_value(value: &str) -> Result<&str, String> {
    if value.len() > VALUE_MAX_LEN {
        return Err(format!("value 长度超限（≤{VALUE_MAX_LEN} 字符）"));
    }
    Ok(value)
}

#[tauri::command]
pub async fn memory_get(app: AppHandle, key: String) -> Result<Option<String>, String> {
    let key = validate_key(&key)?;
    preferences::get(&app, key).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn memory_set(app: AppHandle, key: String, value: String) -> Result<(), String> {
    let key = validate_key(&key)?.to_string();
    let value = validate_value(&value)?.to_string();
    preferences::set(&app, &key, &value)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn memory_list(app: AppHandle) -> Result<Vec<PreferenceItem>, String> {
    preferences::list(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn memory_delete(app: AppHandle, key: String) -> Result<(), String> {
    let key = validate_key(&key)?;
    preferences::delete(&app, key)
        .await
        .map_err(|e| e.to_string())
}
