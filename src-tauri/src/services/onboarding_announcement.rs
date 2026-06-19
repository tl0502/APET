// OnboardingAnnouncement — onboarding 完成时的 system 转场注入（#21 M1 收尾）。
//
// 角色：与 nickname_announcement 对偶。用户走完 Step 1-6 后，往 active conversation
// 写一条 role='system' 的转场消息，让 LLM 在首次回复时知道"刚刚发生了什么 +
// 用户已准备好开始对话"——避免冷启 LLM 突然问"你需要什么帮助"破坏首聊体验。
//
// 触发点：commands/window.rs::onboarding_complete IPC（用户在 SummonInviteView 点
// "开始陪伴"时调用）。
//
// 与 nickname_announcement 的区别：
// - 无开关（onboarding 是一次性事件，没必要让用户关闭"首聊 system 通知"）
// - active conversation 此时可能尚未存在（用户从未点 chat shortcut）→ ensure 自动创建
// - 文案分两版：全新 conv（首次完成 onboarding，常态）→ "first_chat"；非空 conv
//   （NeedReconsent 升级走完整 onboarding，复用旧 conv）→ "reconsent"，避免对老用户说
//   "这是你们的第一次正式对话"造成上下文错位
//
// 幂等：进入主流程前 SELECT 一次,若 conv 内已有"系统通知...首次设置"开头的 system 行,
// 直接 Ok(()) skip。防"onboarding_complete 重试"导致重复插入（emit 失败 → 前端误回到
// summon-invite 重试 → 后端再次调 inject）。
//
// 失败语义：错误向上抛 OnboardingAnnouncementError；调用方 onboarding_complete IPC
// 仅 eprintln 不阻断切窗——转场是辅助行为，掉这一条 system 消息只是"首聊问候稍微平淡"，
// 不应阻塞 onboarding 完成路径。

use chrono::Utc;
use sqlx::Connection;
use tauri::{AppHandle, Runtime};
use thiserror::Error;

use crate::services::chat::conversation::ensure_active_conversation_with_snapshot_with_conn;
use crate::services::db::{open_app_db, DbError};
use crate::services::memory::{build_message_record, insert_message_with_conn, MemoryError};
use crate::services::nickname::get_user_nickname_with_conn;
use crate::services::persona::load_active_persona_with_conn;

const SYSTEM_ROLE: &str = "system";
const ONLINE_MODE: &str = "online";
/// active persona 查询失败时的 fallback（与 seed_builtin 一致）。
const DEFAULT_PERSONA: &str = "momo";
/// 幂等探针：注入消息必以此前缀开头,搜索同前缀的 system 行 = 已注入过。
/// 与 compose_message 同步——改文案前缀时此常量也要改。
const ANNOUNCEMENT_PREFIX: &str = "「系统通知」用户";
const ANNOUNCEMENT_MARKER: &str = "首次设置";

#[derive(Debug, Error)]
pub enum OnboardingAnnouncementError {
    #[error("memory error: {0}")]
    Memory(#[from] MemoryError),
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir error: {0}")]
    AppConfigDir(String),
    #[error("ensure active conversation failed: {0}")]
    EnsureConversation(String),
}

impl From<sqlx::Error> for OnboardingAnnouncementError {
    fn from(e: sqlx::Error) -> Self {
        Self::Database(e.to_string())
    }
}

impl From<DbError> for OnboardingAnnouncementError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => Self::AppConfigDir(s),
            DbError::Database(s) => Self::Database(s),
        }
    }
}

