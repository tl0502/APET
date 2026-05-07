// ChatService IPC binding（#13，ADR-018 Layer 2）。
//
// 3 个 command + 3 个 stream events：
// - sendChat(input, conversationId?) → { messageId }
//     conversationId 不传 → 后端 ensure_active_conversation 复用或新建
//     返回 assistant message ULID；用它对应 chat:stream:* events
// - cancelChat(messageId) → ()
//     按 messageId 触发后端活跃 chat_stream 的 CancellationToken；不存在 = no-op
// - loadChatHistory(conversationId, limit) → Message[]
//     按 created_at 升序返回；ChatPanel 翻历史用
//
// 3 个事件订阅返回 UnlistenFn，调用方 unmount 时务必 unlisten 防泄露。
//
// dev console 验证（settings 窗口 / pet 窗口任一 DevTools，withGlobalTauri 已启）：
//
// ① 简单 send（先确保 #12 已 set OpenAI/CUSTOM provider 的 api_key + base_url + model）：
//   const r = await window.__TAURI__.core.invoke('chat_send', { input: '你好' })
//
// ② 流式可视化 + 完整收尾：
//   const u1 = await window.__TAURI__.event.listen('chat:stream:delta', e => console.log('[delta]', e.payload))
//   const u2 = await window.__TAURI__.event.listen('chat:stream:done',  e => console.log('[done]',  e.payload))
//   const u3 = await window.__TAURI__.event.listen('chat:stream:error', e => console.log('[error]', e.payload))
//   await window.__TAURI__.core.invoke('chat_send', { input: '写一段话' })
//   u1(); u2(); u3()
//
// ③ 取消（chat_send 是 fire-and-forget 流式；cancel 用返回的 messageId）：
//   const r = await window.__TAURI__.core.invoke('chat_send', { input: '写一首长诗' })
//   await new Promise(r => setTimeout(r, 200))
//   await window.__TAURI__.core.invoke('chat_cancel', { messageId: r.messageId })
//
// ④ 看历史：
//   await window.__TAURI__.core.invoke('chat_history', { conversationId: '<ULID>', limit: 100 })

import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { invoke } from './ipc'
import type {
  ChatStreamDeltaPayload,
  ChatStreamDonePayload,
  ChatStreamErrorPayload,
  Message,
  SendResult,
} from '@/types/chat'

export const CHAT_STREAM_DELTA_EVENT = 'chat:stream:delta'
export const CHAT_STREAM_DONE_EVENT = 'chat:stream:done'
export const CHAT_STREAM_ERROR_EVENT = 'chat:stream:error'

/**
 * 发起一轮对话；返回 assistant 消息 ID。
 *
 * 调用前应先订阅 chat:stream:delta / done / error（onChatStreamDelta 等）。
 * 后端流式过程中持续 emit delta；完成 emit done；错误 emit error。
 *
 * @param input 用户输入（trim 后非空，≤ 8000 字符；后端守护）
 * @param conversationId 不传 → 后端复用 active conversation 或新建
 */
export function sendChat(input: string, conversationId?: string): Promise<SendResult> {
  return invoke<SendResult>('chat_send', { input, conversationId })
}

/** 取消进行中的流式（按 sendChat 返回的 messageId）。messageId 不存在 = no-op。 */
export function cancelChat(messageId: string): Promise<void> {
  return invoke<void>('chat_cancel', { messageId })
}

/** 按 conversationId 读历史；按 created_at 升序返回。limit 1-1000（后端 clamp）。 */
export function loadChatHistory(conversationId: string, limit: number): Promise<Message[]> {
  return invoke<Message[]>('chat_history', { conversationId, limit })
}

/** 订阅流式 token 增量。 */
export function onChatStreamDelta(
  handler: (payload: ChatStreamDeltaPayload) => void,
): Promise<UnlistenFn> {
  return listen<ChatStreamDeltaPayload>(CHAT_STREAM_DELTA_EVENT, (e) => handler(e.payload))
}

/** 订阅流式完成（含 cancelled / offline_rule 收尾）。 */
export function onChatStreamDone(
  handler: (payload: ChatStreamDonePayload) => void,
): Promise<UnlistenFn> {
  return listen<ChatStreamDonePayload>(CHAT_STREAM_DONE_EVENT, (e) => handler(e.payload))
}

/** 订阅流式错误（AuthFailed / BadRequest / RateLimit / ParseError；不入库的错误路径）。 */
export function onChatStreamError(
  handler: (payload: ChatStreamErrorPayload) => void,
): Promise<UnlistenFn> {
  return listen<ChatStreamErrorPayload>(CHAT_STREAM_ERROR_EVENT, (e) => handler(e.payload))
}
