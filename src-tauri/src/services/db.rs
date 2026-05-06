// 共享 DB helper（2026-05-06 code-review #1+#2）
//
// 动机：
// - persona / nickname / preferences / memory 4 处都在重新实现"取 app_config_dir + 拼 aipet.db
//   + SqliteConnectOptions builder + connect"，改一处必须改 4 处。
// - 更关键：SQLite 默认 `PRAGMA foreign_keys = OFF`（与 sqlx::SqliteConnectOptions
//   默认 ON 不同 — 但 plugin DbPool 路径 / 未来手动连接路径不保证）。把 PRAGMA 也收口在
//   helper 里，messages.conversation_id / persona_snapshots.persona_id 等 FK 才能稳定生效。
//
// 设计：
// - 用 builder API（不走 from_str("sqlite:...")）— Windows 绝对路径反斜杠会让 URL parsing
//   报 SQLITE_CANTOPEN(code 14)。
// - create_if_missing(false)：plugin migrations 已建库；service 层永远不该建库。测试 fixture
//   里走 create_if_missing(true) 是因为 tempfile 是空目录。
// - PRAGMA 用 conn.execute() 在每次新开连接后立刻发，与该 connection 绑定生效。

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Executor, SqliteConnection};
use tauri::{AppHandle, Manager, Runtime};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
    #[error("database error: {0}")]
    Database(String),
}

impl From<sqlx::Error> for DbError {
    fn from(e: sqlx::Error) -> Self {
        DbError::Database(e.to_string())
    }
}

/// 打开 app DB 主连接（<app_config_dir>/aipet.db）+ 强制启用 FK。
///
/// 返回的 SqliteConnection 的 `PRAGMA foreign_keys` 已为 ON；
/// 调用方应在使用完后 `.close().await`（或借助 Drop）。
pub async fn open_app_db<R: Runtime>(app: &AppHandle<R>) -> Result<SqliteConnection, DbError> {
    let app_config = app
        .path()
        .app_config_dir()
        .map_err(|e| DbError::AppConfigDir(e.to_string()))?;
    let db_path = app_config.join("aipet.db");
    let mut conn = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(false)
        .connect()
        .await?;
    enforce_pragmas(&mut conn).await?;
    Ok(conn)
}

/// 把测试 / 集成连接也拉到与 prod 一致的 PRAGMA 状态。
pub async fn enforce_pragmas(conn: &mut SqliteConnection) -> Result<(), DbError> {
    conn.execute("PRAGMA foreign_keys = ON").await?;
    Ok(())
}
