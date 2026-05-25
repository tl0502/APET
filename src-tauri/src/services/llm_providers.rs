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
//
// Phase A0.5b 分发 gate 收尾（Spec §0.6 / §15.4 P0）：api_key 不再明文存 config 表，
// 而是 set/get/delete 走 SecretRepo（DPAPI 加密）+ config JSON 的 api_key 字段留 ""。
// 向后兼容：legacy 明文 record 仍可读（一次性 eprintln warning），Phase A1 UI 提供
// 重新输入入口才主动迁移。secret_repo 不可用时（测试 / Kernel 未注入）退化到 legacy
// 明文路径，不阻断用户操作。

use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Connection, SqliteConnection};
use tauri::{AppHandle, Manager, Runtime};
use thiserror::Error;
use ulid::Ulid;

use crate::kernel::repos::SecretRepo;
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

/// Phase A0.5b: SecretRepo entry key 命名约定，统一前缀 `llm_provider:<ulid>`。
/// 与 KEY_PROVIDER_PREFIX (`llm:provider:`) 是不同 namespace（前者 secrets 表 key，
/// 后者 config 表 key），命名分开避免误混。
fn secret_repo_key(provider_id: &str) -> String {
    format!("llm_provider:{provider_id}")
}

/// 从 Tauri AppHandle 拿 Kernel.secret_repo 引用。
/// Production: Kernel::boot 后由 lib.rs::setup 注入 `app.manage(kernel)`，永远 Some。
/// 测试: 没 Kernel state → None，调用方退化到 legacy 明文路径。
fn get_secret_repo<R: Runtime>(app: &AppHandle<R>) -> Option<Arc<SecretRepo>> {
    app.try_state::<crate::kernel::Kernel>()
        .map(|kernel| Arc::clone(&kernel.secret_repo))
}

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
    let secret_repo = get_secret_repo(app);
    let result = get_provider_with_conn_and_secret(&mut conn, secret_repo.as_ref(), &id).await?;
    conn.close().await?;
    Ok(result)
}

/// Phase A0.5b 后: 此 wrapper 仅作 backward-compat 保留, tests 直接调它走 legacy
/// 明文路径 (secret_repo = None)。production 走 `_and_secret` 变体。
#[cfg(test)]
pub(crate) async fn get_provider_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<ProviderDetail, LlmProviderError> {
    get_provider_with_conn_and_secret(conn, None, id).await
}

