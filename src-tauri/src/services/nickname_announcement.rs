// NicknameAnnouncement — user_nickname 变更后的 system 转场注入（M1 W2，2026-05-09）
//
// 角色：解决"昵称切换污染对话"问题。当用户改昵称时，往 active conversation 的 messages
// 表插入一条 role='system' 的转场消息，让 LLM 在下一轮看见 + 重置自我称呼。
//
// 研究参考：
// - Vaibhav Kumar "System Prompt Inconsistency"
// - Persona Drift, arxiv 2402.10962
// - stunspot Persona Prompting 指南
// 共识：仅靠开头 system prompt 在长 history 下会被稀释，history 中段插 system 通知能直接重置。
//
// 流程（maybe_inject_user_change）：
// 1. 同名（old == Some(new)）→ 跳过
// 2. 读 config['nickname:user_change_announce']（缺省 'true'）→ 'false' 跳过
// 3. 读 config['chat:active_conversation_id'] → 不存在跳过
// 4. 验 conversations 行还在（孤儿 KV → 跳过）
// 5. 拼文案：首次（old=None）vs 改名（old=Some(prev)）
// 6. 插一条 role='system'、mode='online' 的 record（VALID_ROLES 已含 'system'）
//
// 失败语义：所有错误用 AnnouncementError 上抛；调用方（nickname::set_user_nickname）
// 仅 eprintln，不阻断 setter 主成功路径——注入是辅助行为，setter 不能因日志失败回滚。

use chrono::Utc;
use sqlx::Connection;
use tauri::{AppHandle, Runtime};
use thiserror::Error;

use crate::services::chat::conversation::CONFIG_KEY_ACTIVE_CONVERSATION;
use crate::services::config::{get_with_conn as config_get, ConfigError};
use crate::services::db::{open_app_db, DbError};
use crate::services::memory::{build_message_record, insert_message_with_conn, MemoryError};

const ANNOUNCE_USER_CHANGE_KEY: &str = "nickname:user_change_announce";
const SYSTEM_ROLE: &str = "system";
const ONLINE_MODE: &str = "online";

/// 单一事实源 — config 表里"昵称变更时通知 AI"开关的 key。
/// commands/nickname.rs 必须 use 本常量，避免双源字面量分裂。
pub const CONFIG_KEY_ANNOUNCE_USER_CHANGE: &str = ANNOUNCE_USER_CHANGE_KEY;

#[derive(Debug, Error)]
pub enum AnnouncementError {
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    #[error("memory error: {0}")]
    Memory(#[from] MemoryError),
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir error: {0}")]
    AppConfigDir(String),
}

impl From<sqlx::Error> for AnnouncementError {
    fn from(e: sqlx::Error) -> Self {
        AnnouncementError::Database(e.to_string())
    }
}

impl From<DbError> for AnnouncementError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => AnnouncementError::AppConfigDir(s),
            DbError::Database(s) => AnnouncementError::Database(s),
        }
    }
}

/// 当 user_nickname 从 old → new 时，可能向 active conversation 注入 system 转场消息。
///
/// 注入条件全部满足才生效：开关 ON + active conv 存在 + 行未孤儿 + new != old。
/// 任一不满足均"静默跳过"（返 Ok(())），不报错。
pub async fn maybe_inject_user_change<R: Runtime>(
    app: &AppHandle<R>,
    old: Option<&str>,
    new: &str,
) -> Result<(), AnnouncementError> {
    if old == Some(new) {
        return Ok(());
    }
    let mut conn = open_app_db(app).await?;
    maybe_inject_user_change_with_conn(&mut conn, old, new).await?;
    conn.close().await?;
    Ok(())
}

