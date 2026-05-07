// ChatService 业务编排层（M1 W2 #13，ADR-018 Layer 2）。
//
// 子模块：
// - prompt: 切人格 4 节（# 身份 / # 性格 / # 能力 / # 行为规则）→ 拼 system message →
//           build_messages 输出 Vec<ChatMessage>（含 inline re-anchor 包装当前 user 输入）
// - conversation: 活跃会话 KV（config 表 `chat:active_conversation_id`）+ get-or-create
// - service: ChatService::{send, cancel, history}（#23 任务加入；本 commit 暂不存在）
//
// 错误类型 ChatError 收口下层 5 类（config / db / memory / persona / prompt），
// IPC 层 commands::chat::* 把 ChatError 转 String 给前端，前端按 errorKind 分支。
//
// 设计参考：
// - ADR-018（三层抽象 + AgentService 工具调用框架）
// - persona-design.md §7.1 / §8.2（prompt 拼装顺序）
// - claw-code rust/crates/runtime/src/prompt.rs（静态/动态 boundary + 截断守护模式）
// - arxiv 2402.10962（Persona Drift 的 system prompt repetition + user input re-anchor）

pub mod conversation;
pub mod prompt;
pub mod service;

use thiserror::Error;

use crate::services::config::ConfigError;
use crate::services::db::DbError;
use crate::services::memory::MemoryError;
use crate::services::nickname::NicknameError;
use crate::services::persona::PersonaLookupError;
use prompt::PromptError;

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    #[error("database error: {0}")]
    Database(String),
    #[error("memory error: {0}")]
    Memory(#[from] MemoryError),
    #[error("persona error: {0}")]
    Persona(String),
    #[error("nickname error: {0}")]
    Nickname(#[from] NicknameError),
    #[error("prompt error: {0}")]
    Prompt(#[from] PromptError),
    #[error("config dir error: {0}")]
    AppConfigDir(String),
    #[error("LLM error: {0}")]
    Llm(String),
}

impl From<sqlx::Error> for ChatError {
    fn from(e: sqlx::Error) -> Self {
        ChatError::Database(e.to_string())
    }
}

impl From<DbError> for ChatError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => ChatError::AppConfigDir(s),
            DbError::Database(s) => ChatError::Database(s),
        }
    }
}

impl From<PersonaLookupError> for ChatError {
    fn from(e: PersonaLookupError) -> Self {
        ChatError::Persona(e.to_string())
    }
}
