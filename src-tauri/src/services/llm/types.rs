// LLM Provider 共享数据类型（ADR-018 Layer 1）。
//
// 形状对齐 OpenAI Chat Completions / Anthropic messages / DeepSeek / Moonshot / Qwen / Ollama
// 现行协议的"parts 数组 + tool_calls"基线（2026 实测调研，详见 ADR-018）。
//
// M1 实际产出仅 ContentPart::Text + tools=vec![]；其余 variant typed only，M3+ 补 impl 路径
// 时不需要改这些类型。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    /// role=tool 用于工具执行结果回传给模型；M1 不消费。
    Tool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    Low,
    High,
    Auto,
}

/// 多模态内容 part。
///
/// 序列化形状（OpenAI 协议）：
/// - `Text { text }` → `{"type":"text","text":"..."}`
/// - `ImageUrl { image_url }` → `{"type":"image_url","image_url":{"url":"...","detail":"..."}}`
/// - `Audio { input_audio }` → `{"type":"input_audio","input_audio":{"data":"...","format":"wav"}}`
/// - `File { file }` → `{"type":"file","file":{"filename":"...","file_data":"..."}}`
///
/// M1 实际只走 Text；其余 variant typed only（M3+ 接多模态时实现）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    ImageUrl {
        image_url: ImageUrlValue,
    },
    /// 内部 "结构化 base64" 表示；OpenAIProvider 序列化时会改写成 ImageUrl 的 data URI 形式
    /// （OpenAI 不接受裸 base64，需要 `data:image/png;base64,...`）。M3+ 实现时落实改写。
    #[serde(rename = "image_base64")]
    ImageBase64 {
        mime: String,
        data: String,
    },
    #[serde(rename = "input_audio")]
    Audio {
        input_audio: AudioValue,
    },
    File {
        file: FileValue,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageUrlValue {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<ImageDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioValue {
    /// base64 编码音频
    pub data: String,
    /// "wav" | "mp3"
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileValue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

/// 助手发起的 tool 调用（M1 typed only，调用方不传 tools 故永远不出现）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// JSON 字符串（OpenAI 协议里 arguments 字段就是 string，不是对象，方便流式拼接）
    pub arguments: String,
}

/// Chat 请求中提供的工具定义（M1 调用方总是传空 vec）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// JSON Schema 描述参数 shape（用 serde_json::Value 不强制具体形状）
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    #[default]
    Auto,
    None,
    Required,
    // Specific(name) 暂不支持，M3+ 加。
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: Role,
    /// parts 数组；单 Text part 时 OpenAIProvider 序列化时降级成旧 string content（兼容老 model）
    pub content: Vec<ContentPart>,
    /// 仅 Assistant role 填；M1 总是空
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// 仅 Tool role 回传时填；M1 不用
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    /// 构造纯文本消息（M1 主要用法）。
    pub fn text(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            content: vec![ContentPart::Text { text: text.into() }],
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
    /// 留空时 provider 用自带的 model_default
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub tools: Vec<ToolDefinition>,
    pub tool_choice: ToolChoice,
}

/// 流式增量 callback 单条 emit。
#[derive(Debug, Clone, PartialEq)]
pub enum StreamDelta {
    /// 文本 token 增量（M1 主路径）
    TextDelta(String),
    /// 工具调用增量（M1 typed only，OpenAI 流式协议下逐段拼 arguments JSON 字符串）
    ToolCallDelta {
        index: u32,
        id: Option<String>,
        name: Option<String>,
        arguments_chunk: Option<String>,
    },
    /// 流结束 + 终止原因 + token 用量（OpenAI stream_options.include_usage 会在末 chunk 给）
    Finish {
        reason: FinishReason,
        usage: Option<Usage>,
    },
}

/// 流结束原因。
///
/// `Unknown(String)` 是兜底变体：上游返了我们不认识的 `finish_reason` 字符串时透传原始值，
/// 让前端能区分"上游真返 error"与"上游返了新协议变体"——比早先把未知值统一吞成 `Error` 友好。
/// 去 `Copy`：`Unknown(String)` 持有堆数据，调用方需要显式 `.clone()`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    /// 兜底（stream 中途断 / provider 自身报错）
    Error,
    /// 上游返了未知 finish_reason 字符串；透传原始值便于诊断
    Unknown(String),
}

/// `prompt_tokens_details` 子对象（OpenAI / Qwen / Moonshot 的缓存命中走这里）。
/// 上游可能还带 audio_tokens / reasoning_tokens 等字段，serde 默认忽略未知字段。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptTokensDetails {
    /// prompt 中命中缓存的 token 数（automatic prompt caching）
    #[serde(default)]
    pub cached_tokens: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
    // === 缓存命中可观测（T3-6）===
    // 各 provider 缓存字段名不同；下面三者按 provider 选择性出现，统一由 cached_tokens() 归一化。
    // 此前 Usage 只有上面三字段，serde 默认丢弃未知字段 → 缓存命中信息被静默吞掉、无法检测。
    /// OpenAI / Qwen / Moonshot 风格：`{ prompt_tokens_details: { cached_tokens } }`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<PromptTokensDetails>,
    /// DeepSeek 风格：`prompt_tokens = prompt_cache_hit_tokens + prompt_cache_miss_tokens`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_hit_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_miss_tokens: Option<u32>,
}

