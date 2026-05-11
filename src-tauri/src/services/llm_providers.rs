// LLMProviderService — 多 provider 实例管理（用户增补设计，参考 cc-switch UI）。
//
// 范围（M1）：
// - 用户可保存多个 OpenAI 兼容 provider 实例（OpenAI 工作账号 + DeepSeek 国内备用 + Ollama 本地 等）
// - 任意时刻一个 active；ChatService 用 active 那一份构 OpenAIProvider
// - CRUD：list / get / add / update / delete / activate / test
// - 启动期 migrate_legacy：把 #12 旧 `llm:openai:*` 三键自动搬成"默认 OpenAI" provider
//
// 偏离 #12 的 ADR-018 Layer 1（issue 仅指方向，以本设计为准）：
// - 单 namespace `llm:openai:*` → 多 namespace `llm:provider:<ulid>` (JSON value)
// - active 选择从"看哪个 namespace 有值"→ 显式 `llm:active_id` KV
// - 旧 IPC（set_openai_api_key 等）已删；前端走新 IPC
//
// Schema（沿用 config 表 KV，27 表零迁移原则）：
//   llm:active_id           = "<ulid>" or NULL
//   llm:provider:<ulid>     = JSON {"name","api_key","base_url","model"}
//
// list_providers 实现走 SQL `LIKE 'llm:provider:%' ESCAPE` 走全表扫描；M1 单用户 provider
// 数预期 < 20，性能足够。M3 加密分拆后 api_key 不再走本路径。

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Connection, SqliteConnection};
use tauri::{AppHandle, Runtime};
use thiserror::Error;
use ulid::Ulid;

use crate::services::config::{self, ConfigError};
use crate::services::db::{open_app_db, DbError};

pub const KEY_ACTIVE_ID: &str = "llm:active_id";
pub const KEY_PROVIDER_PREFIX: &str = "llm:provider:";
/// B8：迁移完成时间戳 KV，留作审计；置非空 = 已迁移过。
pub const KEY_LEGACY_MIGRATED_AT: &str = "llm:legacy_migrated_at";
/// 旧 #12 单 namespace 三键，仅启动期 migrate 时读一次。
pub const LEGACY_KEY_API_KEY: &str = "llm:openai:api_key";
pub const LEGACY_KEY_BASE_URL: &str = "llm:openai:base_url";
pub const LEGACY_KEY_MODEL: &str = "llm:openai:model";

pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-4o-mini";
const NAME_MAX_LEN: usize = 50;

#[derive(Debug, Error)]
pub enum LlmProviderError {
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
    #[error("provider not found: {0}")]
    NotFound(String),
    #[error("provider id 不能为空")]
    EmptyId,
    #[error("provider name 不能为空")]
    EmptyName,
    #[error("provider name 超过 {0} 字符")]
    NameTooLong(usize),
    #[error("base_url 不能为空")]
    EmptyBaseUrl,
    #[error("model 不能为空")]
    EmptyModel,
    #[error("不能删除当前激活的 provider；请先切换到其他 provider")]
    CannotDeleteActive,
    #[error("JSON 序列化失败: {0}")]
    Json(String),
}

impl From<sqlx::Error> for LlmProviderError {
    fn from(e: sqlx::Error) -> Self {
        LlmProviderError::Database(e.to_string())
    }
}

impl From<DbError> for LlmProviderError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => LlmProviderError::AppConfigDir(s),
            DbError::Database(s) => LlmProviderError::Database(s),
        }
    }
}

impl From<ConfigError> for LlmProviderError {
    fn from(e: ConfigError) -> Self {
        match e {
            ConfigError::AppConfigDir(s) => LlmProviderError::AppConfigDir(s),
            ConfigError::Database(s) => LlmProviderError::Database(s),
        }
    }
}

impl From<serde_json::Error> for LlmProviderError {
    fn from(e: serde_json::Error) -> Self {
        LlmProviderError::Json(e.to_string())
    }
}

/// JSON value 真实存储结构（含 api_key 明文，M3 G CryptoService 后单独迁出）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRecord {
    pub name: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

/// 列表项（不含 api_key）— 给 UI list 用。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderListItem {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    /// 是否已设置 api_key（永远不返明文；与 #12 设计延续）。
    pub has_api_key: bool,
    pub is_active: bool,
}

