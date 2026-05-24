// 共享 DB helper（2026-05-06 code-review #1+#2）
//
// 动机：
// - persona / nickname / preferences / memory 4 处都在重新实现"取 app_config_dir + 拼 aipet.db
//   + SqliteConnectOptions builder + connect"，改一处必须改 4 处。
// - 更关键：SQLite 默认 `PRAGMA foreign_keys = OFF`（与 sqlx::SqliteConnectOptions
//   默认 ON 不同 — 但 plugin DbPool 路径 / 未来手动连接路径不保证）。把 PRAGMA 也收口在
//   helper 里，messages.conversation_id / persona_snapshots.persona_id 等 FK 才能稳定生效。
//
// 设计：
// - 用 builder API（不走 from_str("sqlite:...")）— Windows 绝对路径反斜杠会让 URL parsing
//   报 SQLITE_CANTOPEN(code 14)。
// - create_if_missing(false)：plugin migrations 已建库；service 层永远不该建库。测试 fixture
//   里走 create_if_missing(true) 是因为 tempfile 是空目录。
// - PRAGMA 用 conn.execute() 在每次新开连接后立刻发，与该 connection 绑定生效。

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Executor, SqliteConnection};
use tauri::{AppHandle, Manager, Runtime};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
    #[error("database error: {0}")]
    Database(String),
}

impl From<sqlx::Error> for DbError {
    fn from(e: sqlx::Error) -> Self {
        DbError::Database(e.to_string())
    }
}

/// 打开 app DB 主连接（<app_config_dir>/aipet.db）+ 强制启用 FK。
///
/// 返回的 SqliteConnection 的 `PRAGMA foreign_keys` 已为 ON；
/// 调用方应在使用完后 `.close().await`（或借助 Drop）。
pub async fn open_app_db<R: Runtime>(app: &AppHandle<R>) -> Result<SqliteConnection, DbError> {
    let app_config = app
        .path()
        .app_config_dir()
        .map_err(|e| DbError::AppConfigDir(e.to_string()))?;
    let db_path = app_config.join("aipet.db");
    let mut conn = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(false)
        .connect()
        .await?;
    enforce_pragmas(&mut conn).await?;
    Ok(conn)
}

/// 把测试 / 集成连接也拉到与 prod 一致的 PRAGMA 状态。
///
/// - `journal_mode = WAL`：CLAUDE.md 声明 "SQLite + WAL"，但 tauri-plugin-sql 默认 builder 不
///   保证开启 WAL，先前版本 enforce_pragmas 也漏设；落到默认 `journal_mode=delete` 时写时
///   整库锁，叠加并发流式 + chat_history + 昵称注入三路写极易触发 BUSY。WAL 是 DB 级持久
///   属性（第一次设置写入文件 header），后续每次连接显式设无副作用，作"确认型 PRAGMA"使用。
///   注：`:memory:` DB 不支持 WAL（SQLite 会静默忽略保持 memory 模式），磁盘 DB 才生效——
///   test_db::fresh_db 走 tempfile 磁盘文件，OK。2026-05-10 code-review Bug 4 修复。
/// - `foreign_keys = ON`：messages.conversation_id / persona_snapshots.persona_id 等 FK
///   能稳定生效（SQLite 默认 OFF，sqlx 默认 ON，但 plugin DbPool 路径 / 手动连接不保证）。
/// - `busy_timeout = 5000`：拿不到写锁时**等 5s 再返 BUSY**（SQLite 默认 0 = 立即报错）。
///   修复缘由：#4 修复期间在 ChatService::prepare 引入了 begin/SELECT/INSERT*2/commit 的
///   显式事务，持有写锁 ~10-50ms。同时 update_last_activity / nickname_announcement /
///   plugin DbPool 路径都在写同一 aipet.db。默认 0 timeout 下两路并发就立即 SQLITE_BUSY
///   (code 5)。5s 内重试是 SQLite 标准做法，远短于人感知阈值且足够吸收正常的并发写。
pub async fn enforce_pragmas(conn: &mut SqliteConnection) -> Result<(), DbError> {
    conn.execute("PRAGMA journal_mode = WAL").await?;
    conn.execute("PRAGMA foreign_keys = ON").await?;
    conn.execute("PRAGMA busy_timeout = 5000").await?;
    Ok(())
}

/// 内部使用: 按显式 path 打开 (kernel PermissionService 用)。
/// 与 `open_app_db` 区别: 不走 AppHandle, 而是接受调用方持有的 path。
/// Phase A0 临时方案; Phase A1 kernel/db 模块完整收口后 deprecate。
pub async fn connect_at(db_path: &std::path::Path) -> Result<SqliteConnection, DbError> {
    let mut conn = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(false)
        .connect()
        .await?;
    enforce_pragmas(&mut conn).await?;
    Ok(conn)
}
