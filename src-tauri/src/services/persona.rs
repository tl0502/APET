// H.1 PersonaService MVP — 加载内置 momo + 写入 SQLite
//
// 范围(plan H.1):
// - parse_persona(&str) 通用解析器(接受任意 .soul.md 字符串,内置 / 用户文件 / 导入复用)
// - seed_builtin(&AppHandle) 启动入口 — 把 include_str! 编译进 binary 的 momo 写入 personas / persona_snapshots
//
// 设计:
// - frontmatter 用 gray_matter (yaml feature) 解析 → 反序列化到 PersonaFrontmatter
// - 必填字段检查(id / name / version / schema_version)用空字符串/0 哨兵 + 显式 MissingField 错误,
//   而不是依赖 serde missing field 错误(那个文案是英文,不便 UI 展示)
// - schema_version 仅接受 1 或 2(persona-design v1.0 §2.3 至少向后兼容前 1 个 schema)
// - markdown 区段不切分,直接把 raw markdown 存 persona_snapshots.content;切分推到 B.2 拼 system prompt 时按需做
//
// DB:
// - tauri-plugin-sql 2.4 的 DbPool 公共方法被注释掉(看 wrapper.rs 行 37-64),Rust 端无法借用 plugin 的 Pool
// - 自己开 sqlx::SqliteConnection 短期连接,做完 drop;DB 路径与 plugin 一致(<app_config>/aipet.db)
// - personas 走 ON CONFLICT(id) DO UPDATE(idempotent),persona_snapshots 走 (persona_id, version) 唯一性守卫

use chrono::Utc;
use gray_matter::engine::YAML;
use gray_matter::Matter;
use serde::{Deserialize, Serialize};
use sqlx::{Connection, Transaction};
use tauri::{AppHandle, Runtime};
use thiserror::Error;

use crate::services::db::{open_app_db, DbError};

/// 内置 momo 人格,编译期注入(与 migrations/001_init.sql 的 include_str! 同款)
const MOMO_RAW: &str = include_str!("../../personas/_builtin/momo.soul.md");

/// 内置 file_path 标识 — 用 `<bundled>:` 前缀区别于用户人格(后者填真实 APPDATA 路径)
const MOMO_BUNDLED_PATH: &str = "<bundled>:_builtin/momo.soul.md";

const SUPPORTED_SCHEMAS: &[u32] = &[1, 2];

#[derive(Debug, Deserialize, Default)]
pub struct PersonaFrontmatter {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    // MVP 只消费上述 4 个必填字段;author / voice_pack / accessories / tone_profile 等会被
    // gray_matter Pod 解析,但 PersonaFrontmatter struct 不声明这些字段 —— serde 默认忽略多余字段。
    // H.2/H.3 GUI 工坊接入时再扩展 struct。
}

#[derive(Debug)]
pub struct ParsedPersona {
    pub frontmatter: PersonaFrontmatter,
    pub raw_markdown: String,
}

#[derive(Debug, Error)]
pub enum PersonaError {
    #[error("frontmatter parse failed: {0}")]
    FrontmatterParse(String),
    #[error("schema_version unsupported: got {0}, want one of {1:?}")]
    UnsupportedSchema(u32, &'static [u32]),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
}

impl From<sqlx::Error> for PersonaError {
    fn from(e: sqlx::Error) -> Self {
        PersonaError::Database(e.to_string())
    }
}

impl From<DbError> for PersonaError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => PersonaError::AppConfigDir(s),
            DbError::Database(s) => PersonaError::Database(s),
        }
    }
}

/// PersonaService 对外契约（与前端 src/types/persona.ts::PersonaSummary 对齐）。
///
/// raw_markdown 是去掉 frontmatter 后的纯 markdown 正文，供 ChatService 拼 system prompt。
#[derive(Debug, Serialize)]
pub struct PersonaSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: String,
    pub raw_markdown: String,
}

