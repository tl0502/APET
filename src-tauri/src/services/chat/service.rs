// chat/service.rs — ChatService::{send, cancel, history}（M1 W2 #13，ADR-018 Layer 2）
//
// 业务编排：把 Persona / Nickname / Memory / Conversation / LLMProvider 串成
// "chat_send → 流式 token → chat:stream:done" 的真业务对话路径。
//
// 关键设计：
// - active_streams: Arc<Mutex<HashMap<message_id, CancellationToken>>>，cancel 用
// - send 流程：load active persona → ensure conv → 写 user msg → 拼 prompt → 现建
//   OpenAIProvider → 注册 token → chat_stream + on_delta emit chat:stream:delta + 累积 buffer
//   → 4 分支收尾（成功 / 取消 / 网络降级 / 其他错误）
// - 取消 = chat:stream:done finishReason='cancelled' + 已收 token 入库
// - 网络/server error = 抽 # 拒答 模板 + 写 mode='offline_rule' + emit done
// - 其他 error（AuthFailed / BadRequest / ParseError / RateLimit）= emit chat:stream:error，不入库
//
// Provider 注入：每次 send 都从 config 表读三键现建 OpenAIProvider（与 #12 chat_send_test
// 同模式）。用户改配置立即生效，无 hot reload 烦恼；M3 多 provider 时改成
// ProviderRegistry::resolve(active_id)。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::Serialize;
use sqlx::Connection;
use tauri::{AppHandle, Emitter, Runtime};
use tokio_util::sync::CancellationToken;
use ulid::Ulid;

use crate::services::chat::conversation::{ensure_active_conversation, update_last_activity};
use crate::services::chat::prompt::build_messages;
use crate::services::chat::ChatError;
use crate::services::config;
use crate::services::db::open_app_db;
use crate::services::llm::{
    ChatOptions, FinishReason, LLMError, LLMProvider, OpenAIProvider, StreamDelta,
};
use crate::services::memory::{
    insert_message_with_conn, list_messages_by_conversation, MessageRecord,
};
use crate::services::nickname::{get_pet_nickname, get_user_nickname};
use crate::services::persona::load_active_persona;

// === IPC 事件名（与前端 src/services/chat.ts onDelta/onDone/onError 契约对齐；Tauri 2.x 用 ':' 不允许 '.'）===
pub const CHAT_STREAM_DELTA_EVENT: &str = "chat:stream:delta";
pub const CHAT_STREAM_DONE_EVENT: &str = "chat:stream:done";
pub const CHAT_STREAM_ERROR_EVENT: &str = "chat:stream:error";

// === LLM 配置 KV key（与 #12 commands/llm.rs 共用 namespace；M3 多 provider 时改成 `llm:<id>:*`）===
pub const CONFIG_KEY_OPENAI_API_KEY: &str = "llm:openai:api_key";
pub const CONFIG_KEY_OPENAI_BASE_URL: &str = "llm:openai:base_url";
pub const CONFIG_KEY_OPENAI_MODEL: &str = "llm:openai:model";
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-4o-mini";

