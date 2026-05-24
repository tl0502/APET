// 业务服务层 — #5 PersonaService MVP + Memory/Nickname 骨架（M1 W1 数据层）→
// #13 ChatService MVP（M1 W2，ADR-018 Layer 2）。
// 后续按 milestone 接入：crypto / secrets / interaction / wardrobe / 等。

pub mod db;

// memory.rs：messages 表 CRUD（#13 ChatService::send 真消费 insert_message_with_conn /
// list_messages_by_conversation；其余 delete_* / cleanup_* 函数留给 M3 设置面板"清空对话"按钮）。
pub mod memory;

// #25 用户头像上传 + #26 VRM 头像导出：落盘到 <app_config>/avatars/，配合 assetProtocol scope。
// 路径由前端通过 memory_set 写到 KV，不另起 DB schema。
pub mod avatars;

pub mod config;
// #16 ConsentService：consent 表读写 + 版本路由判定（启动期 / Onboarding 用）
pub mod consent;
// #21 onboarding 锁死边界：进程级 AtomicBool 闸门，gate=true 等价"已可进 pet 主态"
// （consent.granted && version OK && onboarding KV 不存在）。所有"让主体窗口可见"
// 的路径前置检查；gate=false 时 show 路径改为引导回 onboarding 窗，避免绕过宣誓。
pub mod consent_gate;
// #21 OnboardingService：onboarding 进度持久化（ADR-019）— current_step KV 读写。
pub mod onboarding;
// #13 ChatService 业务编排层（M1 W2 ADR-018 Layer 2）— commands::chat 已真消费。
pub mod chat;
// #12 LLMProvider trait + OpenAIProvider（ADR-018 Layer 1）— #13 ChatService 真消费
// chat_stream / OpenAIProvider / ChatOptions / FinishReason / LLMError / StreamDelta。
// 多模态 ContentPart::ImageUrl 等 variant 仍 typed only，M3+ 接多模态时实现 impl 路径。
pub mod llm;
// 用户增补：多 provider 实例管理（参考 cc-switch UI）；ChatService.build_provider 真消费
// get_active_record。原 #12 单 namespace `llm:openai:*` 已被 migrate_legacy_if_needed 搬迁。
pub mod llm_providers;
pub mod nickname;
// 2026-05-09：user_nickname 切换转场注入（解决 System Prompt Inconsistency 污染对话）。
// nickname::set_user_nickname 成功后调 maybe_inject_user_change 写 system 转场消息。
pub mod nickname_announcement;
// #21 M1 收尾：onboarding 完成转场注入（与 nickname_announcement 同款套路）。
// onboarding_complete IPC 成功路径中调 inject_onboarding_complete 写 system 首聊消息。
pub mod onboarding_announcement;
// #21 M1 收尾：LivingPet 自由活动初版（flows §10）— 状态机骨架 + 5-15min 调度器 + wander。
pub mod living_pet;
pub mod persona;
pub mod preferences;
// #22 ReminderService：提醒 CRUD + 触发 + 启动 catch-up（schema 见 migrations/001_init.sql:99-120）。
pub mod reminder;
// #22 Scheduler 公共抽象：1s polling 单 tokio task；驱动 reminder.find_due + pomodoro.tick。
// #29 待办 / M3 IdleDetector 后续接入。
pub mod scheduler;
// #29 TodoService：待办 CRUD + reminder 联动（schema 见 migrations/001_init.sql:122）。
pub mod todo;
// #29 Phase D — Onboarding reminder intent 启动期实例化（闭合 #21 ADR-019 step 4）。
// 启动 setup 钩子读 KV `onboarding:reminder_intents` → batch create reminders → delete KV，
// 同 tx 原子；前 5 模板与 src/types/reminder.ts:80 REMINDER_TEMPLATES 双写（lessons.md）。
pub mod onboarding_reminders;
// #28 PomodoroService：番茄钟状态机 + drift 校准 + FOCUS/REST 自动转换 + reminder/livingPet 协作 hook。
// 运行时态走 KV `pomodoro:active_session`（不污染 pomodoro_sessions 表；lesson #2）。
pub mod pomodoro;
pub mod shortcuts;
// #30 follow-up I：Rust 端磁吸 solver。
// 修复链式 group-drag 抖动：Rust 端直接 set_position 替代前端 N 次 IPC（μs vs ms 数量级差）。
// 前端是 constraint 权威源，通过 snap_sync_constraints IPC 全量推到 Rust state。
pub mod snap;
// #23-b InteractionRouter（#40，模块 N 主干）：物理交互 5 事件路由 + reaction_table + 抗议滑窗。
// ADR-025 锁定：M2 AABB 单 body 降级；2a-lite 路由 + emit + 最少可见反馈；mood transient 不持久。
pub mod interaction;
pub mod window_state;

// #6 系统托盘 + 窗口动作 helper（M1 W2 主态可达交付物）。
// window_actions 的 show_pet / hide_pet 在 #6 范围内未消费，#7 shortcuts task
// 接入 Ctrl+Alt+Space 全局快捷键时启用；mod 级 #[allow(dead_code)] 屏蔽 dead_code warning，
// 届时去掉本 attr。
pub mod tray;
#[allow(dead_code)]
pub mod window_actions;

// 仅测试期编译：DB 集成测试共享 fixture（详 旧项目 progress/test-coverage-2026-05-04.md）
#[cfg(test)]
pub mod test_db;