#[derive(Debug, Error)]
pub enum PersonaLookupError {
    #[error(transparent)]
    Db(#[from] PersonaError),
    #[error("persona not found: {0}")]
    NotFound(String),
}

impl From<DbError> for PersonaLookupError {
    fn from(e: DbError) -> Self {
        PersonaLookupError::Db(e.into())
    }
}

impl From<sqlx::Error> for PersonaLookupError {
    fn from(e: sqlx::Error) -> Self {
        PersonaLookupError::Db(e.into())
    }
}

/// 解析任意 .soul.md 字符串为 ParsedPersona。
///
/// 不依赖 IO — 内置走 const,用户走 fs::read_to_string,后续 H.2 import 走拖拽 payload。
pub fn parse_persona(content: &str) -> Result<ParsedPersona, PersonaError> {
    let matter = Matter::<YAML>::new();
    let parsed = matter.parse(content);

    let pod = parsed.data.ok_or_else(|| {
        PersonaError::FrontmatterParse("missing frontmatter block (no `---` delimiters?)".into())
    })?;

    let frontmatter: PersonaFrontmatter = pod
        .deserialize()
        .map_err(|e| PersonaError::FrontmatterParse(e.to_string()))?;

    if frontmatter.schema_version == 0 {
        return Err(PersonaError::MissingField("schema_version"));
    }
    if !SUPPORTED_SCHEMAS.contains(&frontmatter.schema_version) {
        return Err(PersonaError::UnsupportedSchema(
            frontmatter.schema_version,
            SUPPORTED_SCHEMAS,
        ));
    }
    if frontmatter.id.is_empty() {
        return Err(PersonaError::MissingField("id"));
    }
    if frontmatter.name.is_empty() {
        return Err(PersonaError::MissingField("name"));
    }
    if frontmatter.version.is_empty() {
        return Err(PersonaError::MissingField("version"));
    }

    Ok(ParsedPersona {
        frontmatter,
        raw_markdown: parsed.content,
    })
}

/// 启动入口:解析内置 momo → UPSERT personas + idempotent INSERT persona_snapshots
///
/// 调用方在 lib.rs setup 阶段用 `tauri::async_runtime::spawn` 异步跑,失败仅 eprintln 到 stderr
/// (MVP 期不阻塞启动 / 不弹错误 UI;H.2 引入 IPC 后可考虑前端反馈)
pub async fn seed_builtin<R: Runtime>(app: &AppHandle<R>) -> Result<(), PersonaError> {
    let parsed = parse_persona(MOMO_RAW)?;
    let mut conn = open_app_db(app).await?;
    seed_persona_with_conn(&mut conn, &parsed, "builtin", MOMO_BUNDLED_PATH).await?;
    conn.close().await?;
    Ok(())
}

/// 读取 persona + 最新 snapshot，拼成 PersonaSummary。NotFound 时返回 NotFound 变体（前端可定向提示）。
pub async fn load_persona<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
) -> Result<PersonaSummary, PersonaLookupError> {
    let mut conn = open_app_db(app).await?;
    let summary = load_persona_with_conn(&mut conn, id).await?;
    conn.close().await.map_err(PersonaError::from)?;
    Ok(summary)
}

/// 读取当前 active persona（personas WHERE is_active = 1 LIMIT 1）。
///
/// #13 ChatService 拼 system prompt 时用：用户没指定时直接拿当前激活人格。
/// 没有 active 行（首启 seed 之前 / 全部停用）→ NotFound("active persona")。
pub async fn load_active_persona<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<PersonaSummary, PersonaLookupError> {
    let mut conn = open_app_db(app).await?;
    let summary = load_active_persona_with_conn(&mut conn).await?;
    conn.close().await.map_err(PersonaError::from)?;
    Ok(summary)
}