/// Phase A0.5b: 加密读路径。JSON 空 + secret 有 → 解密回填；JSON 非空 → 使用 legacy
/// 明文 + 一次性 warning；两边都空 → 未配置（保持 api_key 为 ""，与原行为一致）。
pub(crate) async fn get_provider_with_conn_and_secret(
    conn: &mut SqliteConnection,
    secret_repo: Option<&Arc<SecretRepo>>,
    id: &str,
) -> Result<ProviderDetail, LlmProviderError> {
    let raw = config::get_with_conn(conn, &provider_kv_key(id)).await?;
    let mut record: ProviderRecord = match raw {
        Some(s) => serde_json::from_str(&s)?,
        None => return Err(LlmProviderError::NotFound(id.to_string())),
    };
    enrich_record_from_secret(conn, secret_repo, id, &mut record).await?;
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

/// 内部 helper: 把 record.api_key 按 (legacy 明文 / secrets 解密 / 未配置) 三种来源填好。
/// 不报错（secret_repo.get 失败 / utf8 异常时 log warning，保留 empty），让调用方继续。
async fn enrich_record_from_secret(
    conn: &mut SqliteConnection,
    secret_repo: Option<&Arc<SecretRepo>>,
    id: &str,
    record: &mut ProviderRecord,
) -> Result<(), LlmProviderError> {
    if record.api_key.is_empty() {
        if let Some(repo) = secret_repo {
            match repo.get(conn, &secret_repo_key(id)).await {
                Ok(secret_value) => {
                    record.api_key = String::from_utf8(secret_value.0.clone()).map_err(|e| {
                        LlmProviderError::Database(format!("secret utf8: {e}"))
                    })?;
                }
                Err(crate::kernel::repos::secret_repo::SecretError::NotFound(_)) => {
                    // 没 secret entry 也没 JSON 明文 → 用户未配置 key, 保持 empty
                }
                Err(e) => {
                    eprintln!(
                        "[llm_providers] secret_repo.get failed for {id}: {e} — returning empty key"
                    );
                }
            }
        }
    } else {
        // record.api_key 非空 = legacy 明文。一次性 warning 但不主动迁移
        // （Phase A1 UI 重新输入入口才主动迁移）。
        eprintln!(
            "[llm_providers] LEGACY plaintext api_key detected for provider {id}; \
             consider re-entering via settings UI to migrate to DPAPI"
        );
    }
    Ok(())
}

/// 新建 provider；返回新 ULID。如果是首条，自动设为 active。
pub async fn add_provider<R: Runtime>(
    app: &AppHandle<R>,
    req: AddProviderRequest,
) -> Result<String, LlmProviderError> {
    let mut conn = open_app_db(app).await?;
    let secret_repo = get_secret_repo(app);
    let id = add_provider_with_conn_and_secret(&mut conn, secret_repo.as_ref(), req).await?;
    conn.close().await?;
    Ok(id)
}

/// Phase A0.5b 后: 此 wrapper 仅作 backward-compat 保留, tests + `migrate_legacy_with_conn`
/// 直接调它走 legacy 明文路径 (secret_repo = None)。production add 走 `_and_secret` 变体。
pub(crate) async fn add_provider_with_conn(
    conn: &mut SqliteConnection,
    req: AddProviderRequest,
) -> Result<String, LlmProviderError> {
    add_provider_with_conn_and_secret(conn, None, req).await
}

/// Phase A0.5b: 加密写路径。trimmed api_key 非空 + secret_repo 可用 → 加密入 secrets 表 +
/// JSON api_key 字段留 ""。fallback：加密失败或 secret_repo 不可用 → legacy 明文 +
/// eprintln warning（不阻断用户配置 provider 的能力）。
pub(crate) async fn add_provider_with_conn_and_secret(
    conn: &mut SqliteConnection,
    secret_repo: Option<&Arc<SecretRepo>>,
    req: AddProviderRequest,
) -> Result<String, LlmProviderError> {
    let name = validate_name(&req.name)?;
    let base_url = validate_required(&req.base_url, LlmProviderError::EmptyBaseUrl)?;
    let model = validate_required(&req.model, LlmProviderError::EmptyModel)?;
    let trimmed_key = req.api_key.trim().to_string();

    let id = Ulid::new().to_string();
    let now = Utc::now().to_rfc3339();

    // 加密 api_key 到 secrets 表 (Phase A0.5b 分发 gate);
    // 失败时 fallback 到 JSON 明文路径 + 日志告警 (向后兼容, 不阻断用户操作)。
    let json_api_key = encrypt_or_fallback(conn, secret_repo, &id, &trimmed_key).await;

    let record = ProviderRecord {
        name,
        api_key: json_api_key,
        base_url,
        model,
    };
    let json = serde_json::to_string(&record)?;
    config::set_with_conn(conn, &provider_kv_key(&id), &json, &now).await?;

    // 首条自动设 active；后续保持当前 active 不变
    let existing_active = config::get_with_conn(conn, KEY_ACTIVE_ID).await?;
    if existing_active.is_none() {
        config::set_with_conn(conn, KEY_ACTIVE_ID, &id, &now).await?;
    }
    Ok(id)
}

/// 内部 helper: trimmed_key + secret_repo → 返回该写进 JSON `api_key` 字段的值。
/// - Some(repo) + non-empty key → 加密入 secrets 表, 返回 ""（JSON 字段空 = 真值在 secrets 表）
/// - Some(repo) + empty key → 返回 ""（无需加密；调用方语义"未配置 key"）
/// - None secret_repo → 返回 trimmed_key.clone()（legacy 明文路径, 测试或 Kernel 未注入）
/// - 加密失败 → eprintln warning + 退化到 legacy 明文路径
async fn encrypt_or_fallback(
    conn: &mut SqliteConnection,
    secret_repo: Option<&Arc<SecretRepo>>,
    id: &str,
    trimmed_key: &str,
) -> String {
    match (secret_repo, trimmed_key.is_empty()) {
        (Some(repo), false) => {
            let secret_key = secret_repo_key(id);
            match repo.set(conn, &secret_key, trimmed_key.as_bytes()).await {
                Ok(()) => String::new(),
                Err(e) => {
                    eprintln!(
                        "[llm_providers] DPAPI encrypt failed for provider {id}: {e} — \
                         falling back to plaintext JSON (legacy compat path)"
                    );
                    trimmed_key.to_string()
                }
            }
        }
        (Some(_), true) => String::new(),
        (None, _) => trimmed_key.to_string(),
    }
}

/// 部分更新 provider（None 字段不动）。
pub async fn update_provider<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    req: UpdateProviderRequest,
) -> Result<(), LlmProviderError> {
    let id = validate_id(id)?;
    let mut conn = open_app_db(app).await?;
    let secret_repo = get_secret_repo(app);
    update_provider_with_conn_and_secret(&mut conn, secret_repo.as_ref(), &id, req).await?;
    conn.close().await?;
    Ok(())
}