/// 内部 with_conn 变体；返 `true` = 实际注入了一行，`false` = 任一前置条件未满足跳过。
/// 集成测试用此变体直接对 fresh_db conn 验证副作用（避免拉起 AppHandle）。
pub(crate) async fn maybe_inject_user_change_with_conn(
    conn: &mut sqlx::SqliteConnection,
    old: Option<&str>,
    new: &str,
) -> Result<bool, AnnouncementError> {
    if old == Some(new) {
        return Ok(false);
    }

    // 1. 开关：缺省视为 true（默认 ON）
    if !read_announce_user_change_with_conn(conn).await? {
        return Ok(false);
    }

    // 2. active conversation id
    let conv_id = match config_get(conn, CONFIG_KEY_ACTIVE_CONVERSATION).await? {
        Some(id) => id,
        None => return Ok(false),
    };

    // 3. 验 conversations 行存在且未归档（孤儿 KV / 已归档 → 跳过；不调
    //    ensure_active_conversation 自愈，避免昵称变更顺带建空会话的副作用）。
    //    archived = 0 守护：跨窗口归档场景下 active KV 可能短暂指向归档行（如另一窗口
    //    已 archive 但本窗口的 KV 缓存未刷），若不守 archived 会向归档会话注入 system
    //    消息 + 刷归档行 last_activity_at —— 与 conversation.rs 三处守护理念对齐
    //    （lessons.md §11）。
    let exists: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM conversations WHERE id = ? AND archived = 0",
    )
    .bind(&conv_id)
    .fetch_optional(&mut *conn)
    .await?;
    if exists.is_none() {
        return Ok(false);
    }

    // 4. 拼文案
    let content = compose_user_change_message(old, new);

    // 5. 插 messages 表
    let record = build_message_record(
        conv_id.clone(),
        SYSTEM_ROLE.to_string(),
        content,
        ONLINE_MODE.to_string(),
    )?;
    insert_message_with_conn(conn, &record).await?;

    // 6. 同步 conversations.last_activity_at — 让侧边栏排序前移，
    //    避免"改了昵称、注入了 system 行、但 sidebar 不动"的视觉断层。
    //    AND archived = 0：与 conversation.rs::update_last_activity_with_conn 一致；
    //    上方 SELECT 已守 archived，但归档可能在此期间发生（极小窗口），重复守一道是廉价兜底。
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE conversations SET last_activity_at = ? WHERE id = ? AND archived = 0")
        .bind(&now)
        .bind(&conv_id)
        .execute(&mut *conn)
        .await?;

    Ok(true)
}

/// 拼"系统通知"文案。首次设置 vs 改名走两个分支，让 LLM 拿到足够上下文区分场景。
pub(crate) fn compose_user_change_message(old: Option<&str>, new: &str) -> String {
    match old {
        None => format!(
            "「系统通知」从此刻起，用户希望你称呼TA「{new}」。请在后续回复中使用新称呼；不要混用。"
        ),
        Some(prev) => format!(
            "「系统通知」用户希望你之后称呼TA「{new}」（之前是「{prev}」）。请在后续回复中使用新称呼；不要混用旧称。"
        ),
    }
}

/// B7：读"昵称变更时通知 AI"开关；缺省视为 true（默认 ON）。
///
/// commands/nickname.rs::nickname_get_announce_user_change 直接调本函数，
/// 把"字符串 KV 当布尔的转换规则"统一在一处。将来加第三态（如 'ask'）只改这里。
pub async fn read_announce_user_change<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<bool, AnnouncementError> {
    let mut conn = open_app_db(app).await?;
    let value = read_announce_user_change_with_conn(&mut conn).await?;
    conn.close().await?;
    Ok(value)
}

