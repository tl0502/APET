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
use crate::services::config::{
    delete_with_conn as config_delete, get_with_conn as config_get, set_with_conn as config_set,
};
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
    ensure_active_conversation_with_snapshot_with_conn(conn, persona_id, None).await
}

pub(crate) async fn ensure_active_conversation_with_snapshot_with_conn(
    conn: &mut SqliteConnection,
    persona_id: &str,
    persona_snapshot_id: Option<i64>,
) -> Result<String, ChatError> {
    // 1. 读 config KV
    let stored: Option<String> = config_get(conn, CONFIG_KEY_ACTIVE_CONVERSATION).await?;

    // 2. 验 conversations 行还在 + 未归档
    if let Some(id) = stored {
        let exists: Option<(String,)> =
            sqlx::query_as("SELECT id FROM conversations WHERE id = ? AND archived = 0")
                .bind(&id)
                .fetch_optional(&mut *conn)
                .await?;
        if exists.is_some() {
            return Ok(id);
        }
        // 孤儿 / 归档 KV：行被外部删或归档（M3 B.3.d archive 等场景）→ 落到 fallback 建新
    }

    // 3. 建新 conversation
    let new_id = Ulid::new().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO conversations
            (id, persona_id, persona_snapshot_id, title, archived, started_at, last_activity_at, is_sandbox)
        VALUES (?, ?, ?, NULL, 0, ?, ?, 0)
        "#,
    )
    .bind(&new_id)
    .bind(persona_id)
    .bind(persona_snapshot_id)
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
    // 加 archived = 0 守护：流式收尾期间会话被另一窗口归档时，避免归档行的
    // last_activity_at 被刷新（用户取消归档后会看到时间戳错位 + 排序异常）。
    // 命中 archived = 1 → rows_affected = 0，静默 ok（与"id 不存在 = no-op"语义一致）。
    sqlx::query("UPDATE conversations SET last_activity_at = ? WHERE id = ? AND archived = 0")
        .bind(&now)
        .bind(conversation_id)
        .execute(conn)
        .await?;
    Ok(())
}

/// 单条 conversation summary（侧边栏列表用，不带 messages）。
///
/// `title` 为 NULL 时前端 fallback 到"未命名 + started_at" UI 文案；M3 B.3.d 真重命名 UI
/// 上线后用户填的 title 会替换 NULL。
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct ConversationSummary {
    pub id: String,
    pub persona_id: String,
    pub title: Option<String>,
    pub started_at: String,
    pub last_activity_at: String,
}

/// 列出所有未归档 conversation，按 last_activity_at DESC（与 idx_conversations_active
/// 索引方向一致；ChatPanel 侧边栏直接消费）。limit clamp 到 [1, 200]。
pub async fn list_conversations<R: Runtime>(
    app: &AppHandle<R>,
    limit: u32,
) -> Result<Vec<ConversationSummary>, ChatError> {
    let mut conn = open_app_db(app).await?;
    let rows = list_conversations_with_conn(&mut conn, limit).await?;
    conn.close().await?;
    Ok(rows)
}

pub(crate) async fn list_conversations_with_conn(
    conn: &mut SqliteConnection,
    limit: u32,
) -> Result<Vec<ConversationSummary>, ChatError> {
    let limit = limit.clamp(1, 200) as i64;
    let rows: Vec<ConversationSummary> = sqlx::query_as(
        r#"
        SELECT id, persona_id, title, started_at, last_activity_at
        FROM conversations
        WHERE archived = 0
        ORDER BY last_activity_at DESC
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(conn)
    .await?;
    Ok(rows)
}

/// 显式新建一条 conversation 并把 active KV 切到它（侧边栏"新建对话"按钮路径）。
///
/// 与 ensure_active_conversation 区别：那个是"复用或新建"懒模式；本函数永远建新行。
/// chat_send 后续传入新 id 会按它写 messages。
pub async fn create_conversation<R: Runtime>(
    app: &AppHandle<R>,
    persona_id: &str,
) -> Result<String, ChatError> {
    let mut conn = open_app_db(app).await?;
    let id = create_conversation_with_conn(&mut conn, persona_id).await?;
    conn.close().await?;
    Ok(id)
}

pub async fn create_conversation_for_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    persona_id: &str,
    persona_snapshot_id: i64,
) -> Result<String, ChatError> {
    let mut conn = open_app_db(app).await?;
    let id = create_conversation_with_snapshot_with_conn(
        &mut conn,
        persona_id,
        Some(persona_snapshot_id),
    )
    .await?;
    conn.close().await?;
    Ok(id)
}

pub(crate) async fn create_conversation_with_conn(
    conn: &mut SqliteConnection,
    persona_id: &str,
) -> Result<String, ChatError> {
    create_conversation_with_snapshot_with_conn(conn, persona_id, None).await
}

