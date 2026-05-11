// chat/service.rs — ChatService::{prepare, run_stream, cancel, history}
// （M1 W2 #13 → 修正：用 tauri::ipc::Channel 替代全局 emit；详 plan
// `c-issue-13-https-github-com-tl0502-apet-ancient-moth`）
//
// 业务编排：把 Persona / Nickname / Memory / Conversation / LLMProvider 串成
// "chat_send → prepare 同步返 → spawn run_stream 流式 token → channel.send(Done)"
// 的真业务对话路径。
//
// 关键设计（v2 修正版）：
// - active_streams: Arc<Mutex<HashMap<message_id, CancellationToken>>>，cancel 用
// - prepare 同步：active persona / ensure conv / 写 user msg / 拼 prompt / build_provider /
//   生成 assistant_id / 注册 cancel token；返 PreparedSend
// - run_stream 异步：消费 PreparedSend 跑流式；4 分支收尾通过 channel.send(StreamEvent::*) 回前端
// - 取消 = StreamEvent::Done finishReason='cancelled' + 已收 token 入库
// - 网络/server error = 抽 # 拒答 模板 + 写 mode='offline_rule' + send Delta+Done
// - 其他 error（AuthFailed / BadRequest / ParseError / RateLimit）= send Error，不入库
//
// Provider 注入：每次 prepare 都从 config 表读三键现建 OpenAIProvider（与 #12 chat_send_test
// 同模式）。用户改配置立即生效，无 hot reload 烦恼；M3 多 provider 时改成
// ProviderRegistry::resolve(active_id)。
//
// 为什么用 ipc::Channel 而非 app.emit（修正 #13 原契约）：
// - 老契约 chat_send 是 async fn 直到流式结束才 resolve → 前端拿不到 assistant_id 全程
//   → cancel 按钮成死按钮、切换会话被锁
// - Channel 自带 scope（每个 invoke 一条），不需 messageId 路由；类型安全；并发隔离

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::Mutex;
use serde::Serialize;
use sqlx::Connection;
use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime};
use tokio_util::sync::CancellationToken;
use ulid::Ulid;

use crate::services::chat::conversation::{
    ensure_active_conversation_with_conn, update_last_activity,
};
use crate::services::chat::prompt::build_messages;
use crate::services::chat::ChatError;
use crate::services::db::open_app_db;
use crate::services::llm::{
    ChatMessage, ChatOptions, FinishReason, LLMError, LLMProvider, OpenAIProvider, StreamDelta,
    Usage,
};
use crate::services::llm_providers;
use crate::services::memory::{
    delete_message_with_conn, insert_message_with_conn, list_messages_by_conversation,
    list_messages_by_conversation_with_conn, update_message_content_with_conn, MessageRecord,
};
use crate::services::nickname::get_user_nickname_with_conn;
use crate::services::persona::{load_active_persona_with_conn, PersonaSummary};

// === 业务常量 ===
/// 历史窗口（M1 N=10 取最近 N 条；M3 接 ContextManager 改为 token 预算自适应）。
const HISTORY_LIMIT: u32 = 10;
/// 全局兜底拒答（persona-design.md §7.5 降级链末端）。
const FALLBACK_REFUSAL: &str = "这个我现在没法陪你聊，要不我们换个话题？";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendResult {
    pub message_id: String,
    /// B4 修复：user message id 也返回，前端用它替换乐观 push 的 `pending-user-*` 临时 id，
    /// 避免 session 内 user 气泡 ID 与 DB row 不对应（关闭重开会话才能自愈的小割裂）。
    pub user_message_id: String,
    /// 当前轮所属的 conversation ID（caller 传 None 时由 ensure_active_conversation 决定）。
    /// dev 验收 + ChatPanel 切会话都需要；省一次 IPC。
    pub conversation_id: String,
}

/// 流式事件 —— 通过 `tauri::ipc::Channel<StreamEvent>` 发回前端。
///
/// 序列化形态（前端 onmessage 拿到的 JSON）：
/// - `{ "type": "delta", "token": "你" }`
/// - `{ "type": "done", "totalTokens": 42, "finishReason": "stop" }`
/// - `{ "type": "error", "errorKind": "AuthFailed", "message": "401 ..." }`
///
/// finishReason 取值见 finish_reason_to_str：stop / length / tool_calls / content_filter /
/// error / cancelled / offline_rule。
///
/// channel 自带 scope（一个 invoke 一条），故 payload 不带 messageId（前端从 SendResult
/// 拿到的 messageId 就是这条 channel 服务的 assistant 消息）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum StreamEvent {
    Delta {
        token: String,
    },
    #[serde(rename_all = "camelCase")]
    Done {
        total_tokens: u32,
        finish_reason: String,
    },
    #[serde(rename_all = "camelCase")]
    Error {
        error_kind: String,
        message: String,
    },
}

