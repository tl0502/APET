// #13 Chat IPC commands（M1 W2 ChatService 业务编排入口）
//
// 修正 #13 原契约（详 plan c-issue-13-https-github-com-tl0502-apet-ancient-moth）：
// 老 chat_send 是 async fn 直到流式结束才 resolve → 前端拿不到 messageId 全程 →
// cancel 按钮成死按钮、切换会话被锁。新契约用 tauri::ipc::Channel<StreamEvent>
// 把流式跑在 spawn 内，IPC 立即返 SendResult，cancel 即可用。
//
// 3 个 chat command + 6 个会话管理 command + 3 个 draft command：
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
use tauri::{AppHandle, State};

use crate::services::chat::conversation::{
    archive_conversation, create_conversation_for_snapshot, delete_conversation,
    list_conversations, rename_conversation, set_active_conversation, ConversationSummary,
};
use crate::services::chat::service::{ChatService, SendResult, StreamEvent};
use crate::services::config;
use crate::services::memory::MessageRecord;
use crate::services::persona::{load_active_persona, load_persona};

const INPUT_MAX_LEN: usize = 8000;
const HISTORY_LIMIT_MAX: u32 = 1000;
/// 草稿长度上限：比 INPUT_MAX_LEN 更宽（用户可能在打 长草稿、贴大段笔记）；
/// 超过 reject 防 DB 单 KV 膨胀。
const DRAFT_MAX_LEN: usize = 16000;
/// V3 多对话并发：草稿持久化 KV 前缀。完整 key = `chat:draft:<convId>`。
/// 与 CLAUDE.md "运行时配置走 config 表 KV，key 用 domain:subdomain:field 格式" 对齐。
const DRAFT_KEY_PREFIX: &str = "chat:draft:";

fn draft_key(conv_id: &str) -> String {
    format!("{DRAFT_KEY_PREFIX}{conv_id}")
}

fn validate_input(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input 不能为空".to_string());
    }
    if trimmed.chars().count() > INPUT_MAX_LEN {
        return Err(format!("input 长度超限（≤{INPUT_MAX_LEN} 字符）"));
    }
    // A5 修复：之前返回原文 input 会把前后空白带进 DB + 包装进 wrap_user_input，
    // 让 LLM 看到 "（保持 X 风格）  hello  "。改返 trim 后值，与"用户实际意图"一致。
    Ok(trimmed.to_string())
}

fn validate_conversation_id(id: &str) -> Result<&str, String> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    // Issue #13：早 fail 校验 ULID 格式（schema 内所有 conversation id 都是 ULID）。
    // 拒非 ULID 字符串可避免一路落到 SQL 才报错，前端也能拿到更精准的诊断。
    ulid::Ulid::from_string(trimmed)
        .map_err(|_| format!("conversationId 格式非法（应为 ULID）：{trimmed}"))?;
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
        user_message_id: prepared.user_id.clone(),
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
    // Issue #4：limit=0 改报错（早先静默 clamp 到 1 会掩盖前端 bug）。上限仍 clamp。
    if limit == 0 {
        return Err("limit 必须 ≥ 1".to_string());
    }
    let limit = limit.min(HISTORY_LIMIT_MAX);
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
/// C7：personaId 可选；不传 → 用当前 active persona 兜底（M1 默认行为不变）。
/// 留这个入参是给 M3 多 persona UI 时无痛扩展用——避免到时候改契约破坏老前端。
#[tauri::command]
pub async fn chat_create_conversation(
    app: AppHandle,
    #[allow(non_snake_case)] personaId: Option<String>,
) -> Result<String, String> {
    let persona = match personaId
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(p) => load_persona(&app, p)
            .await
            .map_err(|e| format!("load persona: {e}"))?,
        None => load_active_persona(&app)
            .await
            .map_err(|e| format!("load active persona: {e}"))?,
    };
    let snapshot_id = persona
        .snapshot_id
        .parse::<i64>()
        .map_err(|_| format!("persona {} 缺少可用 snapshot", persona.id))?;
    create_conversation_for_snapshot(&app, &persona.id, snapshot_id)
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

/// 重命名 conversation。
///
/// title 空字符串 → 写 NULL（恢复"未命名"）；非空 → 写 trim 后值（≤100 字符截断）。
/// 不存在 id → Err（前端列表过期则刷一次再试）。
#[tauri::command]
pub async fn chat_rename_conversation(
    app: AppHandle,
    #[allow(non_snake_case)] conversationId: String,
    title: String,
) -> Result<(), String> {
    let id = validate_conversation_id(&conversationId)?.to_string();
    rename_conversation(&app, &id, &title)
        .await
        .map_err(|e| format!("{e}"))
}

/// 归档 conversation（archived = 1，从列表隐藏）；命中 active KV 时清 KV。
#[tauri::command]
pub async fn chat_archive_conversation(
    app: AppHandle,
    #[allow(non_snake_case)] conversationId: String,
) -> Result<(), String> {
    let id = validate_conversation_id(&conversationId)?.to_string();
    archive_conversation(&app, &id)
        .await
        .map_err(|e| format!("{e}"))
}

