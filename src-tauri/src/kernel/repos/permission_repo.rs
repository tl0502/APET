// PermissionRepo — kernel-only, owner of `context_access_log` (Spec §11.6).
// 仅 PermissionService 实现可 instantiate; subsystem 拿不到此 repo。
//
// Phase A0: DenyOnlyPermissionService 永远写 granted=0 记录。

use chrono::Utc;
use sqlx::SqliteConnection;

use super::RepoError;

pub struct PermissionRepo {}

impl PermissionRepo {
    pub fn new() -> Self {
        Self {}
    }

    /// 写一条 deny 记录 (granted=0)。Phase A0 DenyOnly 实现唯一路径。
    pub async fn append_denied(
        &self,
        conn: &mut SqliteConnection,
        scope: &str,
        actor: &str,
        used_for: &str,
        surface_id: Option<&str>,
    ) -> Result<(), RepoError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO context_access_log
             (scope, granted, actor, used_for, surface_id, retention_policy, created_at)
             VALUES (?, 0, ?, ?, ?, 'transient', ?)",
        )
        .bind(scope)
        .bind(actor)
        .bind(used_for)
        .bind(surface_id)
        .bind(&now)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// audit 查询 (设置面板 / debug 用)
    pub async fn list_recent(
        &self,
        conn: &mut SqliteConnection,
        limit: i64,
    ) -> Result<Vec<(String, i64, String, String, String)>, RepoError> {
        let rows = sqlx::query_as::<_, (String, i64, String, String, String)>(
            "SELECT scope, granted, actor, used_for, created_at
             FROM context_access_log ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows)
    }
}

impl Default for PermissionRepo {
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
            "CREATE TABLE context_access_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                scope TEXT NOT NULL,
                granted INTEGER NOT NULL,
                actor TEXT NOT NULL,
                used_for TEXT NOT NULL,
                surface_id TEXT,
                retention_policy TEXT NOT NULL DEFAULT 'transient',
                created_at TEXT NOT NULL,
                permission_granted_at TEXT,
                context_captured_at TEXT
            )",
        )
        .execute(&mut conn)
        .await
        .unwrap();
        conn
    }

    #[tokio::test]
    async fn append_denied_writes_granted_zero() {
        let mut conn = setup_test_db().await;
        let repo = PermissionRepo::new();
        repo.append_denied(
            &mut conn,
            "foreground_app_name",
            "InitiativeSub",
            "proactive_eval",
            None,
        )
        .await
        .unwrap();
        let (scope, granted): (String, i64) =
            sqlx::query_as("SELECT scope, granted FROM context_access_log LIMIT 1")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(scope, "foreground_app_name");
        assert_eq!(granted, 0);
    }
}
