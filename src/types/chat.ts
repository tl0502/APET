// ChatService IPC 类型（#13 → 修正：用 tauri::ipc::Channel<StreamEvent> 替代全局 emit）。
//
// 字段命名规则：
// - Message 是 sqlx::FromRow MessageRecord 直序列化 → snake_case (id / conversation_id / created_at / ...)
// - SendResult / StreamEvent 是 service.rs 用 #[serde(rename_all = "camelCase")] 重命名 → camelCase
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
 * messageId 是 assistant 消息的 ULID；前端用它做 chat_cancel 入参。
 * conversationId 是当前轮所属会话 ID，dev 验收 / ChatPanel 切会话用。
 *
 * 修正后的契约下，IPC 立即返这个值（不再等流式跑完）；前端拿到后立刻能 cancel。
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

/**
 * Channel<StreamEvent> 单条消息（mirror service.rs::StreamEvent）。
 *
 * 三 variant 对应原 chat:stream:{delta,done,error} 三事件，但走 ipc::Channel 通道：
 * - delta: 流式 token 增量（多个 token 可在一帧内连续到达）
 * - done: 流式完成（含 cancelled / offline_rule 收尾）
 * - error: 流式错误（AuthFailed / BadRequest / RateLimit / ParseError；不入库的错误路径）
 *
 * 不带 messageId —— channel 自带 scope，每个 chat_send 调用一条 channel 只服务一个 assistant 消息。
 */
export type StreamEvent =
  | { type: 'delta'; token: string }
  | { type: 'done'; totalTokens: number; finishReason: ChatFinishReason }
  | { type: 'error'; errorKind: LLMErrorKind; message: string }

/**
 * 单条 conversation summary（侧边栏列表用，与 services/chat/conversation.rs::ConversationSummary 对齐）。
 *
 * 字段 snake_case：sqlx::FromRow 默认按列名序列化，未加 rename_all。
 * `title` 为 NULL 时前端 fallback 到"未命名 + started_at" UI 文案。
 */
export interface ConversationSummary {
  id: string
  persona_id: string
  title: string | null
  started_at: string
  last_activity_at: string
}
