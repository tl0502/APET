// services/test_db.rs — DB 集成测试共享 fixture(2026-05-04)
//
// 背景(progress/test-coverage-2026-05-04.md):
// - M1 W1 D3 实施 B.1 LLMProvider 时暴露:7 services / 62 单测全是纯逻辑,
//   没有任何一个真实 DB 写入路径覆盖 → tauri-plugin-sql 2.x add_migrations 是惰性的、
//   未配 plugins.sql.preload 的"地基级" bug 静默潜伏 6 commits。
// - 本模块为 4 个 service(persona / memory / nickname / secrets)的 mod tests 提供:
//   1) `fresh_db()` — tempfile 临时 .db + apply migrations(001 + 002)+ 返回 SqliteConnection
//   2) 共享 helper 不依赖 AppHandle / Tauri runtime,CI 上无 GUI 可跑
//
// 设计:
// - 用 tempfile::TempDir(目录级)而非 NamedTempFile(单文件):sqlite WAL / SHM 同名兄弟文件
//   会被 Drop 自动清理;NamedTempFile 只清原文件。
// - 返回 (TempDir, SqliteConnection):TempDir 必须 keep alive 整个测试期间(drop 即删目录),
//   测试函数签名 `let (_dir, mut conn) = fresh_db().await;` 让 _dir 走出 scope 同时关 conn。
// - Migrations 用 include_str! 编译进 binary,与 lib.rs::migrations() 同一 SQL 源,
//   测试与 prod schema 永远同步(更新 001/002.sql 时测试自动跟进)。
// - 不用 sqlx::test 宏:它的 SQLite 支持需要 sqlite_test_runner + #[sqlx::test(migrator = ...)],
//   配置成本 > 手卷 fresh_db;且 sqlx::test 默认每测试一个 DB,语义与 tempfile 等价。

#![cfg(test)]

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Executor, SqliteConnection};
use tempfile::TempDir;

use crate::services::db::enforce_pragmas;

/// 与 lib.rs::migrations() 同一来源 — 改 001.sql 自动同步到测试。
/// 002 已 merge 进 001（2026-05-06 code-review #7 — 单一 migration 文件）。
const MIGRATION_001: &str = include_str!("../../migrations/001_init.sql");

/// 创建一个全新的临时 sqlite DB,apply 所有 migrations,返回 (TempDir, SqliteConnection)。
///
/// 调用者必须用 `let (_dir, mut conn) = fresh_db().await;` 接收 — TempDir 走出 scope
/// 时整个目录被删除(包括 -wal / -shm 兄弟文件)。
///
/// **不**用 `:memory:` DB:tauri-plugin-sql 在 prod 是磁盘文件,测试要尽量贴近 prod 行为
/// (磁盘 sqlite 与内存 sqlite 在 PRAGMA / WAL / 文件锁等行为上有细微差别)。
///
/// 注:`sqlx::SqliteConnectOptions` 默认开启 `PRAGMA foreign_keys = ON`(与 SQLite 引擎
/// 自身默认 OFF 不同),所以本 fixture 下 FK 约束**即时生效** — 见
/// `memory::insert_message_rejects_unknown_conversation_id` 守护此行为(报告 §4.1 [HIGH])。
pub async fn fresh_db() -> (TempDir, SqliteConnection) {
    let dir = TempDir::new().expect("create temp dir for test DB");
    let db_path = dir.path().join("aipet.db");

    // 与 prod (services/persona.rs::seed_builtin) 同款 builder API;
    // create_if_missing(true) 因为这是测试新 DB
    let mut conn = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .connect()
        .await
        .expect("connect to fresh test DB");

    // Apply migrations 顺序与 lib.rs::migrations() 一致
    conn.execute(MIGRATION_001)
        .await
        .expect("apply 001_init.sql");

    // 与 prod 行为对齐：每个连接显式 PRAGMA foreign_keys = ON
    // （sqlx 默认 ON，但显式调用一次能保证未来 prod 加 PRAGMA 时测试同步生效）
    enforce_pragmas(&mut conn)
        .await
        .expect("enforce pragmas on test conn");

    (dir, conn)
}

#[cfg(test)]
mod self_tests {
    use super::*;

    #[tokio::test]
    async fn fresh_db_creates_all_tables() {
        let (_dir, mut conn) = fresh_db().await;
        // 抽样检验:001_init.sql 里 19 张表至少 5 张存在
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN \
             ('personas', 'persona_snapshots', 'messages', 'nicknames', 'secrets')",
        )
        .fetch_one(&mut conn)
        .await
        .expect("query sqlite_master");
        assert_eq!(count.0, 5, "expected 5 core tables to exist after migrations");
    }

    #[tokio::test]
    async fn fresh_db_seeds_singletons() {
        // 001 末尾的 INSERT 行应该写入 — consent / nicknames / pet_runtime_state / voice_settings
        let (_dir, mut conn) = fresh_db().await;
        let nick_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM nicknames WHERE id = 1")
            .fetch_one(&mut conn)
            .await
            .expect("query nicknames");
        assert_eq!(nick_count.0, 1, "nicknames singleton row must exist");

        let consent_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM consent WHERE id = 1")
            .fetch_one(&mut conn)
            .await
            .expect("query consent");
        assert_eq!(consent_count.0, 1, "consent singleton row must exist");
    }

    #[tokio::test]
    async fn fresh_db_persona_snapshot_unique_index_present() {
        // 002 应已建立 idx_persona_snapshots_unique_persona_version
        let (_dir, mut conn) = fresh_db().await;
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type = 'index' \
             AND name = 'idx_persona_snapshots_unique_persona_version'",
        )
        .fetch_optional(&mut conn)
        .await
        .expect("query sqlite_master indexes");
        assert!(
            row.is_some(),
            "002_persona_snapshot_unique migration must create the unique index"
        );
    }
}
