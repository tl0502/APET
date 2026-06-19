// OpenAI 兼容 provider（覆盖 OpenAI / DeepSeek / Moonshot / Qwen / Ollama / 自定义 base_url）。
//
// M1 文本路径流程：
//   POST {base_url}/chat/completions  body { model, messages, stream:true, stream_options.include_usage }
//   → reqwest::Response::bytes_stream()
//   → eventsource_stream::Eventsource → SSE event 循环
//   → 每条 event 的 data line：JSON 解析 → 抽 choices[0].delta.content / tool_calls / finish_reason
//   → on_delta(TextDelta / ToolCallDelta) + 末尾 on_delta(Finish)
//   → cancel.cancelled() 触发 → break + drop stream（reqwest 自动断 TCP）→ Err(Cancelled)
//
// 序列化策略（ADR-018）：
//   单 ContentPart::Text → {role,content:"s"}（兼容老 model 与 Ollama / Qwen / Moonshot fallback）
//   多 part / 非 Text → {role,content:[{type:"text",text:"s"},{type:"image_url",image_url:{...}}]}
//
// process_event_stream 单独抽出，方便用合成 bytes 流做单测（不需要起 mock HTTP server）。

use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use eventsource_stream::Eventsource;
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::error::{classify_status, LLMError};
use super::types::{
    ChatMessage, ChatOptions, ChatStreamFinish, ContentPart, FinishReason, StreamDelta, ToolChoice,
    Usage,
};
use super::LLMProvider;

/// TCP + TLS 握手必须在此时间内完成（端点不可达 / DNS 挂掉时快速失败）。
const CONNECT_TIMEOUT_SECS: u64 = 15;
/// 单次 read 卡住超过此时间才视为断流。**短于总 timeout 就足够检测 stall**。
/// 注意：deepseek-reasoner / o1 等推理模型在 thinking 阶段可能 30s+ 不出 token；
/// 60s 已留出充足余量，再长就是真的卡死了。
const READ_TIMEOUT_SECS: u64 = 60;
/// 整请求绝对死线兜底（防止 drip-feed 慢服务器永不释放连接）。
/// 历史回归（2026-05-10）：曾用 `Client::timeout(60s)` 做总死线，导致历史对话
/// （长 prompt，TTFB 易超 60s）触发 timeout → `LLMError::Network` → 离线模板覆盖
/// 正常 placeholder。改 `connect_timeout + read_timeout + 大兜底 timeout` 后此 footgun 关闭。
const TOTAL_TIMEOUT_SECS: u64 = 600;

/// 进程级共享 reqwest::Client（T3-5）。
///
/// reqwest::Client 内部以 Arc 持连接池，clone 廉价且共享同一池。此前每轮对话都在
/// `OpenAIProvider::new` 里 `Client::builder().build()` 新建一个 Client → 连接池每轮被丢弃
/// → 每轮都要重新 TCP+TLS 握手。改成所有 provider 复用同一个 Client 后，跨轮 keep-alive
/// 连接得以复用，省掉重复握手延迟。
///
/// 三个 timeout 常量对所有 preset 相同，故单一共享 Client 适配 OpenAI/DeepSeek/Moonshot/
/// Qwen/Ollama/自定义全部路径。构建失败（TLS backend 初始化异常）仍按可恢复错误上抛，
/// 不 panic（保持原 new() 语义）。
fn shared_client() -> Result<Client, LLMError> {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    if let Some(c) = CLIENT.get() {
        return Ok(c.clone());
    }
    let built = Client::builder()
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .read_timeout(Duration::from_secs(READ_TIMEOUT_SECS))
        .timeout(Duration::from_secs(TOTAL_TIMEOUT_SECS))
        .build()
        .map_err(|e| LLMError::Network(format!("build reqwest client: {e}")))?;
    // 并发竞态：set 失败说明已有线程先 set，用已存的那份即可（丢弃本次 built）。
    match CLIENT.set(built) {
        Ok(()) => Ok(CLIENT.get().expect("just set").clone()),
        Err(_) => Ok(CLIENT.get().expect("set by another thread").clone()),
    }
}

