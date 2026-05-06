// PreferencesService — KV 偏好表（架构 §339 memory 表）
//
// 与 services/memory.rs 区分：
// - services/memory.rs 操作 messages 表（对话消息历史，由 ChatService 消费）
// - services/preferences.rs 操作 memory 表（KV：username / wake_time / 等用户偏好，
//   ChatService 拼 system prompt 时注入）
//
// 命名取舍：Rust 模块叫 preferences（清晰），但 IPC command 名继续用 memory_get/set/list/delete
// 与 schema 列名 `memory` 一致（避免破坏前端契约）。
//
// 设计：
// - source 默认 'user_set'；'inferred' 留给将来 LLM 推断后写入用（M3+）
// - get 不存在返回 None；set 走 INSERT ON CONFLICT UPSERT
// - 与 persona/nickname 同款：service 层 with_conn helper + 顶层 open_app_db 包装

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Connection, FromRow, SqliteConnection};
use tauri::{AppHandle, Runtime};
use thiserror::Error;

use crate::services::db::{open_app_db, DbError};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PreferenceItem {
    pub key: String,
    pub value: String,
    pub source: String,
    pub updated_at: String,
}

#[derive(Debug, Error)]
pub enum PreferenceError {
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
}

impl From<sqlx::Error> for PreferenceError {
    fn from(e: sqlx::Error) -> Self {
        PreferenceError::Database(e.to_string())
    }
}

impl From<DbError> for PreferenceError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => PreferenceError::AppConfigDir(s),
            DbError::Database(s) => PreferenceError::Database(s),
        }
    }
}

pub async fn get<R: Runtime>(
    app: &AppHandle<R>,
    key: &str,
) -> Result<Option<String>, PreferenceError> {
    let mut conn = open_app_db(app).await?;
    let v = get_with_conn(&mut conn, key).await?;
    conn.close().await?;
    Ok(v)
}

pub async fn set<R: Runtime>(
    app: &AppHandle<R>,
    key: &str,
    value: &str,
) -> Result<(), PreferenceError> {
    let mut conn = open_app_db(app).await?;
    set_with_conn(&mut conn, key, value, &Utc::now().to_rfc3339()).await?;
    conn.close().await?;
    Ok(())
}

pub async fn list<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<PreferenceItem>, PreferenceError> {
    let mut conn = open_app_db(app).await?;
    let rows = list_with_conn(&mut conn).await?;
    conn.close().await?;
    Ok(rows)
}

pub async fn delete<R: Runtime>(app: &AppHandle<R>, key: &str) -> Result<(), PreferenceError> {
    let mut conn = open_app_db(app).await?;
    delete_with_conn(&mut conn, key).await?;
    conn.close().await?;
    Ok(())
}

// ============================================================================
// Inner helpers — 测试 + prod 共用纯 SQL 路径
// ============================================================================

pub(crate) async fn get_with_conn(
    conn: &mut SqliteConnection,
    key: &str,
) -> Result<Option<String>, PreferenceError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM memory WHERE key = ?")
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
) -> Result<(), PreferenceError> {
    sqlx::query(
        r#"
        INSERT INTO memory (key, value, source, updated_at) VALUES (?, ?, 'user_set', ?)
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            source = 'user_set',
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

pub(crate) async fn list_with_conn(
    conn: &mut SqliteConnection,
) -> Result<Vec<PreferenceItem>, PreferenceError> {
    let rows = sqlx::query_as::<_, PreferenceItem>(
        "SELECT key, value, source, updated_at FROM memory ORDER BY key ASC",
    )
    .fetch_all(conn)
    .await?;
    Ok(rows)
}

pub(crate) async fn delete_with_conn(
    conn: &mut SqliteConnection,
    key: &str,
) -> Result<(), PreferenceError> {
    sqlx::query("DELETE FROM memory WHERE key = ?")
        .bind(key)
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
        set_with_conn(&mut conn, "username", "Alice", &now)
            .await
            .unwrap();
        let v = get_with_conn(&mut conn, "username").await.unwrap();
        assert_eq!(v.as_deref(), Some("Alice"));
    }

    #[tokio::test]
    async fn set_twice_updates_value_and_source() {
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        set_with_conn(&mut conn, "wake_time", "07:00", &now)
            .await
            .unwrap();
        set_with_conn(&mut conn, "wake_time", "08:30", &now)
            .await
            .unwrap();
        let rows = list_with_conn(&mut conn).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].value, "08:30");
        assert_eq!(rows[0].source, "user_set");
    }

    #[tokio::test]
    async fn list_orders_by_key_ascending() {
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        for k in ["zeta", "alpha", "mike"] {
            set_with_conn(&mut conn, k, "v", &now).await.unwrap();
        }
        let rows = list_with_conn(&mut conn).await.unwrap();
        let keys: Vec<_> = rows.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, vec!["alpha", "mike", "zeta"]);
    }

    #[tokio::test]
    async fn delete_removes_only_target() {
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        set_with_conn(&mut conn, "a", "1", &now).await.unwrap();
        set_with_conn(&mut conn, "b", "2", &now).await.unwrap();
        delete_with_conn(&mut conn, "a").await.unwrap();
        assert!(get_with_conn(&mut conn, "a").await.unwrap().is_none());
        assert_eq!(get_with_conn(&mut conn, "b").await.unwrap().as_deref(), Some("2"));
    }
}
