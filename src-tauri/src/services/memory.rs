// F.1 MemoryService MVP — messages 表 CRUD + summary 占位
//
// 范围(plan F.1,选项 A):
// - messages 表 CRUD(insert / list / delete by id / delete by conversation)
// - summary 占位:summarize_conversation 返回固定字符串,M3 引入摘要算法时再填实
// - cleanup_messages_older_than(days) 私有 stub:不在 setup 调,留给将来设置面板触发
//
// 设计决定(2026-05-03 与用户对齐):
// - **不做 90 天自动清理** — 偏离 local-first 精神;桌宠核心价值是"老朋友",自动清空对话反价值
// - **默认无限保留** + 用户主动清理(类似 ChatGPT 网页);UI 入口由后续模块 B.3 / 设置面板提供
// - PRD §73 / 架构 §549 的"90 天默认 + is_deleted 软删"措辞与本实现偏差,留给 doc-aligner 后续对齐
// - messages.id 走 ULID(schema 注释明写;时间序利于 (conversation_id, created_at) 索引)
// - DB 连接同 persona.rs 模式:自开 sqlx 短期连接,DB 路径用 app.path().app_config_dir()

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection, FromRow, SqliteConnection};
use tauri::{AppHandle, Manager, Runtime};
use thiserror::Error;
use ulid::Ulid;

const VALID_ROLES: &[&str] = &["user", "assistant", "system"];
const VALID_MODES: &[&str] = &["online", "offline_rule"];

const SUMMARY_PLACEHOLDER: &str = "[摘要功能将于 M3 引入;此处为占位符,详见 progress/decisions-log.md]";

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MessageRecord {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub mode: String,
    pub created_at: String,
}

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("invalid role: got '{0}', want one of {1:?}")]
    InvalidRole(String, &'static [&'static str]),
    #[error("invalid mode: got '{0}', want one of {1:?}")]
    InvalidMode(String, &'static [&'static str]),
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
}

impl From<sqlx::Error> for MemoryError {
    fn from(e: sqlx::Error) -> Self {
        MemoryError::Database(e.to_string())
    }
}

/// 纯逻辑构造:校验 role / mode + 生成 ULID + RFC3339 时间戳。
///
/// 抽出来便于测试 — insert_message 内部组装时 wrap 这个函数。
pub fn build_message_record(
    conversation_id: String,
    role: String,
    content: String,
    mode: String,
) -> Result<MessageRecord, MemoryError> {
    if !VALID_ROLES.contains(&role.as_str()) {
        return Err(MemoryError::InvalidRole(role, VALID_ROLES));
    }
    if !VALID_MODES.contains(&mode.as_str()) {
        return Err(MemoryError::InvalidMode(mode, VALID_MODES));
    }
    Ok(MessageRecord {
        id: Ulid::new().to_string(),
        conversation_id,
        role,
        content,
        mode,
        created_at: Utc::now().to_rfc3339(),
    })
}

/// 计算 N 天之前的 RFC3339 时间戳 — cleanup_messages_older_than 用。
fn cutoff_for_days(days: u32) -> String {
    (Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339()
}

async fn open_conn<R: Runtime>(app: &AppHandle<R>) -> Result<SqliteConnection, MemoryError> {
    let app_config = app
        .path()
        .app_config_dir()
        .map_err(|e| MemoryError::AppConfigDir(e.to_string()))?;
    let db_path = app_config.join("aipet.db");
    // builder API 避开 URL parsing(Windows 绝对路径反斜杠会让 from_str 报 code 14)
    Ok(SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(false)
        .connect()
        .await?)
}

/// 写入一条消息(ChatService B.2 streaming 完成后调用)。
pub async fn insert_message<R: Runtime>(
    app: &AppHandle<R>,
    conversation_id: String,
    role: String,
    content: String,
    mode: String,
) -> Result<MessageRecord, MemoryError> {
    let record = build_message_record(conversation_id, role, content, mode)?;

    let mut conn = open_conn(app).await?;
    insert_message_with_conn(&mut conn, &record).await?;
    conn.close().await?;

    Ok(record)
}

/// 按 conversation_id 列消息,按 created_at 升序;limit=None 时返回全部。
///
/// ChatService 拼 system prompt + recent messages 时调用;UI 翻历史也用。
pub async fn list_messages_by_conversation<R: Runtime>(
    app: &AppHandle<R>,
    conversation_id: &str,
    limit: Option<u32>,
) -> Result<Vec<MessageRecord>, MemoryError> {
    let mut conn = open_conn(app).await?;
    let records = list_messages_by_conversation_with_conn(&mut conn, conversation_id, limit).await?;
    conn.close().await?;
    Ok(records)
}

/// 按消息 ID 删除(用户主动 "删除某条" 用)。
pub async fn delete_message<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
) -> Result<(), MemoryError> {
    let mut conn = open_conn(app).await?;
    delete_message_with_conn(&mut conn, id).await?;
    conn.close().await?;
    Ok(())
}

