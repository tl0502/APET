//! Reminder IPC commands（#22）— 6 命令 + scheduler eager check 后置。
//!
//! 命名遵循 Tauri 2.x runtime 规范：snake_case [a-zA-Z0-9_]（架构 §566 + 已有 chat_send /
//! persona_load 等命名风格）。架构 §604 文档逻辑写 `reminder.create` 仅作分组语义，注册
//! 名是 `reminder_create`。
//!
//! 每个写后操作（create/update/delete/snooze/complete）都调 `scheduler::reload_reminders`
//! 让 polling 不必等 5s tick——eager check + 防重入双保险。

use tauri::AppHandle;

use crate::services::reminder::{self, CreateInput, Reminder, UpdateInput};
use crate::services::scheduler;

#[tauri::command]
pub async fn reminder_create(
    app: AppHandle,
    input: CreateInput,
) -> Result<Reminder, String> {
    let out = reminder::create(&app, input)
        .await
        .map_err(|e| e.to_string())?;
    // eager check：用户在前端创建后立刻在桌宠上看到（不必等下次 polling）。
    if let Err(e) = scheduler::reload_reminders(&app).await {
        eprintln!("[reminder] reload after create failed: {e}");
    }
    Ok(out)
}

#[tauri::command]
pub async fn reminder_list(app: AppHandle) -> Result<Vec<Reminder>, String> {
    reminder::list(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reminder_update(
    app: AppHandle,
    id: String,
    input: UpdateInput,
) -> Result<Reminder, String> {
    let out = reminder::update(&app, id, input)
        .await
        .map_err(|e| e.to_string())?;
    if let Err(e) = scheduler::reload_reminders(&app).await {
        eprintln!("[reminder] reload after update failed: {e}");
    }
    Ok(out)
}

#[tauri::command]
pub async fn reminder_delete(app: AppHandle, id: String) -> Result<(), String> {
    reminder::delete(&app, id).await.map_err(|e| e.to_string())?;
    // delete 后不需要 eager check（heap 里的 timer 自然被 polling 下次扫描清掉）。
    Ok(())
}

#[tauri::command]
pub async fn reminder_snooze(
    app: AppHandle,
    id: String,
    minutes: u32,
) -> Result<Reminder, String> {
    let out = reminder::snooze(&app, id, minutes)
        .await
        .map_err(|e| e.to_string())?;
    // snooze 不需要 eager（next_fire_at 已往后推 ≥5min）。
    Ok(out)
}

#[tauri::command]
pub async fn reminder_complete(app: AppHandle, id: String) -> Result<(), String> {
    reminder::complete(&app, id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
