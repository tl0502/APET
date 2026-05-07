// ChatService IPC 类型（#13，ADR-018 Layer 2）。
//
// 字段命名规则：
// - Message 是 sqlx::FromRow MessageRecord 直序列化 → snake_case (id / conversation_id / created_at / ...)
// - SendResult / 3 个 stream payload 是 service.rs 用 #[serde(rename_all = "camelCase")] 重命名 → camelCase
//
// 这是 mixed convention，两边类型要严格对齐 Rust 端契约。

import type { LLMErrorKind } from './llm'

/** messages.mode 枚举（schema VALID_MODES 守护）。 */
export type ChatMode = 'online' | 'offline_rule'

/** messages.role 枚举（schema VALID_ROLES 守护）。 */
export type ChatRole = 'user' | 'assistant' | 'system'

/**
 * messages 表行（与 src-tauri/src/services/memory.rs::MessageRecord 对齐）。
 *
 * 字段全 snake_case 是因为 sqlx::FromRow 默认按列名序列化，没加 rename_all。
 * ChatPanel 渲染时直接 `m.content` / `m.created_at`。
 */
export interface Message {
  id: string
  conversation_id: string
  role: ChatRole
  content: string
  mode: ChatMode
  created_at: string
}

/**
 * chat_send 返回值（service.rs::SendResult）。
 *
 * messageId 是 assistant 消息的 ULID，前端用它对应后续 chat:stream:* events。
 * conversationId 是当前轮所属会话 ID，dev 验收 / ChatPanel 切会话用。
 */
export interface SendResult {
  messageId: string
  conversationId: string
}

/**
 * Stream finish_reason 取值。
 *
 * 'stop' / 'length' / 'tool_calls' / 'content_filter' / 'error' 来自
 * service.rs::finish_reason_to_str（mirror Rust FinishReason variants）；
 * 'cancelled' / 'offline_rule' 是 ChatService 业务定义（取消 / 离线降级）。
 */
export type ChatFinishReason =
  | 'stop'
  | 'length'
  | 'tool_calls'
  | 'content_filter'
  | 'error'
  | 'cancelled'
  | 'offline_rule'

/** `chat:stream:delta` event payload。 */
export interface ChatStreamDeltaPayload {
  messageId: string
  /** 单 token 文本增量（多个 token 可在一帧内多次 emit）。 */
  token: string
}

/** `chat:stream:done` event payload。 */
export interface ChatStreamDonePayload {
  messageId: string
  /** OpenAI usage.total_tokens（cancel / offline_rule 路径无 usage 时为 0）。 */
  totalTokens: number
  finishReason: ChatFinishReason
}

/** `chat:stream:error` event payload。 */
export interface ChatStreamErrorPayload {
  messageId: string
  /** LLMError variant 字符串；前端按此分支提示（AuthFailed → 改 API Key 等）。 */
  errorKind: LLMErrorKind
  message: string
}