/// 硬删 conversation（FK ON DELETE CASCADE 自动删 messages）；命中 active KV 时清 KV。
#[tauri::command]
pub async fn chat_delete_conversation(
    app: AppHandle,
    #[allow(non_snake_case)] conversationId: String,
) -> Result<(), String> {
    let id = validate_conversation_id(&conversationId)?.to_string();
    delete_conversation(&app, &id)
        .await
        .map_err(|e| format!("{e}"))?;
    // 级联清 draft KV（chat:draft:<id>）；orphan 草稿没人查，但占 KV 行无意义
    let _ = config::delete(&app, &draft_key(&id)).await;
    Ok(())
}

/// V3 多对话并发：读对话草稿。空字符串 / 不存在 → None。
#[tauri::command]
pub async fn chat_get_draft(
    app: AppHandle,
    #[allow(non_snake_case)] conversationId: String,
) -> Result<Option<String>, String> {
    let id = validate_conversation_id(&conversationId)?;
    config::get(&app, &draft_key(id))
        .await
        .map_err(|e| format!("{e}"))
}

/// V3 多对话并发：写对话草稿。
/// - 空字符串 → 删 KV（不留 empty value 行；前端可放心 set("")）
/// - 非空 → UPSERT
/// 注意：调用方应 debounce（200ms）避免每次按键都打 IPC。
#[tauri::command]
pub async fn chat_set_draft(
    app: AppHandle,
    #[allow(non_snake_case)] conversationId: String,
    draft: String,
) -> Result<(), String> {
    let id = validate_conversation_id(&conversationId)?.to_string();
    if draft.chars().count() > DRAFT_MAX_LEN {
        return Err(format!("draft 长度超限（≤{DRAFT_MAX_LEN} 字符）"));
    }
    let key = draft_key(&id);
    if draft.is_empty() {
        config::delete(&app, &key).await.map_err(|e| format!("{e}"))
    } else {
        config::set(&app, &key, &draft)
            .await
            .map_err(|e| format!("{e}"))
    }
}

/// V3 多对话并发：删对话草稿（不存在 = no-op）。
/// 一般由 chat_delete_conversation 自动级联调用；前端"清空草稿"按钮可显式调。
#[tauri::command]
pub async fn chat_delete_draft(
    app: AppHandle,
    #[allow(non_snake_case)] conversationId: String,
) -> Result<(), String> {
    let id = validate_conversation_id(&conversationId)?;
    config::delete(&app, &draft_key(id))
        .await
        .map_err(|e| format!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_input_trims_surrounding_whitespace() {
        // A5 修复：之前返回原文导致 "  hello  " 进 DB；现在返 trim 后值。
        assert_eq!(validate_input("  hello  ").unwrap(), "hello");
        assert_eq!(validate_input("\nhi\t").unwrap(), "hi");
    }

    #[test]
    fn validate_input_rejects_empty_and_whitespace_only() {
        assert!(validate_input("").is_err());
        assert!(validate_input("   ").is_err());
        assert!(validate_input("\n\t  ").is_err());
    }

    #[test]
    fn validate_input_length_is_measured_after_trim() {
        // 8000 字符 + 100 个空白前后包裹 → trim 后 8000，应通过；之前会按 8200 拒绝。
        let core = "a".repeat(INPUT_MAX_LEN);
        let padded = format!("   {core}   ");
        assert_eq!(
            validate_input(&padded).unwrap().chars().count(),
            INPUT_MAX_LEN
        );
    }

    #[test]
    fn validate_input_rejects_over_limit_after_trim() {
        let huge = "x".repeat(INPUT_MAX_LEN + 1);
        assert!(validate_input(&huge).is_err());
    }

    // Issue #13：ULID 格式校验
    #[test]
    fn validate_conversation_id_accepts_valid_ulid() {
        let id = ulid::Ulid::new().to_string();
        assert_eq!(validate_conversation_id(&id).unwrap(), id);
    }

    #[test]
    fn validate_conversation_id_accepts_ulid_with_padding() {
        let id = ulid::Ulid::new().to_string();
        let padded = format!("  {id}  ");
        assert_eq!(validate_conversation_id(&padded).unwrap(), id);
    }

    #[test]
    fn validate_conversation_id_rejects_non_ulid() {
        // 长度不对
        assert!(validate_conversation_id("abc").is_err());
        // 含 Crockford Base32 不允许的字符（I/L/O/U）
        assert!(validate_conversation_id("01ABCDEFGHILMNOPQRSTUVWXYZ").is_err());
        // 长度对但全是非 base32
        assert!(validate_conversation_id("!!!!!!!!!!!!!!!!!!!!!!!!!!").is_err());
    }

    #[test]
    fn validate_conversation_id_rejects_empty() {
        assert!(validate_conversation_id("").is_err());
        assert!(validate_conversation_id("   ").is_err());
    }
}