/// Phase A0.5b 后: 此 wrapper 仅作 backward-compat 保留, tests 直接调它走 legacy
/// 明文路径 (secret_repo = None)。production 走 `_and_secret` 变体。
#[cfg(test)]
pub(crate) async fn update_provider_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
    req: UpdateProviderRequest,
) -> Result<(), LlmProviderError> {
    update_provider_with_conn_and_secret(conn, None, id, req).await
}

/// Phase A0.5b: 加密 update 路径。
/// - req.api_key = Some(non-empty) + secret_repo 有 → 加密入 secrets（overwrite）+ JSON 留 ""
/// - req.api_key = Some("") + secret_repo 有 → secrets 表 delete（best-effort）+ JSON 留 ""
/// - req.api_key = Some(new) + secret_repo 无 → 走 legacy 明文（与原行为同, 测试上下文）
/// - req.api_key = None → 完全不动 api_key
pub(crate) async fn update_provider_with_conn_and_secret(
    conn: &mut SqliteConnection,
    secret_repo: Option<&Arc<SecretRepo>>,
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
        let trimmed = api_key.trim().to_string();
        match (secret_repo, trimmed.is_empty()) {
            (Some(repo), false) => {
                // 新 key 加密入 secrets（overwrite 已有 entry）
                let secret_key = secret_repo_key(id);
                match repo.set(conn, &secret_key, trimmed.as_bytes()).await {
                    Ok(()) => {
                        record.api_key = String::new();
                    }
                    Err(e) => {
                        eprintln!(
                            "[llm_providers] DPAPI encrypt failed on update for {id}: {e} — \
                             falling back to plaintext JSON (legacy compat path)"
                        );
                        record.api_key = trimmed;
                    }
                }
            }
            (Some(repo), true) => {
                // 用户显式清空 key → 删 secrets entry（best-effort, NotFound 不报）
                let secret_key = secret_repo_key(id);
                match repo.delete(conn, &secret_key).await {
                    Ok(())
                    | Err(crate::kernel::repos::secret_repo::SecretError::NotFound(_)) => {}
                    Err(e) => {
                        eprintln!(
                            "[llm_providers] secret_repo.delete failed on clear for {id}: {e}"
                        );
                    }
                }
                record.api_key = String::new();
            }
            (None, _) => {
                // 测试 / Kernel 未注入: 走 legacy 明文路径（partial update：传 "" 也写入）
                record.api_key = trimmed;
            }
        }
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
    let secret_repo = get_secret_repo(app);
    delete_provider_with_conn_and_secret(&mut conn, secret_repo.as_ref(), &id).await?;
    conn.close().await?;
    Ok(())
}

/// Phase A0.5b 后: 此 wrapper 仅作 backward-compat 保留, tests 直接调它走 legacy
/// 明文路径 (secret_repo = None)。production 走 `_and_secret` 变体。
#[cfg(test)]
pub(crate) async fn delete_provider_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<(), LlmProviderError> {
    delete_provider_with_conn_and_secret(conn, None, id).await
}

