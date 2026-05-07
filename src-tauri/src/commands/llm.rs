// LLM IPC commands（#12，dev console 验证用）。
//
// 6 个 command（4 个 issue 字面 + 2 个 CUSTOM provider 扩展）：
// - set_openai_api_key(key)：写 config 表 KV（key=`llm:openai:api_key`）
// - get_openai_api_key_set()：返 boolean，永不返明文
// - set_openai_config({ api_key?, base_url?, model? })：partial update 三键（CUSTOM
//   provider 用：DeepSeek/Moonshot/Qwen/Ollama/任意 OpenAI 兼容端点）
// - get_openai_config()：返 { api_key_set, base_url, model }，base_url/model 缺省时
//   分别 fallback 到 OpenAI 默认 + gpt-4o-mini
// - chat_send_test(input)：读 config 三键构 OpenAIProvider 单轮调用，收完整字符串返；
//   同时 emit `chat:test:delta` 每 token 给前端可视化（M1 验收时观察 streaming 真实性）
// - cancel_test()：触发活跃 chat_send_test 的 CancellationToken
//
// **存储位置**：`config` 表 KV（与 #10 #11 同款偏离 issue body 字面"settings 表"，
// 因为 schema 没保留 settings 表，"27 表零迁移"D5 原则）。M3 G CryptoService 上线后
// api_key 迁到 `secrets` 表 DPAPI 加密（base_url / model 留 config 不加密）。
//
// **三键 config 命名空间**：`llm:openai:*`（M1 单 provider；M3 多 provider 改成
// `llm:<provider_id>:*` namespace，M3 时由 PreferenceService 处理迁移）。

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio_util::sync::CancellationToken;

use crate::services::config;
use crate::services::llm::{
    ChatMessage, ChatOptions, LLMError, LLMProvider, OpenAIProvider, Role, StreamDelta,
};

pub const CONFIG_KEY_OPENAI_API_KEY: &str = "llm:openai:api_key";
pub const CONFIG_KEY_OPENAI_BASE_URL: &str = "llm:openai:base_url";
pub const CONFIG_KEY_OPENAI_MODEL: &str = "llm:openai:model";
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-4o-mini";
pub const CHAT_TEST_DELTA_EVENT: &str = "chat:test:delta";

