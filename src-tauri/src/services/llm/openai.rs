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
    ChatMessage, ChatOptions, ChatStreamFinish, ContentPart, FinishReason, StreamDelta,
    ToolChoice, Usage,
};
use super::LLMProvider;

const DEFAULT_TIMEOUT_SECS: u64 = 60;

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
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(|e| LLMError::Network(format!("build reqwest client: {e}")))?;
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
        _ => FinishReason::Error,
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
            _ = cancel.cancelled() => return Err(LLMError::Cancelled),
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
            _ = cancel.cancelled() => return Err(LLMError::Cancelled),
            r = send_fut => r.map_err(LLMError::from)?,
        };

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(classify_status(status, &body));
        }

        let bytes_stream = response.bytes_stream();
        let (reason, usage) = process_event_stream(bytes_stream, cancel, &*on_delta).await?;

        // 末尾 emit Finish delta，让 callback 消费方知道流已结束 + 拿 token 用量
        on_delta(StreamDelta::Finish { reason, usage });
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
        assert_eq!(parse_finish_reason("garbage"), FinishReason::Error);
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
                total_tokens: 5
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
        assert!(matches!(r, Err(LLMError::Cancelled)));
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
