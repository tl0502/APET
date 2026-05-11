// LLM Providers IPC commands（参考 cc-switch UI；与 #12 单 namespace 路径冲突以本设计为准）
//
// 7 个 commands：
// - llm_list_providers() → Vec<ProviderListItem>（不含 api_key 明文）
// - llm_get_provider(id) → ProviderDetail（含 api_key，给 edit 弹窗用）
// - llm_add_provider({name, api_key, base_url, model}) → 新 ULID
//     首条自动设为 active；后续保持当前 active 不变
// - llm_update_provider(id, partial) → ()，None 字段不动
// - llm_delete_provider(id) → ()，激活的不允许删
// - llm_activate_provider(id) → ()，id 不存在报 NotFound
// - llm_test_provider(id) → 用对应配置发"你好"测试连通；不影响 active
//
// dev console 验证：
//   await window.__TAURI__.core.invoke('llm_list_providers')
//   const id = await window.__TAURI__.core.invoke('llm_add_provider', {
//     req: { name: 'OpenAI', apiKey: 'sk-...', baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini' }
//   })
//   await window.__TAURI__.core.invoke('llm_test_provider', { id })

use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio_util::sync::CancellationToken;

use crate::services::llm::probe::probe_models;
use crate::services::llm::{
    ChatMessage, ChatOptions, LLMError, LLMProvider, OpenAIProvider, Role, StreamDelta,
};
use crate::services::llm_providers::{
    self, AddProviderRequest, ProviderDetail, ProviderListItem, UpdateProviderRequest,
};

/// 与 #12 chat_send_test 同款：dev 期前端可监听此 event 看流式 token。
pub const LLM_TEST_DELTA_EVENT: &str = "llm:test:delta";

/// 活跃测试连通的 cancel token（同时仅 1 个；新调用抢占旧的）。
/// 沿用 #12 ActiveTestRegistry 模式但本模块自治，避免与已删除的旧 IPC 耦合。
#[derive(Default)]
pub struct LlmTestRegistry {
    pub current: Mutex<Option<CancellationToken>>,
}

#[tauri::command]
pub async fn llm_list_providers<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<ProviderListItem>, String> {
    llm_providers::list_providers(&app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn llm_get_provider<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<ProviderDetail, String> {
    llm_providers::get_provider(&app, &id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn llm_add_provider<R: Runtime>(
    app: AppHandle<R>,
    req: AddProviderRequest,
) -> Result<String, String> {
    llm_providers::add_provider(&app, req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn llm_update_provider<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    req: UpdateProviderRequest,
) -> Result<(), String> {
    llm_providers::update_provider(&app, &id, req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn llm_delete_provider<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    llm_providers::delete_provider(&app, &id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn llm_activate_provider<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<(), String> {
    llm_providers::activate_provider(&app, &id)
        .await
        .map_err(|e| e.to_string())
}

/// 用指定 provider id 的配置发"你好"测试连通；不影响当前 active。
///
/// 返回 LLM 完整回复字符串（前端用前 40 字做 toast preview）。
/// 错误形如 "AuthFailed: ..."（前端 split(': ', 2)[0] 拿 kind 做 UI 分支，与 #12 兼容）。
#[tauri::command]
pub async fn llm_test_provider<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LlmTestRegistry>,
    id: String,
) -> Result<String, String> {
    let detail = llm_providers::get_provider(&app, &id)
        .await
        .map_err(|e| e.to_string())?;
    if detail.api_key.is_empty() {
        return Err("BadRequest: API Key 未填写".to_string());
    }

    let provider = OpenAIProvider::new("openai", &detail.base_url, detail.api_key, &detail.model)
        .map_err(|e| format!("BadRequest: provider init: {e}"))?;

    let cancel = CancellationToken::new();
    {
        let mut slot = state.current.lock();
        // 真抢占：先 cancel 旧 token 让前一次测试连通停下来，再覆盖槽位。
        // 之前的 `*slot = Some(cancel.clone())` 只覆盖引用、未 cancel 旧的 → 前
        // 一次流式继续跑到底，浪费带宽 + 两路 token 走同一个 LLM_TEST_DELTA_EVENT。
        if let Some(prev) = slot.replace(cancel.clone()) {
            prev.cancel();
        }
    }

    let collected = Arc::new(Mutex::new(String::new()));
    let collected_for_cb = collected.clone();
    let app_for_emit = app.clone();
    let on_delta: Box<dyn Fn(StreamDelta) + Send + Sync> = Box::new(move |delta| {
        if let StreamDelta::TextDelta(text) = &delta {
            collected_for_cb.lock().push_str(text);
            let _ = app_for_emit.emit(LLM_TEST_DELTA_EVENT, text.clone());
        }
    });

    let messages = vec![ChatMessage::text(Role::User, "你好".to_string())];
    let options = ChatOptions::default();

    let result = provider
        .chat_stream(messages, options, cancel.clone(), on_delta)
        .await;

    {
        let mut slot = state.current.lock();
        // 自然完成（未被抢占）才清槽。被抢占时 cancel 已被 cancel()，槽里是新 token。
        // 用 is_cancelled() 区分：我自己 cancel 的从未发生（test 路径不主动 cancel）；
        // 槽被新调用 take 时旧 cancel 被新调用 cancel() → is_cancelled=true → 不动槽。
        if !cancel.is_cancelled() {
            *slot = None;
        }
    }

    match result {
        Ok(_finish) => {
            let s = collected.lock().clone();
            // #9：collected 为空时（content_filter / 模型直接 finish 没产 token / Ollama
            // 模型未加载等），不返空字符串避免前端 toast 显示"连通成功："（冒号后空白）。
            if s.is_empty() {
                Ok("(模型返回空内容；连接 OK)".to_string())
            } else {
                Ok(s)
            }
        }
        Err(e) => Err(format!("{}: {}", error_kind(&e), e)),
    }
}

fn error_kind(e: &LLMError) -> &'static str {
    match e {
        LLMError::Network(_) => "Network",
        LLMError::AuthFailed(_) => "AuthFailed",
        LLMError::RateLimit(_) => "RateLimit",
        LLMError::BadRequest(_) => "BadRequest",
        LLMError::ServerError(_) => "ServerError",
        LLMError::Cancelled { .. } => "Cancelled",
        LLMError::ParseError(_) => "ParseError",
    }
}

/// 探测 provider 的 /models 端点（OpenAI 兼容协议）；返模型 id 列表。
///
/// 不读 DB / 不依赖已保存的 provider —— ProviderDrawer 创建模式首次也能用：
/// 用户填了 baseUrl + apiKey 后立即点探测，拿真实列表填 dropdown。
///
/// 错误格式同 `llm_test_provider`："AuthFailed: ..." 等；前端 split kind 做 UI 分支。
#[tauri::command]
pub async fn llm_probe_models(
    #[allow(non_snake_case)] baseUrl: String,
    #[allow(non_snake_case)] apiKey: String,
) -> Result<Vec<String>, String> {
    let base_url = baseUrl.trim();
    if base_url.is_empty() {
        return Err("BadRequest: base_url 不能为空".to_string());
    }
    probe_models(base_url, apiKey.trim())
        .await
        .map_err(|e| format!("{}: {}", error_kind(&e), e))
}

/// 启动期注册 state（lib.rs::setup 调用）。
pub fn setup<R: Runtime>(app: &AppHandle<R>) {
    app.manage(LlmTestRegistry::default());
}