/// 详情项（含 api_key 明文）— 给 edit 弹窗用；调用方需自觉不持久化到 UI 长生命周期 state。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDetail {
    pub id: String,
    pub name: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddProviderRequest {
    pub name: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderRequest {
    pub name: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

fn validate_name(name: &str) -> Result<String, LlmProviderError> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() {
        return Err(LlmProviderError::EmptyName);
    }
    if trimmed.chars().count() > NAME_MAX_LEN {
        return Err(LlmProviderError::NameTooLong(NAME_MAX_LEN));
    }
    Ok(trimmed)
}

fn validate_required(value: &str, err: LlmProviderError) -> Result<String, LlmProviderError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(err);
    }
    Ok(trimmed)
}

fn validate_id(id: &str) -> Result<String, LlmProviderError> {
    let trimmed = id.trim().to_string();
    if trimmed.is_empty() {
        return Err(LlmProviderError::EmptyId);
    }
    Ok(trimmed)
}

fn provider_kv_key(id: &str) -> String {
    format!("{KEY_PROVIDER_PREFIX}{id}")
}

/// 读 provider list（按 name ASC）。M1 < 20 行，全表 LIKE 扫足够。
pub async fn list_providers<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<ProviderListItem>, LlmProviderError> {
    let mut conn = open_app_db(app).await?;
    let result = list_providers_with_conn(&mut conn).await?;
    conn.close().await?;
    Ok(result)
}

pub(crate) async fn list_providers_with_conn(
    conn: &mut SqliteConnection,
) -> Result<Vec<ProviderListItem>, LlmProviderError> {
    let active_id = config::get_with_conn(conn, KEY_ACTIVE_ID).await?;

    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT key, value FROM config
        WHERE key LIKE 'llm:provider:%'
        "#,
    )
    .fetch_all(&mut *conn)
    .await?;

    let mut items: Vec<ProviderListItem> = rows
        .into_iter()
        .filter_map(|(key, value)| {
            let id = key.strip_prefix(KEY_PROVIDER_PREFIX)?.to_string();
            // 损坏 JSON（用户手改 DB / 跨版本 schema 漂移 / 解密残留）：log 警告 + skip。
            // 早先版本 `serde_json::from_str(&value).ok()?` 静默吞错，导致用户在设置面板
            // 看不到那条诡异行也无法修复——若它恰好是 active_id 指向那条，ChatService 会报
            // "未配置 provider"，用户排查无门。2026-05-10 code-review Bug 3 修复。
            match serde_json::from_str::<ProviderRecord>(&value) {
                Ok(record) => Some(ProviderListItem {
                    is_active: active_id.as_deref() == Some(id.as_str()),
                    has_api_key: !record.api_key.is_empty(),
                    id,
                    name: record.name,
                    base_url: record.base_url,
                    model: record.model,
                }),
                Err(e) => {
                    eprintln!(
                        "[llm_providers] skip corrupted provider entry id={id}: {e}"
                    );
                    None
                }
            }
        })
        .collect();
    // 按 name ASC（前端列表稳定排序；M3 拖拽排序时再加 sort_index 字段）
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

/// 读单个 provider 详情（含 api_key）。
pub async fn get_provider<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
) -> Result<ProviderDetail, LlmProviderError> {
    let id = validate_id(id)?;
    let mut conn = open_app_db(app).await?;
    let result = get_provider_with_conn(&mut conn, &id).await?;
    conn.close().await?;
    Ok(result)
}

pub(crate) async fn get_provider_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<ProviderDetail, LlmProviderError> {
    let raw = config::get_with_conn(conn, &provider_kv_key(id)).await?;
    let record: ProviderRecord = match raw {
        Some(s) => serde_json::from_str(&s)?,
        None => return Err(LlmProviderError::NotFound(id.to_string())),
    };
    let active_id = config::get_with_conn(conn, KEY_ACTIVE_ID).await?;
    Ok(ProviderDetail {
        id: id.to_string(),
        is_active: active_id.as_deref() == Some(id),
        name: record.name,
        api_key: record.api_key,
        base_url: record.base_url,
        model: record.model,
    })
}

/// 新建 provider；返回新 ULID。如果是首条，自动设为 active。
pub async fn add_provider<R: Runtime>(
    app: &AppHandle<R>,
    req: AddProviderRequest,
) -> Result<String, LlmProviderError> {
    let mut conn = open_app_db(app).await?;
    let id = add_provider_with_conn(&mut conn, req).await?;
    conn.close().await?;
    Ok(id)
}

