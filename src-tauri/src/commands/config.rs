// #30 磁吸窗口系统：config 表 KV IPC（services/config.rs thin wrapper）。
//
// 与 commands/memory.rs 操作的 `memory` 表不同：本 commands 操作 `config` 表
//（架构 §4：运行时配置 — 窗口位置 / active_conversation_id / 快捷键绑定 等；
//  详 services/config.rs 头注释）。
//
// 用途：前端 src/services/config.ts wrapper → src/lib/snap/persistence.ts
// 读/写 KV `snap:constraints` JSON 数组（ADR-020 *Updated 2026-05-18*）。
//
// 与 commands/memory.rs 结构一致：validate + 调 service + error to string。

use crate::services::config;
use tauri::AppHandle;

const KEY_MAX_LEN: usize = 256;
/// snap:constraints JSON 数组 + 其他运行时配置（如未来 settings 表替代）需要更大空间。
/// 当前 memory.rs 是 8192，本 commands 为通用 config 走 32k。
const VALUE_MAX_LEN: usize = 32_768;

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
pub async fn config_get(app: AppHandle, key: String) -> Result<Option<String>, String> {
    let key = validate_key(&key)?;
    config::get(&app, key).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn config_set(app: AppHandle, key: String, value: String) -> Result<(), String> {
    let key = validate_key(&key)?.to_string();
    let value = validate_value(&value)?.to_string();
    config::set(&app, &key, &value)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn config_delete(app: AppHandle, key: String) -> Result<(), String> {
    let key = validate_key(&key)?;
    config::delete(&app, key).await.map_err(|e| e.to_string())
}