pub(crate) async fn create_conversation_with_snapshot_with_conn(
    conn: &mut SqliteConnection,
    persona_id: &str,
    persona_snapshot_id: Option<i64>,
) -> Result<String, ChatError> {
    let new_id = Ulid::new().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO conversations
            (id, persona_id, persona_snapshot_id, title, archived, started_at, last_activity_at, is_sandbox)
        VALUES (?, ?, ?, NULL, 0, ?, ?, 0)
        "#,
    )
    .bind(&new_id)
    .bind(persona_id)
    .bind(persona_snapshot_id)
    .bind(&now)
    .bind(&now)
    .execute(&mut *conn)
    .await?;
    config_set(conn, CONFIG_KEY_ACTIVE_CONVERSATION, &new_id, &now).await?;
    Ok(new_id)
}

/// 切换活跃 conversation（侧边栏点击列表项路径）。
///
/// B6 修复：写 KV 前先 SELECT 验 conversation 行存在；不存在 → 抛
/// `ChatError::Database("对话不存在或已被归档：{id}")`，前端 catch + toast。
/// 之前的设计假设"前端 list 拿到的 id 必然合法"，但跨窗口同时操作 / 列表过期场景下会
/// 写脏 KV，下次 chat_send 才自愈期间用户切过去看见空对话无法理解发生了什么。
///
/// 加 `AND archived = 0` 守护：归档后另一窗口列表过期撞它会报错触发刷新，避免
/// "set_active 成功 → 消息进归档行 → list 看不见"的不可见割裂。
pub async fn set_active_conversation<R: Runtime>(
    app: &AppHandle<R>,
    conversation_id: &str,
) -> Result<(), ChatError> {
    let mut conn = open_app_db(app).await?;
    set_active_conversation_with_conn(&mut conn, conversation_id).await?;
    conn.close().await?;
    Ok(())
}

pub(crate) async fn set_active_conversation_with_conn(
    conn: &mut SqliteConnection,
    conversation_id: &str,
) -> Result<(), ChatError> {
    let exists: Option<(String,)> =
        sqlx::query_as("SELECT id FROM conversations WHERE id = ? AND archived = 0")
            .bind(conversation_id)
            .fetch_optional(&mut *conn)
            .await?;
    if exists.is_none() {
        return Err(ChatError::Database(format!(
            "对话不存在或已被归档：{conversation_id}"
        )));
    }
    let now = Utc::now().to_rfc3339();
    config_set(conn, CONFIG_KEY_ACTIVE_CONVERSATION, conversation_id, &now).await?;
    Ok(())
}

/// title 长度上限；前端 maxlength 守，后端兜底防直接 IPC 调用。
const TITLE_MAX_LEN: usize = 100;

/// 重命名 conversation。
///
/// trim 后空 → 写 NULL（恢复"未命名"显示）；非空 → 写 title（≤ TITLE_MAX_LEN 字符；
/// 超长截断而非报错，UI 已守 maxlength）。
/// 不存在 id → ChatError::Database("conversation not found: {id}")。
pub async fn rename_conversation<R: Runtime>(
    app: &AppHandle<R>,
    conversation_id: &str,
    new_title: &str,
) -> Result<(), ChatError> {
    let mut conn = open_app_db(app).await?;
    rename_conversation_with_conn(&mut conn, conversation_id, new_title).await?;
    conn.close().await?;
    Ok(())
}

pub(crate) async fn rename_conversation_with_conn(
    conn: &mut SqliteConnection,
    conversation_id: &str,
    new_title: &str,
) -> Result<(), ChatError> {
    let trimmed = new_title.trim();
    let title_opt: Option<String> = if trimmed.is_empty() {
        None
    } else {
        let truncated: String = trimmed.chars().take(TITLE_MAX_LEN).collect();
        Some(truncated)
    };
    let result = sqlx::query("UPDATE conversations SET title = ? WHERE id = ?")
        .bind(&title_opt)
        .bind(conversation_id)
        .execute(conn)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ChatError::Database(format!(
            "conversation not found: {conversation_id}"
        )));
    }
    Ok(())
}

/// 归档 conversation（archived = 1；从 list_conversations 隐藏）。
///
/// 命中 active KV → 删 KV（ensure_active 下次自愈，或前端先切走）。
/// 行不存在 → no-op（前端列表过期容忍）。
pub async fn archive_conversation<R: Runtime>(
    app: &AppHandle<R>,
    conversation_id: &str,
) -> Result<(), ChatError> {
    let mut conn = open_app_db(app).await?;
    archive_conversation_with_conn(&mut conn, conversation_id).await?;
    conn.close().await?;
    Ok(())
}

