// chat/service.rs — ChatService::{prepare, run_stream, cancel, history}
// （M1 W2 #13 → 修正：用 tauri::ipc::Channel 替代全局 emit；详 plan
// `c-issue-13-https-github-com-tl0502-apet-ancient-moth`）
//
// 业务编排：把 Persona / Nickname / Memory / Conversation / LLMProvider 串成
// "chat_send → prepare 同步返 → spawn run_stream 流式 token → channel.send(Done)"
// 的真业务对话路径。
//
// 关键设计（v2 修正版）：
// - active_streams: Arc<Mutex<HashMap<message_id, CancellationToken>>>，cancel 用
// - prepare 同步：active persona / ensure conv / 写 user msg / 拼 prompt / build_provider /
//   生成 assistant_id / 注册 cancel token；返 PreparedSend
// - run_stream 异步：消费 PreparedSend 跑流式；4 分支收尾通过 channel.send(StreamEvent::*) 回前端
// - 取消 = StreamEvent::Done finishReason='cancelled' + 已收 token 入库
// - 网络/server error = 抽 # 拒答 模板 + 写 mode='offline_rule' + send Delta+Done
// - 其他 error（AuthFailed / BadRequest / ParseError / RateLimit）= send Error，不入库
//
// Provider 注入：每次 prepare 都从 config 表读三键现建 OpenAIProvider（与 #12 chat_send_test
// 同模式）。用户改配置立即生效，无 hot reload 烦恼；M3 多 provider 时改成
// ProviderRegistry::resolve(active_id)。
//
// 为什么用 ipc::Channel 而非 app.emit（修正 #13 原契约）：
// - 老契约 chat_send 是 async fn 直到流式结束才 resolve → 前端拿不到 assistant_id 全程
//   → cancel 按钮成死按钮、切换会话被锁
// - Channel 自带 scope（每个 invoke 一条），不需 messageId 路由；类型安全；并发隔离

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::Serialize;
use sqlx::Connection;
use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime};
use tokio_util::sync::CancellationToken;
use ulid::Ulid;

use crate::services::chat::conversation::{ensure_active_conversation, update_last_activity};
use crate::services::chat::prompt::build_messages;
use crate::services::chat::ChatError;
use crate::services::db::open_app_db;
use crate::services::llm::{
    ChatMessage, ChatOptions, FinishReason, LLMError, LLMProvider, OpenAIProvider, StreamDelta,
};
use crate::services::llm_providers;
use crate::services::memory::{
    insert_message_with_conn, list_messages_by_conversation, MessageRecord,
};
use crate::services::nickname::{get_pet_nickname, get_user_nickname};
use crate::services::persona::{load_active_persona, PersonaSummary};

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
    /// dev 验收 + ChatPanel 切会话都需要；省一次 IPC。
    pub conversation_id: String,
}

/// 流式事件 —— 通过 `tauri::ipc::Channel<StreamEvent>` 发回前端。
///
/// 序列化形态（前端 onmessage 拿到的 JSON）：
/// - `{ "type": "delta", "token": "你" }`
/// - `{ "type": "done", "totalTokens": 42, "finishReason": "stop" }`
/// - `{ "type": "error", "errorKind": "AuthFailed", "message": "401 ..." }`
///
/// finishReason 取值见 finish_reason_to_str：stop / length / tool_calls / content_filter /
/// error / cancelled / offline_rule。
///
/// channel 自带 scope（一个 invoke 一条），故 payload 不带 messageId（前端从 SendResult
/// 拿到的 messageId 就是这条 channel 服务的 assistant 消息）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum StreamEvent {
    Delta {
        token: String,
    },
    #[serde(rename_all = "camelCase")]
    Done {
        total_tokens: u32,
        finish_reason: String,
    },
    #[serde(rename_all = "camelCase")]
    Error {
        error_kind: String,
        message: String,
    },
}

