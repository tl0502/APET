// LLM IPC commands（#12，dev console 验证用）。
//
// 4 个 command，对应 issue #12 验收标准：
// - set_openai_api_key(key)：写 config 表 KV（key=`llm:openai:api_key`）
// - get_openai_api_key_set()：返 boolean，永不返明文（dev DevTools 也不能拿）
// - chat_send_test(input)：单轮 LLM 调用，收完整字符串返；同时 emit `chat:test:delta`
//   每 token 给前端可视化（M1 验收时观察 streaming 是否真实发生）
// - cancel_test()：触发活跃 chat_send_test 的 CancellationToken
//
// **API key 存储位置**：`config` 表 KV（与 #10 #11 同款偏离 issue body 字面"settings 表"，
// 因为 schema 没保留 settings 表，"27 表零迁移"D5 原则）。M3 G CryptoService 上线后
// 迁移到 `secrets` 表 DPAPI 加密（ADR-005 / ADR-018）。
//
// **base_url / model 暂硬编码**：M1 issue body 只允许 set_openai_api_key 一个写入点；
// 测 DeepSeek / Moonshot / Qwen 用户可手动 sqlite cli INSERT INTO config，或等 #13
// ChatService MVP 上线后走 settings 面板 Provider tab UI。

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio_util::sync::CancellationToken;

use crate::services::config;
use crate::services::llm::{
    ChatMessage, ChatOptions, LLMError, LLMProvider, OpenAIProvider, Role, StreamDelta,
};

pub const CONFIG_KEY_OPENAI_API_KEY: &str = "llm:openai:api_key";
pub const CHAT_TEST_DELTA_EVENT: &str = "chat:test:delta";

/// 活跃测试调用的 CancellationToken 槽（同时仅 1 个测试运行；多个并发以最新者为准）。
#[derive(Default)]
pub struct ActiveTestRegistry {
    pub current: Mutex<Option<CancellationToken>>,
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
    let api_key = config::get(&app, CONFIG_KEY_OPENAI_API_KEY)
        .await
        .map_err(|e| format!("read api key: {e}"))?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "API Key 未设置；先调 set_openai_api_key('sk-...') 写入".to_string()
        })?;

    let provider = OpenAIProvider::openai(api_key).map_err(|e| format!("{e}"))?;

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

/// 把 LLMError variant 名前缀到错误字符串（前端可 split(': ')[0] 拿 kind 做 UI 分支）。
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
