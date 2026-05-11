// LLM Provider 抽象层（ADR-018 Layer 1）。
//
// 设计：消息进 token 出 + tool_call 透传，不感知工具语义；上层 ChatService（#13）/
// AgentService（M3+）做编排。M1 只接 OpenAI（含 OpenAI 兼容 5 个 preset），types 用
// parts 数组 + tool_calls 形状（架构 §6.1 Superseded by ADR-018）。
//
// 模块布局：
// - types: 数据类型（ChatMessage / ContentPart / ChatOptions / StreamDelta / ...）
// - error: LLMError 分类（issue #12 §3）
// - openai: OpenAI 兼容 provider 实现（#12 范围）
//
// 后续 milestones 在此挂载：anthropic.rs（P1-R1）/ gemini.rs（P1-R2）等。

pub mod error;
pub mod openai;
pub mod probe;
pub mod types;

// 这两个 use 让 services::llm::* 直接拿到 LLMError / OpenAIProvider；types::* 全量再导出。
// 当前 M1 没真消费方（#13 ChatService 才调），编译器会判这些 pub use 为 unused —
// 用 #[allow] 屏蔽到 #18 commands::llm 真消费时；#13 起这些 attr 一并去掉。
#[allow(unused_imports)]
pub use error::LLMError;
#[allow(unused_imports)]
pub use openai::OpenAIProvider;
pub use types::*;

use async_trait::async_trait;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// provider 标识：'openai' | 'deepseek' | ...（M1 实际仅 'openai'）
    fn id(&self) -> &str;

    /// 流式 chat：消息进 → token 出（callback 调用方提供）。
    ///
    /// - cancel 触发 → 立即 break + drop reqwest stream（自动断 TCP）→ Err(LLMError::Cancelled)
    /// - 流自然结束 → 末尾调一次 callback emit StreamDelta::Finish + 返 ChatStreamFinish
    /// - HTTP 4xx / 5xx → 直接返错（不走 callback）
    /// - SSE / JSON 解析失败 → LLMError::ParseError
    ///
    /// callback 类型 `Box<dyn Fn(StreamDelta) + Send + Sync>`：调用方常用模式是用
    /// `Arc<Mutex<...>>` 收集 / 转 Tauri emit；Sync 要求让 process_event_stream 内部
    /// `tokio::select!` 分支借用更顺手。
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        options: ChatOptions,
        cancel: CancellationToken,
        on_delta: Box<dyn Fn(StreamDelta) + Send + Sync>,
    ) -> Result<ChatStreamFinish, LLMError>;

    /// 探活：测 base_url + api_key 可达；返 RTT。
    /// 实现可走最便宜端点（OpenAI 系：GET /models）。
    async fn ping(&self) -> Result<Duration, LLMError>;
}
