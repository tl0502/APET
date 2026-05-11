// ChatService IPC binding（#13 → 修正：tauri::ipc::Channel<StreamEvent>）。
//
// 修正后的契约（详 plan c-issue-13-https-github-com-tl0502-apet-ancient-moth）：
// - sendChat(input, conversationId?, onStream: Channel<StreamEvent>) → { messageId, conversationId }
//     IPC 立即返 IDs（不再等流式跑完）；流式事件通过 onStream 回前端
//     onStream.onmessage = (msg) => switch(msg.type) { 'delta' | 'done' | 'error' }
// - cancelChat(messageId) → ()
//     按 messageId 触发后端活跃 chat_stream 的 CancellationToken；不存在 = no-op
//     关键修复：旧契约下前端要等流式跑完才拿到 messageId → cancel 死按钮；新契约立即可用
// - loadChatHistory(conversationId, limit) → Message[]
//     按 created_at 升序返回；ChatPanel 翻历史用
//
// dev console 验证（settings 窗口 / pet 窗口任一 DevTools，withGlobalTauri 已启）：
//
// ① 简单 send + 流式可视化：
//   const { Channel, invoke } = window.__TAURI__.core
//   const ch = new Channel()
//   ch.onmessage = m => console.log('[stream]', m)
//   const r = await invoke('chat_send', { input: '你好', onStream: ch })
//   console.log('[result]', r)  // { messageId, conversationId } 立即返
//
// ② 取消（关键修复）：
//   const ch = new Channel()
//   ch.onmessage = m => console.log('[stream]', m)
//   const r = await invoke('chat_send', { input: '写一首长诗', onStream: ch })
//   await new Promise(r => setTimeout(r, 200))
//   await invoke('chat_cancel', { messageId: r.messageId })
//
// ③ 看历史：
//   await invoke('chat_history', { conversationId: '<ULID>', limit: 100 })

import { Channel } from '@tauri-apps/api/core'

import { invoke } from './ipc'
import type {
  ConversationSummary,
  Message,
  SendResult,
  StreamEvent,
} from '@/types/chat'

export { Channel }
export type { StreamEvent }

/**
 * 发起一轮对话；流式事件通过 onStream channel 回前端，IPC 立即返 IDs。
 *
 * @param input 用户输入（trim 后非空，≤ 8000 字符；后端守护）
 * @param conversationId 不传 → 后端复用 active conversation 或新建
 * @param onStream Tauri 2 ipc::Channel；调用方挂 onmessage 处理 delta/done/error 三 variant
 */
export function sendChat(
  input: string,
  conversationId: string | undefined,
  onStream: Channel<StreamEvent>,
): Promise<SendResult> {
  return invoke<SendResult>('chat_send', { input, conversationId, onStream })
}

/** 取消进行中的流式（按 sendChat 返回的 messageId）。messageId 不存在 = no-op。 */
export function cancelChat(messageId: string): Promise<void> {
  return invoke<void>('chat_cancel', { messageId })
}

/** 按 conversationId 读历史；按 created_at 升序返回。limit 1-1000（后端 clamp）。 */
export function loadChatHistory(conversationId: string, limit: number): Promise<Message[]> {
  return invoke<Message[]>('chat_history', { conversationId, limit })
}

/** 列出未归档 conversation（侧边栏 ChatGPT 式列表用）；按 last_activity_at DESC。 */
export function listConversations(limit = 50): Promise<ConversationSummary[]> {
  return invoke<ConversationSummary[]>('chat_list_conversations', { limit })
}

/** 显式新建 conversation + 切 active KV；返回新 id。"新建对话"按钮路径。
 *
 * C7：personaId 可选；不传 → 后端用当前 active persona 兜底（M1 默认行为）。
 * 留入参为 M3 多 persona UI 准备。
 */
export function createConversation(personaId?: string): Promise<string> {
  return invoke<string>('chat_create_conversation', { personaId })
}

/** 切 active KV（点列表项路径；不动 messages 表）。 */
export function setActiveConversation(conversationId: string): Promise<void> {
  return invoke<void>('chat_set_active_conversation', { conversationId })
}

/** 重命名 conversation。空字符串 → 后端写 NULL（恢复"未命名"）；非空 → trim + ≤100 字符截断。 */
export function renameConversation(conversationId: string, title: string): Promise<void> {
  return invoke<void>('chat_rename_conversation', { conversationId, title })
}

/** 归档 conversation；列表立刻隐藏；命中 active KV 则后端清 KV。 */
export function archiveConversation(conversationId: string): Promise<void> {
  return invoke<void>('chat_archive_conversation', { conversationId })
}

/** 硬删 conversation；FK ON DELETE CASCADE 自动级联删 messages；命中 active KV 则后端清 KV。
 *  V3：后端会顺带清 chat:draft:<id> KV。 */
export function deleteConversation(conversationId: string): Promise<void> {
  return invoke<void>('chat_delete_conversation', { conversationId })
}

/** V3 多对话并发：读对话草稿。空 / 不存在 → null。 */
export function getChatDraft(conversationId: string): Promise<string | null> {
  return invoke<string | null>('chat_get_draft', { conversationId })
}

/** V3 多对话并发：写对话草稿。空字符串 → 后端删 KV。
 *  调用方应 debounce 200ms，避免每次按键都打 IPC。 */
export function setChatDraft(conversationId: string, draft: string): Promise<void> {
  return invoke<void>('chat_set_draft', { conversationId, draft })
}

/** V3 多对话并发：删对话草稿（不存在 = no-op）。
 *  正常路径由 deleteConversation 自动级联；只在显式"清空草稿"场景手动调。 */
export function deleteChatDraft(conversationId: string): Promise<void> {
  return invoke<void>('chat_delete_draft', { conversationId })
}
