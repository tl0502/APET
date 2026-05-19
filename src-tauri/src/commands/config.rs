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
use tauri::{AppHandle, Emitter, Runtime};

const KEY_MAX_LEN: usize = 256;
/// snap:constraints JSON 数组 + 其他运行时配置（如未来 settings 表替代）需要更大空间。
/// 当前 memory.rs 是 8192，本 commands 为通用 config 走 32k。
const VALUE_MAX_LEN: usize = 32_768;

/// B3 修复：persist+broadcast 原子 IPC 用的事件名，与 useSnapWindow CONSTRAINT_CHANGED_EVT 同源。
/// 前端两处都引用此字串字面量，重命名需同步改。
const SNAP_CONSTRAINT_CHANGED_EVT: &str = "snap:constraint-changed";
/// B3 修复：snap:constraints 持久化的固定 KV key（前端 persistence.ts SNAP_KV_KEY 同源）。
const SNAP_KV_KEY: &str = "snap:constraints";

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

/// B3 修复：原子持久化 + 跨 webview 广播 snap:constraint-changed。
///
/// 之前前端流程：
///   await persistConstraints()              // IPC config_set
///   await emit(CONSTRAINT_CHANGED_EVT, null) // 跨 webview 广播
///
/// 两个 IPC 之间无序保证：emit 可能先于 config_set 完成抵达其他 webview，
/// 其他 webview reload KV 读到旧值 → 状态分歧。
///
/// 现在 Rust 内串行：先 await config::set 完成（KV 写盘），再 app.emit 广播，
/// 保证 emit 抵达任一 webview 时 KV 已经是新值。
///
/// senderId 透传给前端 listener 自过滤（A4 修复）；payload 是 { senderId }。
#[tauri::command]
pub async fn snap_persist_and_broadcast<R: Runtime>(
    app: AppHandle<R>,
    value: String,
    sender_id: String,
) -> Result<(), String> {
    let value = validate_value(&value)?.to_string();
    config::set(&app, SNAP_KV_KEY, &value)
        .await
        .map_err(|e| e.to_string())?;
    // emit 失败不致命（持久化已成功）；但日志记一下方便排查。
    if let Err(e) = app.emit(
        SNAP_CONSTRAINT_CHANGED_EVT,
        serde_json::json!({ "senderId": sender_id }),
    ) {
        eprintln!("[snap_persist_and_broadcast] emit failed: {e}");
    }
    Ok(())
}