pub(crate) async fn add_provider_with_conn(
    conn: &mut SqliteConnection,
    req: AddProviderRequest,
) -> Result<String, LlmProviderError> {
    let record = ProviderRecord {
        name: validate_name(&req.name)?,
        api_key: req.api_key.trim().to_string(), // 允许空（用户先存配置后填 key）
        base_url: validate_required(&req.base_url, LlmProviderError::EmptyBaseUrl)?,
        model: validate_required(&req.model, LlmProviderError::EmptyModel)?,
    };
    let id = Ulid::new().to_string();
    let now = Utc::now().to_rfc3339();
    let json = serde_json::to_string(&record)?;
    config::set_with_conn(conn, &provider_kv_key(&id), &json, &now).await?;

    // 首条自动设 active；后续保持当前 active 不变
    let existing_active = config::get_with_conn(conn, KEY_ACTIVE_ID).await?;
    if existing_active.is_none() {
        config::set_with_conn(conn, KEY_ACTIVE_ID, &id, &now).await?;
    }
    Ok(id)
}

/// 部分更新 provider（None 字段不动）。
pub async fn update_provider<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    req: UpdateProviderRequest,
) -> Result<(), LlmProviderError> {
    let id = validate_id(id)?;
    let mut conn = open_app_db(app).await?;
    update_provider_with_conn(&mut conn, &id, req).await?;
    conn.close().await?;
    Ok(())
}

pub(crate) async fn update_provider_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
    req: UpdateProviderRequest,
) -> Result<(), LlmProviderError> {
    let key = provider_kv_key(id);
    let raw = config::get_with_conn(conn, &key).await?;
    let mut record: ProviderRecord = match raw {
        Some(s) => serde_json::from_str(&s)?,
        None => return Err(LlmProviderError::NotFound(id.to_string())),
    };
    if let Some(name) = req.name {
        record.name = validate_name(&name)?;
    }
    if let Some(api_key) = req.api_key {
        // partial update：传 "" 也写入（用户显式清空）；若调用方不想动，应在前端不带此字段
        record.api_key = api_key.trim().to_string();
    }
    if let Some(base_url) = req.base_url {
        record.base_url = validate_required(&base_url, LlmProviderError::EmptyBaseUrl)?;
    }
    if let Some(model) = req.model {
        record.model = validate_required(&model, LlmProviderError::EmptyModel)?;
    }
    let json = serde_json::to_string(&record)?;
    let now = Utc::now().to_rfc3339();
    config::set_with_conn(conn, &key, &json, &now).await?;
    Ok(())
}

/// 删除 provider；当前激活的不允许删，**除非**它是当前唯一一条 provider（这种情况下
/// 用户没有可切换目标，强制要求"先切再删"会形成死循环 UX）。
/// 删完如果列表空了或当前 active 被删，active_id KV 一并清空。
pub async fn delete_provider<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
) -> Result<(), LlmProviderError> {
    let id = validate_id(id)?;
    let mut conn = open_app_db(app).await?;
    delete_provider_with_conn(&mut conn, &id).await?;
    conn.close().await?;
    Ok(())
}

pub(crate) async fn delete_provider_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<(), LlmProviderError> {
    let active_id = config::get_with_conn(conn, KEY_ACTIVE_ID).await?;
    let is_active = active_id.as_deref() == Some(id);

    // active 但 list 还有其他 provider → 拒删（要求用户先 activate 别的）。
    // active 且唯一一条 → 允许删 + 清 active_id KV（2026-05-10 code-review Bug 5：
    // 早先版本无差别拒删 active，唯一 provider 场景下用户永远走不到"清空 active_id"
    // 路径，必须靠"先 add 第二个 → activate 它 → delete 第一个"绕路）。
    if is_active {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM config WHERE key LIKE 'llm:provider:%'",
        )
        .fetch_one(&mut *conn)
        .await?;
        if count.0 > 1 {
            return Err(LlmProviderError::CannotDeleteActive);
        }
    }

    let key = provider_kv_key(id);
    let result = sqlx::query("DELETE FROM config WHERE key = ?")
        .bind(&key)
        .execute(&mut *conn)
        .await?;
    if result.rows_affected() == 0 {
        return Err(LlmProviderError::NotFound(id.to_string()));
    }

    // 删了 active 那条 → 清 KEY_ACTIVE_ID 防止孤儿 active_id 让 get_active_record 返 None
    // 但 ChatService 报"未配置 provider"时用户在面板看不到任何 active 标记产生困惑。
    if is_active {
        config::delete_with_conn(conn, KEY_ACTIVE_ID).await?;
    }

    Ok(())
}