/// prepare 阶段产出，run_stream 消费。
///
/// 把所有"需要 await DB / IPC"的工作集中到 prepare（同步返），把"需要长时间跑流式"的
/// 工作留给 spawn 出去的 run_stream。
pub struct PreparedSend {
    pub assistant_id: String,
    /// B4：刚 INSERT 的 user message id，IPC 同步返给前端用于替换 optimistic 临时 id。
    pub user_id: String,
    pub conv_id: String,
    pub persona: PersonaSummary,
    pub messages: Vec<ChatMessage>,
    pub provider: OpenAIProvider,
    pub cancel_token: CancellationToken,
}

#[derive(Default, Clone)]
pub struct ChatService {
    /// 在飞 chat_stream 的 cancel token map（key = assistant message_id ULID）。
    /// 用 Arc<Mutex<...>> 让 ChatService 可 Clone 进 spawn 出去的 task。
    active_streams: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl ChatService {
    pub fn new() -> Self {
        Self::default()
    }

    /// chat_send 同步阶段：解析 conv / 拼 prompt / 现建 provider / 写 user msg /
    /// 生成 assistant_id / 注册 cancel token；返 PreparedSend 给上层 spawn。
    ///
    /// B1 修复：prepare 内的 DB 调用合并到一条 conn 上跑（active persona / ensure conv /
    /// get nickname / list history / get active provider / insert user / insert assistant placeholder），
    /// 把 chat_send 单轮的 open/close 周期从 8 次降到 2 次（prepare 1 + run_stream 收尾的 update + last_activity）。
    ///
    /// 顺序约束：build_messages / build_provider 这两个**会 fail 的纯计算**放在
    /// "INSERT user_record"之前；user_record 必须在 placeholder 之前（messages.created_at
    /// 升序排列依赖此先后）。早先版本是"先 INSERT user_record 再 build → 失败 DELETE 回滚"，
    /// 但 DELETE 也失败时（FK / 锁 / 磁盘满）会留孤儿 user 行，#3 修复时调整为现在的顺序。
    pub async fn prepare<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        input: String,
        conversation_id: Option<String>,
    ) -> Result<PreparedSend, ChatError> {
        let mut conn = open_app_db(app).await?;

        // 1. 加载 active persona + 解析或建 conversation
        let active_persona = load_active_persona_with_conn(&mut conn).await?;
        // caller-provided conv_id 的归属校验放到 tx 内做（缩小 race 窗口）；先记下意图，
        // 真正的 SELECT/校验在 INSERT 之前同 tx 内执行。None 路径仍走 ensure_active 自愈。
        let resolved_conv_id: Option<String> = match &conversation_id {
            Some(id) => Some(id.clone()),
            None => {
                Some(ensure_active_conversation_with_conn(&mut conn, &active_persona.id).await?)
            }
        };

        // 2. 加载用户昵称 + 历史 N=10（先读再写：LIMIT N 自然就是"不含当前轮的最近 N 条"，
        //    早先版本"先 INSERT user_record 再 LIMIT N 又过滤当前 ID"会让 LLM 实际只看到
        //    N-1 条历史，长对话时人格连续性受损。宠物名字直接用 persona.name；M1 已无
        //    pet_nickname 机制）。
        //
        //    注：caller 传 conv_id 时这里读历史可能撞到"刚被另一窗口删掉"的状态——读不到
        //    历史就当空历史走，由后面的 tx 内 SELECT + INSERT FK 保护把致命错落地。
        let user_nick = get_user_nickname_with_conn(&mut conn).await?;
        let conv_id_for_history = resolved_conv_id.as_deref().unwrap_or("");
        let history = list_messages_by_conversation_with_conn(
            &mut conn,
            conv_id_for_history,
            Some(HISTORY_LIMIT),
        )
        .await?;
        // A6 修复（2026-05-09）：mode='offline_rule' 的 assistant 是断网时本地拒答模板（非
        // LLM 实际输出）。下次进 LLM history 会让 LLM 看到自己（伪装）说过这话，可能延续
        // 被动语气或反问"我刚才为什么这么说"。UI 仍正常展示（chat_history IPC 不受影响）。
        //
        // Issue #1 修复：mode='cancelled' 同理——是用户中途取消的半句话，下一轮喂给 LLM
        // 会让它"延续半截输出"或反问"我刚才说到哪了"，影响人格连续性。UI 仍可见。
        let history_filtered: Vec<MessageRecord> = history
            .into_iter()
            .filter(|r| {
                !(r.role == "assistant" && (r.mode == "offline_rule" || r.mode == "cancelled"))
            })
            .collect();

        // 3. 拼 messages（含安全前缀占位 + 4 节人格 + 昵称 bullets + re-anchor + 历史 + 包装后的 user）。
        //    在 INSERT user_record 之前做完，任一失败直接 ?；不需要"先 INSERT 再回滚"路径
        //    （后者在 DELETE 也失败时会留孤儿 user 行）。
        let messages = build_messages(
            &active_persona,
            user_nick.as_deref(),
            &active_persona.name,
            &history_filtered,
            &input,
        )?;

        // 4. 现建 OpenAIProvider（与 #12 chat_send_test 同模式）。同样在 INSERT 之前。
        let provider = build_provider_with_conn(&mut conn).await?;

        // 5. caller 传 conv_id 时校验存在 + 归属（**在 tx 外做**）。
        //    为什么不放 tx 内：sqlx 默认 `BEGIN DEFERRED`，tx 第一句是 SELECT 拿读锁，
        //    后续 INSERT 升级写锁；这种 read→write upgrade 模式下，若另一连接已 commit
        //    写入，本 tx 升级会**立即** SQLITE_BUSY，**busy_timeout 不保护此路径**
        //    （SQLite 文档明示：upgrading to a write transaction... 立即失败）。
        //    把 SELECT 移出 tx 后，tx 第一句即 INSERT → 直接拿 RESERVED 写锁，无 upgrade，
        //    busy_timeout = 5000 在 prepare 全程生效。
        //    SELECT 与下面 INSERT 之间有微秒级 race（另一窗口刚 archive/delete 这个 conv），
        //    INSERT 撞 FK 时由下方 FK 错误转译兜底，与 SELECT 路径返同款友好文案。
        let conv_id = match &conversation_id {
            Some(id) => {
                let row: Option<(String,)> =
                    sqlx::query_as("SELECT persona_id FROM conversations WHERE id = ?")
                        .bind(id)
                        .fetch_optional(&mut conn)
                        .await?;
                let row = row.ok_or_else(|| {
                    ChatError::Database(format!("对话不存在或已被删除：{id}"))
                })?;
                if row.0 != active_persona.id {
                    return Err(ChatError::Database(format!(
                        "会话归属不匹配：对话 {id} 属于 persona {}，当前 active 是 {}",
                        row.0, active_persona.id
                    )));
                }
                id.clone()
            }
            None => resolved_conv_id
                .clone()
                .expect("ensure_active_conversation_with_conn 已在前置步骤兜底产出 id"),
        };

        // 6. 构造 user_record + placeholder（conv_id 已决议）。
        let user_record = MessageRecord {
            conversation_id: conv_id.clone(),
            ..build_user_record_pending(&input)
        };
        let assistant_id = Ulid::new().to_string();
        let placeholder = MessageRecord {
            id: assistant_id.clone(),
            conversation_id: conv_id.clone(),
            role: "assistant".to_string(),
            content: String::new(),
            mode: "online".to_string(),
            created_at: Utc::now().to_rfc3339(),
        };

        // 7. 写 user message + placeholder（包在同一 tx 里原子提交）。
        //    #4 修复：早先两次 INSERT 是独立调用，"user_record INSERT 成功 + placeholder
        //    INSERT 失败"（FK / 锁 / 磁盘满）会留孤儿 user 行——下次 chat_history 拉到一条
        //    没有 assistant 回应的孤立 user 句子。tx 任意一步失败整体回滚，DB 状态原子。
        //    placeholder 仍在 prepare 期 INSERT（A4 约束）：流式 token 通过 Channel 发回前端
        //    时 DB 必须已有此行；run_stream 的 4 个收尾分支只 UPDATE / DELETE 此行。
        //
        //    tx 第一句即 INSERT（无 SELECT）→ 直接拿写锁，并发 prepare 撞写锁时由
        //    busy_timeout = 5000 排队等待，而非立即 BUSY。
        //
        //    FK 错误转译：SELECT 与 INSERT 之间有微秒级 race；INSERT 撞 FK 时翻译为与
        //    上方 SELECT 路径同款的友好文案。
        let mut tx = conn.begin().await?;
        if let Err(e) = insert_message_with_conn(&mut tx, &user_record).await {
            let s = e.to_string();
            if s.contains("FOREIGN KEY") {
                return Err(ChatError::Database(format!(
                    "对话不存在或已被删除：{conv_id}"
                )));
            }
            return Err(e.into());
        }
        if let Err(e) = insert_message_with_conn(&mut tx, &placeholder).await {
            let s = e.to_string();
            if s.contains("FOREIGN KEY") {
                return Err(ChatError::Database(format!(
                    "对话不存在或已被删除：{conv_id}"
                )));
            }
            return Err(e.into());
        }
        tx.commit().await?;
        conn.close().await?;

        // 7. 注册 cancel token
        let cancel = CancellationToken::new();
        {
            let mut map = self.active_streams.lock();
            map.insert(assistant_id.clone(), cancel.clone());
        }

        Ok(PreparedSend {
            assistant_id,
            user_id: user_record.id,
            conv_id,
            persona: active_persona,
            messages,
            provider,
            cancel_token: cancel,
        })
    }