/// 注入"首次完成 onboarding"的 system 转场消息（已注入则 skip 保证幂等）。
///
/// 内部步骤：
/// 1. 取 active persona id（失败 → 兜底 "momo"，不让转场失败）
/// 2. ensure_active_conversation（KV 不存在 → 自动建新 conversation）
/// 3. 单次 SELECT 查 conv 状态：已注入过 → skip；msg_count==0 → first_chat 文案；
///    msg_count>0 → reconsent 文案（NeedReconsent 升级路径复用旧 conv 的情况）
/// 4. 取用户昵称（M1 onboarding 不收 nickname，预计 None）
/// 5. 插 messages role=system + 刷 conversations.last_activity_at
pub async fn inject_onboarding_complete<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(), OnboardingAnnouncementError> {
    let mut conn = open_app_db(app).await?;

    let (persona_id, persona_snapshot_id) = match load_active_persona_with_conn(&mut conn).await {
        Ok(p) => {
            let snapshot_id = p.snapshot_id.parse::<i64>().ok();
            (p.id, snapshot_id)
        }
        Err(e) => {
            eprintln!(
                "[onboarding_announcement] load active persona failed, fallback to '{DEFAULT_PERSONA}': {e}"
            );
            (DEFAULT_PERSONA.to_string(), None)
        }
    };

    let conv_id = ensure_active_conversation_with_snapshot_with_conn(
        &mut conn,
        &persona_id,
        persona_snapshot_id,
    )
    .await
    .map_err(|e| OnboardingAnnouncementError::EnsureConversation(e.to_string()))?;

    // 单次 SQL 拿幂等标志 + 消息总数;LIKE pattern 用 ANNOUNCEMENT_PREFIX/MARKER 双锚点
    // 防误命中用户自己写的"系统通知"开头消息（user role 不算；SUM 仅计 system role）。
    let like_pattern = format!("{ANNOUNCEMENT_PREFIX}%{ANNOUNCEMENT_MARKER}%");
    let (has_inject, msg_count): (i64, i64) = sqlx::query_as(
        r#"
        SELECT
          COALESCE(SUM(CASE WHEN role = ? AND content LIKE ? THEN 1 ELSE 0 END), 0) AS has_inject,
          COUNT(*) AS msg_count
        FROM messages
        WHERE conversation_id = ?
        "#,
    )
    .bind(SYSTEM_ROLE)
    .bind(&like_pattern)
    .bind(&conv_id)
    .fetch_one(&mut conn)
    .await?;

    if has_inject > 0 {
        // 幂等:同 conv 已有同款 announcement,跳过本次注入,也不刷 last_activity_at
        // （已注入路径意味着 onboarding_complete 重试,主流程的 hide/show/gate 都已完成）。
        conn.close().await?;
        return Ok(());
    }

    let nickname = get_user_nickname_with_conn(&mut conn).await.ok().flatten();

    let content = if msg_count == 0 {
        compose_first_chat(nickname.as_deref())
    } else {
        // 非空 conv 走 reconsent 文案 — NeedReconsent 升级路径下,用户已有大量历史,
        // "第一次正式对话"会让 LLM 上下文错位。
        compose_reconsent(nickname.as_deref())
    };

    let record = build_message_record(
        conv_id.clone(),
        SYSTEM_ROLE.to_string(),
        content,
        ONLINE_MODE.to_string(),
    )?;
    insert_message_with_conn(&mut conn, &record).await?;

    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE conversations SET last_activity_at = ? WHERE id = ? AND archived = 0")
        .bind(&now)
        .bind(&conv_id)
        .execute(&mut conn)
        .await?;

    conn.close().await?;
    Ok(())
}

fn nickname_clause(nickname: Option<&str>) -> String {
    match nickname {
        Some(n) => {
            let trimmed = n.trim();
            if trimmed.is_empty() {
                String::new()
            } else {
                format!("「{trimmed}」")
            }
        }
        None => String::new(),
    }
}

/// 首次完成 onboarding（conv 全新）。
/// 注:`ANNOUNCEMENT_PREFIX` + `ANNOUNCEMENT_MARKER` 必须出现在文案中保证幂等探针生效。
fn compose_first_chat(nickname: Option<&str>) -> String {
    let name_clause = nickname_clause(nickname);
    format!(
        "{ANNOUNCEMENT_PREFIX}{name_clause}刚刚完成了首次设置（灵魂宣誓 → 选择人格 → 确认快捷键 → 提醒偏好）。这是你们的第一次正式对话——请以你的人格自然问候 TA，让 TA 感受到陪伴的开始。"
    )
}

