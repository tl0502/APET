// F.2 NicknameService — 桌宠/用户昵称管理(单行 nicknames 表 facade)
//
// 范围(plan F.2):
// - get_pet / get_user / set_pet / set_user / restore_pet 5 个 service 方法
// - emit `nickname:changed` event(架构 §711 payload: { which: 'pet'|'user', value })
//
// Schema(migrations/001_init.sql 行 68-74)是单行表(id=1 CHECK):
//   pet_nickname TEXT (nullable, NULL 时 fallback 到 active persona 的 name)
//   pet_nickname_previous TEXT (set_pet 时自动存上次值,restore_pet 用)
//   user_nickname TEXT (nullable, 无 fallback,调用方决定 UI 文案)
//   updated_at TEXT NOT NULL
//
// 设计决定:
// - **不引入 memory KV 表**(架构 §249 那是另一面);F.2 直接用 nicknames 表,精确且少一层间接
// - **restore_pet 用原子 swap**(current ↔ previous):用户可来回切看哪个名字更喜欢,UX 最直观
// - get_pet fallback 顺序:nicknames.pet_nickname → active persona.name → "默默" 兜底字符串
// - DB 连接同 persona/memory 模式:自开 sqlx 短期连接

use chrono::Utc;
use serde::Serialize;
use sqlx::{Connection, SqliteConnection};
use tauri::{AppHandle, Emitter, Runtime};
use thiserror::Error;

use crate::services::db::{open_app_db, DbError};

const NICKNAME_CHANGED_EVENT: &str = "nickname:changed";
const FALLBACK_PET_NAME: &str = "默默";

#[derive(Debug, Error)]
pub enum NicknameError {
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
    #[error("event emit failed: {0}")]
    EventEmit(String),
    #[error("nothing to restore (pet_nickname_previous is NULL)")]
    NothingToRestore,
}

impl From<sqlx::Error> for NicknameError {
    fn from(e: sqlx::Error) -> Self {
        NicknameError::Database(e.to_string())
    }
}

impl From<DbError> for NicknameError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => NicknameError::AppConfigDir(s),
            DbError::Database(s) => NicknameError::Database(s),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct NicknameChangedPayload {
    /// 'pet' 或 'user' — 与架构 §711 IPC event 契约一致
    pub which: String,
    /// 新值;None 表示置空(将来 fallback 到默认)
    pub value: Option<String>,
}

async fn open_conn<R: Runtime>(app: &AppHandle<R>) -> Result<SqliteConnection, NicknameError> {
    Ok(open_app_db(app).await?)
}

fn emit_changed<R: Runtime>(
    app: &AppHandle<R>,
    which: &str,
    value: Option<String>,
) -> Result<(), NicknameError> {
    let payload = NicknameChangedPayload {
        which: which.to_string(),
        value,
    };
    app.emit(NICKNAME_CHANGED_EVENT, payload)
        .map_err(|e| NicknameError::EventEmit(e.to_string()))
}

/// 读 pet_nickname。NULL 时 fallback 到当前 active persona 的 name,再无则返回兜底 "默默"。
pub async fn get_pet_nickname<R: Runtime>(app: &AppHandle<R>) -> Result<String, NicknameError> {
    let mut conn = open_conn(app).await?;
    let result = get_pet_nickname_with_conn(&mut conn).await?;
    conn.close().await?;
    Ok(result)
}

/// 读 user_nickname。无 fallback;NULL 时返回 None,调用方决定 UI 文案。
pub async fn get_user_nickname<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<String>, NicknameError> {
    let mut conn = open_conn(app).await?;
    let row = get_user_nickname_with_conn(&mut conn).await?;
    conn.close().await?;
    Ok(row)
}