pub(crate) async fn archive_conversation_with_conn(
    conn: &mut SqliteConnection,
    conversation_id: &str,
) -> Result<(), ChatError> {
    sqlx::query("UPDATE conversations SET archived = 1 WHERE id = ?")
        .bind(conversation_id)
        .execute(&mut *conn)
        .await?;
    clear_active_kv_if_match(conn, conversation_id).await?;
    Ok(())
}

/// 硬删 conversation（FK ON DELETE CASCADE + db.rs PRAGMA foreign_keys=ON 已开 →
/// messages 自动级联删；db.rs:57）。
///
/// 命中 active KV → 删 KV。行不存在 → no-op。
pub async fn delete_conversation<R: Runtime>(
    app: &AppHandle<R>,
    conversation_id: &str,
) -> Result<(), ChatError> {
    let mut conn = open_app_db(app).await?;
    delete_conversation_with_conn(&mut conn, conversation_id).await?;
    conn.close().await?;
    Ok(())
}

pub(crate) async fn delete_conversation_with_conn(
    conn: &mut SqliteConnection,
    conversation_id: &str,
) -> Result<(), ChatError> {
    sqlx::query("DELETE FROM conversations WHERE id = ?")
        .bind(conversation_id)
        .execute(&mut *conn)
        .await?;
    clear_active_kv_if_match(conn, conversation_id).await?;
    Ok(())
}