/// prepare 阶段产出，run_stream 消费。
///
/// 把所有"需要 await DB / IPC"的工作集中到 prepare（同步返），把"需要长时间跑流式"的
/// 工作留给 spawn 出去的 run_stream。
pub struct PreparedSend {
    pub assistant_id: String,
    pub conv_id: String,
    pub persona: PersonaSummary,
    pub messages: Vec<ChatMessage>,
    pub provider: OpenAIProvider,
    pub cancel_token: CancellationToken,
}

#[derive(Default, Clone)]
pub struct ChatService {
    /// 在飞 chat_stream 的 cancel token map（key = assistant message_id ULID）。
    /// 用 Arc<Mutex<...>> 让 ChatService 可 Clone 进 spawn 出去的 task。
    active_streams: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl ChatService {
    pub fn new() -> Self {
        Self::default()
    }

    /// chat_send 同步阶段：解析 conv / 写 user msg / 拼 prompt / 现建 provider /
    /// 生成 assistant_id / 注册 cancel token；返 PreparedSend 给上层 spawn。
    pub async fn prepare<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        input: String,
        conversation_id: Option<String>,
    ) -> Result<PreparedSend, ChatError> {
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

        Ok(PreparedSend {
            assistant_id,
            conv_id,
            persona: active_persona,
            messages,
            provider,
            cancel_token: cancel,
        })
    }