pub struct OpenAIProvider {
    client: Client,
    base_url: String,
    api_key: String,
    /// 默认 model（调用方 ChatOptions.model 留空时用此）
    model_default: String,
    id: String,
}

impl OpenAIProvider {
    /// 标准 OpenAI 预设（base_url=https://api.openai.com/v1，model=gpt-4o-mini）。
    pub fn openai(api_key: impl Into<String>) -> Result<Self, LLMError> {
        Self::new(
            "openai",
            "https://api.openai.com/v1",
            api_key,
            "gpt-4o-mini",
        )
    }

    pub fn new(
        id: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model_default: impl Into<String>,
    ) -> Result<Self, LLMError> {
        let client = shared_client()?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            model_default: model_default.into(),
            id: id.into(),
        })
    }
}

/// 把我们的 ChatMessage 转成 OpenAI wire format JSON。
///
/// 单 Text part 时 content 走 string；多 part / 非 Text 走 parts 数组。
pub(crate) fn serialize_message(msg: &ChatMessage) -> serde_json::Value {
    let content_value: serde_json::Value = if msg.content.len() == 1 {
        if let ContentPart::Text { text } = &msg.content[0] {
            serde_json::Value::String(text.clone())
        } else {
            serde_json::to_value(&msg.content).unwrap_or(serde_json::Value::Null)
        }
    } else if msg.content.is_empty() {
        serde_json::Value::String(String::new())
    } else {
        serde_json::to_value(&msg.content).unwrap_or(serde_json::Value::Null)
    };

    let mut obj = serde_json::Map::new();
    obj.insert("role".to_string(), serde_json::to_value(msg.role).unwrap());
    obj.insert("content".to_string(), content_value);
    if !msg.tool_calls.is_empty() {
        // OpenAI assistant.tool_calls 协议：[{id, type:"function", function:{name, arguments}}]
        let arr: Vec<_> = msg
            .tool_calls
            .iter()
            .map(|tc| {
                json!({
                    "id": tc.id,
                    "type": "function",
                    "function": { "name": tc.name, "arguments": tc.arguments },
                })
            })
            .collect();
        obj.insert("tool_calls".to_string(), serde_json::Value::Array(arr));
    }
    if let Some(id) = &msg.tool_call_id {
        obj.insert(
            "tool_call_id".to_string(),
            serde_json::Value::String(id.clone()),
        );
    }
    serde_json::Value::Object(obj)
}

#[derive(Debug, Default, Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Default, Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: StreamDeltaWire,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct StreamDeltaWire {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCallWire>,
}

#[derive(Debug, Deserialize)]
struct ToolCallWire {
    index: u32,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ToolCallFnWire>,
}

