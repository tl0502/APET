// F.2 NicknameService — 用户昵称管理
//
// 范围（M1，2026-05-09 重构后）:
// - get_user / set_user 两个 service 方法
// - emit `nickname:changed` event（架构 §711 payload: { which: 'user', value }）
// - set_user 成功后调用 nickname_announcement::maybe_inject_user_change 写 system 转场消息
//
// 已删除（2026-05-09）:
// - pet_nickname / pet_nickname_previous / restore_pet 相关 IPC + service 函数
// - 宠物名字源唯一化为 .soul.md 的 persona.name；ChatService 拼 system prompt 时直接用
//   PersonaSummary.name，不再绕 nicknames 表
// - 表 schema 的 pet_nickname / pet_nickname_previous 列冷藏不删（守 27 表零迁移原则）
//
// Schema（migrations/001_init.sql 行 68-74）单行表（id=1 CHECK）:
//   user_nickname TEXT (nullable，无 fallback；调用方决定 UI 文案)
//   updated_at TEXT NOT NULL
//   pet_nickname / pet_nickname_previous 列保留但不再写入

use chrono::Utc;
use serde::Serialize;
use sqlx::{Connection, SqliteConnection};
use tauri::{AppHandle, Emitter, Runtime};
use thiserror::Error;

use crate::services::db::{open_app_db, DbError};
use crate::services::nickname_announcement;

const NICKNAME_CHANGED_EVENT: &str = "nickname:changed";

#[derive(Debug, Error)]
pub enum NicknameError {
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
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
    /// M1 仅 'user'（pet 改名机制已移除；persona 切换走 'persona:activated' 事件）。
    /// 字段保留以兼容架构 §711 IPC event 契约。
    pub which: String,
    /// 新值。emit 仅发生在 set_user_nickname 写库成功后，必有非空值；
    /// "首次未设置"是 DB NULL 初值，由调用方 get_user_nickname 返 None 表达，与 emit payload 无关。
    /// 2026-05-10：去 Option 死路径（validate_nickname 拒空白，IPC 入口本来就没有清空通道）。
    pub value: String,
}

async fn open_conn<R: Runtime>(app: &AppHandle<R>) -> Result<SqliteConnection, NicknameError> {
    Ok(open_app_db(app).await?)
}

fn emit_changed<R: Runtime>(app: &AppHandle<R>, which: &str, value: String) {
    let payload = NicknameChangedPayload {
        which: which.to_string(),
        value,
    };
    // best-effort：Tauri emit 极少失败；即便失败也不阻断主流程（announcement 注入 / setter 返成功）。
    // 前端 store 若错过此次 event，下次 NicknameForm load 仍能拉到 DB 真值。
    if let Err(e) = app.emit(NICKNAME_CHANGED_EVENT, payload) {
        eprintln!("[nickname] emit {NICKNAME_CHANGED_EVENT} failed: {e}");
    }
}

/// 读 user_nickname。无 fallback；NULL 时返回 None，调用方决定 UI 文案。
pub async fn get_user_nickname<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<String>, NicknameError> {
    let mut conn = open_conn(app).await?;
    let row = get_user_nickname_with_conn(&mut conn).await?;
    conn.close().await?;
    Ok(row)
}