    /// chat_send 异步阶段：跑流式 + 4 分支收尾。
    ///
    /// 内部不返 Result —— 所有错误通过 channel.send(StreamEvent::Error) 回前端；
    /// channel send 失败仅 log，因为 spawn 出去后不再有调用者能 propagate。
    /// 必保证 channel 末尾要么 Done 要么 Error 一次（前端据此清 currentStreamId）。
    pub async fn run_stream<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        prepared: PreparedSend,
        channel: Channel<StreamEvent>,
    ) {
        let PreparedSend {
            assistant_id,
            conv_id,
            persona,
            messages,
            provider,
            cancel_token,
        } = prepared;

        // 流式调用 + 收集 buffer + send Delta
        let buffer = Arc::new(Mutex::new(String::new()));
        let buffer_for_cb = buffer.clone();
        let channel_for_cb = channel.clone();
        let on_delta: Box<dyn Fn(StreamDelta) + Send + Sync> = Box::new(move |delta| {
            if let StreamDelta::TextDelta(text) = &delta {
                if let Ok(mut buf) = buffer_for_cb.lock() {
                    buf.push_str(text);
                }
                if let Err(e) = channel_for_cb.send(StreamEvent::Delta {
                    token: text.clone(),
                }) {
                    eprintln!("[chat] channel send Delta failed: {e}");
                }
            }
            // ToolCallDelta / Finish：M1 不接 tools，不会触发；忽略
        });

        let stream_result = provider
            .chat_stream(messages, ChatOptions::default(), cancel_token, on_delta)
            .await;

        // 清理 token 槽（无论成功失败；下次 prepare 重新登记）
        if let Ok(mut map) = self.active_streams.lock() {
            map.remove(&assistant_id);
        }

        let collected = match buffer.lock() {
            Ok(b) => b.clone(),
            Err(e) => {
                let _ = channel.send(StreamEvent::Error {
                    error_kind: "InternalError".to_string(),
                    message: format!("buffer lock poisoned: {e}"),
                });
                return;
            }
        };

        // 4 分支收尾：成功 / 取消 / 网络降级 / 其他错误
        match stream_result {
            Ok(finish) => {
                if let Err(e) =
                    insert_assistant_msg(app, &assistant_id, &conv_id, &collected, "online").await
                {
                    let _ = channel.send(StreamEvent::Error {
                        error_kind: "DbError".to_string(),
                        message: e.to_string(),
                    });
                    return;
                }
                if let Err(e) = update_last_activity(app, &conv_id).await {
                    eprintln!("[chat] update_last_activity failed: {e}");
                }
                let _ = channel.send(StreamEvent::Done {
                    total_tokens: finish.usage.map(|u| u.total_tokens).unwrap_or(0),
                    finish_reason: finish_reason_to_str(&finish.reason).to_string(),
                });
            }
            Err(LLMError::Cancelled) => {
                // 已收 partial 入库 + Done finishReason='cancelled'
                if let Err(e) =
                    insert_assistant_msg(app, &assistant_id, &conv_id, &collected, "online").await
                {
                    let _ = channel.send(StreamEvent::Error {
                        error_kind: "DbError".to_string(),
                        message: e.to_string(),
                    });
                    return;
                }
                if let Err(e) = update_last_activity(app, &conv_id).await {
                    eprintln!("[chat] update_last_activity failed: {e}");
                }
                let _ = channel.send(StreamEvent::Done {
                    total_tokens: 0,
                    finish_reason: "cancelled".to_string(),
                });
            }
            Err(LLMError::Network(_)) | Err(LLMError::ServerError(_)) => {
                // 离线降级：抽 # 拒答 模板（# 共情 / # 问候 与"网络断了"语义不贴）
                let templates = extract_refusal_templates(&persona.raw_markdown);
                let refusal = pick_refusal(&templates);
                if let Err(e) =
                    insert_assistant_msg(app, &assistant_id, &conv_id, &refusal, "offline_rule")
                        .await
                {
                    let _ = channel.send(StreamEvent::Error {
                        error_kind: "DbError".to_string(),
                        message: e.to_string(),
                    });
                    return;
                }
                if let Err(e) = update_last_activity(app, &conv_id).await {
                    eprintln!("[chat] update_last_activity failed: {e}");
                }
                // send 一个 Delta 让前端能渲染，再 send Done finishReason='offline_rule'
                let _ = channel.send(StreamEvent::Delta {
                    token: refusal.clone(),
                });
                let _ = channel.send(StreamEvent::Done {
                    total_tokens: 0,
                    finish_reason: "offline_rule".to_string(),
                });
            }
            Err(e) => {
                // AuthFailed / RateLimit / BadRequest / ParseError → send Error，不入库
                let _ = channel.send(StreamEvent::Error {
                    error_kind: error_kind_str(&e).to_string(),
                    message: e.to_string(),
                });
            }
        }
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
    let record = llm_providers::get_active_record(app)
        .await
        .map_err(|e| ChatError::Llm(format!("read active provider: {e}")))?
        .ok_or_else(|| {
            ChatError::Llm(
                "未配置 LLM Provider；请到设置面板 → LLM Provider 添加并激活一个".to_string(),
            )
        })?;
    if record.api_key.is_empty() {
        return Err(ChatError::Llm(
            "active provider 的 API Key 为空；请到设置面板填写".to_string(),
        ));
    }
    OpenAIProvider::new("openai", &record.base_url, record.api_key, &record.model)
        .map_err(|e| ChatError::Llm(format!("provider init: {e}")))
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

    #[test]
    fn stream_event_serializes_delta_with_camel_case_tag() {
        let ev = StreamEvent::Delta {
            token: "你好".to_string(),
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["type"], "delta");
        assert_eq!(json["token"], "你好");
    }

    #[test]
    fn stream_event_serializes_done_with_camel_case_fields() {
        let ev = StreamEvent::Done {
            total_tokens: 42,
            finish_reason: "stop".to_string(),
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["type"], "done");
        assert_eq!(json["totalTokens"], 42);
        assert_eq!(json["finishReason"], "stop");
    }

    #[test]
    fn stream_event_serializes_error_with_camel_case_fields() {
        let ev = StreamEvent::Error {
            error_kind: "AuthFailed".to_string(),
            message: "401 Unauthorized".to_string(),
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["type"], "error");
        assert_eq!(json["errorKind"], "AuthFailed");
        assert_eq!(json["message"], "401 Unauthorized");
    }

    #[test]
    fn service_clone_shares_active_streams() {
        // ChatService 必须可 Clone 且共享 active_streams（spawn 出去的 task 通过 cancel 路径
        // 找到同一个 token map）
        let svc = ChatService::new();
        let svc2 = svc.clone();
        let token = CancellationToken::new();
        {
            let mut map = svc.active_streams.lock().unwrap();
            map.insert("shared-id".to_string(), token.clone());
        }
        // svc2 应能 cancel 到同一个 token
        svc2.cancel("shared-id").unwrap();
        assert!(token.is_cancelled());
    }
}