// === 业务常量 ===
/// 历史窗口（M1 N=10；M3 接 ContextManager 改为 token 预算自适应）。
const HISTORY_LIMIT: u32 = 10;
/// 全局兜底拒答（persona-design.md §7.5 降级链末端）。
const FALLBACK_REFUSAL: &str = "这个我现在没法陪你聊，要不我们换个话题？";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendResult {
    pub message_id: String,
    /// 当前轮所属的 conversation ID（caller 传 None 时由 ensure_active_conversation 决定）。
    /// dev 验收 + #14 ChatPanel 切会话都需要；省一次 IPC。
    pub conversation_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatStreamDeltaPayload {
    message_id: String,
    token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatStreamDonePayload {
    message_id: String,
    total_tokens: u32,
    finish_reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatStreamErrorPayload {
    message_id: String,
    error_kind: String,
    message: String,
}

#[derive(Default)]
pub struct ChatService {
    /// 在飞 chat_stream 的 cancel token map（key = assistant message_id ULID）。
    active_streams: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl ChatService {
    pub fn new() -> Self {
        Self::default()
    }

    /// chat_send 业务编排（详细流程见模块顶部注释）。
    ///
    /// 返 { message_id }（assistant 消息的 ULID）；前端用它对应 chat:stream:* events。
    pub async fn send<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        input: String,
        conversation_id: Option<String>,
    ) -> Result<SendResult, ChatError> {
        // 1. 加载 active persona + 解析或建 conversation
        let active_persona = load_active_persona(app).await?;
        let conv_id = match conversation_id {
            Some(id) => id, // caller 须保证 id 存在；否则 insert_message FK fail 会下游报错
            None => ensure_active_conversation(app, &active_persona.id).await?,
        };

        // 2. 写 user message（mode=online，role=user）— 先写入再读历史避免漏当前轮
        let user_record = insert_user_msg(app, &conv_id, &input).await?;

        // 3. 加载昵称 + 历史 N=10
        let user_nick = get_user_nickname(app).await?;
        let pet_nick = get_pet_nickname(app).await?;
        let history = list_messages_by_conversation(app, &conv_id, Some(HISTORY_LIMIT)).await?;
        // 排除刚插入的 user_record；当前 input 由 build_messages 末尾用 wrap_user_input 包装
        let history_excl_current: Vec<MessageRecord> = history
            .into_iter()
            .filter(|r| r.id != user_record.id)
            .collect();

        // 4. 拼 messages（含安全前缀占位 + 4 节人格 + 昵称 bullets + re-anchor + 历史 + 包装后的 user）
        let messages = build_messages(
            &active_persona,
            user_nick.as_deref(),
            &pet_nick,
            &history_excl_current,
            &input,
        )?;

        // 5. 现建 OpenAIProvider（与 #12 chat_send_test 同模式）
        let provider = build_provider(app).await?;

        // 6. 生成 assistant message ID + 注册 cancel token
        let assistant_id = Ulid::new().to_string();
        let cancel = CancellationToken::new();
        {
            let mut map = self
                .active_streams
                .lock()
                .map_err(|e| ChatError::Llm(format!("active_streams lock poisoned: {e}")))?;
            map.insert(assistant_id.clone(), cancel.clone());
        }

        // 7. 流式调用 + 收集 buffer + emit delta
        let buffer = Arc::new(Mutex::new(String::new()));
        let buffer_for_cb = buffer.clone();
        let app_for_emit = app.clone();
        let assistant_id_for_cb = assistant_id.clone();
        let on_delta: Box<dyn Fn(StreamDelta) + Send + Sync> = Box::new(move |delta| {
            if let StreamDelta::TextDelta(text) = &delta {
                if let Ok(mut buf) = buffer_for_cb.lock() {
                    buf.push_str(text);
                }
                let _ = app_for_emit.emit(
                    CHAT_STREAM_DELTA_EVENT,
                    ChatStreamDeltaPayload {
                        message_id: assistant_id_for_cb.clone(),
                        token: text.clone(),
                    },
                );
            }
            // ToolCallDelta / Finish：M1 不接 tools，不会触发；忽略
        });

        let stream_result = provider
            .chat_stream(messages, ChatOptions::default(), cancel.clone(), on_delta)
            .await;

        // 清理 token 槽（无论成功失败；下次 send 重新登记）
        {
            let mut map = self
                .active_streams
                .lock()
                .map_err(|e| ChatError::Llm(format!("active_streams lock poisoned: {e}")))?;
            map.remove(&assistant_id);
        }

        let collected = buffer
            .lock()
            .map_err(|e| ChatError::Llm(format!("buffer lock poisoned: {e}")))?
            .clone();

        // 8. 4 分支收尾：成功 / 取消 / 网络降级 / 其他错误
        match stream_result {
            Ok(finish) => {
                insert_assistant_msg(app, &assistant_id, &conv_id, &collected, "online").await?;
                update_last_activity(app, &conv_id).await?;
                emit_done(
                    app,
                    &assistant_id,
                    finish.usage.map(|u| u.total_tokens).unwrap_or(0),
                    finish_reason_to_str(&finish.reason),
                )?;
            }
            Err(LLMError::Cancelled) => {
                // 已收 partial 入库 + emit done finishReason='cancelled'
                insert_assistant_msg(app, &assistant_id, &conv_id, &collected, "online").await?;
                update_last_activity(app, &conv_id).await?;
                emit_done(app, &assistant_id, 0, "cancelled")?;
            }
            Err(LLMError::Network(_)) | Err(LLMError::ServerError(_)) => {
                // 离线降级：抽 # 拒答 模板（# 共情 / # 问候 与"网络断了"语义不贴）
                let templates = extract_refusal_templates(&active_persona.raw_markdown);
                let refusal = pick_refusal(&templates);
                insert_assistant_msg(app, &assistant_id, &conv_id, &refusal, "offline_rule")
                    .await?;
                update_last_activity(app, &conv_id).await?;
                // emit 一个 delta 让前端能渲染，再 emit done finishReason='offline_rule'
                emit_delta(app, &assistant_id, &refusal)?;
                emit_done(app, &assistant_id, 0, "offline_rule")?;
            }
            Err(e) => {
                // AuthFailed / RateLimit / BadRequest / ParseError → emit error 事件，不入库
                emit_error(app, &assistant_id, error_kind_str(&e), &e.to_string())?;
            }
        }

        Ok(SendResult {
            message_id: assistant_id,
            conversation_id: conv_id,
        })
    }

    /// 按 message_id 触发活跃 chat_stream 的取消。message_id 不存在 = no-op。
    pub fn cancel(&self, message_id: &str) -> Result<(), ChatError> {
        let map = self
            .active_streams
            .lock()
            .map_err(|e| ChatError::Llm(format!("active_streams lock poisoned: {e}")))?;
        if let Some(token) = map.get(message_id) {
            token.cancel();
        }
        Ok(())
    }

    /// 按 conversation_id + limit 列消息（按 created_at 升序）。
    pub async fn history<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        conversation_id: &str,
        limit: u32,
    ) -> Result<Vec<MessageRecord>, ChatError> {
        let records = list_messages_by_conversation(app, conversation_id, Some(limit)).await?;
        Ok(records)
    }
}

// === Helper functions ===

async fn insert_user_msg<R: Runtime>(
    app: &AppHandle<R>,
    conv_id: &str,
    content: &str,
) -> Result<MessageRecord, ChatError> {
    let record = MessageRecord {
        id: Ulid::new().to_string(),
        conversation_id: conv_id.to_string(),
        role: "user".to_string(),
        content: content.to_string(),
        mode: "online".to_string(),
        created_at: Utc::now().to_rfc3339(),
    };
    let mut conn = open_app_db(app).await?;
    insert_message_with_conn(&mut conn, &record).await?;
    conn.close().await?;
    Ok(record)
}

async fn insert_assistant_msg<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    conv_id: &str,
    content: &str,
    mode: &str,
) -> Result<(), ChatError> {
    let record = MessageRecord {
        id: id.to_string(),
        conversation_id: conv_id.to_string(),
        role: "assistant".to_string(),
        content: content.to_string(),
        mode: mode.to_string(),
        created_at: Utc::now().to_rfc3339(),
    };
    let mut conn = open_app_db(app).await?;
    insert_message_with_conn(&mut conn, &record).await?;
    conn.close().await?;
    Ok(())
}

async fn build_provider<R: Runtime>(app: &AppHandle<R>) -> Result<OpenAIProvider, ChatError> {
    let api_key = config::get(app, CONFIG_KEY_OPENAI_API_KEY)
        .await?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ChatError::Llm(
                "API Key 未设置；先调 set_openai_api_key('sk-...') 或 set_openai_config({api_key:'sk-...'})".to_string()
            )
        })?;
    let base_url = config::get(app, CONFIG_KEY_OPENAI_BASE_URL)
        .await?
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_OPENAI_BASE_URL.to_string());
    let model = config::get(app, CONFIG_KEY_OPENAI_MODEL)
        .await?
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.to_string());
    OpenAIProvider::new("openai", &base_url, api_key, &model)
        .map_err(|e| ChatError::Llm(format!("provider init: {e}")))
}