/// 按 conversation_id 清空全部消息(用户在 ChatPanel 点 "清空对话" 时调用 backing)。
/// 返回删除行数。
pub async fn delete_messages_by_conversation<R: Runtime>(
    app: &AppHandle<R>,
    conversation_id: &str,
) -> Result<u64, MemoryError> {
    let mut conn = open_conn(app).await?;
    let count = delete_messages_by_conversation_with_conn(&mut conn, conversation_id).await?;
    conn.close().await?;
    Ok(count)
}

/// 摘要占位 — M3 引入压缩算法时填实(LLM 摘要 / 抽取式摘要等)。
pub async fn summarize_conversation<R: Runtime>(
    _app: &AppHandle<R>,
    _conversation_id: &str,
) -> Result<String, MemoryError> {
    Ok(SUMMARY_PLACEHOLDER.to_string())
}

/// 私有 stub:删除 N 天前的消息。F.1 不在启动时调,留给将来设置面板"自动清理"开关触发。
///
/// 设计意图:用户主动选择"开启 N 天清理"时才生效,默认无限保留(local-first 精神)。
#[allow(dead_code)]
pub(crate) async fn cleanup_messages_older_than<R: Runtime>(
    app: &AppHandle<R>,
    days: u32,
) -> Result<u64, MemoryError> {
    let cutoff = cutoff_for_days(days);
    let mut conn = open_conn(app).await?;
    let count = cleanup_messages_with_conn(&mut conn, &cutoff).await?;
    conn.close().await?;
    Ok(count)
}

// ============================================================================
// Inner helpers(2026-05-04 test-coverage):见 secrets.rs 同段注释
// ============================================================================

pub(crate) async fn insert_message_with_conn(
    conn: &mut SqliteConnection,
    record: &MessageRecord,
) -> Result<(), MemoryError> {
    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, content, mode, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&record.id)
    .bind(&record.conversation_id)
    .bind(&record.role)
    .bind(&record.content)
    .bind(&record.mode)
    .bind(&record.created_at)
    .execute(conn)
    .await?;
    Ok(())
}