/// B1 修复：with_conn 变体，让 ChatService::prepare 把多个 service 调用串到同一条 conn 上，
/// 把单 chat_send 的 open/close 周期从 8 次降到 2 次（prepare 1 + run_stream 收尾 1）。
pub(crate) async fn load_active_persona_with_conn(
    conn: &mut sqlx::SqliteConnection,
) -> Result<PersonaSummary, PersonaLookupError> {
    let active: Option<(String,)> =
        sqlx::query_as("SELECT id FROM personas WHERE is_active = 1 LIMIT 1")
            .fetch_optional(&mut *conn)
            .await
            .map_err(PersonaError::from)?;
    let id = active.ok_or_else(|| PersonaLookupError::NotFound("active persona".to_string()))?;
    load_persona_with_conn(conn, &id.0).await
}

pub(crate) async fn load_persona_with_conn(
    conn: &mut sqlx::SqliteConnection,
    id: &str,
) -> Result<PersonaSummary, PersonaLookupError> {
    let row: Option<(String, String, String, String)> =
        sqlx::query_as("SELECT id, name, version, source FROM personas WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
    let (pid, name, version, source) =
        row.ok_or_else(|| PersonaLookupError::NotFound(id.to_string()))?;
    let snap: Option<(String,)> = sqlx::query_as(
        "SELECT content FROM persona_snapshots          WHERE persona_id = ? AND version = ?          ORDER BY id DESC LIMIT 1",
    )
    .bind(&pid)
    .bind(&version)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(PersonaSummary {
        id: pid,
        name,
        version,
        source,
        raw_markdown: snap.map(|(c,)| c).unwrap_or_default(),
    })
}

/// 把目标 persona 设为 active（其他全部清零）。NotFound 时报 PersonaLookupError::NotFound。
pub async fn activate_persona<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
) -> Result<(), PersonaLookupError> {
    let mut conn = open_app_db(app).await?;
    activate_persona_with_conn(&mut conn, id).await?;
    conn.close().await.map_err(PersonaError::from)?;
    Ok(())
}

pub(crate) async fn activate_persona_with_conn(
    conn: &mut sqlx::SqliteConnection,
    id: &str,
) -> Result<(), PersonaLookupError> {
    let mut tx = conn.begin().await?;
    sqlx::query("UPDATE personas SET is_active = 0")
        .execute(&mut *tx)
        .await?;
    let result = sqlx::query("UPDATE personas SET is_active = 1, updated_at = ? WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() == 0 {
        return Err(PersonaLookupError::NotFound(id.to_string()));
    }
    tx.commit().await?;
    Ok(())
}

/// Inner helper(2026-05-04 test-coverage):接 SqliteConnection,不依赖 AppHandle。
///
/// 与 prod `seed_builtin` 完全等价的 SQL 语义:begin tx → UPSERT persona → INSERT snapshot → commit。
pub(crate) async fn seed_persona_with_conn(
    conn: &mut sqlx::SqliteConnection,
    parsed: &ParsedPersona,
    source: &str,
    file_path: &str,
) -> Result<(), PersonaError> {
    let mut tx = conn.begin().await?;
    upsert_persona(&mut tx, parsed, source, file_path).await?;
    insert_snapshot_if_new(&mut tx, &parsed.frontmatter.id, parsed).await?;
    tx.commit().await?;
    Ok(())
}

async fn upsert_persona(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    parsed: &ParsedPersona,
    source: &str,
    file_path: &str,
) -> Result<(), PersonaError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO personas (id, name, version, source, file_path, is_active, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, 1, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            version = excluded.version,
            file_path = excluded.file_path,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&parsed.frontmatter.id)
    .bind(&parsed.frontmatter.name)
    .bind(&parsed.frontmatter.version)
    .bind(source)
    .bind(file_path)
    .bind(&now)
    .bind(&now)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