/// 内部 helper：active KV 当前值若等于给定 id 则清空（archive/delete 共用）。
async fn clear_active_kv_if_match(
    conn: &mut SqliteConnection,
    conversation_id: &str,
) -> Result<(), ChatError> {
    let stored = config_get(conn, CONFIG_KEY_ACTIVE_CONVERSATION).await?;
    if stored.as_deref() == Some(conversation_id) {
        config_delete(conn, CONFIG_KEY_ACTIVE_CONVERSATION).await?;
    }
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

    #[tokio::test]
    async fn rename_writes_title_then_clears_to_null() {
        let (_dir, mut conn) = fresh_db().await;
        let id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();

        rename_conversation_with_conn(&mut conn, &id, "工作问答")
            .await
            .unwrap();
        let row: (Option<String>,) = sqlx::query_as("SELECT title FROM conversations WHERE id = ?")
            .bind(&id)
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(row.0.as_deref(), Some("工作问答"));

        // 空字符串 → 写 NULL（恢复"未命名"）
        rename_conversation_with_conn(&mut conn, &id, "   ")
            .await
            .unwrap();
        let row: (Option<String>,) = sqlx::query_as("SELECT title FROM conversations WHERE id = ?")
            .bind(&id)
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert!(row.0.is_none());
    }

    #[tokio::test]
    async fn rename_unknown_id_returns_error() {
        let (_dir, mut conn) = fresh_db().await;
        let r = rename_conversation_with_conn(&mut conn, "non-existent", "x").await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn archive_hides_from_list_and_clears_active_kv() {
        let (_dir, mut conn) = fresh_db().await;
        let id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();

        // 归档前在列表里
        let before = list_conversations_with_conn(&mut conn, 50).await.unwrap();
        assert_eq!(before.len(), 1);

        archive_conversation_with_conn(&mut conn, &id)
            .await
            .unwrap();

        // 归档后从列表消失
        let after = list_conversations_with_conn(&mut conn, 50).await.unwrap();
        assert!(after.is_empty(), "archived conversation must be hidden");

        // 命中 active KV → KV 已清
        let kv = config_get(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION)
            .await
            .unwrap();
        assert!(kv.is_none(), "active KV should be cleared after archive");

        // archived 列还能查到（数据未删）
        let row: (i64,) = sqlx::query_as("SELECT archived FROM conversations WHERE id = ?")
            .bind(&id)
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(row.0, 1);
    }

    #[tokio::test]
    async fn delete_cascades_messages_and_clears_active_kv() {
        let (_dir, mut conn) = fresh_db().await;
        let id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();

        // 塞两条 messages 验级联删
        let now = Utc::now().to_rfc3339();
        for content in ["你好", "你好呀"] {
            sqlx::query(
                "INSERT INTO messages (id, conversation_id, role, content, mode, created_at) \
                 VALUES (?, ?, 'user', ?, 'online', ?)",
            )
            .bind(Ulid::new().to_string())
            .bind(&id)
            .bind(content)
            .bind(&now)
            .execute(&mut conn)
            .await
            .unwrap();
        }

        delete_conversation_with_conn(&mut conn, &id).await.unwrap();

        let conv_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM conversations WHERE id = ?")
            .bind(&id)
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(conv_count.0, 0);

        // FK ON DELETE CASCADE + PRAGMA foreign_keys=ON（test_db::fresh_db 已开）→ 0 行
        let msg_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM messages WHERE conversation_id = ?")
                .bind(&id)
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(
            msg_count.0, 0,
            "messages must cascade-delete with conversation"
        );

        let kv = config_get(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION)
            .await
            .unwrap();
        assert!(kv.is_none());
    }

    #[tokio::test]
    async fn archive_non_active_keeps_active_kv() {
        // archive 一条不是 active 的会话 → KV 不动
        let (_dir, mut conn) = fresh_db().await;
        let active_id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        let other_id = create_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        // create_conversation 会把 active KV 切到 other_id；切回 active_id
        let now = Utc::now().to_rfc3339();
        config_set(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION, &active_id, &now)
            .await
            .unwrap();

        archive_conversation_with_conn(&mut conn, &other_id)
            .await
            .unwrap();

        let kv = config_get(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION)
            .await
            .unwrap();
        assert_eq!(kv.as_deref(), Some(active_id.as_str()));
    }

    // ===== B6：set_active_conversation 验存在 =====

    #[tokio::test]
    async fn set_active_existing_id_writes_kv() {
        let (_dir, mut conn) = fresh_db().await;
        let id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        // 改 KV 到一个其他值再切回，验证 set_active 真的写了
        let now = Utc::now().to_rfc3339();
        config_set(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION, "stale", &now)
            .await
            .unwrap();
        set_active_conversation_with_conn(&mut conn, &id)
            .await
            .unwrap();
        let kv = config_get(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION)
            .await
            .unwrap();
        assert_eq!(kv.as_deref(), Some(id.as_str()));
    }

    #[tokio::test]
    async fn set_active_unknown_id_returns_error_and_keeps_kv() {
        // 不存在的 id → Err；KV 不动（前端 catch toast 后用户可重新选）
        let (_dir, mut conn) = fresh_db().await;
        let real_id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        let r = set_active_conversation_with_conn(&mut conn, "01ZZZZZZZZZZZZZZZZZZZZZZZZ").await;
        assert!(r.is_err(), "不存在的 id 应抛错");
        let kv = config_get(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION)
            .await
            .unwrap();
        assert_eq!(
            kv.as_deref(),
            Some(real_id.as_str()),
            "失败时 KV 不应被覆盖为孤儿值"
        );
    }

    // ===== archived 守护（set_active / ensure_active / update_last_activity 三处） =====

    #[tokio::test]
    async fn set_active_archived_id_returns_error_and_keeps_kv() {
        // 跨窗口归档场景：列表过期撞归档行 → Err；不会把消息写进不可见会话
        let (_dir, mut conn) = fresh_db().await;
        let active = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        let other = create_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        // create 把 KV 切到 other；切回 active 后归档 other（不命中 active KV）
        let now = Utc::now().to_rfc3339();
        config_set(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION, &active, &now)
            .await
            .unwrap();
        archive_conversation_with_conn(&mut conn, &other)
            .await
            .unwrap();

        let r = set_active_conversation_with_conn(&mut conn, &other).await;
        assert!(r.is_err(), "切到归档行应报错");
        let kv = config_get(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION)
            .await
            .unwrap();
        assert_eq!(
            kv.as_deref(),
            Some(active.as_str()),
            "归档行 set_active 失败时 KV 不应被改"
        );
    }

    #[tokio::test]
    async fn ensure_active_archived_kv_falls_back_to_new_conversation() {
        // KV 指向归档行（如另一窗口归档但当前窗口的 KV 缓存未刷）→ 视同孤儿，自愈建新
        let (_dir, mut conn) = fresh_db().await;
        let id1 = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        archive_conversation_with_conn(&mut conn, &id1)
            .await
            .unwrap();
        // archive 已清 active KV；手动写回模拟"另一窗口归档但当前 KV 没刷"
        let now = Utc::now().to_rfc3339();
        config_set(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION, &id1, &now)
            .await
            .unwrap();

        let id2 = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        assert_ne!(id1, id2, "归档 KV 必须触发 fallback 建新，不能复用归档行");

        // KV 已被切到新行
        let kv = config_get(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION)
            .await
            .unwrap();
        assert_eq!(kv.as_deref(), Some(id2.as_str()));
    }

    #[tokio::test]
    async fn update_last_activity_skips_archived_conversation() {
        // 流式收尾期间会话被归档：UPDATE 不命中归档行；时间戳保持归档前的值
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
        archive_conversation_with_conn(&mut conn, &id)
            .await
            .unwrap();
        // RFC3339 秒级精度，sleep 1.1s 确保若 UPDATE 命中，时间戳必然不同
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
        assert_eq!(
            before.0, after.0,
            "归档行的 last_activity_at 不应被流式收尾刷新"
        );
    }
}
