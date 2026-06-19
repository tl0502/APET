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

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;
use parking_lot::Mutex;
use serde::Serialize;
use sqlx::Connection;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime};
use tokio_util::sync::CancellationToken;
use ulid::Ulid;

use crate::kernel::repos::conversation_repo::SafetyScanStatus;
use crate::kernel::safety_guard::{SafetyGuard, ScanFinalResult, ScanTokenResult};
use crate::kernel::safety_policy::SafetyScope;
use crate::services::chat::conversation::{
    ensure_active_conversation_with_snapshot_with_conn, update_last_activity,
};
use crate::services::chat::prompt::{build_messages_from_profile, PromptBuildInput};
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
use crate::services::persona::{
    get_snapshot_profile_with_conn, load_active_persona_with_conn,
    load_persona_for_conversation_with_conn, PersonaSummary, SoulRuntimeProfile,
};

// === 业务常量 ===
/// 历史窗口（M1 N=10 取最近 N 条；M3 接 ContextManager 改为 token 预算自适应）。
const HISTORY_LIMIT: u32 = 10;
/// 全局兜底拒答（persona-design.md §7.5 降级链末端）。
const FALLBACK_REFUSAL: &str = "这个我现在没法陪你聊，要不我们换个话题？";

#[derive(Debug, Clone, Default)]
struct StreamSafetyState {
    soft_rule_ids: HashSet<String>,
    hard_rule_id: Option<String>,
}

impl StreamSafetyState {
    fn record_soft(&mut self, rule_id: &str) -> bool {
        self.soft_rule_ids.insert(rule_id.to_string())
    }

    fn record_hard(&mut self, rule_id: &str) -> bool {
        if self.hard_rule_id.is_some() {
            return false;
        }
        self.hard_rule_id = Some(rule_id.to_string());
        true
    }

    fn has_soft_hit(&self) -> bool {
        !self.soft_rule_ids.is_empty()
    }
}

fn derive_final_safety_status(
    final_output_enabled: bool,
    stream_state: &StreamSafetyState,
    scan: &crate::kernel::safety_guard::ScanFinalResult,
) -> SafetyScanStatus {
    if stream_state.hard_rule_id.is_some() {
        return SafetyScanStatus::FinalBlocked;
    }
    if !final_output_enabled {
        return if stream_state.has_soft_hit() {
            SafetyScanStatus::StreamSoftBlocked
        } else {
            SafetyScanStatus::Disabled
        };
    }
    match scan {
        crate::kernel::safety_guard::ScanFinalResult::Ok => SafetyScanStatus::FinalOk,
        crate::kernel::safety_guard::ScanFinalResult::Redacted { .. } => {
            SafetyScanStatus::FinalRedacted
        }
        crate::kernel::safety_guard::ScanFinalResult::Blocked { .. } => {
            SafetyScanStatus::FinalBlocked
        }
        crate::kernel::safety_guard::ScanFinalResult::ScanFailed { .. } => {
            SafetyScanStatus::ScanFailed
        }
    }
}

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
    /// Phase A0: SafetyGuard 命中, 前端按 message_id 覆盖现有 assistant 显示内容 (Spec §6.6)。
    ///
    /// 触发时机:
    /// - scan_final → Redacted / Blocked / ScanFailed (Scope #3, A0 已接入)
    /// - scan_token → SoftBlock (Scope #2 mid-stream, A1 wire)
    ///
    /// 前端契约: 收到此事件后用 new_content 覆盖该 message_id 的累积 Delta 缓冲,
    /// 紧接着的 Done 事件正常处理。reason 字段供 UI 提示分支。
    #[serde(rename_all = "camelCase")]
    ReplaceMessage {
        message_id: String,
        new_content: String,
        reason: ReplaceReason,
    },
}

/// 替换原因, 前端按需调整 UI 提示 (Spec §6.6)。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplaceReason {
    /// stream_soft_blocked 流式中的局部替换 (Phase A0 暂未触发, Phase A1 wire mid-stream scan_token)
    #[allow(dead_code)]
    SoftBlockToken,
    /// scan_final → Redacted
    FinalRedacted,
    /// scan_final → Blocked
    FinalBlocked,
    /// SafetyGuard 自身异常, 保守降级
    ScanFailed,
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

