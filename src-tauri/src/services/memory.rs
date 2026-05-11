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
// - DB 连接走 services::db::open_app_db（统一收口 PRAGMA foreign_keys / busy_timeout / WAL）;
//   早先版本自维护 open_conn 跳过 enforce_pragmas，导致 ChatService::history /
//   启动期 GC 路径并发写时立即 SQLITE_BUSY（无 5s 重试）— 2026-05-10 code-review 修复。

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Connection, FromRow, SqliteConnection};
use tauri::{AppHandle, Runtime};
use thiserror::Error;
use ulid::Ulid;

use crate::services::db::{open_app_db, DbError};

const VALID_ROLES: &[&str] = &["user", "assistant", "system"];
// 'cancelled' 用于 ChatService run_stream 收到 LLMError::Cancelled 但已收到 partial 文本的场景：
// UI 仍可见这条半句，但下一轮 prompt 时 ChatService 会过滤掉（与 'offline_rule' 同理由 — A6 修复）。
const VALID_MODES: &[&str] = &["online", "offline_rule", "cancelled"];

const SUMMARY_PLACEHOLDER: &str =
    "[摘要功能将于 M3 引入;此处为占位符,详见 progress/decisions-log.md]";

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

impl From<DbError> for MemoryError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => MemoryError::AppConfigDir(s),
            DbError::Database(s) => MemoryError::Database(s),
        }
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

/// 写入一条消息(ChatService B.2 streaming 完成后调用)。
pub async fn insert_message<R: Runtime>(
    app: &AppHandle<R>,
    conversation_id: String,
    role: String,
    content: String,
    mode: String,
) -> Result<MessageRecord, MemoryError> {
    let record = build_message_record(conversation_id, role, content, mode)?;

    let mut conn = open_app_db(app).await?;
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
    let mut conn = open_app_db(app).await?;
    let records =
        list_messages_by_conversation_with_conn(&mut conn, conversation_id, limit).await?;
    conn.close().await?;
    Ok(records)
}

