// #5 MemoryService IPC commands — KV 偏好表（架构 §339 memory 表）
//
// 暴露 4 个：memory_get / memory_set / memory_list / memory_delete
// 用途：ChatService 拼 system prompt 时注 username / wake_time 等用户偏好（M3 接 LLM 后启用）。
//
// 注：旧项目 services/memory.rs 主要做 messages 表（对话消息）CRUD；
// 这里 commands/memory.rs 操作的是另一面 — KV 偏好表 memory(key, value, source, updated_at)。
// 走简洁的 commands 层直连模式，不改 services 层（避免污染 dogfood 过的整文件复用代码）。

use chrono::Utc;
use serde::Serialize;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection, FromRow, SqliteConnection};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, FromRow)]
pub struct MemoryItem {
    pub key: String,
    pub value: String,
    pub source: String,
    pub updated_at: String,
}

async fn open_conn(app: &AppHandle) -> Result<SqliteConnection, String> {
    let app_config = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("config dir: {e}"))?;
    let db_path = app_config.join("aipet.db");
    SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(false)
        .connect()
        .await
        .map_err(|e| format!("db connect: {e}"))
}

#[tauri::command]
pub async fn memory_get(app: AppHandle, key: String) -> Result<Option<String>, String> {
    let mut conn = open_conn(&app).await?;
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM memory WHERE key = ?")
        .bind(&key)
        .fetch_optional(&mut conn)
        .await
        .map_err(|e| format!("db query: {e}"))?;
    conn.close().await.map_err(|e| format!("db close: {e}"))?;
    Ok(row.map(|(v,)| v))
}

#[tauri::command]
pub async fn memory_set(app: AppHandle, key: String, value: String) -> Result<(), String> {
    let mut conn = open_conn(&app).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO memory (key, value, source, updated_at) VALUES (?, ?, 'user_set', ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, source = 'user_set', updated_at = excluded.updated_at",
    )
    .bind(&key)
    .bind(&value)
    .bind(&now)
    .execute(&mut conn)
    .await
    .map_err(|e| format!("db upsert: {e}"))?;
    conn.close().await.map_err(|e| format!("db close: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn memory_list(app: AppHandle) -> Result<Vec<MemoryItem>, String> {
    let mut conn = open_conn(&app).await?;
    let rows: Vec<MemoryItem> = sqlx::query_as::<_, MemoryItem>(
        "SELECT key, value, source, updated_at FROM memory ORDER BY key ASC",
    )
    .fetch_all(&mut conn)
    .await
    .map_err(|e| format!("db list: {e}"))?;
    conn.close().await.map_err(|e| format!("db close: {e}"))?;
    Ok(rows)
}

#[tauri::command]
pub async fn memory_delete(app: AppHandle, key: String) -> Result<(), String> {
    let mut conn = open_conn(&app).await?;
    sqlx::query("DELETE FROM memory WHERE key = ?")
        .bind(&key)
        .execute(&mut conn)
        .await
        .map_err(|e| format!("db delete: {e}"))?;
    conn.close().await.map_err(|e| format!("db close: {e}"))?;
    Ok(())
}
