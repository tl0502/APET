// #13 Chat IPC commands（M1 W2 ChatService 业务编排入口）
//
// 3 个 command + 3 个 stream events（events 由 services/chat/service.rs 在流式过程中 emit）：
// - chat_send(input, conversationId?) → { messageId }
//     conversationId None → ensure_active_conversation 复用或新建
//     返回 assistant message ULID；前端用它对应 chat:stream:* events
// - chat_cancel(messageId) → ()
//     按 message_id 触发活跃 chat_stream 的 CancellationToken；不存在 = no-op
// - chat_history(conversationId, limit) → Vec<MessageRecord>
//     按 created_at 升序返回；ChatPanel 翻历史用
//
// dev console 验证脚本（settings 窗口 / pet 窗口任一 DevTools，withGlobalTauri 已启）：
//
// ① 简单 send：
//   await window.__TAURI__.core.invoke('chat_send', { input: '你好' })
//
// ② 流式可视化：
//   const u1 = await window.__TAURI__.event.listen('chat:stream:delta', e => console.log('[delta]', e.payload))
//   const u2 = await window.__TAURI__.event.listen('chat:stream:done', e => console.log('[done]', e.payload))
//   await window.__TAURI__.core.invoke('chat_send', { input: '写一段话' })
//
// ③ 取消（chat_send 是 fire-and-forget 流式；cancel 用返回的 messageId）：
//   const r = await window.__TAURI__.core.invoke('chat_send', { input: '写一首长诗' })
//   await new Promise(r => setTimeout(r, 200))
//   await window.__TAURI__.core.invoke('chat_cancel', { messageId: r.messageId })
//
// ④ 看历史：
//   await window.__TAURI__.core.invoke('chat_history', { conversationId: '<ULID>', limit: 100 })

use tauri::{AppHandle, State};

use crate::services::chat::service::{ChatService, SendResult};
use crate::services::memory::MessageRecord;

const INPUT_MAX_LEN: usize = 8000;
const HISTORY_LIMIT_MAX: u32 = 1000;

fn validate_input(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input 不能为空".to_string());
    }
    if input.chars().count() > INPUT_MAX_LEN {
        return Err(format!("input 长度超限（≤{INPUT_MAX_LEN} 字符）"));
    }
    Ok(input.to_string())
}

fn validate_conversation_id(id: &str) -> Result<&str, String> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    Ok(trimmed)
}

#[tauri::command]
pub async fn chat_send(
    app: AppHandle,
    service: State<'_, ChatService>,
    input: String,
    #[allow(non_snake_case)] conversationId: Option<String>,
) -> Result<SendResult, String> {
    let input = validate_input(&input)?;
    let conv_id = match conversationId {
        Some(id) => Some(validate_conversation_id(&id)?.to_string()),
        None => None,
    };
    service
        .send(&app, input, conv_id)
        .await
        .map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn chat_cancel(
    service: State<'_, ChatService>,
    #[allow(non_snake_case)] messageId: String,
) -> Result<(), String> {
    let id = messageId.trim();
    if id.is_empty() {
        return Err("messageId 不能为空".to_string());
    }
    service.cancel(id).map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn chat_history(
    app: AppHandle,
    service: State<'_, ChatService>,
    #[allow(non_snake_case)] conversationId: String,
    limit: u32,
) -> Result<Vec<MessageRecord>, String> {
    let conv_id = validate_conversation_id(&conversationId)?.to_string();
    let limit = limit.min(HISTORY_LIMIT_MAX).max(1);
    service
        .history(&app, &conv_id, limit)
        .await
        .map_err(|e| format!("{e}"))
}
