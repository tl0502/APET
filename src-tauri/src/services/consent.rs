// M.1 ConsentService — 隐私同意记录持久化（issue #16，ADR-008 灵魂宣誓 v1.0）
//
// 范围（M1 后端管道；前端视图层留 #16b 与 #17 Onboarding 状态机一起做）：
// - get_consent(): 读单行 consent（id=1）当前状态；granted=false 时 accepted_at 归一
//   为 None（防前端误读 seed 占位时间为"用户同意时间"）
// - grant_consent(method): 写 granted=true + method + version=CURRENT_CONSENT_VERSION
//   + accepted_at=now（chrono RFC3339）。method 仅接受 'soul_pledge'（ADR-008 唯一同意
//   路径，防 dev console 调 grant('classic') 绕过宣誓页）；version 由 service 层用
//   常量写入，前端通过 commands/consent_get_current_version 拿到值仅用作"双方一致校验"
// - check_version(): 比对 stored_version 与 CURRENT_CONSENT_VERSION，返回
//   Match | NeedReconsent | NotGranted；stored >= current 都视为 Match（含降级场景），
//   仅 stored < current 才弹宣誓页"我重新确认"
//
// Schema（migrations/001_init.sql 行 31-37）单行 CHECK(id=1)：
//   granted INTEGER, method TEXT, version INTEGER, accepted_at TEXT
//   001 末尾已 seed (1, 0, 'classic', 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'))；
//   首启 granted=0 → check_version 返 NotGranted，#17 路由进 onboarding。
//
// 偏离 issue body：
// - body 写 pledge_resource / policy_resource 字段；schema 没保留这两列（27 表零迁移 D5
//   原则）。改为：method = 'soul_pledge' 即隐含 pledge resource v1；如需多资源版本追溯
//   靠 version + 文件哈希（M3 实际有 v2 时再加列）。

use chrono::Utc;
use serde::Serialize;
use sqlx::{Connection, SqliteConnection};
use tauri::{AppHandle, Runtime};
use thiserror::Error;

use crate::services::db::{open_app_db, DbError};

/// 当前 consent 数据策略版本号（与 assets/legal/data_policy_v1.md 对齐；
/// data_policy_v2 上线时本常量 + 1，启动期 check_version 自动触发重新确认）。
pub const CURRENT_CONSENT_VERSION: i64 = 1;

/// 灵魂宣誓 method 枚举值（写入 consent.method 列）。
///
/// 注：service 层 `grant_consent` 顶层 API **只接受 soul_pledge**（防 dev console
/// 误调 grant('classic') 绕过宣誓页）。'classic' 仅作为 schema seed 默认值存在于
/// migrations/001_init.sql，service 层无需引用为常量；测试断言 method == "classic"
/// 直接用字面量与 schema 字符串对照（schema 即权威源）。
pub const METHOD_SOUL_PLEDGE: &str = "soul_pledge";

#[derive(Debug, Error)]
pub enum ConsentError {
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
    #[error("invalid method: '{0}' (expected 'soul_pledge')")]
    InvalidMethod(String),
}

impl From<sqlx::Error> for ConsentError {
    fn from(e: sqlx::Error) -> Self {
        ConsentError::Database(e.to_string())
    }
}

impl From<DbError> for ConsentError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => ConsentError::AppConfigDir(s),
            DbError::Database(s) => ConsentError::Database(s),
        }
    }
}

/// consent 行 IPC 出参。
///
/// `accepted_at` 仅当 `granted=true` 时有值（用户真同意时间）；首启 seed 行 granted=0
/// 时 DB 列虽有 strftime 写入的占位值，但语义上不代表用户操作 → service 层归一为 None
/// 防前端误读。
#[derive(Debug, Clone, Serialize)]
pub struct ConsentRecord {
    pub granted: bool,
    pub method: String,
    pub version: i64,
    pub accepted_at: Option<String>,
}

/// 启动期路由 / IPC `consent_check_version` 的判定结果。
///
/// `Match` = 已 granted 且 stored_version >= CURRENT_CONSENT_VERSION → 直接进主态
///   （含 stored > current 的降级场景：用户已同意更新版本，不应被迫再次同意旧版本）。
/// `NeedReconsent` = 已 granted 但 stored_version < CURRENT_CONSENT_VERSION → 弹宣誓页。
/// `NotGranted` = 从未同意 → 走完整 Onboarding Step 1。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsentStatus {
    Match,
    NeedReconsent {
        stored_version: i64,
        current_version: i64,
    },
    NotGranted,
}

/// service 顶层 grant API 唯一合法 method（ADR-008 M1 唯一同意路径）。
fn validate_grant_method(method: &str) -> Result<(), ConsentError> {
    if method == METHOD_SOUL_PLEDGE {
        Ok(())
    } else {
        Err(ConsentError::InvalidMethod(method.to_string()))
    }
}

/// 读 consent 单行；schema 已 seed (id=1)，正常情况永不为 None。
pub async fn get_consent<R: Runtime>(app: &AppHandle<R>) -> Result<ConsentRecord, ConsentError> {
    let mut conn = open_app_db(app).await?;
    let record = get_consent_with_conn(&mut conn).await?;
    conn.close().await?;
    Ok(record)
}

