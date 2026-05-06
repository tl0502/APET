// #5 PersonaService IPC commands（H.1 持久化层入口）
//
// 暴露 2 个：persona_load / persona_activate
// - persona_load(id)：从 DB 读 personas + persona_snapshots 拼 PersonaSummary（含 raw markdown，供 ChatService 拼 system prompt）
// - persona_activate(id)：把所有 personas.is_active=0，目标 id 设 is_active=1
//
// services::persona 的 seed_builtin 在启动期已 UPSERT momo（lib.rs setup spawn），所以 M1 直接 load 即可。

use serde::Serialize;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection, SqliteConnection};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
pub struct PersonaSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: String,
    pub raw_markdown: String,
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
pub async fn persona_load(app: AppHandle, id: String) -> Result<PersonaSummary, String> {
    let mut conn = open_conn(&app).await?;
    let row: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT p.id, p.name, p.version, p.source \
         FROM personas p WHERE p.id = ?",
    )
    .bind(&id)
    .fetch_optional(&mut conn)
    .await
    .map_err(|e| format!("db query persona: {e}"))?;
    let (pid, name, version, source) =
        row.ok_or_else(|| format!("persona not found: {id}"))?;
    let snap: Option<(String,)> = sqlx::query_as(
        "SELECT content FROM persona_snapshots \
         WHERE persona_id = ? AND version = ? \
         ORDER BY id DESC LIMIT 1",
    )
    .bind(&pid)
    .bind(&version)
    .fetch_optional(&mut conn)
    .await
    .map_err(|e| format!("db query snapshot: {e}"))?;
    conn.close().await.map_err(|e| format!("db close: {e}"))?;
    Ok(PersonaSummary {
        id: pid,
        name,
        version,
        source,
        raw_markdown: snap.map(|(c,)| c).unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn persona_activate(app: AppHandle, id: String) -> Result<(), String> {
    let mut conn = open_conn(&app).await?;
    let mut tx = conn
        .begin()
        .await
        .map_err(|e| format!("db tx begin: {e}"))?;
    sqlx::query("UPDATE personas SET is_active = 0")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("db clear active: {e}"))?;
    let result = sqlx::query("UPDATE personas SET is_active = 1, updated_at = ? WHERE id = ?")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("db set active: {e}"))?;
    if result.rows_affected() == 0 {
        return Err(format!("persona not found: {id}"));
    }
    tx.commit().await.map_err(|e| format!("db tx commit: {e}"))?;
    conn.close().await.map_err(|e| format!("db close: {e}"))?;
    Ok(())
}
