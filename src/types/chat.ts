// ChatService IPC 类型（#13 → 修正：用 tauri::ipc::Channel<StreamEvent> 替代全局 emit）。
//
// 字段命名规则：
// - Message 是 sqlx::FromRow MessageRecord 直序列化 → snake_case (id / conversation_id / created_at / ...)
// - SendResult / StreamEvent 是 service.rs 用 #[serde(rename_all = "camelCase")] 重命名 → camelCase
//
// 这是 mixed convention，两边类型要严格对齐 Rust 端契约。

import type { LLMErrorKind } from './llm'

/** messages.mode 枚举（与 services/memory.rs::VALID_MODES 对齐）。
 *  'cancelled' 由 ChatService run_stream 在收到 LLMError::Cancelled 且已收到 partial 文本时写入；
 *  ChatService.prepare 下一轮 history filter 会跳过它（与 'offline_rule' 同款理由）。 */
export type ChatMode = 'online' | 'offline_rule' | 'cancelled'

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
 * userMessageId 是后端刚 INSERT 的 user 消息 ULID（B4 修复：用来替换前端 optimistic
 * 的 `pending-user-*` 临时 id，让 session 内 user 气泡 ID 与 DB 对齐）。
 * conversationId 是当前轮所属会话 ID，dev 验收 / ChatPanel 切会话用。
 *
 * 修正后的契约下，IPC 立即返这个值（不再等流式跑完）；前端拿到后立刻能 cancel。
 */
export interface SendResult {
  messageId: string
  userMessageId: string
  conversationId: string
}

/**
 * Stream finish_reason 取值。
 *
 * 'stop' / 'length' / 'tool_calls' / 'content_filter' / 'error' 来自
 * service.rs::finish_reason_to_str（mirror Rust FinishReason variants）；
 * 'cancelled' / 'offline_rule' 是 ChatService 业务定义（取消 / 离线降级）。
 *
 * Issue #3：兜底 `string & {}` 接住 Rust 端 `FinishReason::Unknown(String)` 透传的上游
 * 未知 finish_reason 字符串（如新协议变体）。`& {}` 是 TS 惯用法：让智能感知仍优先提示
 * 上面的字面量值，但允许任意 string 不被类型系统拒绝。
 */
export type ChatFinishReason =
  | 'stop'
  | 'length'
  | 'tool_calls'
  | 'content_filter'
  | 'error'
  | 'cancelled'
  | 'offline_rule'
  | (string & {})

/**
 * Channel<StreamEvent> 单条消息（mirror service.rs::StreamEvent）。
 *
 * 三 variant 对应原 chat:stream:{delta,done,error} 三事件，但走 ipc::Channel 通道：
 * - delta: 流式 token 增量（多个 token 可在一帧内连续到达）
 * - done: 流式完成（含 cancelled / offline_rule 收尾）
 * - error: 流式错误（AuthFailed / BadRequest / RateLimit / ParseError；不入库的错误路径）
 *
 * 不带 messageId —— channel 自带 scope，每个 chat_send 调用一条 channel 只服务一个 assistant 消息。
 *
 * errorKind 取值：除 LLMErrorKind 7 类外，run_stream 在 update/delete 失败时会兜底
 * 发 'DbError'（service.rs 4 处），故联合类型加 'DbError' 与运行时对齐。前端 errorHint
 * default 兜底接住此值。
 */
export type StreamEvent =
  | { type: 'delta'; token: string }
  | { type: 'done'; totalTokens: number; finishReason: ChatFinishReason }
  | { type: 'error'; errorKind: LLMErrorKind | 'DbError'; message: string }

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