    /// chat_send 异步阶段：跑流式 + 4 分支收尾。
    ///
    /// 内部不返 Result —— 所有错误通过 channel.send(StreamEvent::Error) 回前端；
    /// channel send 失败仅 log，因为 spawn 出去后不再有调用者能 propagate。
    /// 必保证 channel 末尾要么 Done 要么 Error 一次（前端据此清 currentStreamId）。
    pub async fn run_stream<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        prepared: PreparedSend,
        channel: Channel<StreamEvent>,
    ) {
        let PreparedSend {
            assistant_id,
            user_id: _,
            conv_id,
            persona,
            messages,
            provider,
            cancel_token,
        } = prepared;

        // 流式调用 + 收集 buffer + send Delta
        let buffer = Arc::new(Mutex::new(String::new()));
        let buffer_for_cb = buffer.clone();
        let channel_for_cb = channel.clone();
        let on_delta: Box<dyn Fn(StreamDelta) + Send + Sync> = Box::new(move |delta| {
            if let StreamDelta::TextDelta(text) = &delta {
                buffer_for_cb.lock().push_str(text);
                if let Err(e) = channel_for_cb.send(StreamEvent::Delta {
                    token: text.clone(),
                }) {
                    eprintln!("[chat] channel send Delta failed: {e}");
                }
            }
            // ToolCallDelta / Finish：M1 不接 tools，不会触发；忽略
        });

        let stream_result = provider
            .chat_stream(messages, ChatOptions::default(), cancel_token, on_delta)
            .await;

        // 清理 token 槽（无论成功失败；下次 prepare 重新登记）
        self.active_streams.lock().remove(&assistant_id);

        let collected = buffer.lock().clone();

        // 4 主分支收尾（每分支内 DB 失败转 Error）：成功 / 取消 / 网络降级 / 其他错误
        // A4 修复：assistant 行已在 prepare 期 INSERT；这里只做 UPDATE / DELETE。
        match stream_result {
            Ok(finish) => {
                if let Err(e) = update_assistant_msg(app, &assistant_id, &collected, "online").await
                {
                    let _ = channel.send(StreamEvent::Error {
                        error_kind: "DbError".to_string(),
                        message: e.to_string(),
                    });
                    return;
                }
                if let Err(e) = update_last_activity(app, &conv_id).await {
                    eprintln!("[chat] update_last_activity failed: {e}");
                }
                let _ = channel.send(StreamEvent::Done {
                    total_tokens: finish.usage.map(|u| u.total_tokens).unwrap_or(0),
                    finish_reason: finish_reason_to_str(&finish.reason),
                });
            }
            Err(LLMError::Cancelled { partial_usage }) => {
                // Issue #1 + #7 修复：
                // - 空 buffer（"还没开始就被 cancel"）→ DELETE placeholder（与 Error 分支同款），
                //   不留下空气泡，UI/DB 都看不见这一轮。
                // - 非空 buffer → UPDATE 写入 mode='cancelled'（不是 'online'），下一轮 history
                //   过滤会跳过这条，避免污染 LLM 上下文（与 offline_rule 同款理由）。
                if collected.is_empty() {
                    if let Err(del_err) = delete_assistant_msg(app, &assistant_id).await {
                        eprintln!(
                            "[chat] delete empty placeholder on cancel failed: {del_err}"
                        );
                    }
                } else if let Err(e) =
                    update_assistant_msg(app, &assistant_id, &collected, "cancelled").await
                {
                    let _ = channel.send(StreamEvent::Error {
                        error_kind: "DbError".to_string(),
                        message: e.to_string(),
                    });
                    return;
                }
                if let Err(e) = update_last_activity(app, &conv_id).await {
                    eprintln!("[chat] update_last_activity failed: {e}");
                }
                // Issue #6 修复：cancel 时透传 provider 已收到的 partial_usage（OpenAI
                // include_usage 在中段就可能下发），别硬编码 0。
                let total_tokens = partial_usage.map(|u: Usage| u.total_tokens).unwrap_or(0);
                let _ = channel.send(StreamEvent::Done {
                    total_tokens,
                    finish_reason: "cancelled".to_string(),
                });
            }
            Err(LLMError::Network(_)) | Err(LLMError::ServerError(_)) => {
                // 离线降级：抽 # 拒答 模板（# 共情 / # 问候 与"网络断了"语义不贴）
                let templates = extract_refusal_templates(&persona.raw_markdown);
                let refusal = pick_refusal(&templates, &assistant_id);
                if let Err(e) =
                    update_assistant_msg(app, &assistant_id, &refusal, "offline_rule").await
                {
                    let _ = channel.send(StreamEvent::Error {
                        error_kind: "DbError".to_string(),
                        message: e.to_string(),
                    });
                    return;
                }
                if let Err(e) = update_last_activity(app, &conv_id).await {
                    eprintln!("[chat] update_last_activity failed: {e}");
                }
                // send 一个 Delta 让前端能渲染，再 send Done finishReason='offline_rule'
                let _ = channel.send(StreamEvent::Delta {
                    token: refusal.clone(),
                });
                let _ = channel.send(StreamEvent::Done {
                    total_tokens: 0,
                    finish_reason: "offline_rule".to_string(),
                });
            }
            Err(e) => {
                // AuthFailed / RateLimit / BadRequest / ParseError → 删预插行 + send Error 不入库
                // 与前端 handleStreamError splice 行为对称（DB 与 UI 都看不见这一轮）。
                //
                // DELETE 失败 best-effort fallback：把 placeholder 改写为 offline_rule + 拒答模板，
                // 改走 Delta+Done='offline_rule' 路径（不再 send Error），与 DB 视图对齐——
                // 否则会出现"DB 留 offline_rule 文案 + UI 当场 splice 删行 + toast"语义分裂，
                // 用户重启后看到一句没见过的 AI 文案。
                match delete_assistant_msg(app, &assistant_id).await {
                    Ok(()) => {
                        let _ = channel.send(StreamEvent::Error {
                            error_kind: error_kind_str(&e).to_string(),
                            message: e.to_string(),
                        });
                    }
                    Err(del_err) => {
                        eprintln!("[chat] delete placeholder on error failed: {del_err}");
                        let templates = extract_refusal_templates(&persona.raw_markdown);
                        let placeholder_text = pick_refusal(&templates, &assistant_id);
                        match update_assistant_msg(
                            app,
                            &assistant_id,
                            &placeholder_text,
                            "offline_rule",
                        )
                        .await
                        {
                            Ok(()) => {
                                if let Err(act_err) = update_last_activity(app, &conv_id).await {
                                    eprintln!(
                                        "[chat] update_last_activity failed: {act_err}"
                                    );
                                }
                                let _ = channel.send(StreamEvent::Delta {
                                    token: placeholder_text.clone(),
                                });
                                let _ = channel.send(StreamEvent::Done {
                                    total_tokens: 0,
                                    finish_reason: "offline_rule".to_string(),
                                });
                            }
                            Err(upd_err) => {
                                eprintln!(
                                    "[chat] best-effort offline_rule fallback also failed: {upd_err}"
                                );
                                // 极端：DELETE 与 UPDATE 都失败 → 仍发 Error 让前端清占位，
                                // DB 残留靠启动期 GC 兜底（这是最后一道防线）。
                                let _ = channel.send(StreamEvent::Error {
                                    error_kind: error_kind_str(&e).to_string(),
                                    message: e.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    /// 按 message_id 触发活跃 chat_stream 的取消。message_id 不存在 = no-op。
    pub fn cancel(&self, message_id: &str) -> Result<(), ChatError> {
        let map = self.active_streams.lock();
        if let Some(token) = map.get(message_id) {
            token.cancel();
        }
        Ok(())
    }

    /// 按 conversation_id + limit 列消息（按 created_at 升序）。
    pub async fn history<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        conversation_id: &str,
        limit: u32,
    ) -> Result<Vec<MessageRecord>, ChatError> {
        let records = list_messages_by_conversation(app, conversation_id, Some(limit)).await?;
        Ok(records)
    }
}

// === Helper functions ===

/// 生成 user 消息记录（conversation_id 留空字符串占位，prepare 内会用真值覆盖）。
///
/// 拆 "pending" 形式是因为 prepare 现在把 conv_id 的最终决定推迟到 tx 内（Issue #2 / #8 修复），
/// 而 ULID + content 可以提前算好。
fn build_user_record_pending(content: &str) -> MessageRecord {
    MessageRecord {
        id: Ulid::new().to_string(),
        conversation_id: String::new(),
        role: "user".to_string(),
        content: content.to_string(),
        mode: "online".to_string(),
        created_at: Utc::now().to_rfc3339(),
    }
}

async fn update_assistant_msg<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    content: &str,
    mode: &str,
) -> Result<(), ChatError> {
    let mut conn = open_app_db(app).await?;
    update_message_content_with_conn(&mut conn, id, content, mode).await?;
    conn.close().await?;
    Ok(())
}

async fn delete_assistant_msg<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<(), ChatError> {
    let mut conn = open_app_db(app).await?;
    delete_message_with_conn(&mut conn, id).await?;
    conn.close().await?;
    Ok(())
}

async fn build_provider_with_conn(
    conn: &mut sqlx::SqliteConnection,
) -> Result<OpenAIProvider, ChatError> {
    let record = llm_providers::get_active_record_with_conn(conn)
        .await
        .map_err(|e| ChatError::Llm(format!("read active provider: {e}")))?
        .ok_or_else(|| {
            ChatError::Llm(
                "未配置 LLM Provider；请到设置面板 → LLM Provider 添加并激活一个".to_string(),
            )
        })?;
    if record.api_key.is_empty() {
        return Err(ChatError::Llm(
            "active provider 的 API Key 为空；请到设置面板填写".to_string(),
        ));
    }
    OpenAIProvider::new("openai", &record.base_url, record.api_key, &record.model)
        .map_err(|e| ChatError::Llm(format!("provider init: {e}")))
}

fn finish_reason_to_str(r: &FinishReason) -> String {
    match r {
        FinishReason::Stop => "stop".to_string(),
        FinishReason::Length => "length".to_string(),
        FinishReason::ToolCalls => "tool_calls".to_string(),
        FinishReason::ContentFilter => "content_filter".to_string(),
        FinishReason::Error => "error".to_string(),
        // Issue #3 修复：透传上游未知值原文（前端 ChatFinishReason 类型有 string 兜底接住）。
        FinishReason::Unknown(s) => s.clone(),
    }
}

fn error_kind_str(e: &LLMError) -> &'static str {
    match e {
        LLMError::AuthFailed(_) => "AuthFailed",
        LLMError::RateLimit(_) => "RateLimit",
        LLMError::BadRequest(_) => "BadRequest",
        LLMError::ParseError(_) => "ParseError",
        // Issue #5：以下三个变体被 run_stream 上游分支拦下后不会到这里。
        // unreachable! 而非删除映射，是为了让"未来若新增 LLMError 变体却忘了在 run_stream 加分支"
        // 能在运行时立即暴露（而不是沉默地路由错信号到前端）。
        LLMError::Network(_) | LLMError::ServerError(_) => unreachable!(
            "Network/ServerError are handled by the offline_rule branch in run_stream"
        ),
        LLMError::Cancelled { .. } => {
            unreachable!("Cancelled is handled by its dedicated branch in run_stream")
        }
    }
}

/// 从 .soul.md raw_markdown 抽 # 离线模板 / ## 拒答 子节的 bullet items。
///
/// 找不到任何模板返空 Vec；调用方 pick_refusal 兜底全局字符串。
pub(crate) fn extract_refusal_templates(raw_md: &str) -> Vec<String> {
    let mut in_offline = false;
    let mut in_refusal = false;
    let mut templates = Vec::new();

    for line in raw_md.lines() {
        // H1 切换（"# " 开头但不是 "## "）
        if line.starts_with("# ") {
            let body = line.trim_start_matches('#').trim();
            in_offline = body.starts_with("离线模板");
            in_refusal = false;
            continue;
        }

        // H2 切换（仅 in_offline 时关注）
        if in_offline && line.starts_with("## ") {
            let body = line.trim_start_matches('#').trim();
            in_refusal = body.starts_with("拒答") || body.starts_with("Refusal");
            continue;
        }

        // 抽 bullet（- 开头）
        if in_refusal {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("- ") {
                let item = rest.trim();
                if !item.is_empty() {
                    templates.push(item.to_string());
                }
            }
        }
    }

    templates
}

/// B10 / #8：用 nano-time + ULID 解析后真随机字段做轻量随机抽样，避免引 rand crate。
///
/// 仅靠 nanos 取模在快速连续两次降级（1ms 内）会模出极接近的 idx。
/// 早先用 `assistant_id.as_bytes().iter().sum()` 做 hash 把 ULID 强随机字段塞进种子，
/// 但 26 字符字节和落在 1500-2000 窄区间 → 对 3-条模板池 mod 几乎产生固定值。
/// 改用 `Ulid::from_string(...).0 as usize` 直接拿 u128 低 64 bit，分布大幅改善。
///
/// **假设**：`assistant_id` 是合法 ULID（prepare 内由 `Ulid::new()` 生成必满足）。
/// 若未来换 id 生成器（uuidv7 / sequence / 业务自造），此处 parse 会一直 fallback 0
/// → 退化为纯纳秒抽样，1ms 内连发又会撞 idx。换 id 时必须同步重写此处。
pub(crate) fn pick_refusal(templates: &[String], assistant_id: &str) -> String {
    if templates.is_empty() {
        return FALLBACK_REFUSAL.to_string();
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as usize)
        .unwrap_or(0);
    // assistant_id 由 prepare 内 Ulid::new() 生成 → 必合法；解析失败兜底 0（不应发生）。
    let id_hash: usize = Ulid::from_string(assistant_id)
        .map(|u| u.0 as usize)
        .unwrap_or(0);
    let idx = (nanos ^ id_hash) % templates.len();
    templates[idx].clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOMO_RAW: &str = include_str!("../../../personas/_builtin/momo.soul.md");

    #[test]
    fn extract_refusal_from_momo_returns_three_items() {
        // momo.soul.md ## 拒答 / Refusal 池有 3 条
        let templates = extract_refusal_templates(MOMO_RAW);
        assert_eq!(
            templates.len(),
            3,
            "momo 拒答模板池应有 3 条；got {} items: {:?}",
            templates.len(),
            templates
        );
        assert!(templates.iter().any(|s| s.contains("不太擅长")));
        assert!(templates.iter().any(|s| s.contains("陪你想想其他的")));
        assert!(templates.iter().any(|s| s.contains("超出我能帮的范围")));
    }

    #[test]
    fn extract_refusal_returns_empty_when_no_offline_section() {
        let raw = "# 身份\nx\n\n# 性格\n- y\n\n# 能力\n- z\n\n# 行为规则\n## Do\n- a";
        let templates = extract_refusal_templates(raw);
        assert!(templates.is_empty());
    }

    #[test]
    fn extract_refusal_returns_empty_when_no_refusal_subsection() {
        let raw = "# 离线模板\n## 共情 / Empathy\n- 嗯\n## 问候 / Greeting\n- 嘿";
        let templates = extract_refusal_templates(raw);
        assert!(
            templates.is_empty(),
            "no ## 拒答 subsection should yield empty"
        );
    }

    #[test]
    fn extract_refusal_skips_when_other_h2_starts() {
        // ## 拒答 后跟 ## 调侃，调侃的 bullet 不能混入
        let raw = "# 离线模板\n## 拒答 / Refusal\n- 拒答1\n- 拒答2\n## 调侃 / Banter\n- 调侃1";
        let templates = extract_refusal_templates(raw);
        assert_eq!(templates.len(), 2);
        assert!(templates.iter().all(|s| !s.contains("调侃")));
    }

    #[test]
    fn extract_refusal_terminates_at_next_h1() {
        // ## 拒答 后跟 # 反应配置（H1），反应配置的 bullet 不能混入
        let raw = "# 离线模板\n## 拒答 / Refusal\n- 拒答1\n# 反应配置\n- click.head: x";
        let templates = extract_refusal_templates(raw);
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0], "拒答1");
    }

    #[test]
    fn pick_refusal_falls_back_when_empty() {
        let result = pick_refusal(&[], "any-id");
        assert_eq!(result, FALLBACK_REFUSAL);
    }

    #[test]
    fn pick_refusal_returns_one_of_templates() {
        let templates = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let picked = pick_refusal(&templates, &Ulid::new().to_string());
        assert!(templates.contains(&picked));
    }

    #[test]
    fn pick_refusal_distributes_across_distinct_assistant_ids() {
        // #8：用 ULID 解析后真随机字段做 hash，3 条模板池在 100 次抽样应能全覆盖。
        // 早先版本用 `assistant_id.as_bytes().sum()` 字节求和落在 1500-2000 窄区间，
        // 对 3 条池 mod 几乎产生固定 idx；50 次 ≥2 种的旧断言无法暴露这个分布问题。
        let templates = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            seen.insert(pick_refusal(&templates, &Ulid::new().to_string()));
        }
        assert_eq!(
            seen.len(),
            3,
            "100 个不同 ULID 应触发全部 3 种模板；got {seen:?}"
        );
    }

    #[test]
    fn pick_refusal_handles_invalid_ulid_gracefully() {
        // 防御性：assistant_id 非法 ULID 时不 panic（fallback 到 hash=0 + 仅靠 nanos 区分）
        let templates = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let picked = pick_refusal(&templates, "not-a-ulid");
        assert!(templates.contains(&picked));
    }

    #[test]
    fn finish_reason_to_str_maps_all_variants() {
        assert_eq!(finish_reason_to_str(&FinishReason::Stop), "stop");
        assert_eq!(finish_reason_to_str(&FinishReason::Length), "length");
        assert_eq!(finish_reason_to_str(&FinishReason::ToolCalls), "tool_calls");
        assert_eq!(
            finish_reason_to_str(&FinishReason::ContentFilter),
            "content_filter"
        );
        assert_eq!(finish_reason_to_str(&FinishReason::Error), "error");
        // Issue #3：Unknown(s) 透传原文，前端 ChatFinishReason 类型有 string 兜底
        assert_eq!(
            finish_reason_to_str(&FinishReason::Unknown("safety_filter_v2".into())),
            "safety_filter_v2"
        );
    }

    #[test]
    fn error_kind_str_maps_terminal_variants() {
        // Issue #5：Network / ServerError / Cancelled 在 run_stream 上游被专门分支拦走，
        // 不会到达 error_kind_str；这里只验证终端分支的映射稳定。
        assert_eq!(
            error_kind_str(&LLMError::AuthFailed("x".into())),
            "AuthFailed"
        );
        assert_eq!(
            error_kind_str(&LLMError::RateLimit("x".into())),
            "RateLimit"
        );
        assert_eq!(
            error_kind_str(&LLMError::BadRequest("x".into())),
            "BadRequest"
        );
        assert_eq!(
            error_kind_str(&LLMError::ParseError("x".into())),
            "ParseError"
        );
    }

    #[test]
    #[should_panic(expected = "Network/ServerError")]
    fn error_kind_str_panics_on_network_unreachable() {
        // 验证 Issue #5 的 unreachable 守护：未来若有新分支漏接 Network/ServerError，立即暴露
        let _ = error_kind_str(&LLMError::Network("should not reach".into()));
    }

    #[test]
    #[should_panic(expected = "Cancelled is handled")]
    fn error_kind_str_panics_on_cancelled_unreachable() {
        let _ = error_kind_str(&LLMError::Cancelled {
            partial_usage: None,
        });
    }

    #[test]
    fn cancel_unknown_message_id_is_noop() {
        let svc = ChatService::new();
        let result = svc.cancel("non-existent");
        assert!(result.is_ok(), "cancel on unknown id must be no-op");
    }

    #[test]
    fn cancel_existing_token_triggers_it() {
        let svc = ChatService::new();
        let token = CancellationToken::new();
        {
            let mut map = svc.active_streams.lock();
            map.insert("test-id".to_string(), token.clone());
        }
        assert!(!token.is_cancelled());
        svc.cancel("test-id").unwrap();
        assert!(token.is_cancelled());
    }

    #[test]
    fn stream_event_serializes_delta_with_camel_case_tag() {
        let ev = StreamEvent::Delta {
            token: "你好".to_string(),
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["type"], "delta");
        assert_eq!(json["token"], "你好");
    }

    #[test]
    fn stream_event_serializes_done_with_camel_case_fields() {
        let ev = StreamEvent::Done {
            total_tokens: 42,
            finish_reason: "stop".to_string(),
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["type"], "done");
        assert_eq!(json["totalTokens"], 42);
        assert_eq!(json["finishReason"], "stop");
    }

    #[test]
    fn stream_event_serializes_error_with_camel_case_fields() {
        let ev = StreamEvent::Error {
            error_kind: "AuthFailed".to_string(),
            message: "401 Unauthorized".to_string(),
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["type"], "error");
        assert_eq!(json["errorKind"], "AuthFailed");
        assert_eq!(json["message"], "401 Unauthorized");
    }

    #[test]
    fn service_clone_shares_active_streams() {
        // ChatService 必须可 Clone 且共享 active_streams（spawn 出去的 task 通过 cancel 路径
        // 找到同一个 token map）
        let svc = ChatService::new();
        let svc2 = svc.clone();
        let token = CancellationToken::new();
        {
            let mut map = svc.active_streams.lock();
            map.insert("shared-id".to_string(), token.clone());
        }
        // svc2 应能 cancel 到同一个 token
        svc2.cancel("shared-id").unwrap();
        assert!(token.is_cancelled());
    }
}