/// NeedReconsent 升级走完整 onboarding（conv 非空,有历史消息）。
/// 同样含 `ANNOUNCEMENT_PREFIX` + `ANNOUNCEMENT_MARKER` 让幂等探针生效。
fn compose_reconsent(nickname: Option<&str>) -> String {
    let name_clause = nickname_clause(nickname);
    format!(
        "{ANNOUNCEMENT_PREFIX}{name_clause}刚刚因数据策略更新而重新完成了首次设置流程（灵魂宣誓 → 选择人格 → 确认快捷键 → 提醒偏好）。请以你的人格自然衔接,不必把这当作初次相见,也不需要逐条复述设置内容。"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::chat::conversation::ensure_active_conversation_with_conn;
    use crate::services::test_db::fresh_db;

    #[test]
    fn first_chat_message_has_system_prefix_and_marker() {
        let msg = compose_first_chat(None);
        assert!(msg.starts_with(ANNOUNCEMENT_PREFIX));
        assert!(msg.contains(ANNOUNCEMENT_MARKER));
        assert!(msg.contains("第一次正式对话"));
        assert!(!msg.contains("「」"));
    }

    #[test]
    fn first_chat_message_with_nickname_includes_name() {
        let msg = compose_first_chat(Some("阿吉"));
        assert!(msg.contains("「阿吉」"));
        assert!(msg.contains(ANNOUNCEMENT_MARKER));
    }

    #[test]
    fn first_chat_message_with_whitespace_nickname_falls_back_to_anonymous() {
        let msg = compose_first_chat(Some("   "));
        assert!(!msg.contains("「   」"));
        assert!(msg.contains(&format!("{ANNOUNCEMENT_PREFIX}刚刚")));
    }

    #[test]
    fn reconsent_message_has_system_prefix_and_marker() {
        let msg = compose_reconsent(None);
        assert!(msg.starts_with(ANNOUNCEMENT_PREFIX));
        assert!(msg.contains(ANNOUNCEMENT_MARKER));
        // 关键差异:不说"第一次"
        assert!(!msg.contains("第一次正式对话"));
        // 关键提示:reconsent 上下文
        assert!(msg.contains("数据策略更新") || msg.contains("不必把这当作初次相见"));
    }

    #[tokio::test]
    async fn inject_message_appears_in_active_conversation() {
        let (_dir, mut conn) = fresh_db().await;
        let conv_id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();

        let content = compose_first_chat(None);
        let record = build_message_record(
            conv_id.clone(),
            SYSTEM_ROLE.to_string(),
            content.clone(),
            ONLINE_MODE.to_string(),
        )
        .unwrap();
        insert_message_with_conn(&mut conn, &record).await.unwrap();

        let row: (String, String) = sqlx::query_as(
            "SELECT role, content FROM messages WHERE conversation_id = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&conv_id)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(row.0, SYSTEM_ROLE);
        assert!(row.1.starts_with(ANNOUNCEMENT_PREFIX));
    }

    /// 幂等探针守护:同 conv 已有 announcement → 二次 SELECT 能命中,主流程会 skip。
    #[tokio::test]
    async fn announcement_query_detects_existing_inject() {
        let (_dir, mut conn) = fresh_db().await;
        let conv_id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();

        // 插一条 announcement
        let record = build_message_record(
            conv_id.clone(),
            SYSTEM_ROLE.to_string(),
            compose_first_chat(None),
            ONLINE_MODE.to_string(),
        )
        .unwrap();
        insert_message_with_conn(&mut conn, &record).await.unwrap();

        let like_pattern = format!("{ANNOUNCEMENT_PREFIX}%{ANNOUNCEMENT_MARKER}%");
        let (has_inject, msg_count): (i64, i64) = sqlx::query_as(
            r#"
            SELECT
              COALESCE(SUM(CASE WHEN role = ? AND content LIKE ? THEN 1 ELSE 0 END), 0),
              COUNT(*)
            FROM messages
            WHERE conversation_id = ?
            "#,
        )
        .bind(SYSTEM_ROLE)
        .bind(&like_pattern)
        .bind(&conv_id)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(has_inject, 1, "已注入的 announcement 必须能被探针命中");
        assert_eq!(msg_count, 1);
    }

    /// 防误命中守护:user role 的"系统通知开头"消息不应被 SUM 计入（仅 system role 计）。
    #[tokio::test]
    async fn announcement_query_ignores_user_role_with_same_prefix() {
        let (_dir, mut conn) = fresh_db().await;
        let conv_id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();

        // 用户角色发了一条"看起来像 announcement"的消息（极端情况）
        let user_fake = build_message_record(
            conv_id.clone(),
            "user".to_string(),
            compose_first_chat(None),
            ONLINE_MODE.to_string(),
        )
        .unwrap();
        insert_message_with_conn(&mut conn, &user_fake)
            .await
            .unwrap();

        let like_pattern = format!("{ANNOUNCEMENT_PREFIX}%{ANNOUNCEMENT_MARKER}%");
        let (has_inject, msg_count): (i64, i64) = sqlx::query_as(
            r#"
            SELECT
              COALESCE(SUM(CASE WHEN role = ? AND content LIKE ? THEN 1 ELSE 0 END), 0),
              COUNT(*)
            FROM messages
            WHERE conversation_id = ?
            "#,
        )
        .bind(SYSTEM_ROLE)
        .bind(&like_pattern)
        .bind(&conv_id)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(has_inject, 0, "user role 的相同前缀消息不应被探针命中");
        assert_eq!(msg_count, 1);
    }
}