/// 设当前激活 provider；id 不存在时报 NotFound（防止 KV 写入孤儿 active_id）。
pub async fn activate_provider<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
) -> Result<(), LlmProviderError> {
    let id = validate_id(id)?;
    let mut conn = open_app_db(app).await?;
    activate_provider_with_conn(&mut conn, &id).await?;
    conn.close().await?;
    Ok(())
}

pub(crate) async fn activate_provider_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<(), LlmProviderError> {
    let exists = config::get_with_conn(conn, &provider_kv_key(id)).await?;
    if exists.is_none() {
        return Err(LlmProviderError::NotFound(id.to_string()));
    }
    let now = Utc::now().to_rfc3339();
    config::set_with_conn(conn, KEY_ACTIVE_ID, id, &now).await?;
    Ok(())
}

/// 读当前 active provider 完整记录（ChatService::build_provider 真消费路径）。
/// active_id 不存在 / 对应 provider 已删 → None（ChatService 会报"未配置 provider"）。
pub async fn get_active_record<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<ProviderRecord>, LlmProviderError> {
    let mut conn = open_app_db(app).await?;
    let result = get_active_record_with_conn(&mut conn).await?;
    conn.close().await?;
    Ok(result)
}

pub(crate) async fn get_active_record_with_conn(
    conn: &mut SqliteConnection,
) -> Result<Option<ProviderRecord>, LlmProviderError> {
    let active_id = match config::get_with_conn(conn, KEY_ACTIVE_ID).await? {
        Some(v) if !v.is_empty() => v,
        _ => return Ok(None),
    };
    let raw = config::get_with_conn(conn, &provider_kv_key(&active_id)).await?;
    match raw {
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
        None => Ok(None),
    }
}

/// 启动期 migration：把 #12 单 namespace 的旧三键搬成"默认 OpenAI" provider。
///
/// 触发条件：
/// - llm:active_id 不存在（未走过新 schema）
/// - llm:openai:api_key 存在且非空（用户在旧 IPC 下已配过）
///
/// 副作用：
/// - 创建一条 provider，name = "默认 OpenAI"
/// - 设为 active
/// - 旧 KV 不删（保留以便用户回滚 / 旧测试 IPC 复用；新 ChatService 不再读它）
pub async fn migrate_legacy_if_needed<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<bool, LlmProviderError> {
    let mut conn = open_app_db(app).await?;
    let migrated = migrate_legacy_with_conn(&mut conn).await?;
    conn.close().await?;
    Ok(migrated)
}

