// ConfigService — `config` 表 KV（架构 §4 line 17：运行时配置走此表）
//
// 与 services/preferences.rs 区分：
// - preferences.rs 操作 `memory` 表（用户偏好 / LLM 推断；ChatService 拼 system prompt 时注入）
// - config.rs 操作 `config` 表（运行时配置；窗口位置 / active_conversation_id / 快捷键绑定 等）
//
// 设计与 preferences.rs 一致：with_conn helper + 顶层 wrapper；INSERT ON CONFLICT UPSERT。

use chrono::Utc;
use sqlx::{Connection, SqliteConnection};
use tauri::{AppHandle, Runtime};
use thiserror::Error;

use crate::services::db::{open_app_db, DbError};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
}

impl From<sqlx::Error> for ConfigError {
    fn from(e: sqlx::Error) -> Self {
        ConfigError::Database(e.to_string())
    }
}

impl From<DbError> for ConfigError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => ConfigError::AppConfigDir(s),
            DbError::Database(s) => ConfigError::Database(s),
        }
    }
}

pub async fn get<R: Runtime>(app: &AppHandle<R>, key: &str) -> Result<Option<String>, ConfigError> {
    let mut conn = open_app_db(app).await?;
    let v = get_with_conn(&mut conn, key).await?;
    conn.close().await?;
    Ok(v)
}

pub async fn set<R: Runtime>(
    app: &AppHandle<R>,
    key: &str,
    value: &str,
) -> Result<(), ConfigError> {
    let mut conn = open_app_db(app).await?;
    set_with_conn(&mut conn, key, value, &Utc::now().to_rfc3339()).await?;
    conn.close().await?;
    Ok(())
}

pub(crate) async fn get_with_conn(
    conn: &mut SqliteConnection,
    key: &str,
) -> Result<Option<String>, ConfigError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM config WHERE key = ?")
        .bind(key)
        .fetch_optional(conn)
        .await?;
    Ok(row.map(|(v,)| v))
}

pub(crate) async fn set_with_conn(
    conn: &mut SqliteConnection,
    key: &str,
    value: &str,
    now_rfc3339: &str,
) -> Result<(), ConfigError> {
    sqlx::query(
        r#"
        INSERT INTO config (key, value, updated_at) VALUES (?, ?, ?)
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(key)
    .bind(value)
    .bind(now_rfc3339)
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::test_db::fresh_db;

    #[tokio::test]
    async fn get_unknown_key_returns_none() {
        let (_dir, mut conn) = fresh_db().await;
        let v = get_with_conn(&mut conn, "no-such-key").await.unwrap();
        assert!(v.is_none());
    }

    #[tokio::test]
    async fn set_then_get_roundtrips() {
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        set_with_conn(&mut conn, "window:pet:last_position", "{\"x\":100}", &now)
            .await
            .unwrap();
        let v = get_with_conn(&mut conn, "window:pet:last_position")
            .await
            .unwrap();
        assert_eq!(v.as_deref(), Some("{\"x\":100}"));
    }

    #[tokio::test]
    async fn set_twice_overwrites() {
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        set_with_conn(&mut conn, "k", "v1", &now).await.unwrap();
        set_with_conn(&mut conn, "k", "v2", &now).await.unwrap();
        let v = get_with_conn(&mut conn, "k").await.unwrap();
        assert_eq!(v.as_deref(), Some("v2"));
    }
}