/// Phase A0.5b: delete provider 同步清 secrets entry（best-effort，
/// legacy 明文 record 没 secret 行 → NotFound 直接忽略）。
pub(crate) async fn delete_provider_with_conn_and_secret(
    conn: &mut SqliteConnection,
    secret_repo: Option<&Arc<SecretRepo>>,
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

    // Phase A0.5b: 同步删 secrets entry（legacy 明文 record 没 secret 行 → NotFound 忽略）
    if let Some(repo) = secret_repo {
        let secret_key = secret_repo_key(id);
        match repo.delete(conn, &secret_key).await {
            Ok(()) | Err(crate::kernel::repos::secret_repo::SecretError::NotFound(_)) => {}
            Err(e) => {
                eprintln!(
                    "[llm_providers] secret_repo.delete failed for {id}: {e} — \
                     config entry already removed, secrets entry may leak"
                );
            }
        }
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
///
/// Phase A0.5b 后: ChatService 已切到 `get_active_record_with_conn_and_secret`,
/// 此 AppHandle wrapper 当前仅作 API 表面保留, 供未来 IPC / debug 命令使用。
#[allow(dead_code)]
pub async fn get_active_record<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<ProviderRecord>, LlmProviderError> {
    let mut conn = open_app_db(app).await?;
    let secret_repo = get_secret_repo(app);
    let result = get_active_record_with_conn_and_secret(&mut conn, secret_repo.as_ref()).await?;
    conn.close().await?;
    Ok(result)
}

/// Phase A0.5b 后: 此 wrapper 仅作 backward-compat 保留, tests 直接调它走 legacy
/// 明文路径 (secret_repo = None)。production 走 `_and_secret` 变体。
#[cfg(test)]
pub(crate) async fn get_active_record_with_conn(
    conn: &mut SqliteConnection,
) -> Result<Option<ProviderRecord>, LlmProviderError> {
    get_active_record_with_conn_and_secret(conn, None).await
}

/// Phase A0.5b: get_active_record + secret enrichment（与 get_provider 同语义）。
/// ChatService::build_provider_with_conn 真消费此路径，需保证 api_key 被解密回填。
pub(crate) async fn get_active_record_with_conn_and_secret(
    conn: &mut SqliteConnection,
    secret_repo: Option<&Arc<SecretRepo>>,
) -> Result<Option<ProviderRecord>, LlmProviderError> {
    let active_id = match config::get_with_conn(conn, KEY_ACTIVE_ID).await? {
        Some(v) if !v.is_empty() => v,
        _ => return Ok(None),
    };
    let raw = config::get_with_conn(conn, &provider_kv_key(&active_id)).await?;
    let mut record: ProviderRecord = match raw {
        Some(s) => serde_json::from_str(&s)?,
        None => return Ok(None),
    };
    enrich_record_from_secret(conn, secret_repo, &active_id, &mut record).await?;
    Ok(Some(record))
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

    // === Phase A0.5b: DPAPI 加密迁移测试 (Windows-only) ===
    //
    // 这 6 个测试覆盖:
    //  1. add_with_secret → secrets 表有 ciphertext + config JSON api_key 留空
    //  2. add_without_secret → 退化到 legacy 明文 JSON (向后兼容)
    //  3. get_with_secret → JSON 空 + secret 解密 → 返明文 api_key
    //  4. get_legacy_plaintext → JSON 直接含明文 → 用 AS-IS (warning 但不报错)
    //  5. update_with_secret → 覆写 secrets entry, 旧 ciphertext 被替换
    //  6. delete_with_secret → secrets entry 被同步清掉
    //
    // 非 Windows: DPAPI stub 报错; 6 测试需要真实加解密 round-trip, 故 gate 整段。

    #[cfg(target_os = "windows")]
    mod secret_migration {
        use super::*;
        use crate::kernel::crypto::DpapiCryptoService;
        use crate::kernel::repos::SecretRepo;
        use std::sync::Arc;

        fn make_repo() -> Arc<SecretRepo> {
            Arc::new(SecretRepo::new(Arc::new(DpapiCryptoService)))
        }

        /// 抓 config 表里那条 provider JSON value 的 api_key 字段。
        async fn read_json_api_key(conn: &mut SqliteConnection, id: &str) -> String {
            let raw = config::get_with_conn(conn, &provider_kv_key(id))
                .await
                .unwrap()
                .expect("provider entry should exist");
            let record: ProviderRecord = serde_json::from_str(&raw).unwrap();
            record.api_key
        }

        /// 检查 secrets 表是否有指定 key 的行。
        async fn secrets_row_exists(conn: &mut SqliteConnection, secret_key: &str) -> bool {
            let count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM secrets WHERE key = ?")
                    .bind(secret_key)
                    .fetch_one(&mut *conn)
                    .await
                    .unwrap();
            count.0 > 0
        }

        #[tokio::test]
        async fn add_provider_with_secret_writes_ciphertext_to_secrets_table() {
            // Distribution-gate 核心 spot-check: 加密路径下 JSON api_key 必须为空,
            // 真值只存在 secrets 表 ciphertext blob。
            let (_dir, mut conn) = fresh_db().await;
            let repo = make_repo();
            let id = add_provider_with_conn_and_secret(
                &mut conn,
                Some(&repo),
                req("OpenAI", "sk-secret-value-12345"),
            )
            .await
            .unwrap();

            // 1) JSON api_key 字段必须为 "" (明文不再入 config 表)
            let json_key = read_json_api_key(&mut conn, &id).await;
            assert_eq!(
                json_key, "",
                "Phase A0.5b distribution gate: JSON api_key 必须为空, 真值移至 secrets 表"
            );

            // 2) secrets 表有 row, 且 ciphertext 不是 plaintext
            let secret_key = secret_repo_key(&id);
            assert!(
                secrets_row_exists(&mut conn, &secret_key).await,
                "secrets 表应该有 llm_provider:<id> 的 row"
            );
            let ciphertext: Vec<u8> =
                sqlx::query_scalar("SELECT ciphertext FROM secrets WHERE key = ?")
                    .bind(&secret_key)
                    .fetch_one(&mut conn)
                    .await
                    .unwrap();
            let ciphertext_str = String::from_utf8_lossy(&ciphertext);
            assert!(
                !ciphertext_str.contains("sk-secret-value-12345"),
                "ciphertext 不应该含 plaintext substring"
            );
        }

        #[tokio::test]
        async fn add_provider_without_secret_writes_plaintext_legacy_path() {
            // 测试上下文: 没 secret_repo → 退化到 legacy 明文 JSON, 与 #12 行为一致。
            // 这保证现有 18 单测在 Phase A0.5b 后语义不变 (回归保护)。
            let (_dir, mut conn) = fresh_db().await;
            let id = add_provider_with_conn_and_secret(
                &mut conn,
                None,
                req("OpenAI", "sk-legacy-plaintext"),
            )
            .await
            .unwrap();

            // JSON api_key 应为明文 (legacy 路径)
            let json_key = read_json_api_key(&mut conn, &id).await;
            assert_eq!(json_key, "sk-legacy-plaintext");

            // secrets 表不应有 row
            let secret_key = secret_repo_key(&id);
            assert!(
                !secrets_row_exists(&mut conn, &secret_key).await,
                "无 secret_repo 时不应该写 secrets 表"
            );
        }

        #[tokio::test]
        async fn get_provider_decrypts_from_secrets_when_json_empty() {
            // 写入走加密 → 读回应自动解密回填 plaintext (round-trip)
            let (_dir, mut conn) = fresh_db().await;
            let repo = make_repo();
            let id = add_provider_with_conn_and_secret(
                &mut conn,
                Some(&repo),
                req("OpenAI", "sk-roundtrip-test"),
            )
            .await
            .unwrap();

            let detail =
                get_provider_with_conn_and_secret(&mut conn, Some(&repo), &id)
                    .await
                    .unwrap();
            assert_eq!(
                detail.api_key, "sk-roundtrip-test",
                "get_provider 应该从 secrets 表解密回填 api_key"
            );

            // get_active_record 同语义验证 (ChatService 真消费路径)
            let active = get_active_record_with_conn_and_secret(&mut conn, Some(&repo))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(active.api_key, "sk-roundtrip-test");
        }

        #[tokio::test]
        async fn get_provider_legacy_plaintext_path_still_works() {
            // 回归保护: 直接走 None secret_repo 写入 (模拟已有 legacy 安装),
            // 再用 secret_repo 读取应直接拿 JSON 里的明文 (warning 但不报错)。
            let (_dir, mut conn) = fresh_db().await;
            let repo = make_repo();
            // 用 None 写入 → 明文存 JSON
            let id = add_provider_with_conn_and_secret(
                &mut conn,
                None,
                req("LegacyProvider", "sk-legacy-key"),
            )
            .await
            .unwrap();

            // 即使用 Some(repo) 读取, legacy 明文路径仍生效 (record.api_key 非空 = legacy)
            let detail =
                get_provider_with_conn_and_secret(&mut conn, Some(&repo), &id)
                    .await
                    .unwrap();
            assert_eq!(
                detail.api_key, "sk-legacy-key",
                "legacy 明文 record 应继续可读"
            );

            // secrets 表不应有 entry (legacy 路径根本没写过)
            assert!(!secrets_row_exists(&mut conn, &secret_repo_key(&id)).await);
        }

        #[tokio::test]
        async fn update_provider_with_secret_overwrites_ciphertext() {
            // 二次 update 应 overwrite 已存的 secrets ciphertext, 而非追加。
            let (_dir, mut conn) = fresh_db().await;
            let repo = make_repo();
            let id = add_provider_with_conn_and_secret(
                &mut conn,
                Some(&repo),
                req("OpenAI", "sk-initial"),
            )
            .await
            .unwrap();

            // 拿初始 ciphertext
            let secret_key = secret_repo_key(&id);
            let ct_initial: Vec<u8> =
                sqlx::query_scalar("SELECT ciphertext FROM secrets WHERE key = ?")
                    .bind(&secret_key)
                    .fetch_one(&mut conn)
                    .await
                    .unwrap();

            // update 换 key
            update_provider_with_conn_and_secret(
                &mut conn,
                Some(&repo),
                &id,
                UpdateProviderRequest {
                    api_key: Some("sk-rotated".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

            // ciphertext 应不同 (新明文 + DPAPI nonce 双重保证)
            let ct_after: Vec<u8> =
                sqlx::query_scalar("SELECT ciphertext FROM secrets WHERE key = ?")
                    .bind(&secret_key)
                    .fetch_one(&mut conn)
                    .await
                    .unwrap();
            assert_ne!(
                ct_initial, ct_after,
                "update 后 ciphertext 应该被 overwrite"
            );

            // 解密回读应是新值
            let detail =
                get_provider_with_conn_and_secret(&mut conn, Some(&repo), &id)
                    .await
                    .unwrap();
            assert_eq!(detail.api_key, "sk-rotated");

            // JSON api_key 字段仍为空
            let json_key = read_json_api_key(&mut conn, &id).await;
            assert_eq!(json_key, "");

            // secrets 表应只有 1 行 (overwrite 而非追加)
            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM secrets WHERE key = ?")
                .bind(&secret_key)
                .fetch_one(&mut conn)
                .await
                .unwrap();
            assert_eq!(count.0, 1, "应只有一行 (overwrite 语义)");
        }

        #[tokio::test]
        async fn delete_provider_removes_secret_entry_too() {
            // 删 provider 时, secrets 表对应 row 也应被清掉, 避免泄漏密文 + 占用 disk。
            let (_dir, mut conn) = fresh_db().await;
            let repo = make_repo();
            let id = add_provider_with_conn_and_secret(
                &mut conn,
                Some(&repo),
                req("Solo", "sk-to-be-deleted"),
            )
            .await
            .unwrap();

            // 确认初始有 secrets row
            let secret_key = secret_repo_key(&id);
            assert!(secrets_row_exists(&mut conn, &secret_key).await);

            // 删 (唯一 active, 走 Bug 5 路径 — 允许删 + 清 KEY_ACTIVE_ID)
            delete_provider_with_conn_and_secret(&mut conn, Some(&repo), &id)
                .await
                .unwrap();

            // secrets entry 必须被清
            assert!(
                !secrets_row_exists(&mut conn, &secret_key).await,
                "delete_provider 应该同步删 secrets entry"
            );
        }
    }
}
