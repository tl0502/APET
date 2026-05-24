// ConversationRepo — owner of `conversations` + `messages` tables (Constitution #2).
// Phase A0 仅落 messages 表的 safety_scan_status 写入接口; conversations 全套接口 Phase A1 扩。
//
// Spec: §8.1 Repository Pattern; §6.6 SafetyGuard FSM 写 messages.safety_scan_status

use sqlx::SqliteConnection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("not found: {0}")]
    NotFound(String),
}

/// messages.safety_scan_status 7 状态枚举 (Spec §6.6)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyScanStatus {
    Pending,
    Streaming,
    StreamSoftBlocked,
    FinalOk,
    FinalRedacted,
    FinalBlocked,
    ScanFailed,
}

impl SafetyScanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Streaming => "streaming",
            Self::StreamSoftBlocked => "stream_soft_blocked",
            Self::FinalOk => "final_ok",
            Self::FinalRedacted => "final_redacted",
            Self::FinalBlocked => "final_blocked",
            Self::ScanFailed => "scan_failed",
        }
    }
}

pub struct ConversationRepo {
    // Phase A0: 不持 Pool, 每次操作时 caller 提供 SqliteConnection
    // (复用现有 services::db::open_app_db 路径, 渐进迁移)
    // Phase A1 加 Arc<SqlitePool> + Repository 内部 acquire
}

impl ConversationRepo {
    pub fn new() -> Self {
        Self {}
    }

    /// Phase A0 唯一需求: SafetyGuard FSM 转移 messages.safety_scan_status
    pub async fn update_safety_status(
        &self,
        conn: &mut SqliteConnection,
        message_id: &str,
        status: SafetyScanStatus,
    ) -> Result<(), RepoError> {
        let res = sqlx::query("UPDATE messages SET safety_scan_status = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(message_id)
            .execute(&mut *conn)
            .await?;
        if res.rows_affected() == 0 {
            return Err(RepoError::NotFound(format!("message {}", message_id)));
        }
        Ok(())
    }

    /// Phase A0 SafetyGuard final_redacted / final_blocked 时回填 content
    pub async fn update_message_content_and_status(
        &self,
        conn: &mut SqliteConnection,
        message_id: &str,
        new_content: &str,
        status: SafetyScanStatus,
    ) -> Result<(), RepoError> {
        let res = sqlx::query(
            "UPDATE messages SET content = ?, safety_scan_status = ? WHERE id = ?",
        )
        .bind(new_content)
        .bind(status.as_str())
        .bind(message_id)
        .execute(&mut *conn)
        .await?;
        if res.rows_affected() == 0 {
            return Err(RepoError::NotFound(format!("message {}", message_id)));
        }
        Ok(())
    }
}

impl Default for ConversationRepo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::ConnectOptions;

    async fn setup_test_db() -> SqliteConnection {
        let mut conn = SqliteConnectOptions::new()
            .in_memory(true)
            .connect()
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                token_count INTEGER,
                safety_scan_status TEXT NOT NULL DEFAULT 'pending'
            )",
        )
        .execute(&mut conn)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, conversation_id, role, content, created_at, safety_scan_status)
             VALUES ('msg_1', 'conv_1', 'assistant', 'hello', '2026-05-24T00:00:00Z', 'pending')",
        )
        .execute(&mut conn)
        .await
        .unwrap();
        conn
    }

    #[tokio::test]
    async fn update_safety_status_transitions_pending_to_streaming() {
        let mut conn = setup_test_db().await;
        let repo = ConversationRepo::new();
        repo.update_safety_status(&mut conn, "msg_1", SafetyScanStatus::Streaming)
            .await
            .unwrap();
        let status: String =
            sqlx::query_scalar("SELECT safety_scan_status FROM messages WHERE id = 'msg_1'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(status, "streaming");
    }

    #[tokio::test]
    async fn update_safety_status_all_7_states_serialize_correctly() {
        let mut conn = setup_test_db().await;
        let repo = ConversationRepo::new();
        for s in [
            SafetyScanStatus::Pending,
            SafetyScanStatus::Streaming,
            SafetyScanStatus::StreamSoftBlocked,
            SafetyScanStatus::FinalOk,
            SafetyScanStatus::FinalRedacted,
            SafetyScanStatus::FinalBlocked,
            SafetyScanStatus::ScanFailed,
        ] {
            repo.update_safety_status(&mut conn, "msg_1", s)
                .await
                .unwrap();
            let stored: String =
                sqlx::query_scalar("SELECT safety_scan_status FROM messages WHERE id = 'msg_1'")
                    .fetch_one(&mut conn)
                    .await
                    .unwrap();
            assert_eq!(stored, s.as_str());
        }
    }

    #[tokio::test]
    async fn update_safety_status_returns_not_found_for_missing_message() {
        let mut conn = setup_test_db().await;
        let repo = ConversationRepo::new();
        let result = repo
            .update_safety_status(&mut conn, "ghost", SafetyScanStatus::FinalOk)
            .await;
        assert!(matches!(result, Err(RepoError::NotFound(_))));
    }

    #[tokio::test]
    async fn update_message_content_and_status_replaces_redacted_text() {
        let mut conn = setup_test_db().await;
        let repo = ConversationRepo::new();
        repo.update_message_content_and_status(
            &mut conn,
            "msg_1",
            "*** redacted ***",
            SafetyScanStatus::FinalRedacted,
        )
        .await
        .unwrap();
        let row: (String, String) = sqlx::query_as(
            "SELECT content, safety_scan_status FROM messages WHERE id = 'msg_1'",
        )
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(row.0, "*** redacted ***");
        assert_eq!(row.1, "final_redacted");
    }
}