async fn insert_snapshot_if_new(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    persona_id: &str,
    parsed: &ParsedPersona,
) -> Result<(), PersonaError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO persona_snapshots (persona_id, version, content, created_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(persona_id, version) DO NOTHING
        "#,
    )
    .bind(persona_id)
    .bind(&parsed.frontmatter.version)
    .bind(&parsed.raw_markdown)
    .bind(&now)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_momo_succeeds() {
        let parsed = parse_persona(MOMO_RAW).expect("momo should parse");
        assert_eq!(parsed.frontmatter.id, "momo");
        assert_eq!(parsed.frontmatter.name, "默默");
        assert_eq!(parsed.frontmatter.version, "1.0.0");
        assert_eq!(parsed.frontmatter.schema_version, 2);
        assert!(!parsed.raw_markdown.is_empty());
    }

    #[test]
    fn parse_invalid_yaml_fails() {
        let bad = "---\nid: : : broken\n---\n# 身份\nx";
        let result = parse_persona(bad);
        assert!(matches!(result, Err(PersonaError::FrontmatterParse(_))));
    }

    #[test]
    fn parse_missing_id_fails() {
        let no_id = "---\nschema_version: 2\nname: 默默\nversion: 1.0.0\n---\n# 身份\nx";
        let result = parse_persona(no_id);
        assert!(matches!(result, Err(PersonaError::MissingField("id"))));
    }

    #[test]
    fn parse_unsupported_schema_fails() {
        let future = "---\nschema_version: 999\nid: x\nname: x\nversion: 1.0.0\n---\n# 身份\nx";
        let result = parse_persona(future);
        assert!(matches!(
            result,
            Err(PersonaError::UnsupportedSchema(999, _))
        ));
    }

    #[test]
    fn parse_schema_v1_compatible() {
        let v1 = "---\nschema_version: 1\nid: legacy\nname: 老人格\nversion: 0.9.0\n---\n# 身份\nx";
        let parsed = parse_persona(v1).expect("schema v1 must still parse");
        assert_eq!(parsed.frontmatter.schema_version, 1);
        assert_eq!(parsed.frontmatter.id, "legacy");
    }

    #[test]
    fn parse_strips_frontmatter_from_raw() {
        let parsed = parse_persona(MOMO_RAW).expect("momo should parse");
        assert!(
            !parsed.raw_markdown.contains("schema_version:"),
            "raw_markdown must not contain frontmatter"
        );
        assert!(
            parsed.raw_markdown.contains("# 身份"),
            "raw_markdown must contain # 身份 heading"
        );
    }

    // ===== DB 集成测试(2026-05-04 test-coverage P0)=====

    use crate::services::test_db::fresh_db;

    #[tokio::test]
    async fn seed_builtin_writes_personas_row() {
        let (_dir, mut conn) = fresh_db().await;
        let parsed = parse_persona(MOMO_RAW).unwrap();
        seed_persona_with_conn(&mut conn, &parsed, "builtin", MOMO_BUNDLED_PATH)
            .await
            .unwrap();

        // personas 表应有 momo 一行,is_active = 1
        let row: (String, String, String, String, String, i64) = sqlx::query_as(
            "SELECT id, name, version, source, file_path, is_active FROM personas WHERE id = ?",
        )
        .bind(&parsed.frontmatter.id)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(row.0, "momo");
        assert_eq!(row.1, "默默");
        assert_eq!(row.2, "1.0.0");
        assert_eq!(row.3, "builtin");
        assert_eq!(row.4, MOMO_BUNDLED_PATH);
        assert_eq!(row.5, 1, "seeded persona must be active");
    }

    #[tokio::test]
    async fn seed_builtin_writes_persona_snapshot() {
        let (_dir, mut conn) = fresh_db().await;
        let parsed = parse_persona(MOMO_RAW).unwrap();
        seed_persona_with_conn(&mut conn, &parsed, "builtin", MOMO_BUNDLED_PATH)
            .await
            .unwrap();

        // persona_snapshots 表应有 (momo, 1.0.0) 一行
        let row: (String, String) = sqlx::query_as(
            "SELECT persona_id, version FROM persona_snapshots WHERE persona_id = ?",
        )
        .bind(&parsed.frontmatter.id)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(row.0, "momo");
        assert_eq!(row.1, "1.0.0");
    }

    #[tokio::test]
    async fn seed_builtin_is_idempotent() {
        // 关键:启动时 seed 多次(用户重启 / dev tooling reload)不能重复插入 snapshot
        let (_dir, mut conn) = fresh_db().await;
        let parsed = parse_persona(MOMO_RAW).unwrap();

        // 跑 3 次 seed
        for _ in 0..3 {
            seed_persona_with_conn(&mut conn, &parsed, "builtin", MOMO_BUNDLED_PATH)
                .await
                .unwrap();
        }

        // personas 仍然只有 1 行(UPSERT)
        let persona_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM personas WHERE id = 'momo'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(persona_count.0, 1, "personas idempotent under repeat seed");

        // persona_snapshots 也只有 1 行(ON CONFLICT(persona_id, version) DO NOTHING + 002 unique idx)
        let snap_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM persona_snapshots WHERE persona_id = 'momo' AND version = '1.0.0'",
        )
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(
            snap_count.0, 1,
            "snapshot must be deduped by (persona_id, version)"
        );
    }

    #[tokio::test]
    async fn persona_snapshot_unique_index_blocks_direct_duplicate_insert() {
        // 防御:002 migration 的 UNIQUE INDEX 必须能挡住绕过 ON CONFLICT 的直接 INSERT
        // (例如未来 H.2 import flow 用了 INSERT 不带 ON CONFLICT)
        let (_dir, mut conn) = fresh_db().await;
        let parsed = parse_persona(MOMO_RAW).unwrap();
        seed_persona_with_conn(&mut conn, &parsed, "builtin", MOMO_BUNDLED_PATH)
            .await
            .unwrap();

        // 直接 INSERT 同 (persona_id, version) — 不走 ON CONFLICT,期待 unique 索引报错
        let result = sqlx::query(
            "INSERT INTO persona_snapshots (persona_id, version, content, created_at) \
             VALUES ('momo', '1.0.0', 'duplicate', '2026-05-04')",
        )
        .execute(&mut conn)
        .await;
        assert!(
            result.is_err(),
            "direct duplicate INSERT must violate idx_persona_snapshots_unique_persona_version"
        );
    }

    #[tokio::test]
    async fn seed_different_versions_keeps_history_in_snapshots() {
        // 用户改 .soul.md 的 version 字段:同 persona_id 不同 version 应在 snapshots 累积历史
        let (_dir, mut conn) = fresh_db().await;
        let v1 = parse_persona(
            "---\nschema_version: 2\nid: momo\nname: 默默\nversion: 1.0.0\n---\n# 身份\nv1",
        )
        .unwrap();
        let v2 = parse_persona(
            "---\nschema_version: 2\nid: momo\nname: 默默\nversion: 1.1.0\n---\n# 身份\nv2",
        )
        .unwrap();

        seed_persona_with_conn(&mut conn, &v1, "user", "/tmp/momo.v1.md")
            .await
            .unwrap();
        seed_persona_with_conn(&mut conn, &v2, "user", "/tmp/momo.v2.md")
            .await
            .unwrap();

        // personas 表 UPSERT 后只有 1 行 momo,name/version 是 v2 的
        let persona_row: (String, String) =
            sqlx::query_as("SELECT name, version FROM personas WHERE id = 'momo'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(persona_row.1, "1.1.0", "personas.version reflects latest");

        // persona_snapshots 应有 2 行历史(1.0.0 + 1.1.0)
        let snap_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM persona_snapshots WHERE persona_id = 'momo'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(
            snap_count.0, 2,
            "snapshots accumulate per-version history (audit trail)"
        );
    }
}