/// 设置 pet_nickname。自动把当前值搬到 pet_nickname_previous(为后续 restore 备份)。
/// 触发 `nickname:changed` { which: "pet", value: Some(name) }。
///
/// 实现走 explicit 两步(SELECT 当前 user_nickname → INSERT/UPDATE 显式 bind),
/// 避免老实现的子查询路径在 nicknames 行被外部 DELETE 后丢 user_nickname:
/// `INSERT ... VALUES(..., (SELECT user_nickname FROM nicknames WHERE id = 1), ...)`
/// 在 INSERT 分支(行不存在)时子查询返回 NULL,user_nickname 永久丢失。
/// 详 progress/code-review-2026-05-03.md M-3。
pub async fn set_pet_nickname<R: Runtime>(
    app: &AppHandle<R>,
    name: String,
) -> Result<(), NicknameError> {
    let now = Utc::now().to_rfc3339();
    let mut conn = open_conn(app).await?;
    set_pet_nickname_with_conn(&mut conn, &name, &now).await?;
    conn.close().await?;

    emit_changed(app, "pet", Some(name))?;
    Ok(())
}

/// 设置 user_nickname。
/// 触发 `nickname:changed` { which: "user", value: Some(name) }。
pub async fn set_user_nickname<R: Runtime>(
    app: &AppHandle<R>,
    name: String,
) -> Result<(), NicknameError> {
    let now = Utc::now().to_rfc3339();
    let mut conn = open_conn(app).await?;
    set_user_nickname_with_conn(&mut conn, &name, &now).await?;
    conn.close().await?;

    emit_changed(app, "user", Some(name))?;
    Ok(())
}

/// pet_nickname 与 pet_nickname_previous 原子 swap(用户可来回切两个曾用名)。
/// 若 previous 为 NULL,返回 NothingToRestore 错误,调用方提示 UI。
/// 触发 `nickname:changed` { which: "pet", value: <swap 后的 current> }。
pub async fn restore_pet_nickname<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<String>, NicknameError> {
    let now = Utc::now().to_rfc3339();
    let mut conn = open_conn(app).await?;
    let new_current = restore_pet_nickname_with_conn(&mut conn, &now).await?;
    conn.close().await?;

    emit_changed(app, "pet", new_current.clone())?;
    Ok(new_current)
}

// ============================================================================
// Inner helpers(不依赖 AppHandle / 不发事件)
//
// 抽出动机(2026-05-04 test-coverage):见 secrets.rs 同段注释。
// 外层 `<R: Runtime>` 函数 = open_conn → inner → close_conn → emit_changed,
// 行为完全等价。inner 不发事件以保持纯 SQL,事件留给 prod 路径。
// ============================================================================

pub(crate) async fn get_pet_nickname_with_conn(
    conn: &mut SqliteConnection,
) -> Result<String, NicknameError> {
    let stored: Option<(Option<String>,)> =
        sqlx::query_as("SELECT pet_nickname FROM nicknames WHERE id = 1")
            .fetch_optional(&mut *conn)
            .await?;
    if let Some((Some(name),)) = stored {
        return Ok(name);
    }
    let persona: Option<(String,)> =
        sqlx::query_as("SELECT name FROM personas WHERE is_active = 1 LIMIT 1")
            .fetch_optional(&mut *conn)
            .await?;
    Ok(persona
        .map(|(n,)| n)
        .unwrap_or_else(|| FALLBACK_PET_NAME.to_string()))
}

pub(crate) async fn get_user_nickname_with_conn(
    conn: &mut SqliteConnection,
) -> Result<Option<String>, NicknameError> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT user_nickname FROM nicknames WHERE id = 1")
            .fetch_optional(conn)
            .await?;
    Ok(row.and_then(|(v,)| v))
}

