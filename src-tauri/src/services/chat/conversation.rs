// chat/conversation.rs — 活跃 conversation KV + get-or-create（M1 极简）
//
// 偏离 issue #13 body 的 user_state 表：schema 没保留 user_state，运行时配置统一走
// `config` 表 KV（与 #10 #11 #12 同款）。守"27 表零迁移"D5 原则。
//
// IPC chat_send 不传 conversation_id 时 → ensure_active_conversation 复用或新建：
// - KV 存在且 conversations 行还在 → 返该 ID（M1 单 conversation 复用路径）
// - KV 不存在 / 行被删（孤儿 KV，M3 archive 场景）→ 建新 conversation + 写 KV → 返新 ID
//
// M3 B.3.d 多 conversation UI 上线后，本模块作底层 fallback 保留；UI 自管 conversation 列表。

use chrono::Utc;
use sqlx::{Connection, SqliteConnection};
use tauri::{AppHandle, Runtime};
use ulid::Ulid;

use crate::services::chat::ChatError;
use crate::services::config::{get_with_conn as config_get, set_with_conn as config_set};
use crate::services::db::open_app_db;

/// config 表 key：当前活跃 conversation ID。
pub const CONFIG_KEY_ACTIVE_CONVERSATION: &str = "chat:active_conversation_id";

/// 取当前活跃 conversation ID；不存在或孤儿 → 新建 + 更新 KV。
pub async fn ensure_active_conversation<R: Runtime>(
    app: &AppHandle<R>,
    persona_id: &str,
) -> Result<String, ChatError> {
    let mut conn = open_app_db(app).await?;
    let id = ensure_active_conversation_with_conn(&mut conn, persona_id).await?;
    conn.close().await?;
    Ok(id)
}

pub(crate) async fn ensure_active_conversation_with_conn(
    conn: &mut SqliteConnection,
    persona_id: &str,
) -> Result<String, ChatError> {
    // 1. 读 config KV
    let stored: Option<String> = config_get(conn, CONFIG_KEY_ACTIVE_CONVERSATION).await?;

    // 2. 验 conversations 行还在
    if let Some(id) = stored {
        let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM conversations WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *conn)
            .await?;
        if exists.is_some() {
            return Ok(id);
        }
        // 孤儿 KV：行被外部删（M3 B.3.d archive 等场景）→ 落到 fallback 建新
    }

    // 3. 建新 conversation
    let new_id = Ulid::new().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO conversations
            (id, persona_id, title, archived, started_at, last_activity_at, is_sandbox)
        VALUES (?, ?, NULL, 0, ?, ?, 0)
        "#,
    )
    .bind(&new_id)
    .bind(persona_id)
    .bind(&now)
    .bind(&now)
    .execute(&mut *conn)
    .await?;

    // 4. 更新 KV
    config_set(conn, CONFIG_KEY_ACTIVE_CONVERSATION, &new_id, &now).await?;

    Ok(new_id)
}

/// chat_send 完成后调，触发 last_activity_at 更新（M3 B.3.d 会话排序用）。
pub async fn update_last_activity<R: Runtime>(
    app: &AppHandle<R>,
    conversation_id: &str,
) -> Result<(), ChatError> {
    let mut conn = open_app_db(app).await?;
    update_last_activity_with_conn(&mut conn, conversation_id).await?;
    conn.close().await?;
    Ok(())
}

pub(crate) async fn update_last_activity_with_conn(
    conn: &mut SqliteConnection,
    conversation_id: &str,
) -> Result<(), ChatError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE conversations SET last_activity_at = ? WHERE id = ?")
        .bind(&now)
        .bind(conversation_id)
        .execute(conn)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::test_db::fresh_db;

    #[tokio::test]
    async fn first_call_creates_conversation_and_sets_kv() {
        let (_dir, mut conn) = fresh_db().await;
        let id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();

        assert_eq!(id.len(), 26, "should return 26-char ULID");

        // conversations 行存在
        let row: (String, String, i64) =
            sqlx::query_as("SELECT id, persona_id, archived FROM conversations WHERE id = ?")
                .bind(&id)
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(row.0, id);
        assert_eq!(row.1, "momo");
        assert_eq!(row.2, 0);

        // config KV 已写
        let stored = config_get(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION)
            .await
            .unwrap();
        assert_eq!(stored.as_deref(), Some(id.as_str()));
    }

    #[tokio::test]
    async fn second_call_reuses_existing_conversation() {
        let (_dir, mut conn) = fresh_db().await;
        let first = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        let second = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        assert_eq!(first, second, "second call must reuse active conversation");

        // conversations 表只 1 行
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM conversations")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(count.0, 1, "no extra conversation row should be created");
    }

    #[tokio::test]
    async fn orphaned_kv_falls_back_to_new_conversation() {
        // KV 指向不存在的 conv_id（如 M3 B.3.d archive 删行场景）→ 自愈
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        config_set(
            &mut conn,
            CONFIG_KEY_ACTIVE_CONVERSATION,
            "01ZZZZZZZZZZZZZZZZZZZZZZZZ",
            &now,
        )
        .await
        .unwrap();

        let id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        assert_ne!(
            id, "01ZZZZZZZZZZZZZZZZZZZZZZZZ",
            "orphan KV must not be returned"
        );

        // 行确实存在
        let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM conversations WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut conn)
            .await
            .unwrap();
        assert!(exists.is_some(), "new conversation must be inserted");

        // KV 已更新到新 ID
        let stored = config_get(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION)
            .await
            .unwrap();
        assert_eq!(stored.as_deref(), Some(id.as_str()));
    }

    #[tokio::test]
    async fn update_last_activity_modifies_timestamp() {
        let (_dir, mut conn) = fresh_db().await;
        let id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();

        let before: (String,) =
            sqlx::query_as("SELECT last_activity_at FROM conversations WHERE id = ?")
                .bind(&id)
                .fetch_one(&mut conn)
                .await
                .unwrap();

        // RFC3339 秒级精度，sleep 1.1s 确保时间戳不同
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        update_last_activity_with_conn(&mut conn, &id)
            .await
            .unwrap();

        let after: (String,) =
            sqlx::query_as("SELECT last_activity_at FROM conversations WHERE id = ?")
                .bind(&id)
                .fetch_one(&mut conn)
                .await
                .unwrap();

        assert!(
            after.0 > before.0,
            "after must be later than before; got before={} after={}",
            before.0,
            after.0
        );
    }

    #[tokio::test]
    async fn update_last_activity_unknown_id_is_noop() {
        // 不存在的 conv_id：UPDATE 影响 0 行，不应报错
        let (_dir, mut conn) = fresh_db().await;
        let result = update_last_activity_with_conn(&mut conn, "non-existent").await;
        assert!(
            result.is_ok(),
            "update on non-existent id must be a no-op, not error"
        );
    }
}