/// 活跃测试调用的 CancellationToken 槽（同时仅 1 个测试运行；多个并发以最新者为准）。
#[derive(Default)]
pub struct ActiveTestRegistry {
    pub current: Mutex<Option<CancellationToken>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpenaiConfigUpdate {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenaiConfigSnapshot {
    pub api_key_set: bool,
    pub base_url: String,
    pub model: String,
}

#[tauri::command]
pub async fn set_openai_api_key<R: Runtime>(app: AppHandle<R>, key: String) -> Result<(), String> {
    config::set(&app, CONFIG_KEY_OPENAI_API_KEY, &key)
        .await
        .map_err(|e| format!("write api key: {e}"))
}

#[tauri::command]
pub async fn get_openai_api_key_set<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let v = config::get(&app, CONFIG_KEY_OPENAI_API_KEY)
        .await
        .map_err(|e| format!("read api key: {e}"))?;
    Ok(v.as_deref().map(|s| !s.is_empty()).unwrap_or(false))
}

/// CUSTOM provider 用：partial update 三键（DeepSeek / Moonshot / Qwen / Ollama / 自定义）。
/// 任意字段 None 表示不动该键；空字符串等价于"清空"，下次读会 fallback 默认。
#[tauri::command]
pub async fn set_openai_config<R: Runtime>(
    app: AppHandle<R>,
    config: OpenaiConfigUpdate,
) -> Result<(), String> {
    if let Some(api_key) = &config.api_key {
        crate::services::config::set(&app, CONFIG_KEY_OPENAI_API_KEY, api_key)
            .await
            .map_err(|e| format!("write api_key: {e}"))?;
    }
    if let Some(base_url) = &config.base_url {
        crate::services::config::set(&app, CONFIG_KEY_OPENAI_BASE_URL, base_url)
            .await
            .map_err(|e| format!("write base_url: {e}"))?;
    }
    if let Some(model) = &config.model {
        crate::services::config::set(&app, CONFIG_KEY_OPENAI_MODEL, model)
            .await
            .map_err(|e| format!("write model: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_openai_config<R: Runtime>(
    app: AppHandle<R>,
) -> Result<OpenaiConfigSnapshot, String> {
    let api_key = config::get(&app, CONFIG_KEY_OPENAI_API_KEY)
        .await
        .map_err(|e| format!("read api_key: {e}"))?;
    let base_url = config::get(&app, CONFIG_KEY_OPENAI_BASE_URL)
        .await
        .map_err(|e| format!("read base_url: {e}"))?;
    let model = config::get(&app, CONFIG_KEY_OPENAI_MODEL)
        .await
        .map_err(|e| format!("read model: {e}"))?;
    Ok(OpenaiConfigSnapshot {
        api_key_set: api_key.as_deref().map(|s| !s.is_empty()).unwrap_or(false),
        base_url: base_url
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_OPENAI_BASE_URL.to_string()),
        model: model
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.to_string()),
    })
}

#[tauri::command]
pub async fn cancel_test(state: State<'_, ActiveTestRegistry>) -> Result<(), String> {
    // 短锁取出 token clone，再在锁外 cancel（避免阻塞 chat_send_test 的写入路径）
    let token = state
        .current
        .lock()
        .map_err(|e| format!("registry lock poisoned: {e}"))?
        .as_ref()
        .cloned();
    if let Some(token) = token {
        token.cancel();
    }
    Ok(())
}

#[tauri::command]
pub async fn chat_send_test<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, ActiveTestRegistry>,
    input: String,
) -> Result<String, String> {
    // 读 config 三键（api_key 必填；base_url / model 缺省 fallback 默认 OpenAI）
    let api_key = config::get(&app, CONFIG_KEY_OPENAI_API_KEY)
        .await
        .map_err(|e| format!("read api key: {e}"))?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "API Key 未设置；先调 set_openai_api_key('sk-...') 或 set_openai_config({api_key:'sk-...'})"
                .to_string()
        })?;
    let base_url = config::get(&app, CONFIG_KEY_OPENAI_BASE_URL)
        .await
        .map_err(|e| format!("read base_url: {e}"))?
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_OPENAI_BASE_URL.to_string());
    let model = config::get(&app, CONFIG_KEY_OPENAI_MODEL)
        .await
        .map_err(|e| format!("read model: {e}"))?
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.to_string());

    // CUSTOM provider id：M1 始终沿用 "openai"（trait id 不影响 wire format）；M3 多
    // provider 时由 ProviderRegistry 按用户配置切换 id（'openai' / 'deepseek' / ...）。
    let provider = OpenAIProvider::new("openai", &base_url, api_key, &model)
        .map_err(|e| format!("{e}"))?;

    let cancel = CancellationToken::new();
    {
        // 短锁登记当前 token，cancel_test 通过它发信号
        let mut slot = state
            .current
            .lock()
            .map_err(|e| format!("registry lock poisoned: {e}"))?;
        *slot = Some(cancel.clone());
    }

    // 收集流式 text delta 拼成完整字符串（M1 IPC 字面契约「返完整字符串」）
    // 同时 emit chat:test:delta 给前端实时可视化（M1 验收时观察是否真流式）
    let collected = std::sync::Arc::new(Mutex::new(String::new()));
    let collected_for_cb = collected.clone();
    let app_for_emit = app.clone();
    let on_delta: Box<dyn Fn(StreamDelta) + Send + Sync> = Box::new(move |delta| {
        if let StreamDelta::TextDelta(text) = &delta {
            if let Ok(mut buf) = collected_for_cb.lock() {
                buf.push_str(text);
            }
            let _ = app_for_emit.emit(CHAT_TEST_DELTA_EVENT, text.clone());
        }
        // ToolCallDelta / Finish：M1 chat_send_test 不接 tools，不会触发；忽略
    });

    let messages = vec![ChatMessage::text(Role::User, input)];
    let options = ChatOptions::default();

    let result = provider.chat_stream(messages, options, cancel, on_delta).await;

    // 清理 token 槽（无论成功失败；下次 chat_send_test 重新登记）
    {
        let mut slot = state
            .current
            .lock()
            .map_err(|e| format!("registry lock poisoned: {e}"))?;
        *slot = None;
    }

    match result {
        Ok(_finish) => {
            let s = collected
                .lock()
                .map_err(|e| format!("collect lock poisoned: {e}"))?
                .clone();
            Ok(s)
        }
        Err(e) => Err(format!("{}: {}", error_kind(&e), e)),
    }
}

/// 把 LLMError variant 名前缀到错误字符串（前端 split(': ', 2)[0] 拿 kind 做 UI 分支）。
fn error_kind(e: &LLMError) -> &'static str {
    match e {
        LLMError::Network(_) => "Network",
        LLMError::AuthFailed(_) => "AuthFailed",
        LLMError::RateLimit(_) => "RateLimit",
        LLMError::BadRequest(_) => "BadRequest",
        LLMError::ServerError(_) => "ServerError",
        LLMError::Cancelled => "Cancelled",
        LLMError::ParseError(_) => "ParseError",
    }
}

/// 启动期注册 state（lib.rs::setup 调用）。
pub fn setup<R: Runtime>(app: &AppHandle<R>) {
    app.manage(ActiveTestRegistry::default());
}