/// ChatService — chat 业务编排, 持 SafetyGuard 引用以按 SafetyPolicy 包装 prompt 和扫输入/输出。
///
/// Phase A0.7 重要变更: `safety_guard: Arc<dyn SafetyGuard>` 必填字段, `new` 改为
/// `new(safety_guard)`。原 `Default` impl 被移除 —— ChatService 永远必须有 SafetyGuard；
/// 具体 scope 是否启用由 SafetyPolicy 决定。
///
/// `Clone` 保留: `active_streams` 是 `Arc<Mutex<...>>`, `safety_guard` 是 `Arc<dyn ...>`,
/// clone 仅增加引用计数, 不复制状态; spawn 出去的 task 共享同一 token map 与 guard 实例。
#[derive(Clone)]
pub struct ChatService {
    /// 在飞 chat_stream 的 cancel token map（key = assistant message_id ULID）。
    /// 用 Arc<Mutex<...>> 让 ChatService 可 Clone 进 spawn 出去的 task。
    active_streams: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// SafetyGuard 注入点 (Phase A0.7, runtime contract)。
    /// 来源: Kernel::boot 时 Arc 构造, 经 lib.rs setup 传入此 ChatService::new。
    safety_guard: Arc<dyn SafetyGuard>,
}

impl ChatService {
    pub fn new(safety_guard: Arc<dyn SafetyGuard>) -> Self {
        Self {
            active_streams: Arc::new(Mutex::new(HashMap::new())),
            safety_guard,
        }
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

        // Phase A0.7: Scan Scope #1 — user input pre-flight (Spec §6.6.2)。
        // Redacted 用脱敏文本继续 (用户照常拿到回复, DB user 行内容也是脱敏后),
        // Blocked → 直接 UnsafeInput 错误抛回 IPC; ScanFailed → 保守 Safety 错误。
        let input = match self.safety_guard.scan_user_input(&input) {
            crate::kernel::safety_guard::ScanFinalResult::Ok => input,
            crate::kernel::safety_guard::ScanFinalResult::Redacted { redacted_text, .. } => {
                redacted_text
            }
            crate::kernel::safety_guard::ScanFinalResult::Blocked { rule_ids, .. } => {
                return Err(ChatError::UnsafeInput(format!(
                    "blocked by rules: {:?}",
                    rule_ids
                )));
            }
            crate::kernel::safety_guard::ScanFinalResult::ScanFailed { reason, .. } => {
                return Err(ChatError::SafetyScanFailed(format!(
                    "user input scan failed: {}",
                    reason
                )));
            }
        };

        // 1. 加载 active persona 仅作为“无 conversation_id 且 active KV 不可用时新建会话”的 fallback。
        //    真正用于 prompt 的 persona 必须来自 conversation.persona_snapshot_id，避免旧会话
        //    随 active persona / active snapshot 改变而漂移。
        let active_persona = load_active_persona_with_conn(&mut conn).await?;
        let active_snapshot_id = active_persona.snapshot_id.parse::<i64>().map_err(|_| {
            ChatError::Persona(format!(
                "active persona {} missing valid snapshot id",
                active_persona.id
            ))
        })?;
        let conv_id = match &conversation_id {
            Some(id) => id.clone(),
            None => {
                ensure_active_conversation_with_snapshot_with_conn(
                    &mut conn,
                    &active_persona.id,
                    Some(active_snapshot_id),
                )
                .await?
            }
        };
        let persona = load_persona_for_conversation_with_conn(&mut conn, &conv_id).await?;

        // 2. 加载用户昵称 + 历史 N=10（先读再写：LIMIT N 自然就是"不含当前轮的最近 N 条"，
        //    早先版本"先 INSERT user_record 再 LIMIT N 又过滤当前 ID"会让 LLM 实际只看到
        //    N-1 条历史，长对话时人格连续性受损。宠物名字直接用 persona.name；M1 已无
        //    pet_nickname 机制）。
        //
        //    注：caller 传 conv_id 时这里读历史可能撞到"刚被另一窗口删掉"的状态——读不到
        //    历史就当空历史走，由后面的 tx 内 SELECT + INSERT FK 保护把致命错落地。
        let user_nick = get_user_nickname_with_conn(&mut conn).await?;
        let history =
            list_messages_by_conversation_with_conn(&mut conn, &conv_id, Some(HISTORY_LIMIT))
                .await?;
        // A6 修复（2026-05-09）：mode='offline_rule' 的 assistant 是断网时本地拒答模板（非
        // LLM 实际输出）。下次进 LLM history 会让 LLM 看到自己（伪装）说过这话，可能延续
        // 被动语气或反问"我刚才为什么这么说"。UI 仍正常展示（chat_history IPC 不受影响）。
        //
        // Issue #1 修复：mode='cancelled' 同理——是用户中途取消的半句话，下一轮喂给 LLM
        // 会让它"延续半截输出"或反问"我刚才说到哪了"，影响人格连续性。UI 仍可见。
        //
        // Task 7 review Important 2 修复：Phase A0 安全降级 3 mode 与 offline_rule / cancelled
        // 同理 —— 内容是安全 fallback / redacted text，不是 LLM 实际输出，喂回历史会让 LLM
        // "I just said X" 错乱（复发 A6 pattern）。这些 mode 是 legacy 读兼容；
        // 新写入保持 mode='online'，终态写入 safety_scan_status。
        let history_filtered: Vec<MessageRecord> = history
            .into_iter()
            .filter(|r| {
                !(r.role == "assistant"
                    && (r.mode == "offline_rule"
                        || r.mode == "cancelled"
                        || r.mode == "safety_redacted"
                        || r.mode == "safety_blocked"
                        || r.mode == "safety_scan_failed"))
            })
            .collect();

        // 3. 拼 messages：从 conversation 绑定的 snapshot profile 读取人格材料，
        //    注入 identity/style/examples + 昵称 bullets + re-anchor + 历史 + 包装后的 user。
        //    在 INSERT user_record 之前做完，任一失败直接 ?；不需要"先 INSERT 再回滚"路径
        //    （后者在 DELETE 也失败时会留孤儿 user 行）。
        // A2 invariant: do not fallback to persona.raw_markdown here. Missing profile means the
        // snapshot is not runnable under the runtime contract and must fail before any DB message write.
        let runtime_profile = load_runtime_profile_for_persona(&mut conn, &persona).await?;
        let messages = build_messages_from_profile(PromptBuildInput {
            runtime_profile: &runtime_profile,
            persona_name: &persona.name,
            user_nickname: user_nick.as_deref(),
            pet_nickname: &persona.name,
            history: &history_filtered,
            current_input: &input,
        })?;

        // Phase A0.7: SafetyGuard.wrap_messages 在 PrefixInjection ON 时注入 prefix。
        // 注入点在 build_messages 之后，启用时保证 prefix 是 system message 第一位。
        let messages = self
            .safety_guard
            .wrap_messages(messages, crate::kernel::safety_guard::Locale::ZhCn);

        // 4. 现建 OpenAIProvider（与 #12 chat_send_test 同模式）。同样在 INSERT 之前。
        // Phase A0.5b: 拿 Kernel.secret_repo 让 get_active_record 走 DPAPI 解密回填 api_key。
        // Kernel 未注入（理论上只发生于早于 setup 完成的极端情况）→ secret_repo = None,
        // get_active_record 退化 legacy 明文路径。
        let secret_repo = app
            .try_state::<crate::kernel::Kernel>()
            .map(|kernel| std::sync::Arc::clone(&kernel.secret_repo));
        let provider = build_provider_with_conn(&mut conn, secret_repo.as_ref()).await?;

        // 5. conversation 存在性与 snapshot binding 已在 load_persona_for_conversation_with_conn
        //    里完成。这里不再用 active persona 做归属校验：A1 要求已有会话保持自己的
        //    persona_snapshot_id，不随 active persona 改变而重绑。
        //
        //    历史说明：旧代码在此校验 conversation.persona_id == active_persona.id，会导致切换
        //    active persona 后旧会话不可继续；这是 Persona Snapshot A1 明确要修掉的漂移点。
        //
        //    下面 INSERT 仍由 FK 兜底处理 SELECT 与 INSERT 之间的删除 race。
        //
        //    原并发背景：
        //    为什么不放 tx 内：sqlx 默认 `BEGIN DEFERRED`，tx 第一句是 SELECT 拿读锁，
        //    后续 INSERT 升级写锁；这种 read→write upgrade 模式下，若另一连接已 commit
        //    写入，本 tx 升级会**立即** SQLITE_BUSY，**busy_timeout 不保护此路径**
        //    （SQLite 文档明示：upgrading to a write transaction... 立即失败）。
        //    把 SELECT 移出 tx 后，tx 第一句即 INSERT → 直接拿 RESERVED 写锁，无 upgrade，
        //    busy_timeout = 5000 在 prepare 全程生效。
        //    SELECT 与下面 INSERT 之间有微秒级 race（另一窗口刚 archive/delete 这个 conv），
        //    INSERT 撞 FK 时由下方 FK 错误转译兜底，与 SELECT 路径返同款友好文案。
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
            persona,
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
        let stream_safety_state = Arc::new(Mutex::new(StreamSafetyState::default()));
        let buffer_for_cb = buffer.clone();
        let stream_safety_state_for_cb = Arc::clone(&stream_safety_state);
        let channel_for_cb = channel.clone();
        let safety_guard_for_cb = Arc::clone(&self.safety_guard);
        let app_for_cb = app.clone();
        let assistant_id_for_cb = assistant_id.clone();
        let cancel_token_for_cb = cancel_token.clone();
        let on_delta: Box<dyn Fn(StreamDelta) + Send + Sync> = Box::new(move |delta| {
            if let StreamDelta::TextDelta(text) = &delta {
                buffer_for_cb.lock().push_str(text);
                if let Err(e) = channel_for_cb.send(StreamEvent::Delta {
                    token: text.clone(),
                }) {
                    eprintln!("[chat] channel send Delta failed: {e}");
                }
                if safety_guard_for_cb.is_enabled(SafetyScope::StreamToken) {
                    let acc = buffer_for_cb.lock().clone();
                    match safety_guard_for_cb.scan_token(text, &acc, false) {
                        ScanTokenResult::Pass => {}
                        ScanTokenResult::SoftBlock {
                            rule_id,
                            replace_last_n,
                            placeholder,
                        } => {
                            let mut stream_state = stream_safety_state_for_cb.lock();
                            if !stream_state.record_soft(&rule_id) {
                                return;
                            }
                            drop(stream_state);

                            let mut buf = buffer_for_cb.lock();
                            let chars: Vec<char> = buf.chars().collect();
                            if chars.len() >= replace_last_n {
                                let kept: String =
                                    chars[..chars.len() - replace_last_n].iter().collect();
                                *buf = format!("{}{}", kept, placeholder);
                            } else {
                                *buf = placeholder.clone();
                            }
                            let new_content = buf.clone();
                            drop(buf);

                            let _ = channel_for_cb.send(StreamEvent::ReplaceMessage {
                                message_id: assistant_id_for_cb.clone(),
                                new_content: new_content.clone(),
                                reason: ReplaceReason::SoftBlockToken,
                            });

                            let app2 = app_for_cb.clone();
                            let id2 = assistant_id_for_cb.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = update_assistant_msg_with_safety_status(
                                    &app2,
                                    &id2,
                                    &new_content,
                                    "online",
                                    SafetyScanStatus::StreamSoftBlocked,
                                )
                                .await
                                {
                                    eprintln!("[chat] mid-stream soft block update failed: {e}");
                                }
                            });
                        }
                        ScanTokenResult::HardEnd { rule_id } => {
                            let mut stream_state = stream_safety_state_for_cb.lock();
                            if !stream_state.record_hard(&rule_id) {
                                return;
                            }
                            drop(stream_state);

                            let fallback = FALLBACK_REFUSAL.to_string();
                            let _ = channel_for_cb.send(StreamEvent::ReplaceMessage {
                                message_id: assistant_id_for_cb.clone(),
                                new_content: fallback.clone(),
                                reason: ReplaceReason::FinalBlocked,
                            });
                            let _ = channel_for_cb.send(StreamEvent::Done {
                                total_tokens: 0,
                                finish_reason: "safety_blocked".to_string(),
                            });

                            let app2 = app_for_cb.clone();
                            let id2 = assistant_id_for_cb.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) =
                                    finalize_safety_blocked_msg(&app2, &id2, &fallback).await
                                {
                                    eprintln!("[chat] hard safety finalization failed: {e}");
                                }
                            });
                            cancel_token_for_cb.cancel();
                        }
                    }
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
                let stream_state = stream_safety_state.lock().clone();
                if stream_state.hard_rule_id.is_some() {
                    return;
                }
                let final_output_enabled = self.safety_guard.is_enabled(SafetyScope::FinalOutput);
                let scan = if final_output_enabled {
                    self.safety_guard
                        .scan_final(&collected, &persona.snapshot_id)
                } else {
                    ScanFinalResult::Ok
                };
                let final_status =
                    derive_final_safety_status(final_output_enabled, &stream_state, &scan);
                let (final_text, replace_reason): (String, Option<ReplaceReason>) = match scan {
                    ScanFinalResult::Ok => (collected, None),
                    ScanFinalResult::Redacted { redacted_text, .. } => {
                        (redacted_text, Some(ReplaceReason::FinalRedacted))
                    }
                    ScanFinalResult::Blocked { fallback, .. } => {
                        (fallback, Some(ReplaceReason::FinalBlocked))
                    }
                    ScanFinalResult::ScanFailed { fallback, .. } => {
                        (fallback, Some(ReplaceReason::ScanFailed))
                    }
                };
                if let Err(e) = update_assistant_msg_with_safety_status(
                    app,
                    &assistant_id,
                    &final_text,
                    "online",
                    final_status,
                )
                .await
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
                // SafetyGuard 改写了 content → 先 emit ReplaceMessage 让前端覆盖累积显示,
                // 再 emit Done 让前端走正常收尾路径 (清 currentStreamId)。
                if let Some(reason) = replace_reason {
                    let _ = channel.send(StreamEvent::ReplaceMessage {
                        message_id: assistant_id.clone(),
                        new_content: final_text,
                        reason,
                    });
                }
                let _ = channel.send(StreamEvent::Done {
                    total_tokens: finish.usage.map(|u| u.total_tokens).unwrap_or(0),
                    finish_reason: finish_reason_to_str(&finish.reason),
                });
            }
            Err(LLMError::Cancelled { partial_usage }) => {
                if stream_safety_state.lock().hard_rule_id.is_some() {
                    return;
                }
                // Issue #1 + #7 修复：
                // - 空 buffer（"还没开始就被 cancel"）→ DELETE placeholder（与 Error 分支同款），
                //   不留下空气泡，UI/DB 都看不见这一轮。
                // - 非空 buffer → UPDATE 写入 mode='cancelled'（不是 'online'），下一轮 history
                //   过滤会跳过这条，避免污染 LLM 上下文（与 offline_rule 同款理由）。
                if collected.is_empty() {
                    if let Err(del_err) = delete_assistant_msg(app, &assistant_id).await {
                        eprintln!("[chat] delete empty placeholder on cancel failed: {del_err}");
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
                                    eprintln!("[chat] update_last_activity failed: {act_err}");
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

async fn update_assistant_msg_with_safety_status<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    content: &str,
    mode: &str,
    safety_status: SafetyScanStatus,
) -> Result<(), ChatError> {
    let mut conn = open_app_db(app).await?;
    update_message_content_with_conn(&mut conn, id, content, mode).await?;
    let repo = crate::kernel::repos::ConversationRepo::new();
    repo.update_safety_status(&mut conn, id, safety_status)
        .await
        .map_err(|e| ChatError::Database(format!("update_safety_status: {}", e)))?;
    conn.close().await?;
    Ok(())
}

async fn finalize_safety_blocked_msg<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    fallback: &str,
) -> Result<(), ChatError> {
    update_assistant_msg_with_safety_status(
        app,
        id,
        fallback,
        "online",
        SafetyScanStatus::FinalBlocked,
    )
    .await
}

async fn delete_assistant_msg<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<(), ChatError> {
    let mut conn = open_app_db(app).await?;
    delete_message_with_conn(&mut conn, id).await?;
    conn.close().await?;
    Ok(())
}

async fn load_runtime_profile_for_persona(
    conn: &mut sqlx::SqliteConnection,
    persona: &PersonaSummary,
) -> Result<SoulRuntimeProfile, ChatError> {
    let snapshot_id = persona.snapshot_id.parse::<i64>().map_err(|_| {
        ChatError::Persona(format!(
            "persona {} missing valid snapshot id {}",
            persona.id, persona.snapshot_id
        ))
    })?;
    get_snapshot_profile_with_conn(conn, snapshot_id)
        .await
        .map_err(|e| ChatError::Persona(e.to_string()))
}

async fn build_provider_with_conn(
    conn: &mut sqlx::SqliteConnection,
    secret_repo: Option<&std::sync::Arc<crate::kernel::repos::SecretRepo>>,
) -> Result<OpenAIProvider, ChatError> {
    let record = llm_providers::get_active_record_with_conn_and_secret(conn, secret_repo)
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
        LLMError::Network(_) | LLMError::ServerError(_) => {
            unreachable!("Network/ServerError are handled by the offline_rule branch in run_stream")
        }
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

    /// Phase A0.7 test-only 构造: 用 inline test prefix 起一个 SafetyGuardImpl,
    /// 替代旧 `ChatService::new()` 空构造。Default 已按 runtime contract 移除,
    /// 测试需要显式注入一个最小可用 guard 才能构造 ChatService。
    fn test_chat_service() -> ChatService {
        use crate::kernel::safety_guard::{SafetyGuard, SafetyGuardImpl};
        let guard: Arc<dyn SafetyGuard> =
            Arc::new(SafetyGuardImpl::from_text("TEST_GUARD_PREFIX").unwrap());
        ChatService::new(guard)
    }

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

    #[tokio::test]
    async fn load_runtime_profile_for_persona_errors_when_profile_missing() {
        use crate::services::test_db::fresh_db;

        let (_dir, mut conn) = fresh_db().await;
        sqlx::query(
            "INSERT INTO personas \
             (id, name, version, source, file_path, is_active, created_at, updated_at, active_snapshot_id) \
             VALUES ('momo', '默默', '1.0.0', 'user', 'workshop://momo.soul.md', 1, \
                     '2026-06-20T00:00:00Z', '2026-06-20T00:00:00Z', NULL)",
        )
        .execute(&mut conn)
        .await
        .unwrap();
        let snapshot_id = sqlx::query(
            "INSERT INTO persona_snapshots (persona_id, version, content, created_at) \
             VALUES ('momo', '1.0.0', '# 身份\n旧源码', '2026-06-20T00:00:00Z')",
        )
        .execute(&mut conn)
        .await
        .unwrap()
        .last_insert_rowid();
        sqlx::query("UPDATE personas SET active_snapshot_id = ? WHERE id = 'momo'")
            .bind(snapshot_id)
            .execute(&mut conn)
            .await
            .unwrap();

        let persona =
            crate::services::persona::load_persona_snapshot_with_conn(&mut conn, snapshot_id)
                .await
                .unwrap();

        let result = load_runtime_profile_for_persona(&mut conn, &persona).await;

        assert!(
            matches!(result, Err(ChatError::Persona(message)) if message.contains("snapshot not found"))
        );
    }

    #[tokio::test]
    async fn runtime_profile_for_existing_conversation_uses_bound_snapshot_after_active_changes() {
        use crate::services::persona::{
            activate_snapshot_with_conn, get_snapshot_profile_with_conn, save_draft_with_conn,
            PersonaSimpleDraft, PersonaSourceDraft, PersonaStructuredDraft,
        };
        use crate::services::test_db::fresh_db;

        fn draft(version: &str, identity: &str) -> PersonaSourceDraft {
            PersonaSourceDraft {
                persona_id: "momo".to_string(),
                version: version.to_string(),
                source: "user".to_string(),
                simple: PersonaSimpleDraft {
                    name: "默默".to_string(),
                    tagline: "安静陪伴".to_string(),
                    relationship_style: "companion".to_string(),
                    warmth: 3,
                    playfulness: 2,
                    formality: 2,
                    proactivity: 3,
                    brevity: 4,
                    speech_length: "short".to_string(),
                    initiative: "sometimes".to_string(),
                    dislikes: vec!["空洞鼓励".to_string()],
                    examples: vec!["用户：你好\n默默：我在。".to_string()],
                },
                structured: PersonaStructuredDraft {
                    identity: identity.to_string(),
                    personality: "- 温和\n- 克制".to_string(),
                    capabilities: "- 陪伴".to_string(),
                    rules_do: vec!["短句回应".to_string()],
                    rules_dont: vec!["不空洞鼓励".to_string()],
                    offline_templates: "## 拒答 / Refusal\n- 这个我现在不适合处理。"
                        .to_string(),
                    reactions: "- click.head: 轻声回应".to_string(),
                    examples: "- 用户：你好\n  默默：我在。".to_string(),
                },
                source_text: String::new(),
                preserved_unknown_text: String::new(),
            }
        }

        let (_dir, mut conn) = fresh_db().await;
        let first = save_draft_with_conn(&mut conn, draft("1.0.0", "第一版身份"), true)
            .await
            .unwrap();
        let second = save_draft_with_conn(&mut conn, draft("1.0.0", "第二版身份"), true)
            .await
            .unwrap();
        let conv_id = "01PROFILEBOUND000000000000001";
        sqlx::query(
            "INSERT INTO conversations \
             (id, persona_id, persona_snapshot_id, started_at, last_activity_at) \
             VALUES (?, 'momo', ?, '2026-06-20T00:00:00Z', '2026-06-20T00:00:00Z')",
        )
        .bind(conv_id)
        .bind(first.snapshot_id.parse::<i64>().unwrap())
        .execute(&mut conn)
        .await
        .unwrap();
        activate_snapshot_with_conn(&mut conn, second.snapshot_id.parse::<i64>().unwrap())
            .await
            .unwrap();

        let persona = load_persona_for_conversation_with_conn(&mut conn, conv_id)
            .await
            .unwrap();
        let profile = load_runtime_profile_for_persona(&mut conn, &persona).await.unwrap();

        assert_eq!(persona.snapshot_id, first.snapshot_id);
        assert_eq!(profile.identity_prompt, "第一版身份");
        let active_profile =
            get_snapshot_profile_with_conn(&mut conn, second.snapshot_id.parse::<i64>().unwrap())
                .await
                .unwrap();
        assert_eq!(active_profile.identity_prompt, "第二版身份");
    }

    #[test]
    fn derive_final_safety_status_respects_2026_06_18_priority() {
        use crate::kernel::repos::conversation_repo::SafetyScanStatus;
        use crate::kernel::safety_guard::ScanFinalResult;

        let mut hard = StreamSafetyState::default();
        assert!(hard.record_hard("自杀"));
        assert_eq!(
            derive_final_safety_status(false, &hard, &ScanFinalResult::Ok),
            SafetyScanStatus::FinalBlocked
        );

        let mut soft = StreamSafetyState::default();
        assert!(soft.record_soft("违禁"));
        assert_eq!(
            derive_final_safety_status(false, &soft, &ScanFinalResult::Ok),
            SafetyScanStatus::StreamSoftBlocked
        );

        assert_eq!(
            derive_final_safety_status(false, &StreamSafetyState::default(), &ScanFinalResult::Ok),
            SafetyScanStatus::Disabled
        );

        assert_eq!(
            derive_final_safety_status(true, &soft, &ScanFinalResult::Ok),
            SafetyScanStatus::FinalOk
        );
    }

    #[test]
    fn cancel_unknown_message_id_is_noop() {
        let svc = test_chat_service();
        let result = svc.cancel("non-existent");
        assert!(result.is_ok(), "cancel on unknown id must be no-op");
    }

    #[test]
    fn cancel_existing_token_triggers_it() {
        let svc = test_chat_service();
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
        let svc = test_chat_service();
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

    /// Phase A0.7 集成 smoke test: pin ChatService 持 SafetyGuard +
    /// wrap_messages 注入 prefix 的 shape (`[system(prefix), user(...)]`)。
    /// 完整 FSM 路径 (prepare → scan → wrap → build → run_stream → scan_final → ReplaceMessage)
    /// 留 Task 8 集成测试覆盖 (本 test 仅 pin 构造契约)。
    #[test]
    fn chat_service_holds_safety_guard_and_wraps_messages() {
        use crate::kernel::safety_guard::{Locale, SafetyGuard, SafetyGuardImpl};
        use crate::services::llm::{ChatMessage, Role};
        use std::sync::Arc;

        let guard = Arc::new(
            SafetyGuardImpl::from_text("INJECTED_TEST_PREFIX_FOR_CHAT_INTEGRATION").unwrap(),
        ) as Arc<dyn SafetyGuard>;
        let _svc = ChatService::new(Arc::clone(&guard));

        // 同样调一遍 wrap_messages 确认 prefix 注入 ([user] → [system(prefix), user])
        let wrapped = guard.wrap_messages(vec![ChatMessage::text(Role::User, "hi")], Locale::ZhCn);
        assert_eq!(wrapped.len(), 2);
        assert_eq!(wrapped[0].role, Role::System);
        if let crate::services::llm::ContentPart::Text { text } = &wrapped[0].content[0] {
            assert!(text.contains("INJECTED_TEST_PREFIX_FOR_CHAT_INTEGRATION"));
        } else {
            panic!("expected Text part as first system content");
        }
    }
}