/// 设置 user_nickname。
/// 触发 `nickname:changed` { which: "user", value: name }。
/// 成功后调 nickname_announcement::maybe_inject_user_change 写 system 转场消息进 active
/// conversation（受 config['nickname:user_change_announce'] 开关控制；默认 ON）。
/// 注入失败仅 eprintln，不阻断主成功路径——转场注入是辅助行为。
///
/// **同名短路**：old == Some(new) 时直接返 Ok(())，不写 DB、不 emit、不调 announcement——
/// 避免无意义刷 updated_at + 给多窗口订阅方制造无差异 event。announcement 内部也有同款
/// 短路，但前两步浪费这里就拦住。
///
/// **非原子 trade-off**：set_user_nickname_with_conn 与 maybe_inject_user_change 不在同一 tx
/// （两次 open_app_db + 两次 commit）。中间崩溃（断电 / 强杀）会留下"昵称已变但 conversation
/// 里没转场消息"的状态——LLM 下一轮会延续旧称呼直到再次触发任意 system message 写入。
/// M1 单机崩溃率低 + 注入本身就是辅助行为（announcement 跳过任何步骤都返 Ok），可接受；
/// 若 M3 之后想严格化，把两段合并到一条 conn 内顺序写即可。
pub async fn set_user_nickname<R: Runtime>(
    app: &AppHandle<R>,
    name: String,
) -> Result<(), NicknameError> {
    let mut conn = open_conn(app).await?;
    let old = get_user_nickname_with_conn(&mut conn).await?;

    // 同名短路（P1-2）：与 announcement 内部短路语义对齐。
    if old.as_deref() == Some(name.as_str()) {
        conn.close().await?;
        return Ok(());
    }

    let now = Utc::now().to_rfc3339();
    set_user_nickname_with_conn(&mut conn, &name, &now).await?;
    conn.close().await?;

    // emit 是 best-effort（P1-3）：失败仅 log，不阻断下方 announcement 注入。
    // 即便 emit 失败，前端 store 下次 NicknameForm load 仍能拉到 DB 真值。
    emit_changed(app, "user", name.clone());

    if let Err(e) =
        nickname_announcement::maybe_inject_user_change(app, old.as_deref(), &name).await
    {
        eprintln!("[nickname] inject user-change announcement failed: {e}");
    }
    Ok(())
}

// ============================================================================
// Inner helpers（不依赖 AppHandle / 不发事件）
//
// 抽出动机（2026-05-04 test-coverage）：见 secrets.rs 同段注释。
// 外层 `<R: Runtime>` 函数 = open_conn → inner → close_conn → emit_changed,
// 行为完全等价。inner 不发事件以保持纯 SQL，事件留给 prod 路径。
// ============================================================================

pub(crate) async fn get_user_nickname_with_conn(
    conn: &mut SqliteConnection,
) -> Result<Option<String>, NicknameError> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT user_nickname FROM nicknames WHERE id = 1")
            .fetch_optional(conn)
            .await?;
    Ok(row.and_then(|(v,)| v))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn payload_serializes_to_event_contract() {
        let p = NicknameChangedPayload {
            which: "user".to_string(),
            value: "Alice".to_string(),
        };
        let json: Value = serde_json::to_value(&p).expect("payload must serialize");
        // 架构 §711 契约：{ which: 'user', value }（M1 起 pet 已移除；value 永远非空字符串）
        assert_eq!(json["which"], "user");
        assert_eq!(json["value"], "Alice");
    }

    #[test]
    fn event_name_matches_arch_contract() {
        // 架构 §711 IPC event 表第 22 行：'nickname:changed'
        // 注：Tauri 2.x event name 仅允许 [a-zA-Z0-9\-/:_]，不允许 `.`
        // 历史 `nickname.changed` 命名运行时报 "event emit failed"，2026-05-04 全量改 `:`
        assert_eq!(NICKNAME_CHANGED_EVENT, "nickname:changed");
    }

    // ===== DB 集成测试（2026-05-04 test-coverage P0）=====

    use crate::services::test_db::fresh_db;

    #[tokio::test]
    async fn fresh_db_has_singleton_row_with_null_user_nickname() {
        // 001 末尾 INSERT 了 nicknames(id=1) — user_nickname 默认 NULL
        let (_dir, mut conn) = fresh_db().await;
        let user = get_user_nickname_with_conn(&mut conn).await.unwrap();
        assert!(user.is_none(), "user_nickname starts NULL");
    }

    #[tokio::test]
    async fn set_user_then_get_user_returns_set_value() {
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        set_user_nickname_with_conn(&mut conn, "Alice", &now)
            .await
            .unwrap();
        let got = get_user_nickname_with_conn(&mut conn).await.unwrap();
        assert_eq!(got.as_deref(), Some("Alice"));
    }

    #[tokio::test]
    async fn set_user_overwrites_previous_value() {
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        set_user_nickname_with_conn(&mut conn, "Alice", &now)
            .await
            .unwrap();
        set_user_nickname_with_conn(&mut conn, "Bob", &now)
            .await
            .unwrap();
        let got = get_user_nickname_with_conn(&mut conn).await.unwrap();
        assert_eq!(got.as_deref(), Some("Bob"));
    }
}