/// 按消息 ID 删除(用户主动 "删除某条" 用)。
pub async fn delete_message<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<(), MemoryError> {
    let mut conn = open_app_db(app).await?;
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
    let mut conn = open_app_db(app).await?;
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
    let mut conn = open_app_db(app).await?;
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
        Some(n) => {
            // H1 修复（2026-05-09）：原 `ORDER BY created_at ASC LIMIT ?` 在长对话
            // （消息数 > limit）时取的是**最早 N 条**而非"最近 N 条"，导致 ChatService.prepare
            // 拉到几个月前的开场寒暄推给 LLM，看似"上下文窗口太小"实则 LIMIT 方向反了。
            // 修复：内层 DESC LIMIT N 取最近 N 条，外层翻 ASC 保持调用方升序契约不变
            // （prompt.rs:273-274 build_messages 注释依赖此排序）。
            sqlx::query_as::<_, MessageRecord>(
                r#"
            SELECT id, conversation_id, role, content, mode, created_at
            FROM (
                SELECT id, conversation_id, role, content, mode, created_at
                FROM messages
                WHERE conversation_id = ?
                ORDER BY created_at DESC
                LIMIT ?
            ) AS recent
            ORDER BY created_at ASC
            "#,
            )
            .bind(conversation_id)
            .bind(n)
            .fetch_all(conn)
            .await?
        }
        None => {
            sqlx::query_as::<_, MessageRecord>(
                r#"
            SELECT id, conversation_id, role, content, mode, created_at
            FROM messages
            WHERE conversation_id = ?
            ORDER BY created_at ASC
            "#,
            )
            .bind(conversation_id)
            .fetch_all(conn)
            .await?
        }
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

/// 按 message id 更新 content + mode（A4 修复：ChatService 预插入空行后流式末尾改写）。
///
/// content / mode 校验沿用 build_message_record 的 VALID_MODES（'online' | 'offline_rule' | 'cancelled'）；
/// 调用方需保证 mode 合法（service 层硬编码三值，不做 IPC 暴露）。
/// 不存在 id → no-op（rows_affected=0；调用方 ChatService 已守 prepare 期 INSERT 必成功）。
pub(crate) async fn update_message_content_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
    content: &str,
    mode: &str,
) -> Result<(), MemoryError> {
    if !VALID_MODES.contains(&mode) {
        return Err(MemoryError::InvalidMode(mode.to_string(), VALID_MODES));
    }
    sqlx::query("UPDATE messages SET content = ?, mode = ? WHERE id = ?")
        .bind(content)
        .bind(mode)
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

/// #6 启动期 GC：清理上次进程退出时 detached spawn 的 run_stream 还没收尾就被
/// 强杀（托盘"退出" / 系统重启 / 崩溃）留下的孤儿 assistant placeholder。
///
/// 这些行的特征：role='assistant' AND mode='online' AND content=''，在 chat_history
/// 视图里渲染为"空气泡"，UX 体验差。
///
/// `cutoff_rfc3339` 必须由 caller 在启动早期捕获（一般 `Utc::now().to_rfc3339()`）：
/// 加了这个上界后，新进程启动后立刻收到 chat_send 也不会被误删（新 placeholder 的
/// created_at >= 启动时间快照）。
///
/// 失败语义：caller 仅 log 不阻断启动；返删除行数（一般 0-N，N 通常 < 5）。
pub async fn cleanup_orphan_assistant_placeholders<R: Runtime>(
    app: &AppHandle<R>,
    cutoff_rfc3339: &str,
) -> Result<u64, MemoryError> {
    let mut conn = open_app_db(app).await?;
    let count = cleanup_orphan_assistant_placeholders_with_conn(&mut conn, cutoff_rfc3339).await?;
    conn.close().await?;
    Ok(count)
}

pub(crate) async fn cleanup_orphan_assistant_placeholders_with_conn(
    conn: &mut SqliteConnection,
    cutoff_rfc3339: &str,
) -> Result<u64, MemoryError> {
    let result = sqlx::query(
        r#"
        DELETE FROM messages
        WHERE role = 'assistant'
          AND mode = 'online'
          AND content = ''
          AND created_at < ?
        "#,
    )
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
                let r =
                    build_message_record("c".into(), (*role).into(), "x".into(), (*mode).into());
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
        // H1 修复（2026-05-09）：原测试只断 len()=3 通过，但 ASC LIMIT 取的是
        // [msg-0, msg-1, msg-2]（最早 3 条），而 ChatService 期望"最近 3 条"。
        // 修复后 SQL 用子查询 DESC LIMIT N → 外层 ASC，应取 [msg-2, msg-3, msg-4]。
        let (_dir, mut conn) = fresh_db().await;
        for i in 0..5 {
            insert_user_msg(&mut conn, "conv-limit", &format!("msg-{i}")).await;
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        let limited = list_messages_by_conversation_with_conn(&mut conn, "conv-limit", Some(3))
            .await
            .unwrap();
        assert_eq!(limited.len(), 3);
        assert_eq!(
            limited[0].content, "msg-2",
            "limit=3 应取最近 3 条（msg-2/3/4），而非最早 3 条（msg-0/1/2）"
        );
        assert_eq!(limited[1].content, "msg-3");
        assert_eq!(limited[2].content, "msg-4");
    }

    #[tokio::test]
    async fn delete_message_removes_only_target() {
        let (_dir, mut conn) = fresh_db().await;
        let kept = insert_user_msg(&mut conn, "conv-d", "keep").await;
        let removed = insert_user_msg(&mut conn, "conv-d", "remove").await;

        delete_message_with_conn(&mut conn, &removed.id)
            .await
            .unwrap();
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
        let removed = cleanup_messages_with_conn(&mut conn, &cutoff)
            .await
            .unwrap();
        assert_eq!(removed, 1, "only the 2020 message should be cleaned");

        let remaining = list_messages_by_conversation_with_conn(&mut conn, "conv-cleanup", None)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].content, "fresh");
    }

    // ===== #6 启动期 GC：孤儿 assistant placeholder =====

    #[tokio::test]
    async fn cleanup_orphan_placeholders_only_drops_empty_online_assistant() {
        // 只删 role=assistant + mode=online + content='' 的；其他都保留
        let (_dir, mut conn) = fresh_db().await;
        ensure_conversation(&mut conn, "conv-gc").await;

        // 1. 孤儿 placeholder（应被清）
        let orphan = build_message_record(
            "conv-gc".into(),
            "assistant".into(),
            String::new(),
            "online".into(),
        )
        .unwrap();
        insert_message_with_conn(&mut conn, &orphan).await.unwrap();

        // 2. 已完成的 assistant 行（content 非空，应保留）
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let completed = build_message_record(
            "conv-gc".into(),
            "assistant".into(),
            "你好呀".into(),
            "online".into(),
        )
        .unwrap();
        insert_message_with_conn(&mut conn, &completed)
            .await
            .unwrap();

        // 3. offline_rule 拒答行（mode=offline_rule，应保留——降级路径写的是非空 refusal，
        //    但即便 content='' 也不该按"未完成的 placeholder"处理）
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let offline = build_message_record(
            "conv-gc".into(),
            "assistant".into(),
            "暂时没法陪你聊".into(),
            "offline_rule".into(),
        )
        .unwrap();
        insert_message_with_conn(&mut conn, &offline).await.unwrap();

        // 4. user 空消息（理论不可能但防御性，应保留）
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let user_empty = build_message_record(
            "conv-gc".into(),
            "user".into(),
            String::new(),
            "online".into(),
        )
        .unwrap();
        insert_message_with_conn(&mut conn, &user_empty)
            .await
            .unwrap();

        // 用足够远的 future cutoff 让所有现有行都 < cutoff
        let future_cutoff = (Utc::now() + chrono::Duration::seconds(60)).to_rfc3339();
        let removed = cleanup_orphan_assistant_placeholders_with_conn(&mut conn, &future_cutoff)
            .await
            .unwrap();
        assert_eq!(removed, 1, "只删 1 个空 online assistant placeholder");

        let remaining = list_messages_by_conversation_with_conn(&mut conn, "conv-gc", None)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 3);
        assert!(!remaining.iter().any(|r| r.id == orphan.id));
        assert!(remaining.iter().any(|r| r.id == completed.id));
        assert!(remaining.iter().any(|r| r.id == offline.id));
        assert!(remaining.iter().any(|r| r.id == user_empty.id));
    }

    #[tokio::test]
    async fn cleanup_orphan_placeholders_respects_cutoff_filter() {
        // cutoff 之后的 placeholder 不被误删（保护新进程刚 INSERT 的 placeholder）
        let (_dir, mut conn) = fresh_db().await;
        ensure_conversation(&mut conn, "conv-cutoff").await;

        // 旧的孤儿（启动前）
        let old_orphan_id = "01OLDPLACEHOLDER0000000000";
        sqlx::query(
            "INSERT INTO messages (id, conversation_id, role, content, mode, created_at) \
             VALUES (?, 'conv-cutoff', 'assistant', '', 'online', '2024-01-01T00:00:00Z')",
        )
        .bind(old_orphan_id)
        .execute(&mut conn)
        .await
        .unwrap();

        // 新生成的（启动后）
        let new_placeholder = build_message_record(
            "conv-cutoff".into(),
            "assistant".into(),
            String::new(),
            "online".into(),
        )
        .unwrap();
        insert_message_with_conn(&mut conn, &new_placeholder)
            .await
            .unwrap();

        // cutoff = 2025 年（介于两条之间）
        let cutoff = "2025-01-01T00:00:00Z";
        let removed = cleanup_orphan_assistant_placeholders_with_conn(&mut conn, cutoff)
            .await
            .unwrap();
        assert_eq!(removed, 1, "只删 cutoff 之前的");

        let remaining = list_messages_by_conversation_with_conn(&mut conn, "conv-cutoff", None)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, new_placeholder.id);
    }

    #[tokio::test]
    async fn cleanup_orphan_placeholders_returns_zero_on_clean_db() {
        let (_dir, mut conn) = fresh_db().await;
        let cutoff = (Utc::now() + chrono::Duration::seconds(60)).to_rfc3339();
        let removed = cleanup_orphan_assistant_placeholders_with_conn(&mut conn, &cutoff)
            .await
            .unwrap();
        assert_eq!(removed, 0, "干净 DB 上 GC 应是 no-op，返 0");
    }
}