#[derive(Debug, Default, Deserialize)]
struct ToolCallFnWire {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

pub(crate) fn parse_finish_reason(s: &str) -> FinishReason {
    match s {
        "stop" => FinishReason::Stop,
        "length" => FinishReason::Length,
        "tool_calls" | "function_call" => FinishReason::ToolCalls,
        "content_filter" => FinishReason::ContentFilter,
        // 上游返了未知 finish_reason 字符串：透传原始值，调用方能区分"真 error"与"新协议变体"。
        // 历史上这里把所有未知值映射到 FinishReason::Error，丢了诊断信息。
        other => {
            eprintln!("[llm] unknown finish_reason from upstream: {other}");
            FinishReason::Unknown(other.to_string())
        }
    }
}

/// 处理 SSE event 流：解析 → 调 callback → 追踪 finish_reason / usage。
///
/// 抽出来便于用合成 Bytes 流做单测（无需 mock HTTP server）。
pub(crate) async fn process_event_stream<S, E>(
    stream: S,
    cancel: CancellationToken,
    on_delta: &(dyn Fn(StreamDelta) + Send + Sync),
) -> Result<(FinishReason, Option<Usage>), LLMError>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin + Eventsource,
    E: std::error::Error + Send + Sync + 'static,
{
    let mut sse_stream = stream.eventsource();
    let mut last_finish_reason: Option<FinishReason> = None;
    let mut last_usage: Option<Usage> = None;

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(LLMError::Cancelled { partial_usage: last_usage }),
            evt = sse_stream.next() => {
                let Some(evt) = evt else { break; };
                let evt = evt.map_err(|e| LLMError::ParseError(format!("SSE: {e}")))?;
                let data = evt.data;
                if data.trim() == "[DONE]" {
                    break;
                }
                if data.trim().is_empty() {
                    continue;
                }
                let chunk: StreamChunk = serde_json::from_str(&data)
                    .map_err(|e| LLMError::ParseError(format!("JSON: {e} (data={data})")))?;
                if let Some(usage) = chunk.usage {
                    last_usage = Some(usage);
                }
                for choice in chunk.choices {
                    if let Some(text) = choice.delta.content {
                        if !text.is_empty() {
                            on_delta(StreamDelta::TextDelta(text));
                        }
                    }
                    for tc in choice.delta.tool_calls {
                        on_delta(StreamDelta::ToolCallDelta {
                            index: tc.index,
                            id: tc.id,
                            name: tc.function.as_ref().and_then(|f| f.name.clone()),
                            arguments_chunk: tc.function.and_then(|f| f.arguments),
                        });
                    }
                    if let Some(reason_str) = choice.finish_reason {
                        last_finish_reason = Some(parse_finish_reason(&reason_str));
                    }
                }
            }
        }
    }

    let reason = last_finish_reason.unwrap_or(FinishReason::Stop);
    Ok((reason, last_usage))
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    fn id(&self) -> &str {
        &self.id
    }

    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        options: ChatOptions,
        cancel: CancellationToken,
        on_delta: Box<dyn Fn(StreamDelta) + Send + Sync>,
    ) -> Result<ChatStreamFinish, LLMError> {
        let model = if options.model.is_empty() {
            self.model_default.clone()
        } else {
            options.model.clone()
        };

        let messages_wire: Vec<_> = messages.iter().map(serialize_message).collect();
        let mut body = json!({
            "model": model,
            "messages": messages_wire,
            "stream": true,
            "stream_options": { "include_usage": true },
        });
        if let Some(max) = options.max_tokens {
            body["max_tokens"] = json!(max);
        }
        if let Some(t) = options.temperature {
            body["temperature"] = json!(t);
        }
        if !options.tools.is_empty() {
            // OpenAI tools 协议：[{type:"function", function:{name,description,parameters}}]
            let tools_arr: Vec<_> = options
                .tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::Value::Array(tools_arr);
            body["tool_choice"] = match options.tool_choice {
                ToolChoice::Auto => json!("auto"),
                ToolChoice::None => json!("none"),
                ToolChoice::Required => json!("required"),
            };
        }

        let url = format!("{}/chat/completions", self.base_url);

        // 发送请求 vs cancel 竞速
        let send_fut = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send();
        let response = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(LLMError::Cancelled { partial_usage: None }),
            r = send_fut => r.map_err(LLMError::from)?,
        };

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(classify_status(status, &body));
        }

        let bytes_stream = response.bytes_stream();
        let (reason, usage) = process_event_stream(bytes_stream, cancel, &*on_delta).await?;

        // T3-6：把 token 用量 + 缓存命中打到日志，让"prompt 有没有命中缓存"可观测。
        // 各 provider 缓存字段名不同，cached_tokens()/cache_hit_rate() 已在 types.rs 归一化。
        // 同一长前缀第二轮起 cached 应从 0 跳升 → 即可确认缓存生效。
        if let Some(u) = usage.as_ref() {
            eprintln!(
                "[llm] usage provider={} prompt={} completion={} total={} cached={} hit_rate={:.0}%",
                self.id,
                u.prompt_tokens,
                u.completion_tokens,
                u.total_tokens,
                u.cached_tokens(),
                u.cache_hit_rate() * 100.0,
            );
        }

        // 末尾 emit Finish delta，让 callback 消费方知道流已结束 + 拿 token 用量
        // FinishReason 不再 Copy（Unknown(String) 持有堆数据），双用必须显式 clone。
        on_delta(StreamDelta::Finish {
            reason: reason.clone(),
            usage,
        });
        Ok(ChatStreamFinish { reason, usage })
    }

    async fn ping(&self) -> Result<Duration, LLMError> {
        // 用 GET /models 做最便宜的探活；OpenAI 系（DeepSeek/Moonshot/Qwen）都支持
        let start = std::time::Instant::now();
        let url = format!("{}/models", self.base_url);
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(LLMError::from)?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(classify_status(status, &body));
        }
        Ok(start.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::llm::types::{
        ChatMessage, ContentPart, ImageDetail, ImageUrlValue, Role, ToolCall,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn provider_new_reuses_shared_client_across_instances() {
        // T3-5：所有 provider 复用进程级共享 reqwest::Client。
        // reqwest 不暴露连接池身份，无法直接断言"同一池"；此处验证多次构造均成功、
        // 字段正确，并确保 shared_client() 在热路径上可重复调用不报错（回归守卫：
        // 防止有人把它改回每轮 fallible 的 per-call build）。
        let p1 = OpenAIProvider::new("openai", "https://api.openai.com/v1/", "k1", "gpt-4o-mini")
            .unwrap();
        let p2 = OpenAIProvider::new("deepseek", "https://api.deepseek.com", "k2", "deepseek-chat")
            .unwrap();
        assert_eq!(p1.id, "openai");
        assert_eq!(p2.id, "deepseek");
        assert_eq!(p1.base_url, "https://api.openai.com/v1"); // 末尾斜杠被 trim
        assert_eq!(p2.base_url, "https://api.deepseek.com");
    }

    #[test]
    fn serialize_single_text_part_uses_string_content() {
        let msg = ChatMessage::text(Role::User, "你好");
        let v = serialize_message(&msg);
        assert_eq!(v["role"], "user");
        assert_eq!(v["content"], "你好"); // 旧 string 协议
        assert!(v.get("tool_calls").is_none());
    }

    #[test]
    fn serialize_multi_part_uses_parts_array() {
        let msg = ChatMessage {
            role: Role::User,
            content: vec![
                ContentPart::Text {
                    text: "see this".into(),
                },
                ContentPart::ImageUrl {
                    image_url: ImageUrlValue {
                        url: "https://x.test/a.png".into(),
                        detail: Some(ImageDetail::Auto),
                    },
                },
            ],
            tool_calls: vec![],
            tool_call_id: None,
        };
        let v = serialize_message(&msg);
        assert!(v["content"].is_array());
        let arr = v["content"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["type"], "text");
        assert_eq!(arr[0]["text"], "see this");
        assert_eq!(arr[1]["type"], "image_url");
        assert_eq!(arr[1]["image_url"]["url"], "https://x.test/a.png");
    }

    #[test]
    fn serialize_assistant_tool_calls_uses_function_envelope() {
        let msg = ChatMessage {
            role: Role::Assistant,
            content: vec![],
            tool_calls: vec![ToolCall {
                id: "call_1".into(),
                name: "do_thing".into(),
                arguments: r#"{"x":1}"#.into(),
            }],
            tool_call_id: None,
        };
        let v = serialize_message(&msg);
        let arr = v["tool_calls"].as_array().unwrap();
        assert_eq!(arr[0]["id"], "call_1");
        assert_eq!(arr[0]["type"], "function");
        assert_eq!(arr[0]["function"]["name"], "do_thing");
        assert_eq!(arr[0]["function"]["arguments"], r#"{"x":1}"#);
    }

    #[test]
    fn parse_finish_reason_mappings() {
        assert_eq!(parse_finish_reason("stop"), FinishReason::Stop);
        assert_eq!(parse_finish_reason("length"), FinishReason::Length);
        assert_eq!(parse_finish_reason("tool_calls"), FinishReason::ToolCalls);
        assert_eq!(
            parse_finish_reason("function_call"),
            FinishReason::ToolCalls
        );
        assert_eq!(
            parse_finish_reason("content_filter"),
            FinishReason::ContentFilter
        );
        // 未知值现在透传原文为 Unknown(String)，不再被吞成 Error
        assert_eq!(
            parse_finish_reason("garbage"),
            FinishReason::Unknown("garbage".to_string())
        );
        assert_eq!(
            parse_finish_reason("safety_filter_v2"),
            FinishReason::Unknown("safety_filter_v2".to_string())
        );
    }

    /// 合成一个 OpenAI 风格的 SSE bytes 流，验证 process_event_stream 解析正确。
    /// 覆盖：多个 text delta + finish_reason=stop + 末尾 [DONE]。
    #[tokio::test]
    async fn process_event_stream_parses_text_deltas_and_finish() {
        let sse = "\
data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
data: {\"choices\":[],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5}}\n\n\
data: [DONE]\n\n";
        let stream = futures_util::stream::iter(vec![Ok::<_, std::io::Error>(Bytes::from(sse))]);
        let collected: Arc<Mutex<Vec<StreamDelta>>> = Arc::new(Mutex::new(Vec::new()));
        let collected2 = collected.clone();
        let cb: Box<dyn Fn(StreamDelta) + Send + Sync> = Box::new(move |d: StreamDelta| {
            collected2.lock().unwrap().push(d);
        });
        let cancel = CancellationToken::new();
        let (reason, usage) = process_event_stream(stream, cancel, &*cb).await.unwrap();
        assert_eq!(reason, FinishReason::Stop);
        assert_eq!(
            usage,
            Some(Usage {
                prompt_tokens: 3,
                completion_tokens: 2,
                total_tokens: 5,
                ..Default::default()
            })
        );
        let deltas = collected.lock().unwrap().clone();
        assert_eq!(
            deltas,
            vec![
                StreamDelta::TextDelta("你".into()),
                StreamDelta::TextDelta("好".into()),
            ]
        );
    }

    /// cancel 在流处理过程中触发 → 返回 Cancelled。
    /// 用 unfold 构造一个永远 pending 的流，cancel 50ms 后触发。
    #[tokio::test]
    async fn process_event_stream_returns_cancelled_when_token_fires() {
        let stream = futures_util::stream::pending::<Result<Bytes, std::io::Error>>();
        let cb: Box<dyn Fn(StreamDelta) + Send + Sync> = Box::new(|_d| {});
        let cancel = CancellationToken::new();
        let cancel_for_task = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel_for_task.cancel();
        });
        let r = process_event_stream(stream, cancel, &*cb).await;
        assert!(matches!(r, Err(LLMError::Cancelled { .. })));
    }

    /// cancel 触发时，若中段已收到 usage chunk，应通过 `partial_usage` 透传（issue #6 修复）。
    #[tokio::test]
    async fn process_event_stream_cancel_carries_partial_usage() {
        // 一个 chunk 携带 usage，再之后流挂起 → cancel 50ms 后触发 → Err 应带 partial_usage Some
        let sse = "\
data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n\
data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}}\n\n";
        let head = futures_util::stream::iter(vec![Ok::<_, std::io::Error>(Bytes::from(sse))]);
        let tail = futures_util::stream::pending::<Result<Bytes, std::io::Error>>();
        let stream = head.chain(tail);
        let cb: Box<dyn Fn(StreamDelta) + Send + Sync> = Box::new(|_d| {});
        let cancel = CancellationToken::new();
        let cancel_for_task = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            cancel_for_task.cancel();
        });
        let r = process_event_stream(stream, cancel, &*cb).await;
        match r {
            Err(LLMError::Cancelled { partial_usage }) => {
                let u = partial_usage.expect("usage already received before cancel");
                assert_eq!(u.total_tokens, 12);
            }
            other => panic!("expected Cancelled with partial_usage, got {other:?}"),
        }
    }

    /// 解析失败 → ParseError（防御性：DeepSeek/Moonshot 偶有非标准 chunk）
    #[tokio::test]
    async fn process_event_stream_parse_error_on_invalid_json() {
        let sse = "data: {not valid json\n\n";
        let stream = futures_util::stream::iter(vec![Ok::<_, std::io::Error>(Bytes::from(sse))]);
        let cb: Box<dyn Fn(StreamDelta) + Send + Sync> = Box::new(|_d| {});
        let cancel = CancellationToken::new();
        let r = process_event_stream(stream, cancel, &*cb).await;
        assert!(matches!(r, Err(LLMError::ParseError(_))));
    }
}