/// B7：读"昵称变更时通知 AI"开关；缺省视为 true（默认 ON）。
///
/// 字符串 KV 当布尔的转换规则这里就一份；将来加第三态（如 'ask'）只改这里。
///
/// 2026-05-10：从"只把 'false' 当 OFF，其他全 ON"改为正向白名单——
/// 只有 None（缺省）和明确 "true" 视为 ON，其余（"false" / "0" / "no" / 老版本写错的字面）
/// 全视为 OFF。M1 写入路径只产生 "true"/"false"（commands::nickname::nickname_set_announce_user_change），
/// 但语义上"未知值默认开"对隐私性开关偏激进；改为白名单后用户手动改 DB 写错也能 fail-safe。
pub(crate) async fn read_announce_user_change_with_conn(
    conn: &mut sqlx::SqliteConnection,
) -> Result<bool, ConfigError> {
    let raw = config_get(conn, ANNOUNCE_USER_CHANGE_KEY).await?;
    Ok(matches!(raw.as_deref(), None | Some("true")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_first_set_includes_new_name_and_no_previous() {
        let msg = compose_user_change_message(None, "Alice");
        assert!(msg.contains("Alice"));
        assert!(msg.contains("从此刻起"));
        assert!(!msg.contains("之前是"));
    }

    #[test]
    fn compose_rename_includes_both_names_with_previous_marker() {
        let msg = compose_user_change_message(Some("Alice"), "Bob");
        assert!(msg.contains("Bob"));
        assert!(msg.contains("Alice"));
        assert!(msg.contains("之前是"));
        assert!(msg.contains("不要混用旧称"));
    }

    #[test]
    fn compose_message_starts_with_system_marker() {
        // 文案首字母是「系统通知」标记，UI 灰条 + LLM 都依赖这个语义信号
        let msg = compose_user_change_message(None, "X");
        assert!(msg.starts_with("「系统通知」"));
    }

    // ===== A3 集成测试：注入后 conversations.last_activity_at 必须被更新 =====
    use crate::services::chat::conversation::ensure_active_conversation_with_conn;
    use crate::services::config::set_with_conn as config_set;
    use crate::services::test_db::fresh_db;

    #[tokio::test]
    async fn inject_updates_conversation_last_activity_at() {
        let (_dir, mut conn) = fresh_db().await;
        let conv_id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        let before: (String,) =
            sqlx::query_as("SELECT last_activity_at FROM conversations WHERE id = ?")
                .bind(&conv_id)
                .fetch_one(&mut conn)
                .await
                .unwrap();

        // RFC3339 秒级精度，sleep 1.1s 确保时间戳不同
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        let injected = maybe_inject_user_change_with_conn(&mut conn, None, "Alice")
            .await
            .unwrap();
        assert!(injected, "fresh DB + active conv 默认开关 ON → 应注入");

        // messages 表新增了一条 system 行
        let msg_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ? AND role = 'system'",
        )
        .bind(&conv_id)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(msg_count.0, 1);

        // last_activity_at 必须前移
        let after: (String,) =
            sqlx::query_as("SELECT last_activity_at FROM conversations WHERE id = ?")
                .bind(&conv_id)
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert!(
            after.0 > before.0,
            "last_activity_at 应在注入后 > 注入前；before={} after={}",
            before.0,
            after.0
        );
    }

    #[tokio::test]
    async fn switch_off_skips_injection_and_keeps_last_activity() {
        // 开关 OFF → 不注入；conversations.last_activity_at 不动
        let (_dir, mut conn) = fresh_db().await;
        let conv_id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        let now = Utc::now().to_rfc3339();
        config_set(&mut conn, ANNOUNCE_USER_CHANGE_KEY, "false", &now)
            .await
            .unwrap();

        let injected = maybe_inject_user_change_with_conn(&mut conn, None, "Alice")
            .await
            .unwrap();
        assert!(!injected, "开关 OFF 应跳过注入");

        let msg_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ? AND role = 'system'",
        )
        .bind(&conv_id)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(msg_count.0, 0);
    }

    #[tokio::test]
    async fn no_active_conversation_skips_injection() {
        // 没 active conv → 不注入（也不报错）
        let (_dir, mut conn) = fresh_db().await;
        let injected = maybe_inject_user_change_with_conn(&mut conn, None, "Alice")
            .await
            .unwrap();
        assert!(!injected);
    }

    #[tokio::test]
    async fn rename_with_old_value_propagates_both_names() {
        let (_dir, mut conn) = fresh_db().await;
        let conv_id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        let injected = maybe_inject_user_change_with_conn(&mut conn, Some("Alice"), "Bob")
            .await
            .unwrap();
        assert!(injected);
        let row: (String,) = sqlx::query_as(
            "SELECT content FROM messages WHERE conversation_id = ? AND role = 'system'",
        )
        .bind(&conv_id)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert!(row.0.contains("Bob"));
        assert!(row.0.contains("Alice"));
        assert!(row.0.contains("之前是"));
    }

    // ===== P2-7 白名单：未知值 fail-safe 为 OFF =====

    #[tokio::test]
    async fn announce_switch_unknown_value_treated_as_off() {
        // 2026-05-10 P2-7：旧实现"只把 'false' 当 OFF" → "0"/"no"/typo 都被误判为 ON。
        // 新白名单：仅 None / "true" → ON；其他全 OFF。用户手动改 DB 写错也能 fail-safe。
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        for bad in ["0", "no", "off", "TRUE", "True", "", "1", "yes"] {
            config_set(&mut conn, ANNOUNCE_USER_CHANGE_KEY, bad, &now)
                .await
                .unwrap();
            assert!(
                !read_announce_user_change_with_conn(&mut conn).await.unwrap(),
                "raw {:?} 应视为 OFF（白名单 fail-safe）",
                bad
            );
        }
    }

    #[tokio::test]
    async fn announce_switch_canonical_values() {
        let (_dir, mut conn) = fresh_db().await;
        // 缺省（KV 不存在）→ ON（默认 ON 契约）
        assert!(read_announce_user_change_with_conn(&mut conn).await.unwrap());

        let now = Utc::now().to_rfc3339();
        config_set(&mut conn, ANNOUNCE_USER_CHANGE_KEY, "true", &now)
            .await
            .unwrap();
        assert!(read_announce_user_change_with_conn(&mut conn).await.unwrap());

        config_set(&mut conn, ANNOUNCE_USER_CHANGE_KEY, "false", &now)
            .await
            .unwrap();
        assert!(!read_announce_user_change_with_conn(&mut conn).await.unwrap());
    }

    #[tokio::test]
    async fn same_name_short_circuits_no_injection() {
        // 同名（old == Some(new)）→ 任何前置条件都不查，直接返 false（不注入）。
        // 与 nickname::set_user_nickname 顶部短路对齐——两层都要拦，避免依赖单一防御。
        let (_dir, mut conn) = fresh_db().await;
        let conv_id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        let injected = maybe_inject_user_change_with_conn(&mut conn, Some("Alice"), "Alice")
            .await
            .unwrap();
        assert!(!injected);
        let msg_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ? AND role = 'system'",
        )
        .bind(&conv_id)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(msg_count.0, 0);
    }

    // ===== 2026-05-10 code-review Bug 2：archived=0 守护 =====

    #[tokio::test]
    async fn archived_active_conversation_skips_injection() {
        // 跨窗口归档场景：active KV 仍指向已归档行（KV 缓存未刷 / 另一窗口 archive 后未广播）。
        // 不守 archived 会向归档会话注入 system 消息 + 刷归档行 last_activity_at →
        // 用户取消归档后看到一条幽灵 system 行 + 时间戳错位（lessons.md §11 同款病灶）。
        let (_dir, mut conn) = fresh_db().await;
        let conv_id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();
        // 模拟"另一窗口归档但当前窗口的 active KV 缓存未刷"
        sqlx::query("UPDATE conversations SET archived = 1 WHERE id = ?")
            .bind(&conv_id)
            .execute(&mut conn)
            .await
            .unwrap();
        let now = Utc::now().to_rfc3339();
        config_set(&mut conn, CONFIG_KEY_ACTIVE_CONVERSATION, &conv_id, &now)
            .await
            .unwrap();
        let before: (String,) =
            sqlx::query_as("SELECT last_activity_at FROM conversations WHERE id = ?")
                .bind(&conv_id)
                .fetch_one(&mut conn)
                .await
                .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        let injected = maybe_inject_user_change_with_conn(&mut conn, None, "Alice")
            .await
            .unwrap();
        assert!(!injected, "归档会话不应被注入");

        // messages 表无新增 system 行
        let msg_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ? AND role = 'system'",
        )
        .bind(&conv_id)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(msg_count.0, 0, "归档会话不应留下幽灵 system 行");

        // 归档行 last_activity_at 也不应被刷
        let after: (String,) =
            sqlx::query_as("SELECT last_activity_at FROM conversations WHERE id = ?")
                .bind(&conv_id)
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(
            before.0, after.0,
            "归档行 last_activity_at 不应被昵称注入路径刷新"
        );
    }
}