fn emit_delta<R: Runtime>(
    app: &AppHandle<R>,
    message_id: &str,
    token: &str,
) -> Result<(), ChatError> {
    app.emit(
        CHAT_STREAM_DELTA_EVENT,
        ChatStreamDeltaPayload {
            message_id: message_id.to_string(),
            token: token.to_string(),
        },
    )
    .map_err(|e| ChatError::Llm(format!("emit delta: {e}")))
}

fn emit_done<R: Runtime>(
    app: &AppHandle<R>,
    message_id: &str,
    total_tokens: u32,
    finish_reason: &str,
) -> Result<(), ChatError> {
    app.emit(
        CHAT_STREAM_DONE_EVENT,
        ChatStreamDonePayload {
            message_id: message_id.to_string(),
            total_tokens,
            finish_reason: finish_reason.to_string(),
        },
    )
    .map_err(|e| ChatError::Llm(format!("emit done: {e}")))
}

fn emit_error<R: Runtime>(
    app: &AppHandle<R>,
    message_id: &str,
    error_kind: &str,
    message: &str,
) -> Result<(), ChatError> {
    app.emit(
        CHAT_STREAM_ERROR_EVENT,
        ChatStreamErrorPayload {
            message_id: message_id.to_string(),
            error_kind: error_kind.to_string(),
            message: message.to_string(),
        },
    )
    .map_err(|e| ChatError::Llm(format!("emit error: {e}")))
}