pub(crate) async fn list_messages_by_conversation_with_conn(
    conn: &mut SqliteConnection,
    conversation_id: &str,
    limit: Option<u32>,
) -> Result<Vec<MessageRecord>, MemoryError> {
    let records: Vec<MessageRecord> = match limit {
        Some(n) => sqlx::query_as::<_, MessageRecord>(
            r#"
            SELECT id, conversation_id, role, content, mode, created_at
            FROM messages
            WHERE conversation_id = ?
            ORDER BY created_at ASC
            LIMIT ?
            "#,
        )
        .bind(conversation_id)
        .bind(n)
        .fetch_all(conn)
        .await?,
        None => sqlx::query_as::<_, MessageRecord>(
            r#"
            SELECT id, conversation_id, role, content, mode, created_at
            FROM messages
            WHERE conversation_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(conversation_id)
        .fetch_all(conn)
        .await?,
    };
    Ok(records)
}

pub(crate) async fn delete_message_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<(), MemoryError> {
    sqlx::query("DELETE FROM messages WHERE id = ?")
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}

pub(crate) async fn delete_messages_by_conversation_with_conn(
    conn: &mut SqliteConnection,
    conversation_id: &str,
) -> Result<u64, MemoryError> {
    let result = sqlx::query("DELETE FROM messages WHERE conversation_id = ?")
        .bind(conversation_id)
        .execute(conn)
        .await?;
    Ok(result.rows_affected())
}

pub(crate) async fn cleanup_messages_with_conn(
    conn: &mut SqliteConnection,
    cutoff_rfc3339: &str,
) -> Result<u64, MemoryError> {
    let result = sqlx::query("DELETE FROM messages WHERE created_at < ?")
        .bind(cutoff_rfc3339)
        .execute(conn)
        .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    #[test]
    fn build_message_record_creates_ulid_and_rfc3339() {
        let record = build_message_record(
            "conv-test".to_string(),
            "user".to_string(),
            "hello".to_string(),
            "online".to_string(),
        )
        .expect("valid input should build");
        assert_eq!(record.id.len(), 26, "ULID should be 26 chars");
        DateTime::parse_from_rfc3339(&record.created_at).expect("created_at must be RFC3339");
        assert_eq!(record.role, "user");
        assert_eq!(record.mode, "online");
    }

    #[test]
    fn build_message_record_rejects_invalid_role() {
        let result = build_message_record(
            "conv".to_string(),
            "admin".to_string(),
            "x".to_string(),
            "online".to_string(),
        );
        assert!(matches!(result, Err(MemoryError::InvalidRole(_, _))));
    }

    #[test]
    fn build_message_record_rejects_invalid_mode() {
        let result = build_message_record(
            "conv".to_string(),
            "user".to_string(),
            "x".to_string(),
            "voice".to_string(),
        );
        assert!(matches!(result, Err(MemoryError::InvalidMode(_, _))));
    }

    #[test]
    fn build_message_record_accepts_all_valid_roles_and_modes() {
        for role in VALID_ROLES {
            for mode in VALID_MODES {
                let r = build_message_record(
                    "c".into(),
                    (*role).into(),
                    "x".into(),
                    (*mode).into(),
                );
                assert!(r.is_ok(), "role={role}, mode={mode} should be accepted");
            }
        }
    }

    #[test]
    fn cutoff_for_days_is_in_the_past() {
        let cutoff = cutoff_for_days(90);
        let parsed: DateTime<Utc> = DateTime::parse_from_rfc3339(&cutoff)
            .expect("cutoff must be RFC3339")
            .with_timezone(&Utc);
        let now = Utc::now();
        let delta = now.signed_duration_since(parsed);
        // 允许 ±1 天误差应对测试调度延迟
        assert!(
            delta.num_days() >= 89 && delta.num_days() <= 91,
            "expected ~90 days; got {} days",
            delta.num_days()
        );
    }

    #[test]
    fn summary_placeholder_is_recognizable() {
        assert!(
            SUMMARY_PLACEHOLDER.contains("M3"),
            "summary placeholder must mention M3 milestone"
        );
        assert!(
            SUMMARY_PLACEHOLDER.contains("占位"),
            "summary placeholder must be self-identifying as stub"
        );
    }

    // ===== DB 集成测试(2026-05-04 test-coverage P0)=====

    use crate::services::test_db::fresh_db;

    /// 工具:确保 conversations 表里存在 conv_id 行,绕过 messages.conversation_id FK 约束。
    ///
    /// **重要 prod note**:sqlx 默认 `PRAGMA foreign_keys = ON`(与 SQLite 本身默认 OFF 不同),
    /// 所以 `insert_message` 在 prod 也必须先有 conversations 行。这意味着 B.2 ChatService
    /// 在调用 insert_message 之前必须先 ensure conversation 存在(预期由 ConversationStore 负责)。
    /// 详 progress/test-coverage-2026-05-04.md § "暴露的 prod 隐患"。
    async fn ensure_conversation(conn: &mut SqliteConnection, conv_id: &str) {
        sqlx::query(
            "INSERT OR IGNORE INTO conversations (id, persona_id, started_at, last_activity_at) \
             VALUES (?, 'momo', '2026-05-04T00:00:00Z', '2026-05-04T00:00:00Z')",
        )
        .bind(conv_id)
        .execute(conn)
        .await
        .unwrap();
    }

    /// 工具:直接构造 record 并插入。先确保 conversation 存在(FK 约束)。
    async fn insert_user_msg(
        conn: &mut SqliteConnection,
        conv_id: &str,
        content: &str,
    ) -> MessageRecord {
        ensure_conversation(conn, conv_id).await;
        let record = build_message_record(
            conv_id.to_string(),
            "user".to_string(),
            content.to_string(),
            "online".to_string(),
        )
        .expect("build_message_record");
        insert_message_with_conn(conn, &record).await.unwrap();
        record
    }

    #[tokio::test]
    async fn insert_message_rejects_unknown_conversation_id() {
        // 防御 FK:不存在的 conversation_id 必须被 REFERENCES 守住
        // 这同时验证了 B.2 ChatService 实施前的契约 — insert_message 调用方必须保证
        // conversations 行存在,否则报 FK 错误
        let (_dir, mut conn) = fresh_db().await;
        let record = build_message_record(
            "non-existent-conv".to_string(),
            "user".to_string(),
            "hi".to_string(),
            "online".to_string(),
        )
        .unwrap();
        let result = insert_message_with_conn(&mut conn, &record).await;
        assert!(
            result.is_err(),
            "insert_message must FK-reject unknown conversation_id"
        );
    }

    #[tokio::test]
    async fn insert_then_list_returns_inserted_message() {
        let (_dir, mut conn) = fresh_db().await;
        let inserted = insert_user_msg(&mut conn, "conv-1", "hello").await;

        let list = list_messages_by_conversation_with_conn(&mut conn, "conv-1", None)
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, inserted.id);
        assert_eq!(list[0].conversation_id, "conv-1");
        assert_eq!(list[0].role, "user");
        assert_eq!(list[0].content, "hello");
        assert_eq!(list[0].mode, "online");
    }

    #[tokio::test]
    async fn list_orders_by_created_at_ascending() {
        // 同一 conversation 多条消息,list 必须升序 — ChatPanel 渲染依赖此排序
        let (_dir, mut conn) = fresh_db().await;
        let m1 = insert_user_msg(&mut conn, "conv-2", "first").await;
        // ULID 单调递增(时间序),sleep 1ms 避免同毫秒 ULID 接近
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let m2 = insert_user_msg(&mut conn, "conv-2", "second").await;
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let m3 = insert_user_msg(&mut conn, "conv-2", "third").await;

        let list = list_messages_by_conversation_with_conn(&mut conn, "conv-2", None)
            .await
            .unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].id, m1.id);
        assert_eq!(list[1].id, m2.id);
        assert_eq!(list[2].id, m3.id);
        assert_eq!(list[0].content, "first");
        assert_eq!(list[2].content, "third");
    }

    #[tokio::test]
    async fn list_filters_by_conversation_id() {
        // 多 conversation 隔离 — conv-A 不能看到 conv-B 的消息
        let (_dir, mut conn) = fresh_db().await;
        insert_user_msg(&mut conn, "conv-A", "a-msg").await;
        insert_user_msg(&mut conn, "conv-B", "b-msg-1").await;
        insert_user_msg(&mut conn, "conv-B", "b-msg-2").await;

        let a_list = list_messages_by_conversation_with_conn(&mut conn, "conv-A", None)
            .await
            .unwrap();
        let b_list = list_messages_by_conversation_with_conn(&mut conn, "conv-B", None)
            .await
            .unwrap();
        assert_eq!(a_list.len(), 1);
        assert_eq!(b_list.len(), 2);
        assert!(b_list.iter().all(|r| r.conversation_id == "conv-B"));
    }

    #[tokio::test]
    async fn list_respects_limit() {
        let (_dir, mut conn) = fresh_db().await;
        for i in 0..5 {
            insert_user_msg(&mut conn, "conv-limit", &format!("msg-{i}")).await;
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        let limited = list_messages_by_conversation_with_conn(&mut conn, "conv-limit", Some(3))
            .await
            .unwrap();
        assert_eq!(limited.len(), 3);
    }

    #[tokio::test]
    async fn delete_message_removes_only_target() {
        let (_dir, mut conn) = fresh_db().await;
        let kept = insert_user_msg(&mut conn, "conv-d", "keep").await;
        let removed = insert_user_msg(&mut conn, "conv-d", "remove").await;

        delete_message_with_conn(&mut conn, &removed.id).await.unwrap();
        let list = list_messages_by_conversation_with_conn(&mut conn, "conv-d", None)
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, kept.id);
    }

    #[tokio::test]
    async fn delete_messages_by_conversation_returns_count_and_isolates() {
        let (_dir, mut conn) = fresh_db().await;
        insert_user_msg(&mut conn, "conv-clear", "x").await;
        insert_user_msg(&mut conn, "conv-clear", "y").await;
        insert_user_msg(&mut conn, "conv-keep", "z").await;

        let count = delete_messages_by_conversation_with_conn(&mut conn, "conv-clear")
            .await
            .unwrap();
        assert_eq!(count, 2, "delete returns row count");
        let cleared = list_messages_by_conversation_with_conn(&mut conn, "conv-clear", None)
            .await
            .unwrap();
        assert!(cleared.is_empty());
        let kept = list_messages_by_conversation_with_conn(&mut conn, "conv-keep", None)
            .await
            .unwrap();
        assert_eq!(kept.len(), 1, "other conversation untouched");
    }

    #[tokio::test]
    async fn cleanup_with_conn_drops_old_messages() {
        // 验证 cutoff 策略:历史消息(created_at < cutoff)被删,新消息保留
        let (_dir, mut conn) = fresh_db().await;
        ensure_conversation(&mut conn, "conv-cleanup").await;
        // 手动构造一条"古老"消息(2020 年)
        let old_id = "01ABCDEF1234567890ABCDEFGH";
        sqlx::query(
            "INSERT INTO messages (id, conversation_id, role, content, mode, created_at) \
             VALUES (?, 'conv-cleanup', 'user', 'ancient', 'online', '2020-01-01T00:00:00Z')",
        )
        .bind(old_id)
        .execute(&mut conn)
        .await
        .unwrap();
        // 一条新消息
        insert_user_msg(&mut conn, "conv-cleanup", "fresh").await;

        // 用 90 天 cutoff(now - 90d ≫ 2020-01-01)
        let cutoff = cutoff_for_days(90);
        let removed = cleanup_messages_with_conn(&mut conn, &cutoff).await.unwrap();
        assert_eq!(removed, 1, "only the 2020 message should be cleaned");

        let remaining = list_messages_by_conversation_with_conn(&mut conn, "conv-cleanup", None)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].content, "fresh");
    }
}
