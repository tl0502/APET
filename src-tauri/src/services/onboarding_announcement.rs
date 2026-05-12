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
// - 文案聚焦"刚完成首设 + 第一次正式对话 + 请自然问候"，不告诉具体步骤细节
//
// 失败语义：错误向上抛 OnboardingAnnouncementError；调用方 onboarding_complete IPC
// 仅 eprintln 不阻断切窗——转场是辅助行为，掉这一条 system 消息只是"首聊问候稍微平淡"，
// 不应阻塞 onboarding 完成路径。

use chrono::Utc;
use sqlx::Connection;
use tauri::{AppHandle, Runtime};
use thiserror::Error;

use crate::services::chat::conversation::ensure_active_conversation_with_conn;
use crate::services::db::{open_app_db, DbError};
use crate::services::memory::{build_message_record, insert_message_with_conn, MemoryError};
use crate::services::nickname::get_user_nickname_with_conn;
use crate::services::persona::load_active_persona_with_conn;

const SYSTEM_ROLE: &str = "system";
const ONLINE_MODE: &str = "online";
/// active persona 查询失败时的 fallback（与 seed_builtin 一致）。
const DEFAULT_PERSONA: &str = "momo";

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

/// 注入"首次完成 onboarding"的 system 转场消息。
///
/// 内部步骤：
/// 1. 取 active persona id（失败 → 兜底 "momo"，不让转场失败）
/// 2. ensure_active_conversation（KV 不存在 → 自动建新 conversation）
/// 3. 取用户昵称（M1 onboarding 不收 nickname，预计 None；前置 nickname:user 设过的用户除外）
/// 4. 拼文案（有昵称 → 带昵称版本）
/// 5. 插 messages role=system + 刷 conversations.last_activity_at
pub async fn inject_onboarding_complete<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(), OnboardingAnnouncementError> {
    let mut conn = open_app_db(app).await?;

    // 1. active persona → 取 id；失败兜底默认 momo（onboarding Step 2 走完应该已 activate
    //    某个 persona，但 db 故障 / 用户跳过 Step 2 路径理论上不存在；兜底是廉价 fail-safe）
    let persona_id = match load_active_persona_with_conn(&mut conn).await {
        Ok(p) => p.id,
        Err(e) => {
            eprintln!(
                "[onboarding_announcement] load active persona failed, fallback to '{DEFAULT_PERSONA}': {e}"
            );
            DEFAULT_PERSONA.to_string()
        }
    };

    // 2. ensure_active_conversation：M1 onboarding 完成 = 用户首次进入主态，conversations
    //    表里大概率没行，会建新行 + 写 active KV
    let conv_id = ensure_active_conversation_with_conn(&mut conn, &persona_id)
        .await
        .map_err(|e| OnboardingAnnouncementError::EnsureConversation(e.to_string()))?;

    // 3. 昵称（M1 onboarding 不收，预计 None）；查询失败视为无昵称，不阻断主路径
    let nickname = get_user_nickname_with_conn(&mut conn)
        .await
        .ok()
        .flatten();

    // 4. 拼文案
    let content = compose_message(nickname.as_deref());

    // 5. 插 messages + 刷 last_activity_at（与 nickname_announcement 同款 archived=0 守护）
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

/// 文案拼装；有昵称走带名版本（更自然），无昵称走通用版本。
///
/// 注：以「系统通知」前缀开头与 nickname_announcement 对齐——LLM 在 chat history 中
/// 识别 system role 配合开头标记，能稳定区分"用户消息"与"系统事件"。
fn compose_message(nickname: Option<&str>) -> String {
    let name_clause = match nickname {
        Some(n) => {
            let trimmed = n.trim();
            if trimmed.is_empty() {
                String::new()
            } else {
                format!("「{trimmed}」")
            }
        }
        None => String::new(),
    };
    format!(
        "「系统通知」用户{name_clause}刚刚完成了首次设置（灵魂宣誓 → 选择人格 → 确认快捷键 → 提醒偏好）。这是你们的第一次正式对话——请以你的人格自然问候 TA，让 TA 感受到陪伴的开始。"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::test_db::fresh_db;

    #[test]
    fn message_without_nickname_has_system_prefix() {
        let msg = compose_message(None);
        assert!(msg.starts_with("「系统通知」"));
        assert!(msg.contains("首次设置"));
        // 无昵称版本不应出现「」夹空内容
        assert!(!msg.contains("「」"));
    }

    #[test]
    fn message_with_nickname_includes_name() {
        let msg = compose_message(Some("阿吉"));
        assert!(msg.contains("「阿吉」"));
        assert!(msg.contains("首次设置"));
    }

    #[test]
    fn message_with_whitespace_nickname_falls_back_to_anonymous() {
        // 空白昵称走匿名分支，避免拼成 "用户「   」刚刚..."
        let msg = compose_message(Some("   "));
        assert!(!msg.contains("「   」"));
        assert!(msg.contains("用户刚刚完成"));
    }

    #[tokio::test]
    async fn inject_message_appears_in_active_conversation() {
        // 集成测试：在 fresh DB 上模拟 onboarding 完成 → 应在 active conv 留 system 行。
        // 直接调 with_conn-级别 helpers 走通：ensure_active_conversation_with_conn 自动建新行。
        let (_dir, mut conn) = fresh_db().await;
        let conv_id = ensure_active_conversation_with_conn(&mut conn, "momo")
            .await
            .unwrap();

        let content = compose_message(None);
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
        assert!(row.1.starts_with("「系统通知」"));
    }
}