fn finish_reason_to_str(r: &FinishReason) -> &'static str {
    match r {
        FinishReason::Stop => "stop",
        FinishReason::Length => "length",
        FinishReason::ToolCalls => "tool_calls",
        FinishReason::ContentFilter => "content_filter",
        FinishReason::Error => "error",
    }
}

fn error_kind_str(e: &LLMError) -> &'static str {
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

/// 从 .soul.md raw_markdown 抽 # 离线模板 / ## 拒答 子节的 bullet items。
///
/// 找不到任何模板返空 Vec；调用方 pick_refusal 兜底全局字符串。
pub(crate) fn extract_refusal_templates(raw_md: &str) -> Vec<String> {
    let mut in_offline = false;
    let mut in_refusal = false;
    let mut templates = Vec::new();

    for line in raw_md.lines() {
        // H1 切换（"# " 开头但不是 "## "）
        if line.starts_with("# ") {
            let body = line.trim_start_matches('#').trim();
            in_offline = body.starts_with("离线模板");
            in_refusal = false;
            continue;
        }

        // H2 切换（仅 in_offline 时关注）
        if in_offline && line.starts_with("## ") {
            let body = line.trim_start_matches('#').trim();
            in_refusal = body.starts_with("拒答") || body.starts_with("Refusal");
            continue;
        }

        // 抽 bullet（- 开头）
        if in_refusal {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("- ") {
                let item = rest.trim();
                if !item.is_empty() {
                    templates.push(item.to_string());
                }
            }
        }
    }

    templates
}

