// #13 Chat IPC commands（M1 W2 ChatService 业务编排入口）
//
// 修正 #13 原契约（详 plan c-issue-13-https-github-com-tl0502-apet-ancient-moth）：
// 老 chat_send 是 async fn 直到流式结束才 resolve → 前端拿不到 messageId 全程 →
// cancel 按钮成死按钮、切换会话被锁。新契约用 tauri::ipc::Channel<StreamEvent>
// 把流式跑在 spawn 内，IPC 立即返 SendResult，cancel 即可用。
//
// 3 个 chat command + 3 个会话管理 command：
// - chat_send(input, conversationId?, onStream: Channel<StreamEvent>) → { messageId, conversationId }
//     conversationId None → ensure_active_conversation 复用或新建
//     立即返 IDs；流式事件通过 onStream 回前端（Delta/Done/Error 三 variant）
// - chat_cancel(messageId) → ()
//     按 message_id 触发活跃 chat_stream 的 CancellationToken；不存在 = no-op
// - chat_history(conversationId, limit) → Vec<MessageRecord>
//     按 created_at 升序返回；ChatPanel 翻历史用
//
// dev console 验证脚本（settings 窗口 / pet 窗口任一 DevTools，withGlobalTauri 已启）：
//
// ① 简单 send + 流式可视化（Channel API 用法）：
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
//   await invoke('chat_cancel', { messageId: r.messageId })  // ← 现在能用，因为 messageId 已拿到
//
// ③ 看历史：
//   await invoke('chat_history', { conversationId: '<ULID>', limit: 100 })

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::services::chat::conversation::{
    create_conversation, list_conversations, set_active_conversation, ConversationSummary,
};
use crate::services::chat::service::{ChatService, SendResult, StreamEvent};
use crate::services::memory::MessageRecord;
use crate::services::persona::load_active_persona;

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
    #[allow(non_snake_case)] onStream: Channel<StreamEvent>,
) -> Result<SendResult, String> {
    let input = validate_input(&input)?;
    let conv_id = match conversationId {
        Some(id) => Some(validate_conversation_id(&id)?.to_string()),
        None => None,
    };
    // 同步阶段：失败直接 reject IPC（前端走 catch）
    let prepared = service
        .prepare(&app, input, conv_id)
        .await
        .map_err(|e| format!("{e}"))?;
    let result = SendResult {
        message_id: prepared.assistant_id.clone(),
        conversation_id: prepared.conv_id.clone(),
    };
    // 异步阶段：spawn 跑流式；clone ChatService 进 task（active_streams 是 Arc，零成本）
    let svc = service.inner().clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        svc.run_stream(&app_clone, prepared, onStream).await;
    });
    Ok(result)
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

/// 列出未归档 conversation（侧边栏 ChatGPT 式列表用）。
///
/// limit 默认 50，clamp [1, 200]；按 last_activity_at DESC 返回。
#[tauri::command]
pub async fn chat_list_conversations(
    app: AppHandle,
    limit: Option<u32>,
) -> Result<Vec<ConversationSummary>, String> {
    let limit = limit.unwrap_or(50);
    list_conversations(&app, limit)
        .await
        .map_err(|e| format!("{e}"))
}

/// 显式新建 conversation + 切换 active KV（侧边栏"新建对话"按钮路径）。
///
/// 不接 personaId 入参：M1 永远使用当前 active persona（与 ensure_active_conversation 同款）。
/// 返回新 conversation id；前端立即把它作为 chat_send 的 conversationId 入参。
#[tauri::command]
pub async fn chat_create_conversation(app: AppHandle) -> Result<String, String> {
    let persona = load_active_persona(&app)
        .await
        .map_err(|e| format!("load active persona: {e}"))?;
    create_conversation(&app, &persona.id)
        .await
        .map_err(|e| format!("{e}"))
}

/// 切换活跃 conversation（点列表项路径；只更新 KV，不影响 messages 表）。
#[tauri::command]
pub async fn chat_set_active_conversation(
    app: AppHandle,
    #[allow(non_snake_case)] conversationId: String,
) -> Result<(), String> {
    let id = validate_conversation_id(&conversationId)?.to_string();
    set_active_conversation(&app, &id)
        .await
        .map_err(|e| format!("{e}"))
}