pub(crate) async fn set_pet_nickname_with_conn(
    conn: &mut SqliteConnection,
    name: &str,
    now_rfc3339: &str,
) -> Result<(), NicknameError> {
    // Step 1:读当前 user_nickname(行不存在则为 None,后续 INSERT 写 NULL — 与 schema 默认一致)
    let existing_user: Option<(Option<String>,)> =
        sqlx::query_as("SELECT user_nickname FROM nicknames WHERE id = 1")
            .fetch_optional(&mut *conn)
            .await?;
    let user_nickname = existing_user.and_then(|(v,)| v);

    // Step 2:UPSERT 显式 bind 旧 user_nickname,INSERT 分支也不丢失
    sqlx::query(
        r#"
        INSERT INTO nicknames (id, pet_nickname, pet_nickname_previous, user_nickname, updated_at)
        VALUES (1, ?, NULL, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            pet_nickname_previous = nicknames.pet_nickname,
            pet_nickname = excluded.pet_nickname,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(name)
    .bind(&user_nickname)
    .bind(now_rfc3339)
    .execute(conn)
    .await?;
    Ok(())
}

pub(crate) async fn set_user_nickname_with_conn(
    conn: &mut SqliteConnection,
    name: &str,
    now_rfc3339: &str,
) -> Result<(), NicknameError> {
    sqlx::query(
        r#"
        INSERT INTO nicknames (id, user_nickname, updated_at)
        VALUES (1, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            user_nickname = excluded.user_nickname,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(name)
    .bind(now_rfc3339)
    .execute(conn)
    .await?;
    Ok(())
}

/// 返回 swap 后的新 current(即原 previous)。previous 为 NULL 时返回 NothingToRestore。
pub(crate) async fn restore_pet_nickname_with_conn(
    conn: &mut SqliteConnection,
    now_rfc3339: &str,
) -> Result<Option<String>, NicknameError> {
    let row: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT pet_nickname, pet_nickname_previous FROM nicknames WHERE id = 1")
            .fetch_optional(&mut *conn)
            .await?;

    let (_current, previous) = match row {
        Some(t) => t,
        None => return Err(NicknameError::NothingToRestore),
    };

    if previous.is_none() {
        return Err(NicknameError::NothingToRestore);
    }

    // SQLite UPDATE 同行 swap:右值取的是行更新前的值,所以 swap 是原子的
    sqlx::query(
        r#"
        UPDATE nicknames
        SET
            pet_nickname = pet_nickname_previous,
            pet_nickname_previous = pet_nickname,
            updated_at = ?
        WHERE id = 1
        "#,
    )
    .bind(now_rfc3339)
    .execute(conn)
    .await?;

    // swap 后 current 等于原 previous(肯定 Some,上面已守卫)
    Ok(previous)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn payload_serializes_to_event_contract() {
        let p = NicknameChangedPayload {
            which: "pet".to_string(),
            value: Some("小默".to_string()),
        };
        let json: Value = serde_json::to_value(&p).expect("payload must serialize");
        // 架构 §711 契约:{ which: 'pet'|'user', value }
        assert_eq!(json["which"], "pet");
        assert_eq!(json["value"], "小默");
    }

    #[test]
    fn payload_serializes_null_value() {
        let p = NicknameChangedPayload {
            which: "user".to_string(),
            value: None,
        };
        let json: Value = serde_json::to_value(&p).expect("payload must serialize");
        assert_eq!(json["which"], "user");
        assert!(json["value"].is_null());
    }

    #[test]
    fn fallback_pet_name_is_momo_default() {
        // 出厂兜底必须与 ADR-009 + persona-design v1.0 §5.1 momo 一致
        assert_eq!(FALLBACK_PET_NAME, "默默");
    }

    #[test]
    fn event_name_matches_arch_contract() {
        // 架构 §711 IPC event 表第 22 行:'nickname:changed'
        // 注:Tauri 2.x event name 仅允许 [a-zA-Z0-9\-/:_],不允许 `.`
        // 历史 `nickname.changed` 命名运行时报 "event emit failed",2026-05-04 全量改 `:`
        assert_eq!(NICKNAME_CHANGED_EVENT, "nickname:changed");
    }

    // ===== DB 集成测试(2026-05-04 test-coverage P0)=====

    use crate::services::test_db::fresh_db;

    #[tokio::test]
    async fn fresh_db_has_singleton_row_with_null_nicknames() {
        // 001 末尾 INSERT 了 nicknames(id=1) — pet_nickname / previous / user_nickname 三列默认 NULL
        let (_dir, mut conn) = fresh_db().await;
        let user = get_user_nickname_with_conn(&mut conn).await.unwrap();
        assert!(user.is_none(), "user_nickname starts NULL");
        let pet = get_pet_nickname_with_conn(&mut conn).await.unwrap();
        assert_eq!(
            pet, FALLBACK_PET_NAME,
            "pet_nickname NULL + no active persona must fallback to 默默"
        );
    }

    #[tokio::test]
    async fn set_pet_then_get_pet_returns_set_value() {
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        set_pet_nickname_with_conn(&mut conn, "小默", &now)
            .await
            .unwrap();
        let got = get_pet_nickname_with_conn(&mut conn).await.unwrap();
        assert_eq!(got, "小默");
    }

    #[tokio::test]
    async fn set_pet_preserves_existing_user_nickname_in_upsert_branch() {
        // 防御 M-3 bug:set_pet 必须保留 user_nickname,不能在 INSERT 分支丢失
        // 模拟"行被 DELETE 后 user_nickname 走子查询丢失"的反例 — 我们的实现走 explicit 两步,应该 OK
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();

        // 先设 user
        set_user_nickname_with_conn(&mut conn, "Alice", &now)
            .await
            .unwrap();
        // 再 set_pet — UPSERT 走 UPDATE 分支(行已存在),user 应保留
        set_pet_nickname_with_conn(&mut conn, "默默", &now)
            .await
            .unwrap();
        let user = get_user_nickname_with_conn(&mut conn).await.unwrap();
        assert_eq!(
            user.as_deref(),
            Some("Alice"),
            "set_pet must not clobber user_nickname"
        );
    }

    #[tokio::test]
    async fn set_pet_twice_then_restore_swaps() {
        // 核心 UX:set "小默" → set "momo" → restore swap 回 "小默" → 再 restore 回 "momo"
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();

        set_pet_nickname_with_conn(&mut conn, "小默", &now)
            .await
            .unwrap();
        set_pet_nickname_with_conn(&mut conn, "momo", &now)
            .await
            .unwrap();
        // 第二次 set 后:current = "momo", previous = "小默"
        assert_eq!(get_pet_nickname_with_conn(&mut conn).await.unwrap(), "momo");

        // restore #1:swap → current = "小默", previous = "momo"
        let restored = restore_pet_nickname_with_conn(&mut conn, &now)
            .await
            .unwrap();
        assert_eq!(restored.as_deref(), Some("小默"));
        assert_eq!(get_pet_nickname_with_conn(&mut conn).await.unwrap(), "小默");

        // restore #2:再 swap → current = "momo", previous = "小默"
        let restored2 = restore_pet_nickname_with_conn(&mut conn, &now)
            .await
            .unwrap();
        assert_eq!(restored2.as_deref(), Some("momo"));
        assert_eq!(get_pet_nickname_with_conn(&mut conn).await.unwrap(), "momo");
    }

    #[tokio::test]
    async fn restore_when_previous_null_returns_nothing_to_restore() {
        // fresh DB:既没 set 过 pet,也没 previous,restore 必须返回 NothingToRestore
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        let result = restore_pet_nickname_with_conn(&mut conn, &now).await;
        assert!(matches!(result, Err(NicknameError::NothingToRestore)));
    }

    #[tokio::test]
    async fn get_pet_falls_back_to_active_persona_name() {
        // pet_nickname NULL → fallback 到 personas WHERE is_active = 1 的 name
        let (_dir, mut conn) = fresh_db().await;
        // 手动插入 active persona
        sqlx::query(
            "INSERT INTO personas (id, name, version, source, file_path, is_active, created_at, updated_at) \
             VALUES ('momo', '默默-from-persona', '1.0.0', 'builtin', '<bundled>:x', 1, '2026-05-04', '2026-05-04')",
        )
        .execute(&mut conn)
        .await
        .unwrap();

        let pet = get_pet_nickname_with_conn(&mut conn).await.unwrap();
        assert_eq!(
            pet, "默默-from-persona",
            "should fallback to persona name, not constant 默默"
        );
    }
}