/// 用 nano-time 取模做轻量随机抽样，避免引 rand crate。
pub(crate) fn pick_refusal(templates: &[String]) -> String {
    if templates.is_empty() {
        return FALLBACK_REFUSAL.to_string();
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as usize)
        .unwrap_or(0);
    let idx = nanos % templates.len();
    templates[idx].clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOMO_RAW: &str = include_str!("../../../personas/_builtin/momo.soul.md");

    #[test]
    fn extract_refusal_from_momo_returns_three_items() {
        // momo.soul.md ## 拒答 / Refusal 池有 3 条
        let templates = extract_refusal_templates(MOMO_RAW);
        assert_eq!(
            templates.len(),
            3,
            "momo 拒答模板池应有 3 条；got {} items: {:?}",
            templates.len(),
            templates
        );
        assert!(templates.iter().any(|s| s.contains("不太擅长")));
        assert!(templates.iter().any(|s| s.contains("陪你想想其他的")));
        assert!(templates.iter().any(|s| s.contains("超出我能帮的范围")));
    }

    #[test]
    fn extract_refusal_returns_empty_when_no_offline_section() {
        let raw = "# 身份\nx\n\n# 性格\n- y\n\n# 能力\n- z\n\n# 行为规则\n## Do\n- a";
        let templates = extract_refusal_templates(raw);
        assert!(templates.is_empty());
    }

    #[test]
    fn extract_refusal_returns_empty_when_no_refusal_subsection() {
        let raw = "# 离线模板\n## 共情 / Empathy\n- 嗯\n## 问候 / Greeting\n- 嘿";
        let templates = extract_refusal_templates(raw);
        assert!(
            templates.is_empty(),
            "no ## 拒答 subsection should yield empty"
        );
    }

    #[test]
    fn extract_refusal_skips_when_other_h2_starts() {
        // ## 拒答 后跟 ## 调侃，调侃的 bullet 不能混入
        let raw = "# 离线模板\n## 拒答 / Refusal\n- 拒答1\n- 拒答2\n## 调侃 / Banter\n- 调侃1";
        let templates = extract_refusal_templates(raw);
        assert_eq!(templates.len(), 2);
        assert!(templates.iter().all(|s| !s.contains("调侃")));
    }

    #[test]
    fn extract_refusal_terminates_at_next_h1() {
        // ## 拒答 后跟 # 反应配置（H1），反应配置的 bullet 不能混入
        let raw = "# 离线模板\n## 拒答 / Refusal\n- 拒答1\n# 反应配置\n- click.head: x";
        let templates = extract_refusal_templates(raw);
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0], "拒答1");
    }

    #[test]
    fn pick_refusal_falls_back_when_empty() {
        let result = pick_refusal(&[]);
        assert_eq!(result, FALLBACK_REFUSAL);
    }

    #[test]
    fn pick_refusal_returns_one_of_templates() {
        let templates = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let picked = pick_refusal(&templates);
        assert!(templates.contains(&picked));
    }

    #[test]
    fn finish_reason_to_str_maps_all_variants() {
        assert_eq!(finish_reason_to_str(&FinishReason::Stop), "stop");
        assert_eq!(finish_reason_to_str(&FinishReason::Length), "length");
        assert_eq!(finish_reason_to_str(&FinishReason::ToolCalls), "tool_calls");
        assert_eq!(
            finish_reason_to_str(&FinishReason::ContentFilter),
            "content_filter"
        );
        assert_eq!(finish_reason_to_str(&FinishReason::Error), "error");
    }

    #[test]
    fn error_kind_str_maps_all_variants() {
        assert_eq!(error_kind_str(&LLMError::Network("x".into())), "Network");
        assert_eq!(
            error_kind_str(&LLMError::AuthFailed("x".into())),
            "AuthFailed"
        );
        assert_eq!(
            error_kind_str(&LLMError::RateLimit("x".into())),
            "RateLimit"
        );
        assert_eq!(
            error_kind_str(&LLMError::BadRequest("x".into())),
            "BadRequest"
        );
        assert_eq!(
            error_kind_str(&LLMError::ServerError("x".into())),
            "ServerError"
        );
        assert_eq!(error_kind_str(&LLMError::Cancelled), "Cancelled");
        assert_eq!(
            error_kind_str(&LLMError::ParseError("x".into())),
            "ParseError"
        );
    }

    #[test]
    fn cancel_unknown_message_id_is_noop() {
        let svc = ChatService::new();
        let result = svc.cancel("non-existent");
        assert!(result.is_ok(), "cancel on unknown id must be no-op");
    }

    #[test]
    fn cancel_existing_token_triggers_it() {
        let svc = ChatService::new();
        let token = CancellationToken::new();
        {
            let mut map = svc.active_streams.lock().unwrap();
            map.insert("test-id".to_string(), token.clone());
        }
        assert!(!token.is_cancelled());
        svc.cancel("test-id").unwrap();
        assert!(token.is_cancelled());
    }
}