pub(crate) async fn migrate_legacy_with_conn(
    conn: &mut SqliteConnection,
) -> Result<bool, LlmProviderError> {
    if config::get_with_conn(conn, KEY_ACTIVE_ID).await?.is_some() {
        return Ok(false); // 已走过新 schema
    }
    let legacy_key = config::get_with_conn(conn, LEGACY_KEY_API_KEY)
        .await?
        .filter(|s| !s.is_empty());
    let Some(api_key) = legacy_key else {
        return Ok(false); // 旧配置也没有；无需 migrate
    };
    let base_url = config::get_with_conn(conn, LEGACY_KEY_BASE_URL)
        .await?
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_OPENAI_BASE_URL.to_string());
    let model = config::get_with_conn(conn, LEGACY_KEY_MODEL)
        .await?
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.to_string());

    add_provider_with_conn(
        conn,
        AddProviderRequest {
            name: "默认 OpenAI".to_string(),
            api_key,
            base_url,
            model,
        },
    )
    .await?;

    // B8：迁移成功后清理旧 KV + 写时间戳标记，免得三个旧 key 永远活在 DB 里。
    // M1 没有真正的"回滚"路径（旧 IPC 已删），保留它们只是噪音。
    let now = Utc::now().to_rfc3339();
    config::set_with_conn(conn, KEY_LEGACY_MIGRATED_AT, &now, &now).await?;
    sqlx::query("DELETE FROM config WHERE key IN (?, ?, ?)")
        .bind(LEGACY_KEY_API_KEY)
        .bind(LEGACY_KEY_BASE_URL)
        .bind(LEGACY_KEY_MODEL)
        .execute(&mut *conn)
        .await?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::test_db::fresh_db;

    fn req(name: &str, key: &str) -> AddProviderRequest {
        AddProviderRequest {
            name: name.to_string(),
            api_key: key.to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
        }
    }

    #[tokio::test]
    async fn fresh_db_has_no_providers_and_no_active() {
        let (_dir, mut conn) = fresh_db().await;
        let items = list_providers_with_conn(&mut conn).await.unwrap();
        assert!(items.is_empty());
        assert!(get_active_record_with_conn(&mut conn)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn add_first_auto_activates() {
        let (_dir, mut conn) = fresh_db().await;
        let id = add_provider_with_conn(&mut conn, req("OpenAI", "sk-1"))
            .await
            .unwrap();
        let items = list_providers_with_conn(&mut conn).await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_active);
        assert_eq!(items[0].id, id);
        assert!(items[0].has_api_key);
    }

    #[tokio::test]
    async fn add_second_does_not_change_active() {
        let (_dir, mut conn) = fresh_db().await;
        let id1 = add_provider_with_conn(&mut conn, req("A", "sk-1"))
            .await
            .unwrap();
        let _id2 = add_provider_with_conn(&mut conn, req("B", "sk-2"))
            .await
            .unwrap();
        let active = get_active_record_with_conn(&mut conn).await.unwrap();
        assert_eq!(active.unwrap().api_key, "sk-1");
        let items = list_providers_with_conn(&mut conn).await.unwrap();
        let active_item = items.iter().find(|i| i.is_active).unwrap();
        assert_eq!(active_item.id, id1);
    }

    #[tokio::test]
    async fn activate_switches_active_id() {
        let (_dir, mut conn) = fresh_db().await;
        let _id1 = add_provider_with_conn(&mut conn, req("A", "sk-1"))
            .await
            .unwrap();
        let id2 = add_provider_with_conn(&mut conn, req("B", "sk-2"))
            .await
            .unwrap();
        activate_provider_with_conn(&mut conn, &id2).await.unwrap();
        let active = get_active_record_with_conn(&mut conn).await.unwrap();
        assert_eq!(active.unwrap().api_key, "sk-2");
    }

    #[tokio::test]
    async fn activate_unknown_id_returns_not_found() {
        let (_dir, mut conn) = fresh_db().await;
        let r = activate_provider_with_conn(&mut conn, "01ZZZZZZZZZZZZZZZZZZZZZZZZ").await;
        assert!(matches!(r, Err(LlmProviderError::NotFound(_))));
    }

    #[tokio::test]
    async fn delete_active_blocks_with_helpful_error_when_others_exist() {
        // 旧测试单 active 触发拒删 → 2026-05-10 Bug 5 修复后语义变成"唯一 active 允许删"。
        // 这里覆盖"还有其他 provider"的拒删路径：active=A，再 add B；尝试删 A 应被拒。
        let (_dir, mut conn) = fresh_db().await;
        let id_a = add_provider_with_conn(&mut conn, req("A", "sk-1"))
            .await
            .unwrap();
        let _id_b = add_provider_with_conn(&mut conn, req("B", "sk-2"))
            .await
            .unwrap();
        let r = delete_provider_with_conn(&mut conn, &id_a).await;
        assert!(matches!(r, Err(LlmProviderError::CannotDeleteActive)));
    }

    #[tokio::test]
    async fn delete_unique_active_succeeds_and_clears_active_kv() {
        // Bug 5：active 且唯一一条时允许删，避免"先 add 第二个 → activate → delete 第一个"绕路。
        // 删完 active_id KV 应被清空，否则 get_active_record 会返 None 但孤儿 KV 残留。
        let (_dir, mut conn) = fresh_db().await;
        let id = add_provider_with_conn(&mut conn, req("Solo", "sk-1"))
            .await
            .unwrap();
        // 此时 list.len()==1 且 active==Solo
        delete_provider_with_conn(&mut conn, &id).await.unwrap();

        let items = list_providers_with_conn(&mut conn).await.unwrap();
        assert!(items.is_empty(), "唯一 provider 删除后 list 应为空");
        let active_kv = config::get_with_conn(&mut conn, KEY_ACTIVE_ID).await.unwrap();
        assert!(
            active_kv.is_none(),
            "唯一 active 删除后 KEY_ACTIVE_ID 应被清，避免孤儿"
        );
    }

    #[tokio::test]
    async fn delete_inactive_succeeds() {
        let (_dir, mut conn) = fresh_db().await;
        let _id1 = add_provider_with_conn(&mut conn, req("A", "sk-1"))
            .await
            .unwrap();
        let id2 = add_provider_with_conn(&mut conn, req("B", "sk-2"))
            .await
            .unwrap();
        delete_provider_with_conn(&mut conn, &id2).await.unwrap();
        let items = list_providers_with_conn(&mut conn).await.unwrap();
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn update_partial_keeps_other_fields() {
        let (_dir, mut conn) = fresh_db().await;
        let id = add_provider_with_conn(&mut conn, req("A", "sk-1"))
            .await
            .unwrap();
        update_provider_with_conn(
            &mut conn,
            &id,
            UpdateProviderRequest {
                name: Some("A2".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let detail = get_provider_with_conn(&mut conn, &id).await.unwrap();
        assert_eq!(detail.name, "A2");
        assert_eq!(detail.api_key, "sk-1"); // 未传不动
        assert_eq!(detail.base_url, "https://api.openai.com/v1");
    }

    #[tokio::test]
    async fn validate_empty_name_rejects() {
        let (_dir, mut conn) = fresh_db().await;
        let r = add_provider_with_conn(&mut conn, req("   ", "sk-1")).await;
        assert!(matches!(r, Err(LlmProviderError::EmptyName)));
    }

    #[tokio::test]
    async fn migrate_no_legacy_returns_false() {
        let (_dir, mut conn) = fresh_db().await;
        assert!(!migrate_legacy_with_conn(&mut conn).await.unwrap());
    }

    #[tokio::test]
    async fn migrate_with_legacy_creates_default_provider_and_activates() {
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        config::set_with_conn(&mut conn, LEGACY_KEY_API_KEY, "sk-legacy", &now)
            .await
            .unwrap();
        config::set_with_conn(
            &mut conn,
            LEGACY_KEY_BASE_URL,
            "https://api.deepseek.com",
            &now,
        )
        .await
        .unwrap();
        config::set_with_conn(&mut conn, LEGACY_KEY_MODEL, "deepseek-chat", &now)
            .await
            .unwrap();

        let migrated = migrate_legacy_with_conn(&mut conn).await.unwrap();
        assert!(migrated);

        let items = list_providers_with_conn(&mut conn).await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_active);
        assert_eq!(items[0].name, "默认 OpenAI");
        assert_eq!(items[0].base_url, "https://api.deepseek.com");

        let detail = get_provider_with_conn(&mut conn, &items[0].id)
            .await
            .unwrap();
        assert_eq!(detail.api_key, "sk-legacy");

        // B8：迁移成功后旧三键应被清；KEY_LEGACY_MIGRATED_AT 写入时间戳。
        for legacy in [LEGACY_KEY_API_KEY, LEGACY_KEY_BASE_URL, LEGACY_KEY_MODEL] {
            let v = config::get_with_conn(&mut conn, legacy).await.unwrap();
            assert!(v.is_none(), "迁移成功后 {legacy} 应被清；got {v:?}");
        }
        let stamp = config::get_with_conn(&mut conn, KEY_LEGACY_MIGRATED_AT)
            .await
            .unwrap();
        assert!(stamp.is_some(), "KEY_LEGACY_MIGRATED_AT 应写入时间戳");
    }

    #[tokio::test]
    async fn migrate_skips_when_active_already_set() {
        // 已经走过新 schema → 不再迁移
        let (_dir, mut conn) = fresh_db().await;
        let _id = add_provider_with_conn(&mut conn, req("A", "sk-1"))
            .await
            .unwrap();
        // 即使留了旧 KV，也不应再 migrate
        let now = Utc::now().to_rfc3339();
        config::set_with_conn(&mut conn, LEGACY_KEY_API_KEY, "sk-legacy", &now)
            .await
            .unwrap();
        let migrated = migrate_legacy_with_conn(&mut conn).await.unwrap();
        assert!(!migrated);
        let items = list_providers_with_conn(&mut conn).await.unwrap();
        assert_eq!(items.len(), 1, "已存在的 provider 不应被复制");
    }
}