pub(crate) async fn get_consent_with_conn(
    conn: &mut SqliteConnection,
) -> Result<ConsentRecord, ConsentError> {
    let row: (i64, String, i64, String) =
        sqlx::query_as("SELECT granted, method, version, accepted_at FROM consent WHERE id = 1")
            .fetch_one(conn)
            .await?;
    let granted = row.0 != 0;
    Ok(ConsentRecord {
        granted,
        method: row.1,
        version: row.2,
        accepted_at: if granted { Some(row.3) } else { None },
    })
}

/// 用户点"我懂了"路径：写 granted=1 + method='soul_pledge' + version=CURRENT + accepted_at=now。
///
/// 顶层 API 锁死 method=soul_pledge 与 version=CURRENT_CONSENT_VERSION（防 dev console
/// 调 invoke('consent_grant', {method:'classic'}) 绕过 ADR-008 灵魂宣誓路径，
/// 也防前端硬编码 version 在 v2 上线时 stale 写入）。
///
/// 走 UPDATE 而非 UPSERT — 001 seed 已确保 id=1 行存在，UPDATE 影响行数 0 表示
/// schema 损坏（被外部清表），返 Database 错让上层感知。
pub async fn grant_consent<R: Runtime>(
    app: &AppHandle<R>,
    method: &str,
) -> Result<(), ConsentError> {
    validate_grant_method(method)?;
    let now = Utc::now().to_rfc3339();
    let mut conn = open_app_db(app).await?;
    grant_consent_with_conn(&mut conn, method, CURRENT_CONSENT_VERSION, &now).await?;
    conn.close().await?;
    Ok(())
}

pub(crate) async fn grant_consent_with_conn(
    conn: &mut SqliteConnection,
    method: &str,
    version: i64,
    now_rfc3339: &str,
) -> Result<(), ConsentError> {
    let result = sqlx::query(
        r#"
        UPDATE consent
        SET granted = 1,
            method = ?,
            version = ?,
            accepted_at = ?
        WHERE id = 1
        "#,
    )
    .bind(method)
    .bind(version)
    .bind(now_rfc3339)
    .execute(conn)
    .await?;
    if result.rows_affected() == 0 {
        return Err(ConsentError::Database(
            "consent row missing (id=1); schema seed lost".to_string(),
        ));
    }
    Ok(())
}

/// 与 CURRENT_CONSENT_VERSION 比对，返回路由判定。
///
/// #17 Onboarding 状态机调：Match → 跳过宣誓页直接到 Step 2；NeedReconsent → 弹
/// "我重新确认" 文案；NotGranted → 走 Step 1 完整版。
pub async fn check_version<R: Runtime>(app: &AppHandle<R>) -> Result<ConsentStatus, ConsentError> {
    let record = get_consent(app).await?;
    Ok(check_status(&record, CURRENT_CONSENT_VERSION))
}