impl Usage {
    /// 归一化的缓存命中 token 数（屏蔽 provider 协议差异）。
    /// OpenAI/Qwen/Moonshot → `prompt_tokens_details.cached_tokens`；DeepSeek → `prompt_cache_hit_tokens`；
    /// 都没有（Ollama / 未命中）→ 0。
    pub fn cached_tokens(&self) -> u32 {
        self.prompt_tokens_details
            .map(|d| d.cached_tokens)
            .filter(|&n| n > 0)
            .or(self.prompt_cache_hit_tokens)
            .unwrap_or(0)
    }

    /// 缓存命中率 = cached_tokens / prompt_tokens（prompt_tokens=0 → 0.0）。
    pub fn cache_hit_rate(&self) -> f32 {
        if self.prompt_tokens == 0 {
            return 0.0;
        }
        self.cached_tokens() as f32 / self.prompt_tokens as f32
    }
}

/// chat_stream 完成后返回的汇总（与最末 Finish delta 信息一致；调用方二选一消费）。
#[derive(Debug, Clone)]
pub struct ChatStreamFinish {
    pub reason: FinishReason,
    pub usage: Option<Usage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Role::Tool).unwrap(), "\"tool\"");
    }

    #[test]
    fn content_part_text_tagged_serialization() {
        let part = ContentPart::Text {
            text: "hello".into(),
        };
        let v = serde_json::to_value(&part).unwrap();
        assert_eq!(v, serde_json::json!({ "type": "text", "text": "hello" }));
    }

    #[test]
    fn content_part_image_url_tagged_serialization() {
        let part = ContentPart::ImageUrl {
            image_url: ImageUrlValue {
                url: "https://x.test/a.png".into(),
                detail: Some(ImageDetail::High),
            },
        };
        let v = serde_json::to_value(&part).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "type": "image_url",
                "image_url": { "url": "https://x.test/a.png", "detail": "high" }
            })
        );
    }

    #[test]
    fn chat_message_text_factory() {
        let m = ChatMessage::text(Role::User, "hi");
        assert_eq!(m.role, Role::User);
        assert_eq!(m.content.len(), 1);
        assert!(matches!(m.content[0], ContentPart::Text { .. }));
        assert!(m.tool_calls.is_empty());
        assert!(m.tool_call_id.is_none());
    }

    #[test]
    fn finish_reason_serde_snake_case() {
        let v = serde_json::to_string(&FinishReason::ToolCalls).unwrap();
        assert_eq!(v, "\"tool_calls\"");
        let r: FinishReason = serde_json::from_str("\"content_filter\"").unwrap();
        assert_eq!(r, FinishReason::ContentFilter);
    }

    #[test]
    fn usage_parses_openai_cached_tokens() {
        // OpenAI / Qwen / Moonshot：prompt_tokens_details.cached_tokens
        let u: Usage = serde_json::from_str(
            r#"{"prompt_tokens":2006,"completion_tokens":300,"total_tokens":2306,"prompt_tokens_details":{"cached_tokens":1920}}"#,
        )
        .unwrap();
        assert_eq!(u.cached_tokens(), 1920);
        assert!((u.cache_hit_rate() - 1920.0 / 2006.0).abs() < 1e-6);
    }

    #[test]
    fn usage_parses_deepseek_cache_hit_tokens() {
        // DeepSeek：另一套字段名，prompt_tokens = hit + miss
        let u: Usage = serde_json::from_str(
            r#"{"prompt_tokens":100,"completion_tokens":50,"total_tokens":150,"prompt_cache_hit_tokens":80,"prompt_cache_miss_tokens":20}"#,
        )
        .unwrap();
        assert_eq!(u.cached_tokens(), 80);
        assert_eq!(u.prompt_cache_miss_tokens, Some(20));
        assert!((u.cache_hit_rate() - 0.8).abs() < 1e-6);
    }

    #[test]
    fn usage_without_cache_fields_reports_zero() {
        // 旧三字段响应（Ollama / 未命中）仍正常解析，缓存视为 0，不破坏既有路径。
        let u: Usage =
            serde_json::from_str(r#"{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}"#)
                .unwrap();
        assert_eq!(u.cached_tokens(), 0);
        assert_eq!(u.cache_hit_rate(), 0.0);
        assert!(u.prompt_tokens_details.is_none());
    }

    #[test]
    fn usage_openai_cold_cache_reports_zero() {
        // OpenAI 返了 prompt_tokens_details 但 cached_tokens=0（冷启动 / 未命中）→ 归一化为 0。
        let u: Usage = serde_json::from_str(
            r#"{"prompt_tokens":50,"completion_tokens":5,"total_tokens":55,"prompt_tokens_details":{"cached_tokens":0}}"#,
        )
        .unwrap();
        assert_eq!(u.cached_tokens(), 0);
    }
}