pub(crate) fn check_status(record: &ConsentRecord, current_version: i64) -> ConsentStatus {
    if !record.granted {
        return ConsentStatus::NotGranted;
    }
    // stored >= current 都视为 Match：
    // - stored == current：常态，直接进主态
    // - stored > current（降级场景）：用户已同意更新版本的条款，旧 client 不该让用户
    //   "重新同意一个更旧的版本"再写回 stored=current（这会让用户后续升回新 client 时
    //   被迫再次同意）。当前 client 用旧条款是 dev 责任，不该惩罚用户。
    if record.version >= current_version {
        ConsentStatus::Match
    } else {
        ConsentStatus::NeedReconsent {
            stored_version: record.version,
            current_version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::test_db::fresh_db;

    /// strftime('%Y-%m-%dT%H:%M:%fZ','now') 输出形如 "2026-05-10T14:25:30.123Z"。
    /// 兼容 chrono::Utc::now().to_rfc3339() 的 "+00:00" / "Z" 两种 UTC 表示。
    fn is_rfc3339_like(s: &str) -> bool {
        // 简单判别：含 'T' 分隔 + 以 'Z' 或 '+' 收尾
        s.contains('T') && (s.ends_with('Z') || s.contains('+'))
    }

    #[tokio::test]
    async fn fresh_db_consent_starts_not_granted_with_classic_method() {
        // 001 seed: (1, 0, 'classic', 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        let (_dir, mut conn) = fresh_db().await;
        let r = get_consent_with_conn(&mut conn).await.unwrap();
        assert!(!r.granted, "fresh DB must have granted=0");
        assert_eq!(r.method, "classic", "seed default method is 'classic'");
        assert_eq!(r.version, 1, "seed version is 1");
        assert!(
            r.accepted_at.is_none(),
            "granted=false → accepted_at 归一为 None（防前端误读 seed 占位值）"
        );
    }

    #[tokio::test]
    async fn fresh_db_seed_accepted_at_is_rfc3339_format() {
        // 守护 H1 修复：001 seed 用 strftime 而非 datetime('now')。
        // 即使被 get_consent 归零为 None，DB 列本身也应是 RFC3339（防其他直读路径失序）。
        let (_dir, mut conn) = fresh_db().await;
        let row: (String,) = sqlx::query_as("SELECT accepted_at FROM consent WHERE id = 1")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert!(
            is_rfc3339_like(&row.0),
            "seed accepted_at 必须是 RFC3339 格式，实际 = {:?}",
            row.0
        );
    }

    #[tokio::test]
    async fn grant_with_soul_pledge_marks_granted_and_records_method() {
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        grant_consent_with_conn(&mut conn, METHOD_SOUL_PLEDGE, 1, &now)
            .await
            .unwrap();

        let r = get_consent_with_conn(&mut conn).await.unwrap();
        assert!(r.granted);
        assert_eq!(r.method, "soul_pledge");
        assert_eq!(r.version, 1);
        assert_eq!(r.accepted_at.as_deref(), Some(now.as_str()));
    }

    #[tokio::test]
    async fn grant_twice_overwrites_method_and_version() {
        // 用户 v1 同意后，data policy 升 v2 → "我重新确认" 应覆盖 method/version/accepted_at
        let (_dir, mut conn) = fresh_db().await;
        let t1 = "2026-05-08T10:00:00+00:00";
        let t2 = "2026-06-01T10:00:00+00:00";
        grant_consent_with_conn(&mut conn, METHOD_SOUL_PLEDGE, 1, t1)
            .await
            .unwrap();
        grant_consent_with_conn(&mut conn, METHOD_SOUL_PLEDGE, 2, t2)
            .await
            .unwrap();

        let r = get_consent_with_conn(&mut conn).await.unwrap();
        assert_eq!(r.version, 2);
        assert_eq!(r.accepted_at.as_deref(), Some(t2));
    }

    #[test]
    fn check_status_returns_not_granted_when_granted_false() {
        let r = ConsentRecord {
            granted: false,
            method: "classic".to_string(),
            version: 1,
            accepted_at: None,
        };
        assert_eq!(check_status(&r, 1), ConsentStatus::NotGranted);
    }

    #[test]
    fn check_status_returns_match_when_versions_equal() {
        let r = ConsentRecord {
            granted: true,
            method: "soul_pledge".to_string(),
            version: 1,
            accepted_at: Some("x".to_string()),
        };
        assert_eq!(check_status(&r, 1), ConsentStatus::Match);
    }

    #[test]
    fn check_status_returns_need_reconsent_when_stored_is_older() {
        // 升级场景：stored=1, current=2 → 需要让用户重新同意 v2 条款
        let r = ConsentRecord {
            granted: true,
            method: "soul_pledge".to_string(),
            version: 1,
            accepted_at: Some("x".to_string()),
        };
        let status = check_status(&r, 2);
        assert_eq!(
            status,
            ConsentStatus::NeedReconsent {
                stored_version: 1,
                current_version: 2,
            }
        );
    }

    #[test]
    fn check_status_returns_match_when_stored_is_newer_than_current() {
        // 降级场景（H3 修复守护）：stored=2, current=1 → 用户已同意更新条款，视为 Match
        // 不让旧 client 强制用户"重新同意旧版本"再写回 stored=1
        let r = ConsentRecord {
            granted: true,
            method: "soul_pledge".to_string(),
            version: 2,
            accepted_at: Some("x".to_string()),
        };
        assert_eq!(
            check_status(&r, 1),
            ConsentStatus::Match,
            "stored > current 时不应让用户被迫降级同意"
        );
    }

    #[test]
    fn validate_grant_method_accepts_soul_pledge_only() {
        // H2 守护：顶层 grant 仅允许 soul_pledge；'classic' 拒绝（schema seed 互操作走 _with_conn 私 API）
        assert!(validate_grant_method("soul_pledge").is_ok());
        assert!(matches!(
            validate_grant_method("classic"),
            Err(ConsentError::InvalidMethod(_))
        ));
        assert!(matches!(
            validate_grant_method("magic"),
            Err(ConsentError::InvalidMethod(_))
        ));
    }

    #[test]
    fn current_consent_version_is_one_for_v1_data_policy() {
        // 与 assets/legal/data_policy_v1.md 绑定；改 data policy → 改本常量
        assert_eq!(CURRENT_CONSENT_VERSION, 1);
    }

    #[tokio::test]
    async fn grant_consent_with_conn_reports_schema_loss_on_zero_rows() {
        // 防御路径：若 consent(id=1) 行被外部清表，UPDATE 影响 0 行 → Database 错
        let (_dir, mut conn) = fresh_db().await;
        sqlx::query("DELETE FROM consent WHERE id = 1")
            .execute(&mut conn)
            .await
            .unwrap();
        let now = Utc::now().to_rfc3339();
        let r = grant_consent_with_conn(&mut conn, METHOD_SOUL_PLEDGE, 1, &now).await;
        match r {
            Err(ConsentError::Database(msg)) => {
                assert!(msg.contains("consent row missing"), "msg = {msg}")
            }
            other => panic!("expected Database error, got {other:?}"),
        }
    }
}
