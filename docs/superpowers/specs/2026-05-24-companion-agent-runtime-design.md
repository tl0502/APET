---
title: Companion Agent Runtime v1 — Top-Level Architecture Spec
updated: 2026-05-24
related:
  - ../../decisions.md
  - ../../architecture/system-architecture.md
  - ../../persona/persona-design.md
  - ../../requirements/prd.md
  - ../../roadmap/development-roadmap.md
---

# Companion Agent Runtime v1 — Top-Level Architecture Spec

> **Audience**: 项目核心开发 + 第三方架构审核。本文档**自包含**,不要求读者预读其他 docs/。
> **Status**: Brainstorm-locked,等 first review;通过后进入 implementation plan 阶段。
> **Spec scope**: 顶层 Runtime 架构 + 6 个子系统的契约边界 + MVP 实现范围。各子系统**内部**实现细节由独立子 spec 跟进。

---

## 0. 背景与项目信息（third-party review 必读）

### 0.1 项目简介

**AIPET**（AI Desktop Pet, 内部代号）是一个 Windows 桌面 AI 桌宠应用。10 周 MVP, 单人 vibecoding 项目。差异化定位三引擎:

1. **用户自主人格**: 角色定义文件 `.soul.md` 归属用户(参考 OpenClaw 开源项目验证可行)。用户可编辑、可分享、可从零创建。
2. **主动陪伴**: 桌宠基于本地空闲信号(GetLastInputInfo)主动起话题。**不**读窗口标题/应用名/输入内容/麦克风。
3. **共同活动**: 物理交互(摸头/抗议/拖动反应)、装扮系统、声音表情、本地小游戏(3 个) + LLM 小游戏(2 个,故事接龙 + 角色扮演)。

仓库: <https://github.com/tl0502/APET>

### 0.2 技术栈

| 层 | 选型 | 理由 / ADR |
|---|---|---|
| 桌面框架 | **Tauri 2.x** (Rust 主进程 + WebView2 前端) | ADR-001;不是 Electron |
| 前端 | **Vue 3.5 + TypeScript 5.6 + Pinia + Vite 7** | ADR-001 |
| 组件库 | **Element Plus 2.13** 全量 import | ADR-017 |
| 3D 渲染 | **Three.js + @pixiv/three-vrm** | ADR-002;原 Live2D 路线 Superseded |
| 存储 | **SQLite + WAL** (sqlx); secrets 走 Windows DPAPI | — |
| LLM 协议 | **OpenAI 兼容** (6 preset: OpenAI/DeepSeek/Moonshot/Qwen/Ollama/自定义) | ADR-005 |
| 流式 IPC | `tauri::ipc::Channel<StreamEvent>` (非全局 emit) | ADR-018 Updated 2026-05-08 |
| 构建/包管 | pnpm + Cargo;release profile `lto=true / panic=abort / strip=true` | ADR-016 |

性能预算: 总常驻 ≤ 250MB / CPU ≤ 5% / 冷启动 ≤ 5s / 对话首 token p50 ≤ 1.5s。

### 0.3 当前进度（2026-05-24, M2 W3 收尾）

- M1 W1-W2 (壳层 + 对话): ✅ 18/18 issue 完成 + 美化补丁。
- M2 W3 (任务三件套 + 物理交互 + 磁吸 + workspace 壳): ✅ 11/11 完成(物理交互待办)。
- 测试覆盖: **264 cargo test pass / 293 vitest pass**。
- 24 项 ADR 已敲定(立项期 15 + 实施期 9: 016-024)。
- 当前正在做 issue #23 物理交互 + 心情/精力 + 摸鱼模式。

**已落地的 AI 相关代码** (位于 `src-tauri/src/services/`):
- `chat/{service.rs, prompt.rs, conversation.rs, mod.rs}` — ChatService L2 编排,流式 + cancel + 4 分支收尾。
- `llm/{types.rs, openai.rs, error.rs, probe.rs}` — LLMProvider L1 协议层(OpenAI 兼容)。typed 接口含 `ContentPart` 5 variant(Text/ImageUrl/ImageBase64/Audio/File) + `ToolCall`/`ToolDefinition`/`ToolChoice`/`StreamDelta::ToolCallDelta`/`FinishReason::ToolCalls`。
- `persona.rs` — `.soul.md` 加载 / 热切换 / 试聊沙盒。
- `memory.rs` — message + nickname KV(注: 现在的 memory 表是 nickname / preference,**不是** Companion 意义的 episodic memory)。
- `living_pet.rs` — mood / energy / wandering(当前 tick-based,本 spec 改 lazy)。
- `llm_providers.rs` — provider 配置 CRUD。

### 0.4 团队 / 时间 / 工作模式

- **团队**: 单人 vibecoding。**不**走 reviewer / CI / 灰度发布 / KPI 门禁等团队流程。
- **时间窗**: 10 周 MVP (M1-M5)。当前 M2 W3, 剩 ~6-8 周。
- **决策记录**: 单文件 `docs/decisions.md`(ADR-NNN, 三句话格式),不分文件夹。
- **commit 风格**: `<type>: <subject>`,type 自由。
- **文档头**: 3 字段 YAML frontmatter(title / updated / related)。

### 0.5 项目设计哲学（不可被本 spec 违反）

1. **Local-first**: 不引入用户数据强制上传。所有 LLM 调用经用户配置的 provider。
2. **用户自主权**: `.soul.md`/装扮/设置完全归用户;用户能读、能改、能导出、能迁移。
3. **非养成原则**: 不引入流失/死亡/必须签到机制。
4. **隐私边界**: 不读应用名/窗口标题/输入内容/麦克风(CI 静态扫描黑名单)。
5. **安全护栏不可绕过**: 任何人格/游戏场景不能覆盖系统安全前缀。

### 0.6 当前 AI 路径关键缺口（本 spec 解决的根本问题）

> 本节诚实列出当前(M2 W3)AI 路径的真实状态,为审核者提供判断基线。

| 缺口 | 当前状态 | 影响 |
|---|---|---|
| **🔴 安全前缀 = None** | `src-tauri/src/services/chat/prompt.rs:34 const SAFETY_PREFIX: Option<&str> = None;` | LLM 调用当前**完全没有 ADR-006 安全前缀**;任何对外分发版本前必须修复 |
| **🔴 SecurityGuard 模块不存在** | `grep -r SecurityGuard` 0 命中 | LLM 输出无二次扫描;命中违禁词不会替换 |
| **🟠 Memory 接入 prompt 缺失** | `chat/prompt.rs:208 build_system_message` 只注入 nickname bullet | 桌宠"记得用户偏好"的设计承诺当前失效;`memory` 表中的 KV 不进 system message |
| **🟠 history 硬切 N=10** | `chat/service.rs:59 const HISTORY_LIMIT: u32 = 10;` | 长对话丢上下文;无摘要/压缩机制 |
| **🟠 AgentService 不存在** | `grep -r AgentService` 0 命中 | LLM 即使 emit tool_call 也被 ConversationSubsystem 主动吞掉 (`service.rs:338 注释 "M1 不接 tools,不会触发;忽略"`) |
| **🟠 ToolRegistry 不存在** | 0 文件 | 6 起步 tool (Read/Edit/Write/Glob/Grep/Bash) 全无实现 |
| **🔴 ADR-025 沙盒未拍板** | ADR-018 明示"具体路径白名单/命令沙盒细则待 **ADR-019** 决议",但 ADR-019 是 Onboarding 续接,不是沙盒;沙盒 ADR 编号未分配、内容未写 | Layer 3 AgentService 完全 block |
| **🟠 API Key 明文** | M1 期 `config` 表 KV;`secrets` 表 + DPAPI 未上 | M3 G `CryptoService` 落地前不能分发 |
| **🟠 ProactiveCare 未实现** | 规划在 M3 W5-6;`IdleDetector / ProactiveCareService` 文件不存在 | 当前桌宠纯 reactive |
| **🟢 流式 + cancel + 4 分支收尾** | ChatService 已扎实落地 | 已是 production-ready 水平,**本 spec 保留并升级为 ConversationSubsystem** |
| **🟢 typed 多模态 + tool 接口** | LLM types.rs 已 typed 完整 | ContentPart 5 variant + ToolCall/Definition/Choice/StreamDelta 全 typed,**本 spec 不重新设计** |

### 0.7 本 spec 解决什么 / 不解决什么

**解决** (顶层契约 + 6 子系统边界):
- ✅ Runtime 总体架构 (Kernel + 6 Subsystems + Soul Overlay)
- ✅ State ownership (谁拥有什么表 / 谁能写 / 谁能读)
- ✅ Lifecycle 状态机 (Boot/Live/Suspend/Wake/Shutdown)
- ✅ Event 模型 (9 event + sync/async 边界)
- ✅ 15 trait 完整签名 + capability token 编译期约束
- ✅ Memory MVP 三层 + FTS5 retrieval pipeline
- ✅ Initiative MVP 4 trigger + hard gate + Soul score
- ✅ Tool MVP 3 read-only + whitelist + audit
- ✅ AIPET 现有 service → Runtime subsystem 迁移路径
- ✅ ADR 增量清单

**不解决** (后续独立 spec / ADR):
- ❌ 各子系统**内部**详细实现 (例如 PromptBuilder 具体怎么拼、ToneShaper 具体怎么改写) → 各子系统独立 spec 跟进
- ❌ ADR-025 沙盒规则的**完整决议** (本 spec 给 MVP 默认作为审核参考;正式 ADR-025 文档独立提交)
- ❌ Writable tool (Edit/Write/Bash) / 长期任务 / multi-agent / semantic memory + embeddings → P1+
- ❌ UI 设计 / 装扮系统 / 声音系统 (与 AI runtime 正交;沿用现有 PRD/architecture)
- ❌ MCP server bridge (P1+,本 spec 不留专用 slot,通过 ToolRegistry 通用机制接入)

---

## 1. Scope & Impact（third-party review 必读）

> 列出本 spec 实施时**会动到的具体代码 / 表 / ADR / milestone**。审核者据此判断 blast radius。

### 1.1 影响的代码文件

**需要新建** (10+ 文件):

```
src-tauri/src/kernel/                     新建 kernel 层
├── mod.rs                                ← 5 trait 聚合 export
├── safety_guard.rs                       ← 实施 ADR-006 真注入 + 输出扫描
├── lifecycle_manager.rs                  ← FSM (Section 6)
├── event_bus.rs                          ← typed pub/sub + event_log writer
├── scheduler.rs                          ← cron / idle / one-shot / periodic 统一
├── state_store.rs                        ← capability token + WriteQuery 抽象
└── capability.rs                         ← WriterCap<T> + Owned trait + KernelSecret

src-tauri/src/subsystems/                 新建 subsystem 层
├── mod.rs
├── persona/                              ← 从 services/persona.rs 升级
├── memory/                               ← 新建,3 层 memory
│   ├── service.rs
│   ├── working.rs
│   ├── episodic.rs
│   └── ranker.rs
├── conversation/                         ← 从 services/chat/ 升级
├── initiative/                           ← 新建,从 ProactiveCare 规划落地
├── tool/                                 ← 新建,3 read-only tool
│   ├── service.rs
│   ├── glob.rs
│   ├── grep.rs
│   ├── read.rs
│   └── whitelist.rs
└── living/                               ← 从 services/living_pet.rs 升级 + 改 lazy

src-tauri/src/soul/                       新建 soul overlay 层
├── mod.rs
├── prompt_builder.rs
├── tone_shaper.rs
├── initiative_weights.rs
└── retrieval_ranker.rs
```

**需要改造** (现有文件,改造非重写):

| 现有文件 | 改造内容 | 改造规模 |
|---|---|---|
| `services/chat/service.rs` | 提取 ConversationSubsystem trait 实现;hot path 集成 SafetyGuard wrap_messages + scan_final;tool_call 处理(原 `service.rs:338` 注释吞 tool 改为转 ToolSubsystem.execute) | ~30% 改造,保留 prepare/run_stream 4 分支结构 |
| `services/chat/prompt.rs` | `SAFETY_PREFIX = None` 改为从 SafetyGuard 注入;build_system_message 接入 MemorySubsystem.retrieve | ~20% 改造 |
| `services/chat/conversation.rs` | 几乎不动(ConversationStore 已是好设计) | < 5% |
| `services/persona.rs` | 实现 PersonaSubsystem trait;init 时拿到 EventBus handle,activate 时 publish | ~10% |
| `services/living_pet.rs` | tick-based 改 lazy aging;实现 LivingSubsystem trait;mood_changed 改 EventBus publish | ~40% |
| `services/llm/openai.rs` | 不改 (L1 已成熟) | 0% |
| `services/llm/types.rs` | 加 ChatMessage 多模态工厂(`image()` / `audio()` / `file()`);实现 ImageBase64 → data URI 改写 | ~10%,P1 才用到 |
| `services/memory.rs` | 含义改:从"nickname/preference"扩为 KV 偏好 + episodic head pointers;实现 MemorySubsystem.set_fact | ~30% |
| `services/scheduler.rs` | 现有是空骨架,本 spec 完整实现 | ~80% 新写 |
| `lib.rs setup` | 启动序列重组 (Section 6.B 8 步) | ~50% |

**保留不动**:
- `services/llm/openai.rs` / `error.rs` / `probe.rs`
- `services/llm_providers.rs`
- `services/db.rs` / `migration.rs` / `config.rs`
- `services/consent.rs` / `consent_gate.rs` / `onboarding.rs`
- `services/reminder.rs` / `pomodoro.rs` / `todo.rs` (TaskService 独立,与 6 subsystem 正交)
- `services/snap.rs` (磁吸窗口系统)
- `services/window_state.rs` / `window_actions.rs` / `shortcuts.rs` / `tray.rs`
- `services/avatars.rs` / `preferences.rs`

### 1.2 影响的 SQLite 表

> 现有 27 表 (`docs/architecture/system-architecture.md` §4 详列);本 spec 改 2 张、新增 4 张、不动 21 张。

**修改 schema** (走 migration):

| 表 | 改动 | 迁移方式 |
|---|---|---|
| `messages` | 加 `token_count INTEGER DEFAULT NULL` + `safety_scan_status TEXT DEFAULT 'pending'` | `ALTER TABLE` + default 兼容 |
| `pet_runtime_state` | 加 `last_mood_event_at TEXT` + `last_energy_event_at TEXT` 支持 lazy aging | `ALTER TABLE` |

**新增 schema** (走 migration):

| 表 | 用途 | Owner |
|---|---|---|
| `episodic_memory` | 压缩后的 episode (Section 9) | MemorySubsystem |
| `episodic_memory_fts` | FTS5 virtual table + 3 trigger | MemorySubsystem (内部) |
| `event_log` | EventBus 持久化关键事件 | Kernel (EventBus) |
| `tool_audit_log` | tool 执行审计 | ToolSubsystem |

**不动**:
- 21 张表(personas / persona_snapshots / nicknames / conversations / memory / config / secrets / consent / schema_version / reminders / reminder_history / todos / pomodoro_sessions / proactive_care_log / milestones / user_anniversaries / accessories_inventory / wardrobe_decisions / voice_packs / voice_settings / game_sessions / game_session_events / diary_drafts / telemetry_queue / error_logs)

### 1.3 影响的 ADR

**新增 ADR** (3 条):

| ADR | 主题 | 由本 spec 落地 |
|---|---|---|
| **ADR-025** | Agent 工具沙盒规则(path whitelist + denylist + capability + grant UX) | Section 11 提供 MVP 默认作为参考,完整 ADR 独立提交 |
| **ADR-026** | Companion Agent Runtime 顶层架构(本 spec 摘要) | 本 spec 提交后归档为 ADR-026 |
| **ADR-027** | Memory 三层架构 (working / episodic / semantic) + FTS5 retrieval | Section 9 |

**Updated ADR** (3 条):

| ADR | 现状 | 本 spec 影响 |
|---|---|---|
| **ADR-006** | "安全前缀 v1.0,通用核心 + 地区补充" | Updated: prefix 实际注入路径明确化(SafetyGuard.wrap_messages,kernel-owned trait,subsystem 无法 bypass) |
| **ADR-015** | "对话面板三形态架构" | Updated: ConversationStore 升级为 ConversationSubsystem;三 surface(pet/chat/workspace)共享 data layer 通过 EventBus 多 surface broadcast |
| **ADR-018** | "LLM 三层抽象 + AgentService 工具调用框架" | Updated: Layer 2 ChatService → ConversationSubsystem;Layer 3 AgentService → ToolSubsystem (本 spec MVP 仅 read-only 3 件;writable tool 推 P1);沙盒细则推到 ADR-025 |

**Superseded ADR**: 无 (本 spec 是增量,不推翻已有决策)

### 1.4 影响的 milestone

| Milestone | 原计划 | 本 spec 影响 |
|---|---|---|
| **M2 W4** (当前) | 物理交互 + 心情/精力 + 摸鱼 (#23) | LivingPetService tick → lazy 改造需对齐本 spec (#23 实施时同步) |
| **M3 W5-6** | LLM Provider + SecurityGuard + MigrationService + UpdaterService + IdleDetector + ProactiveCareService + FileDropHandler + MilestoneService + LivingPetService 日常时段表 | **本 spec 是 M3 主线 spec**;ProactiveCareService → InitiativeSubsystem;IdleDetector 归 Scheduler;Memory/Tool subsystem 新增工作量 |
| **M4 W7-8** | WardrobeService + VoiceEffectPlayer + 用户纪念日 + 装扮工坊 | 无直接影响 |
| **M5 W9-10** | GameEngine + 5 游戏 + 自测 + 可发布版 | LLMGameRunner 走 ConversationSubsystem + ToolSubsystem 同样接口;无新设计 |

**新增 milestone insertion**: 建议在 M3 W5 启动前加一周 "Runtime Foundation Phase" (kernel + 6 subsystem 骨架 + 现有 service 改造),约 1.5 周;之后 M3 W6-W7 完成 MVP 实现。

### 1.5 不影响的承诺

本 spec **不动**:
- ✅ PRD §1-§4 业务范围 / 用户故事 / 验收口径
- ✅ ADR-001 到 ADR-024 所有 Accepted 状态的 ADR(本 spec 仅 Updated 3 条增量)
- ✅ 装扮系统(模块 O) / 声音系统(模块 P) / 游戏系统(模块 Q) / 物理交互(模块 N)的产品设计
- ✅ VRM 渲染管线 + 配饰挂载点(ADR-002 / ADR-003)
- ✅ Onboarding 续接(ADR-019)
- ✅ 磁吸窗口系统(ADR-020)
- ✅ Workspace 单窗壳(ADR-021)
- ✅ 现有 264 cargo test + 293 vitest test 的语义(测试可能需要小幅适配新 trait 接口,但断言不变)
- ✅ 性能预算: 250MB 内存 / 5% CPU / 5s 冷启 / 1.5s 首 token
- ✅ 安装包目标 ≤ 80MB

---

## 2. First Principles（不可妥协的设计前提）

### 2.1 Core / Soul 双层分离

> **本 spec 的核心思想**,源自审核者对话中的提议。

Runtime 严格分两层,**强单向依赖**:

- **Core Runtime (rational, 理性层)**: tool / task / memory store / scheduler / context / safety — **不感知 persona**
- **Soul Overlay (expressive, 人格层)**: persona prompt / tone / mood / initiative weights / reaction style — 包在 Core 外侧

**Soul 影响 Core 仅两条通道**:
1. **System message 注入**: PersonaPromptBuilder 在 ConversationSubsystem `build_messages` 之前组装 system message
2. **Decision weight 加权**: InitiativeWeights / RetrievalRanker 在排序时调整 score

**Soul 不能**:
- ❌ 改 tool registry / 跳 safety guard
- ❌ 直读写 memory store / 改 scheduler 硬约束(quiet hours / token budget)
- ❌ publish event / subscribe event

**为什么这样设计** (审核者必读):

参考 Inworld AI / Pixar 角色 AI 系统 / Apple Siri "personality envelope" 同款思路。工程上避免三个经典坑:
1. **人格控制 runtime 会让 tool use 失控** — persona 想"调皮"可能让 LLM 误调危险工具
2. **人格污染 memory 会让长期记忆退化** — persona 主观偏好不该决定客观 fact 留存
3. **人格主控 initiative 难维护** — 不同 persona 切换时, scheduler 硬约束(quota / quiet hours)会被 reset

### 2.2 Hybrid Event-Driven Lifecycle

Tauri 主进程常驻 (process always-on),但 **state machine tick + agent loop 都是事件驱动**:
- mood / energy 走 **lazy aging function** (查询时按时间衰减重算,不 tick)
- Agent loop 仅在 user input / proactive trigger / scheduled tick 启动
- Scheduler 是 timer **唯一**持有者,subsystem 不允许自封 timer

参考主流: LangGraph / OpenAI Agents SDK / Mastra。

### 2.3 Kernel + Subsystems + Soul Overlay

参考 Andrej Karpathy "LLM-OS" 提议 (2023) + Linux kernel modules + Mach/L4 微内核:
- **Kernel (5 件套, 极小, 不可被绕过)**: SafetyGuard / LifecycleManager / EventBus / Scheduler / StateStore
- **Subsystems (6 件套, 可独立演进)**: PersonaSub / MemorySub / ConversationSub / InitiativeSub / ToolSub / LivingSub
- **Soul Overlay (4 件套, stateless function)**: PersonaPromptBuilder / ToneShaper / InitiativeWeights / RetrievalRanker

**为什么不选 actor / hexagonal** (审核者必读):
- **Actor (Akka/Orleans 风格)**: Rust 无原生 framework, `actix` 维护放缓;AIPET 现有 264 test 全在 service 层,actor 模型要全部重写,ROI 不匹配。
- **Hexagonal (Cockburn ports & adapters)**: 与 ADR-018 三层抽象同构,但**不是天然 event-driven**(要额外加 EventBus),且**不强制隔离靠纪律**;Companion runtime 安全要求需编译期 hard isolation。
- **Kernel + Subsystems**: 与 Core/Soul 双层完美映射(kernel = core 最硬部分, subsystem = core 中层, soul = overlay);AIPET 现有 service 1:1 映射到 subsystem,改造成本最低;safety/lifecycle 在 kernel = 编译期约束不可被越权。

---

## 3. Constitution（8 条不变量,违反 = build fail 或 panic）

| # | 不变量 | 工程落地 |
|---|---|---|
| **1. Safety Sovereignty** | SafetyGuard 永远第一 | `safety_prefix` 由 kernel 强制拼到 system message 第一位;subsystem 拿到的 messages 已包好;任何 LLM stream finish 必经 `SafetyGuard.scan_output()` |
| **2. Single Writer per Table** | 每张 SQLite 表恰好 1 个 owner | StateStore 暴露 `WriterCap<T: Owned>`(capability token);writer trait 需 token,跨 ownership SQL write 编译期拒绝 |
| **3. Event-or-Direct** | subsystem 间通信只有两路 | (a) 同步调 owner read trait(仅 read);(b) 经 EventBus publish(异步通知,owner 自己 subscribe + write)。**禁止**第三种"直接调 owner write 方法" |
| **4. No Self-Ticking** | subsystem 不自封 timer | 仅 Scheduler 持有 tokio runtime handle;subsystem 实现 `on_scheduled_tick(reason)` callback |
| **5. Soul Boundary** | Soul 单向 | `soul/` 模块 `use` 黑名单含所有 write trait + EventBus publish + Scheduler/StateStore mut;Soul 只能 read trait + 返 prompt 文本 / score 数值 |
| **6. Lazy First** | 衰减类计算 lazy | mood / energy / wandering = store source + recompute on read;tick 仅在 Scheduler 三种触发(cron / idle-cross / one-shot / periodic) |
| **7. Hot Path Sync** | user → reply 全程同步 | EventBus 仅用于 post-hoc(persist / broadcast / telemetry / proactive evaluate);不在 hot path 上 |
| **8. MVP First** | 新概念必经 MVP 必要性证明 | trait ≤ 15(kernel 5 + subsystem 6 + soul 4);event ≤ 15;subsystem = 6 封顶,新功能挂到现有 subsystem |

---

## 4. Architectural Map

### 4.1 全图

```
┌─────────────────── 4 SURFACES (UI 表面) ──────────────────────────────────┐
│   [Pet Window]   [Chat Panel]   [Workspace]   [Tray + Notification]       │
│        │              │              │                 │                   │
│        └──────────────┴──────────────┴─────────────────┘                   │
│                            ↑ event subscriptions (read-only on state)      │
└───────────────────────────│───────────────────────────────────────────────┘
                            │
┌──────────────── SOUL OVERLAY (expressive, stateless) ─────────────────────┐
│  PersonaPromptBuilder | ToneShaper | InitiativeWeights | RetrievalRanker  │
│  仅两条通道:                                                               │
│  ① system message 注入  ② decision weight 加权                            │
└──────────────────────────────│────────────────────────────────────────────┘
                               │ 单向调用 (Soul → Core, 反向禁止)
┌─────────────────────── 6 SUBSYSTEMS (rational core) ──────────────────────┐
│                                                                            │
│  ConversationSub | MemorySub | InitiativeSub                               │
│  (chat agent loop)| (working+episodic | (proactive trigger eval)          │
│   ← ChatService   | + FTS5 search)    | ← ProactiveCare 规划落地          │
│                   |  ← 新建            |                                    │
│                                                                            │
│  PersonaSub      | ToolSub           | LivingSub                          │
│  (.soul.md)      | (read-only:        | (mood/energy/wandering            │
│  ← PersonaService| Glob/Grep/Read)   | lazy aging)                        │
│                  | ← 新建             | ← LivingPetService 改造           │
└──────────────────────────────│────────────────────────────────────────────┘
                               │ subsystem 之间禁止直接调用,必须经 kernel
┌─────────────── KERNEL (5 件套, hard, never bypassable) ───────────────────┐
│  SafetyGuard     | LifecycleManager | EventBus      | Scheduler | StateStore│
│  (prefix +       | (FSM 5 states)   | (typed pub/sub| (cron+idle| (sqlx +   │
│   output filter) |                  | + persistence)| +one-shot)| capability)│
│  ADR-006 落地    |                  |               |           |           │
└────────────────────────────────────────────────────────────────────────────┘
                               │
                       [LLMProvider]    ← capability, 不在 kernel
                       [Tauri IPC]      ← platform adapter, 不在 kernel
```

### 4.2 Kernel 5 件套（hard, 永不可被 subsystem 越权）

| Kernel 组件 | 责任 | 不变量 |
|---|---|---|
| **SafetyGuard** | ADR-006 prompt prefix 注入 + LLM 输出二次扫描 + 拒答降级链 | Prefix 永远位于 system message 第一位;subsystem 不能跳过 |
| **LifecycleManager** | Boot/Live/Suspend/Wake/Shutdown FSM + 启动顺序 + dependency wiring | 同一时刻 Runtime 状态唯一;Wake 后 state 一致性自检 |
| **EventBus** | 类型化 pub/sub + 同步派发 + 持久化关键事件到 `event_log` | 任一 publish 都至少持久化 schema_version + payload |
| **Scheduler** | cron + idle-threshold + one-shot + periodic 4 种触发统一调度 | 单实例 tokio runtime;不允许 subsystem 自起 timer |
| **StateStore** | SQLite + config KV + secrets(DPAPI) 抽象;capability token + 事务边界 | 写入必经 cap;secrets 必经 CryptoService |

### 4.3 6 Subsystems（rational, 实现可独立演进）

| Subsystem | 责任 | 与 AIPET 现有 service 映射 | MVP 改造工作量 |
|---|---|---|---|
| **ConversationSubsystem** | chat agent loop;消息流;tool_call 处理(MVP 仅 read-only);多模态接口预留(MVP 不实现,LLM types 已 typed) | `services/chat/*` 1:1 升级 | ~1 周(拆 agent loop + 接 EventBus + SafetyGuard wrap) |
| **MemorySubsystem** | working memory(对话窗口) + episodic memory(SQLite+FTS5 检索 + LLM 摘要压缩) | **新建**(现有 `services/memory.rs` 是 nickname/preference,与本 sub 正交;扩展含义) | ~2 周(schema + FTS5 + 摘要 hook + ranker) |
| **InitiativeSubsystem** | proactive trigger 评估(idle / mood / quota / quiet hours);candidate selection | `services/proactive_care.rs`(规划未实施)落地;`living_pet.rs` 主动起话题逻辑分离 | ~1 周 |
| **PersonaSubsystem** | `.soul.md` 加载/热切换/试聊沙盒/snapshot | `services/persona.rs` 90% 复用 | ~0.5 周(加 EventBus hook) |
| **ToolSubsystem** | Tool Registry + 路径白名单 + 执行 + 审计(MVP: Glob / Grep / Read) | **新建**(ADR-025 阻塞前置) | ~1.5 周(ADR-025 写 ~0.5 周 + 实现 ~1 周) |
| **LivingSubsystem** | mood / energy / wandering 的 lazy aging function + 触发条件 | `services/living_pet.rs` 改造(去 tick,改 lazy + event-driven) | ~1 周 |

### 4.4 Soul Overlay 4 件套（stateless function, 无 state）

| Component | 输入 | 输出 | 钩入点 |
|---|---|---|---|
| **PersonaPromptBuilder** | active persona / nicknames / mood / context | system message 文本 | ConversationSubsystem `build_messages` 之前 |
| **ToneShaper** | core 准备发出的 raw text + persona + mood | tone 包装后的 text | LLM 调用前 / proactive 文案最后一步 |
| **InitiativeWeights** | candidates + persona + mood | weighted score | InitiativeSubsystem `select_candidate` 排序时 |
| **RetrievalRanker** | episodic memory 候选 + 当前 context + emotional relevance | re-ranked list | MemorySubsystem `search` 后处理 |

**Soul 不变量**(编译期约束):
- Soul 不读写 StateStore / 不直接调 ToolSubsystem / 不改 Scheduler / 不绕 SafetyGuard
- Soul 是 stateless function — 输入相同则输出相同(除 LLM 本身非确定性)
- `soul/` 模块 `use` 黑名单含所有 write trait

### 4.5 Surfaces 4 件套（UI 表面, 仅 read state 经 IPC）

| Surface | 内容 | 状态来源 |
|---|---|---|
| Pet Window | 透明置顶 VRM 桌宠 | ConversationSubsystem.read + LivingSubsystem.read_current |
| Chat Panel | 磁吸浮窗/嵌入式 chat UI | ConversationSubsystem (流式 Channel) |
| Workspace | 单窗多 panel 壳 (ADR-021) | 全部 subsystem read trait |
| Tray + Notification | OS 托盘 + 系统通知 | EventBus subscribe (broadcast) |

---

## 5. State Ownership Map

### 5.1 18 张表 Ownership(含 4 新增)

| 表名 | Owner (writer) | Readers | Lifecycle |
|---|---|---|---|
| `conversations` | ConversationSub | InitiativeSub(last_activity) / MemorySub(摘要时) | persistent |
| `messages` | ConversationSub | MemorySub(扫描转 episodic) | persistent(+ `token_count` / `safety_scan_status` 两字段) |
| `personas` | PersonaSub | ConversationSub / InitiativeSub | persistent |
| `persona_snapshots` | PersonaSub | — | persistent |
| `memory` | MemorySub | ConversationSub(prompt 注入) | persistent(扩为 KV 偏好 + episodic head pointers) |
| `nicknames` | PersonaSub | ConversationSub / InitiativeSub | persistent |
| `pet_runtime_state` | LivingSub | InitiativeSub | persistent(+ `last_mood_event_at` / `last_energy_event_at`,改 lazy aging) |
| `proactive_care_log` | InitiativeSub | — | persistent (7d retention) |
| `config` / `secrets` / `consent` / `schema_version` | **Kernel** (StateStore) | 全部 subsystem | persistent |
| `reminders` / `pomodoro_sessions` / `todos` | **TaskService**(保留独立,不进 6 subsystem) | — | persistent;与 6 subsystem 正交,经 EventBus 通知 LivingSub |
| 🆕 `episodic_memory` | MemorySub | ConversationSub(retrieve) | persistent + FTS5 |
| 🆕 `episodic_memory_fts` | MemorySub (FTS5 internal) | MemorySub only | persistent |
| 🆕 `working_memory` (in-mem) | ConversationSub | — | **transient (in-mem)** + persist on shutdown |
| 🆕 `event_log` | **Kernel** (EventBus) | observability / debug | persistent (30d retention) |
| 🆕 `tool_audit_log` | ToolSub | safety review | persistent |

### 5.2 Sync vs Async 分类（防止 hot path 误经 EventBus）

**Sync hot path** (直接调用链, 禁经 EventBus):
```
user_input → ConversationSub.handle_user_msg
  → PersonaSub.read_active (sync)
  → MemorySub.retrieve_relevant (sync, FTS5 < 50ms)
  → Soul.build_prompt (stateless)
  → SafetyGuard.wrap_messages (kernel)
  → LLMProvider.chat_stream
  → SafetyGuard.scan_output (kernel)
  → Soul.tone_shape (stateless)
  → multi-surface emit (此处转 async)
```

**Async (EventBus, post-hoc)**:
- `chat.message_done` → MemorySub.persist_episodic
- `chat.message_done` → surface_broadcast (pet / chat / workspace)
- `persona.activated` → InitiativeSub.refresh_weights
- `scheduler.idle_threshold_crossed` → InitiativeSub.evaluate_proactive
- `living.mood_changed` → InitiativeSub.rescore
- `tool.executed` → tool_audit_log + telemetry
- `safety.violation` → log + telemetry
- `task.reminder_fired` → LivingSub.maybe_react
- `consent.changed` → LifecycleManager.gate_recheck
- `wake.completed` → InitiativeSub.missed-job evaluate

**事件总数 9 个** (< 15 上限 ✓)

### 5.3 Persistent / Transient / Lazy

| 类别 | 内容 |
|---|---|
| **Persistent** | 18 张表全 |
| **Transient (in-memory)** | `working_memory` / `active_streams` CancellationToken map / cached active_persona / SchedulerJobs handles |
| **Lazy-computed** (store source + recompute on read) | `mood = decay(last_event, base, persona_modifier)` / `energy = decay(last_event, recent_activities)` / `wandering_target` / `memory_retrieval_score = recency + fts_match + emotional_weight` |

### 5.4 Hot Path 延迟预算

| 路径 | 类型 | 预算 |
|---|---|---|
| user input → first token | hot sync | **p50 ≤ 1.5s** (架构 §10.2 现有口径) |
| persona switch → new reply | hot sync | < 500ms |
| tool exec (Read 文件) → result | hot sync | < 200ms (含沙盒 check) |
| mood / energy recompute on read | lazy | < 5ms |
| memory retrieval (FTS5) | hot sync | < 50ms |
| proactive evaluate (scheduler tick) | async | < 100ms, 不阻塞主进程 |
| episodic memory persist (post chat) | async | < 200ms, 不卡用户 |
| multi-surface broadcast | async | < 50ms |

---

## 6. Lifecycle Graph

### 6.1 顶层状态机（5 states）

```
   ┌──────────── process spawn ─────────────┐
   │                                         │
   ↓                                         │
 ╔══════╗  consent ✓     ╔══════╗  tray quit │
 ║ Boot ║─────────────→ ║ Live ║───────────→ ║ Shutdown ║ → exit
 ╚══════╝               ╚══╤═══╝               ╚══════════╝
                           │
                    OS suspend │ OS wake
                           ↓
                       ╔═══════╗   consistency ✓     ╔═══════╗
                       ║Suspend║ ────────────────→  ║ Wake  ║ → Live
                       ╚═══════╝                     ╚═══════╝
```

### 6.2 Boot entry actions（sync, 严格 dependency order）

```
1. MigrationService.run()         schema check + 备份 + 升级
2. StateStore.open()              sqlx pool + WAL + PRAGMA fk
3. SafetyGuard.load()             prefix_v1.txt + regional
4. EventBus.init()                typed pub/sub + event_log writer
5. Scheduler.start()              tokio runtime + cron tab 空启
6. 6 subsystems.init(handles)     每个拿到 (StateStore cap, EventBus, Scheduler, SafetyGuard)
   ├ PersonaSub: load_active
   ├ MemorySub: open episodic + alloc working
   ├ ConversationSub: cancel_token map
   ├ ToolSub: load whitelist + 注册 3 tool
   ├ InitiativeSub: read quiet_hours/quota
   └ LivingSub: restore last mood/energy
7. LifecycleManager.gate(consent) onboarding 续接 / consent.version
8. → Live (sub-state = Idle)
```

### 6.3 Live 4 个 sub-state

```
                        Live
                          │
       ┌──────────────────┼──────────────────────┐
       ↓                  ↓                        ↓
     Idle  ←──→     Conversing      Proactive
                          ↕
                     Toolusing
                  (within Conversing)
```

| Sub-state | 入条件 | 出条件 | 主要动作 |
|---|---|---|---|
| **Idle** | Live default | user_input / proactive fire | mood/energy lazy aging accumulate;surface 静态/微动 |
| **Conversing** | user_input from any surface | message_done | hot path sync(见 §5.2) |
| **Toolusing** | LLM stream emit tool_call | tool result inject | ToolSub safety check → exec → 回 Conversing |
| **Proactive** | InitiativeSub.evaluate=fire | proactive_done OR user 回复 → Conversing | Soul-selected template;偶 light LLM;surface 弹气泡 |

### 6.4 Suspend / Wake（OS-driven）

**Suspend entry** (Win `PBT_APMSUSPEND`):
- 取消所有 `active_streams.cancel_all()`
- `working_memory.flush_to_persistent`
- `pet_runtime_state.snapshot(mood_energy_now)`
- `Scheduler.pause_all_jobs`

**Wake entry** (`PBT_APMRESUMEAUTOMATIC`):
1. Consistency check (WAL recovery / schema_version)
2. `LivingSub.recompute_mood_energy(elapsed_since_suspend)` — **lazy aging 跨 suspend 一次性 catch up**
3. `Scheduler.resume_jobs` + 检查 missed jobs (quota 限上限)
4. RDP 检测 (沿用现有逻辑)
5. → Live (Idle)

### 6.5 Tick 位置（明确, Constitution #4 落地）

**Tick 仅 4 处, 全部 Scheduler 拥有**:

| Tick 类型 | 频率 | 用途 |
|---|---|---|
| **Cron job** | by trigger spec | ReminderService task trigger |
| **Idle threshold cross** | poll 30s, 跨阈值时 publish event | IdleDetector |
| **One-shot timer** | 单次 | Pomodoro / scheduled action |
| **Periodic maintenance** | 5 min (可调) | working_memory 压缩 → episodic |

**没有的 tick** (都改 lazy / event-driven):
- ❌ mood/energy 不 tick(lazy aging)
- ❌ wandering 不 tick(scheduler 周期触发 + 一次算路径 + 播放完即停)
- ❌ Conversing/Toolusing 完全 event-driven

### 6.6 Lazy 计算（read-time compute, 不持久化中间值）

| Lazy 对象 | source | recompute formula | trigger |
|---|---|---|---|
| `current_mood` | `pet_runtime_state.mood + last_mood_event_at` | `clamp(base + Σ event_deltas · decay(elapsed), 0, 100)` | InitiativeSub.evaluate / Soul.build_prompt / surface render |
| `current_energy` | `state.energy + last_event + recent_activities` | `clamp(base + recovery(elapsed) - cost(activities), 0, 100)` | 同上 |
| `proactive_score(candidate)` | candidate triggers + persona + mood + quota | weighted sum(见 §10) | InitiativeSub.select_candidate |
| `memory_relevance(item, query)` | FTS5 match + recency + emotional_weight | `0.5·fts + 0.3·recency + 0.2·emotion` | MemorySub.retrieve |

### 6.7 状态迁移决策表

| 当前 | 事件 | 下一 | sync/async |
|---|---|---|---|
| — | process spawn | Boot | sync |
| Boot | gate pass | Live(Idle) | sync |
| Boot | consent expired | Live + onboarding window | sync(走 ADR-019 续接) |
| Live(Idle) | user_input | Live(Conversing) | **sync hot** |
| Live(Conversing) | message_done | Live(Idle) | sync |
| Live(Conversing) | LLM emit tool_call | Live(Toolusing) | **sync hot** |
| Live(Toolusing) | tool.executed | Live(Conversing) | sync(回 agent loop) |
| Live(Toolusing) | safety.violation | Live(Conversing) + 拒答替换 | sync |
| Live(Idle) | InitiativeSub.evaluate=fire | Live(Proactive) | async |
| Live(Proactive) | done OR user reply | Live(Idle) OR Live(Conversing) | async |
| Live(*) | OS suspend signal | Suspend | sync(强制 cancel streams) |
| Suspend | OS wake signal | Wake | sync |
| Wake | consistency pass | Live(Idle) | sync |
| Live(*) | tray "Quit" | Shutdown | sync |
| Shutdown | flush done | exit | sync |

---

## 7. Event Model

### 7.1 Event Catalog（9 个, Constitution #8 上限 15 内）

| # | Event | Publisher | Subscribers | Payload | Persist |
|---|---|---|---|---|---|
| 1 | `chat.message_done` | ConversationSub | MemorySub(persist_episodic) / surface_broadcast | `{conv_id, msg_id, role, content, mode, token_count, finish_reason}` | yes(event_log)+ messages 表 |
| 2 | `persona.activated` | PersonaSub | InitiativeSub(refresh_weights) / surface_broadcast | `{persona_id, name, previous_id}` | yes |
| 3 | `scheduler.idle_threshold_crossed` | Scheduler | InitiativeSub(evaluate_proactive) | `{idle_ms, threshold_ms, since}` | no(transient) |
| 4 | `living.mood_changed` | LivingSub | InitiativeSub(rescore) / surface_broadcast | `{from, to, transient: bool, trigger, source_event_id}` | yes(7d) |
| 5 | `tool.executed` | ToolSub | tool_audit_log writer / telemetry | `{tool_id, args_hash, status, latency_ms, exit_code, capability_token_id}` | yes(persistent) |
| 6 | `safety.violation` | SafetyGuard | telemetry / error_logs | `{violation_kind, rule_id, snippet_hash, persona_id, action_taken}` | yes |
| 7 | `task.reminder_fired` | TaskService(reminders 独立) | LivingSub(maybe_react) / surface(notification) | `{reminder_id, title, priority}` | already in reminder_history |
| 8 | `consent.changed` | LifecycleManager | gate_recheck (all subsystems via subscribe) | `{version_old, version_new, granted, method}` | yes |
| 9 | `wake.completed` | LifecycleManager | InitiativeSub(missed-job evaluate) | `{suspended_at, woke_at, elapsed_secs}` | yes |

**故意不做的 event**(防止泛滥):
- ❌ `chat.token` — hot path, 走 Tauri Channel 不入 EventBus
- ❌ `tool.started` — `tool.executed` 已含 status 字段就够
- ❌ `pet.energy_changed` — Living 内部, 通过 surface read mood 即可
- ❌ `nickname.changed` / `persona.edited` — 走 Tauri emit, 不影响 runtime
- ❌ `wardrobe.changed` — 装扮系统不在 6 subsystem 内
- ❌ `pomodoro.tick` — 高频, 走 Tauri emit

### 7.2 EventBus 类型化签名

```rust
trait EventBus: Send + Sync {
    /// publish 是 sync: 写 event_log(若 PERSIST) + spawn 通知 subscribers
    /// 不阻塞 publisher; 通知 spawn 用 mpsc 队列 per-subscriber FIFO
    fn publish<E: Event>(&self, event: E) -> Result<EventId, EventError>;

    /// subscribe 仅在 Boot 期 subsystems.init 阶段调用
    /// Live 期 dynamic subscribe 拒绝(避免 race + 防 Soul 越权 — Constitution #5)
    fn subscribe<E: Event>(
        &self,
        subscriber_id: SubscriberId,
        handler: Box<dyn Fn(&E) -> Result<(), HandlerError> + Send + Sync>,
    );
}

trait Event: Serialize + DeserializeOwned + Send + Sync + 'static {
    const KIND: &'static str;       // 'chat.message_done' 等
    const PERSIST: bool;            // 是否写 event_log
    const SCHEMA_VERSION: u32;      // 未来 schema 演进
}
```

### 7.3 Publish 语义（明确 sync / async 边界）

```
publisher 调 .publish(event)         [sync, ≤ 5ms]
  ↓
  ├ if PERSIST: INSERT event_log (sync)
  ├ for each subscriber:
  │    enqueue to subscriber's mpsc channel (sync)
  └ return EventId                   [sync 完成]

subscriber tokio task (per subscriber)  [async, FIFO]
  loop {
      let event = channel.recv().await;
      match handler(&event) {
          Ok(_) => continue,
          Err(e) => error_logs.append(e); continue;    // 不传染, 不重试
      }
  }
```

**关键规则**:
- publisher **同步完成入 log + enqueue**, 不等 subscriber
- subscriber **FIFO per subscriber**, 跨 subscriber 不保序
- subscriber handler 错误 → 写 `error_logs` + **继续下一条**(不重试, 不死信队列)
- publish 失败(DB 写不进)→ **panic** → 走 Suspend(kernel-level invariant 已破)
- subscriber 内 publish **同 KIND** event → Boot 期 wire 时静态校验拒绝(防环)

### 7.4 Replay / Recovery（MVP 不做）

| 能力 | MVP | P1+ |
|---|---|---|
| event_log persistence | ✅ | ✅ |
| observability / debug 查询 | ✅(直接 sql) | ✅ |
| event_sourcing replay 重建 state | ❌ | 评估 |
| dynamic subscribe(Live 期挂新 subscriber) | ❌ | 评估 |
| event filtering / batching | ❌ | 评估 |

**理由**: MVP state 已通过 SQLite 主表 + FTS5 持久化,不需要 event 重建。`event_log` 在 MVP 仅 observability。

### 7.5 Event vs Direct call 决策树

```
是否跨 subsystem 边界?
├─ 否 → 内部方法调用 (无 event)
└─ 是 ┐
      ├ 是否在 hot path (user→reply)?
      │  └─ 是 → 严禁 publish, 走 sync read trait
      │         (Constitution #7)
      │
      ├ 是否需要"事后通知 + 不阻塞 caller"?
      │  └─ 是 → publish event
      │
      └ 需要写跨 ownership 的表?
         └─ 是 → 必须 publish event (由 owner subscribe + write)
                (Constitution #3)
```

具体例:
- ConversationSub 收完 msg → MemorySub 写 episodic → cross-sub + write + post-hoc → **`chat.message_done`** ✓
- ConversationSub 拼 prompt 时读 active persona → cross-sub + read + hot path → **直接调 `PersonaSub.read_active()`** ✓
- LivingSub mood 变 → InitiativeSub 想知道 → cross-sub + read + post-hoc → **`living.mood_changed`** ✓
- ToolSub 执行后写自家 audit_log → 自家 owner, 无需 event → **直接 `self.audit_log.write()`** ✓(`tool.executed` event 是给 telemetry 的另一目的)

---

## 8. Core Contracts

### 8.1 Capability Token 模式（Constitution #2 编译期落地）

```rust
/// 每张被 owned 的表实现 Owned; 关联 Owner 类型 = subsystem
pub trait Owned: Send + Sync + 'static {
    const TABLE: &'static str;
    type Owner;
}

/// WriterCap 由 kernel 在 Boot 期颁发给 owner subsystem; 无法跨 sub 转让
pub struct WriterCap<T: Owned> {
    _marker: PhantomData<T>,
    _kernel_secret: KernelSecret,   // 私有 newtype, subsystem 无法构造
}

/// StateStore.write 强制要求 cap; PersonaSub 拿不到 ConversationSub 的 cap
/// → 编译期拒绝跨 ownership write
impl StateStore {
    pub async fn write<T: Owned>(
        &self,
        cap: &WriterCap<T>,
        query: WriteQuery,
    ) -> Result<()>;
}
```

### 8.2 Kernel 5 Traits

```rust
//═══════════════════════════════════════════════════════════════════
// 1. SafetyGuard — Constitution #1
//═══════════════════════════════════════════════════════════════════
pub trait SafetyGuard: Send + Sync {
    /// LLM 调用前包装(prefix 强制第一位 + 地区补充); subsystem 拿到的已是包装后
    fn wrap_messages(&self, messages: Vec<ChatMessage>, locale: Locale) -> Vec<ChatMessage>;
    /// 流式增量扫描(快速); 命中违禁 → 返 Scrub(替换文本)
    fn scan_token(&self, partial: &str, finished: bool) -> ScanResult;
    /// 流终态全文二次扫描; 命中 → publish safety.violation + 触发拒答降级链
    fn scan_final(&self, full_text: &str, persona_id: &str) -> ScanFinalResult;
}

//═══════════════════════════════════════════════════════════════════
// 2. LifecycleManager — §6 落地
//═══════════════════════════════════════════════════════════════════
pub trait LifecycleManager: Send + Sync {
    fn current_state(&self) -> LifecycleState;
    async fn transition(&self, to: LifecycleState) -> Result<(), TransitionError>;
    fn subscribe_state_change(&self, sub_id: SubscriberId, h: StateChangeHandler);
}

//═══════════════════════════════════════════════════════════════════
// 3. EventBus — §7 已定义, 此处只列签名
//═══════════════════════════════════════════════════════════════════
pub trait EventBus: Send + Sync {
    fn publish<E: Event>(&self, event: E) -> Result<EventId, EventError>;
    fn subscribe<E: Event>(&self, sub_id: SubscriberId, h: EventHandler<E>);
}

//═══════════════════════════════════════════════════════════════════
// 4. Scheduler — Constitution #4 唯一持有 timer
//═══════════════════════════════════════════════════════════════════
pub trait Scheduler: Send + Sync {
    fn register_cron(&self, spec: CronSpec, h: TickHandler) -> JobId;
    fn register_idle_threshold(&self, ms: u64, h: TickHandler) -> JobId;
    fn register_one_shot(&self, when: Instant, h: TickHandler) -> JobId;
    fn register_periodic(&self, every: Duration, h: TickHandler) -> JobId;
    fn cancel(&self, job: JobId);
    fn pause_all(&self);   // Suspend
    fn resume_all(&self);  // Wake, 含 missed-job 检查
}

//═══════════════════════════════════════════════════════════════════
// 5. StateStore — Constitution #2 capability 落地处
//═══════════════════════════════════════════════════════════════════
pub trait StateStore: Send + Sync {
    fn kernel_table(&self) -> KernelTableHandle;
    async fn write<T: Owned>(&self, cap: &WriterCap<T>, q: WriteQuery) -> Result<()>;
    async fn read(&self, q: ReadQuery) -> Result<Rows>;
    async fn tx<F, R>(&self, f: F) -> Result<R>
        where F: for<'a> AsyncFnOnce(&'a mut Tx) -> Result<R>;
}
```

### 8.3 Subsystem 6 Traits

```rust
//─── 1. PersonaSubsystem ─────────────────────────────────────
pub trait PersonaSubsystem: Send + Sync {
    async fn read_active(&self) -> Result<PersonaSummary>;
    async fn read_by_id(&self, id: &str) -> Result<Persona>;
    async fn list_all(&self) -> Result<Vec<PersonaMeta>>;
    async fn activate(&self, id: &str) -> Result<()>;        // publish persona.activated
    async fn save(&self, draft: PersonaDraft) -> Result<(PersonaId, Version)>;
    async fn sandbox_chat(&self, draft_md: &str, input: &str) -> Result<String>;
}

//─── 2. MemorySubsystem ─────────────────────────────────────
pub trait MemorySubsystem: Send + Sync {
    /// hot path: prompt 拼装时调用; < 50ms
    async fn retrieve(&self, ctx: RetrievalContext) -> Result<RetrievedMemory>;

    /// async: chat.message_done handler 内调用
    async fn persist_message_to_working(&self, conv_id: &str, msg: &MessageRecord) -> Result<()>;

    /// periodic_maintenance tick 内调用(5min); working → episodic
    async fn compress_working_to_episodic(&self) -> Result<u32>;

    /// KV 偏好(扩展现有 memory 表)
    async fn set_fact(&self, key: &str, value: &str, source: FactSource) -> Result<()>;
}

//─── 3. InitiativeSubsystem ─────────────────────────────────────
pub trait InitiativeSubsystem: Send + Sync {
    /// scheduler.idle_threshold_crossed handler 内
    async fn evaluate_proactive(&self, trigger: ProactiveTrigger) -> Result<EvalResult>;

    /// persona.activated / living.mood_changed handler 内(in-memory 重排)
    fn rescore_candidates(&self);

    async fn record_response(&self, log_id: i64, response: UserResponse) -> Result<()>;
    async fn get_settings(&self) -> Result<InitiativeSettings>;
    async fn set_settings(&self, s: InitiativeSettings) -> Result<()>;
}

//─── 4. ConversationSubsystem ─────────────────────────────────────
pub trait ConversationSubsystem: Send + Sync {
    /// hot path entry: 任一 surface 输入触发
    async fn handle_user_message(
        &self,
        surface: SurfaceId,
        input: String,
        conv_id: Option<String>,
        channel: Channel<StreamEvent>,
    ) -> Result<SendResult>;

    fn cancel(&self, message_id: &str);
    async fn history(&self, conv_id: &str, limit: u32) -> Result<Vec<MessageRecord>>;

    // conversation 管理(已有 6 IPC 1:1 升级)
    async fn list_conversations(&self, limit: u32) -> Result<Vec<ConvSummary>>;
    async fn create_conversation(&self, persona_id: &str) -> Result<ConvId>;
    async fn rename(&self, conv_id: &str, title: &str) -> Result<()>;
    async fn archive(&self, conv_id: &str) -> Result<()>;
    async fn delete(&self, conv_id: &str) -> Result<()>;
    async fn set_active(&self, conv_id: &str) -> Result<()>;
}

//─── 5. ToolSubsystem ─────────────────────────────────────
pub trait ToolSubsystem: Send + Sync {
    /// hot path: ConversationSub 在 LLM emit tool_call 后调用
    async fn execute(
        &self,
        tool_id: &str,
        args: ToolArgs,
        ctx: ToolContext,                  // 含 persona_id / conv_id / 用户授权 grant
    ) -> Result<ToolResult, ToolError>;

    fn list_available(&self) -> Vec<ToolDefinition>;
    fn whitelist(&self) -> &PathWhitelist;
}

//─── 6. LivingSubsystem ─────────────────────────────────────
pub trait LivingSubsystem: Send + Sync {
    /// hot path read: Soul prompt 注入 + InitiativeSub 评估 + surface render; < 5ms
    fn read_current(&self) -> LiveState;       // lazy recompute mood/energy

    async fn record_event(&self, event: LivingEvent) -> Result<()>;
    async fn set_feature_enabled(&self, feature: LiveFeature, enabled: bool) -> Result<()>;
}
```

### 8.4 Soul Overlay 4 Components

```rust
//─── 1. PersonaPromptBuilder ─────────────────────────────────────
pub trait PersonaPromptBuilder: Send + Sync {
    fn build(
        &self,
        persona: &PersonaSummary,
        nicknames: &Nicknames,
        live: &LiveState,             // mood/energy 影响语气提示
        context: &PromptContext,
    ) -> SystemMessage;
}

//─── 2. ToneShaper ─────────────────────────────────────
pub trait ToneShaper: Send + Sync {
    /// proactive 文案最后一步 / 部分场景 tool 调用前 narration
    fn shape(&self, raw: &str, persona: &PersonaSummary, live: &LiveState) -> String;
}

//─── 3. InitiativeWeights ─────────────────────────────────────
pub trait InitiativeWeights: Send + Sync {
    fn score(
        &self,
        candidate: &ProactiveCandidate,
        persona: &PersonaSummary,
        live: &LiveState,
    ) -> f32;
}

//─── 4. RetrievalRanker ─────────────────────────────────────
pub trait RetrievalRanker: Send + Sync {
    fn rank(
        &self,
        items: Vec<RetrievalCandidate>,
        context: &RetrievalContext,
        persona: &PersonaSummary,
    ) -> Vec<RetrievalCandidate>;
}
```

### 8.5 依赖图（subsystem 启动 wire 顺序 + 谁调谁）

```
         ┌─────────────────────────────────────────────────────┐
         │                  KERNEL (Boot 1-5)                   │
         │  StateStore → SafetyGuard → EventBus → Scheduler →   │
         │                 LifecycleManager                      │
         └─┬──────────┬──────────┬──────────┬──────────┬───────┘
           │ all sub  │ Conver-  │ all sub  │ Init/Mem │ all sub
           │ get cap  │ sation   │ subscribe│ schedule │ check state
           │ + handle │ + Tool   │ events   │ tick     │
           ↓          ↓          ↓          ↓          ↓
   ┌─────────────────────────────────────────────────────────┐
   │              SUBSYSTEMS (Boot 6, parallel init)          │
   │                                                          │
   │  PersonaSub  ─→ (read) ──→  ConversationSub  ←─ ToolSub │
   │       ↑                          │  ↓                    │
   │       │                          │  ↓ (call execute)    │
   │  InitiativeSub ←(read mood)─ LivingSub                  │
   │       ↑                                                  │
   │   MemorySub                                              │
   │                                                          │
   │  ※ 箭头 = sync read trait 调用方向                       │
   │  ※ 跨 sub write 全部经 EventBus(图未画, 见 §7)         │
   └──────────────────────────────────────────────────────────┘
              ↑                                  ↑
              │ Soul read 6 subsystem            │ Surfaces read 6 subsystem
              │ + write nothing                  │ + 通过 IPC 调 trait
              │                                  │
   ┌──────────────────────────┐   ┌──────────────────────────┐
   │       SOUL OVERLAY        │   │      SURFACES (Tauri)     │
   │  PromptBuilder            │   │  Pet / Chat / Workspace   │
   │  ToneShaper               │   │  / Tray + Notification    │
   │  InitiativeWeights        │   │                            │
   │  RetrievalRanker          │   │                            │
   └──────────────────────────┘   └──────────────────────────┘
```

### 8.6 Trait 总额清算

| 层 | trait 数 | method 总数 |
|---|---|---|
| Kernel | 5 | 20 |
| Subsystem | 6 | 31 |
| Soul | 4 | 4 |
| **总计** | **15** (Constitution #8 天花板) | **55** |

每个 method 都是 hot path / async event handler / settings UI 三类之一, 无装饰性方法。

---

## 9. Memory Schema (MVP)

### 9.1 3 层 memory 分工

| 层 | 介质 | 持久性 | 写入路径 |
|---|---|---|---|
| **working_memory** | in-memory `VecDeque<MessageRef>` per conv | transient + flush on shutdown | hot path, chat.message_done event handler |
| **episodic_memory** | SQLite + FTS5 | persistent | periodic_maintenance tick(5min), 压缩 working → episodic |
| **semantic_memory** | SQLite `memory` 表 KV(扩展现有) | persistent | 用户主动 set / 摘要任务提取 long-term fact |

### 9.2 Episodic Schema

```sql
CREATE TABLE episodic_memory (
    id TEXT PRIMARY KEY,                       -- ULID
    conversation_id TEXT,                       -- 可空(proactive episode 无 conv)
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL,
    summary TEXT NOT NULL,                      -- 100-300 字 LLM 摘要
    entities TEXT NOT NULL DEFAULT '[]',        -- JSON: ["项目X","猫",...]
    emotional_tags TEXT NOT NULL DEFAULT '[]',  -- JSON: ["happy","concerned",...]
    emotional_weight REAL NOT NULL DEFAULT 0.5, -- [0,1]
    source_message_ids TEXT NOT NULL,           -- JSON ULID[], 回溯原文
    persona_id TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_episodic_persona ON episodic_memory(persona_id, ended_at DESC);
CREATE INDEX idx_episodic_emotion ON episodic_memory(emotional_weight DESC, ended_at DESC);

CREATE VIRTUAL TABLE episodic_memory_fts USING fts5(
    summary, entities, emotional_tags,
    content='episodic_memory', content_rowid='id'
);

CREATE TRIGGER episodic_ai AFTER INSERT ON episodic_memory BEGIN
    INSERT INTO episodic_memory_fts(rowid, summary, entities, emotional_tags)
    VALUES (new.id, new.summary, new.entities, new.emotional_tags);
END;
CREATE TRIGGER episodic_ad AFTER DELETE ON episodic_memory BEGIN
    INSERT INTO episodic_memory_fts(episodic_memory_fts, rowid, summary, entities, emotional_tags)
    VALUES('delete', old.id, old.summary, old.entities, old.emotional_tags);
END;
CREATE TRIGGER episodic_au AFTER UPDATE ON episodic_memory BEGIN
    INSERT INTO episodic_memory_fts(episodic_memory_fts, rowid, summary, entities, emotional_tags)
    VALUES('delete', old.id, old.summary, old.entities, old.emotional_tags);
    INSERT INTO episodic_memory_fts(rowid, summary, entities, emotional_tags)
    VALUES (new.id, new.summary, new.entities, new.emotional_tags);
END;
```

### 9.3 Retrieval Pipeline（hot path, p50 < 50ms）

```
ConversationSub.handle_user_message → MemorySub.retrieve(ctx)
  1. FTS5 query: MATCH input_keywords + persona_id filter, LIMIT 20
  2. score per item:
       fts        = rank()                    -- FTS5 内置 bm25
       recency    = exp(-elapsed_days / 30)   -- 半衰期 30 天
       emotion    = item.emotional_weight
       score      = 0.5·fts + 0.3·recency + 0.2·emotion
  3. Soul.RetrievalRanker.rank(items, ctx, persona) → re-ranked
  4. take top K=3 (MVP 固定)
  5. format memory_bullets:
       "我们之前聊过 {summary}(那时你 {emotional_tag})"
  6. return RetrievedMemory { bullets, debug_score }
```

### 9.4 压缩流程（periodic_maintenance tick, 5min）

```
MemorySub.compress_working_to_episodic
  1. 扫所有 working_memory[conv_id], 找 last_activity > 30min 的 conv
  2. 取这段 5-10 turn 拼 raw_dialog
  3. 调 LLM 用 "摘要 persona"(不污染 active persona)生成:
       { summary, entities, emotional_tags, emotional_weight }
  4. INSERT episodic_memory + source_message_ids
  5. working_memory[conv_id] compact (保留 last 3 turn for continuity)
```

**MVP scope**:
- ✅ FTS5 retrieval / 摘要压缩 / 三因子排序 / Soul re-rank
- 🟡 P1+: embeddings/vector / 跨 conv entity 链接 / 用户主动 forget
- 🔴 不做: 多模态 memory item / memory diff / forget cascade

---

## 10. Initiative Pipeline (MVP)

### 10.1 4 种 trigger

| Trigger | 来源 | 频率 |
|---|---|---|
| `idle_threshold_crossed` | IdleDetector → Scheduler | 跨阈值 1 次 |
| `living.mood_changed` | LivingSub → EventBus | mood 变化时 |
| `task.reminder_fired` | TaskService → EventBus | 用户提醒触发 |
| `wake.completed` | LifecycleManager → EventBus | wake 完成 |

### 10.2 Evaluation pipeline（async event handler）

```
InitiativeSub.evaluate_proactive(trigger)

  ─── Step 0: hard gates (Constitution #4/6, 不可被 Soul 跳过) ─────────
  if in_quiet_hours()                  return Skip("quiet_hours")
  if proactive_today_count >= 4        return Skip("daily_quota")
  if last_fired_within(2.hours)        return Skip("cooldown")
  if user_disabled                     return Skip("user_disabled")

  ─── Step 1: 生成 candidates ─────────────────────────────────────
  let candidates = match trigger {
      Idle         => [empathy, greeting, banter],
      MoodChanged  => [empathy, comfort],
      ReminderFired=> [gentle_remind],
      WakeCompleted=> [greeting],
  }

  ─── Step 2: Soul 加权(only score, not select) ───────────────────
  let persona = persona_sub.read_active()
  let live    = living_sub.read_current()        // lazy recompute
  let scored: Vec<(Candidate, f32)> =
      candidates.iter().map(|c| (c, soul.weights.score(c, &persona, &live)))

  ─── Step 3: 选 top 1 + 抽人格模板 ──────────────────────────────
  let chosen   = scored.max_by_score()
  let template = persona_sub.read_offline_template(chosen.category, &persona)

  ─── Step 4: Soul ToneShape ─────────────────────────────────────
  let text = soul.tone_shaper.shape(&template, &persona, &live)

  ─── Step 5: log + surface emit ─────────────────────────────────
  proactive_care_log.insert(...)
  surface_emit_proactive(text)
  return Fired(log_id)
```

### 10.3 MVP candidate categories

| Category | Trigger | Example template |
|---|---|---|
| empathy | idle / mood_negative | "{user} 怎么样, 还顺利吗?" |
| greeting | wake / idle long | "嘿, 又见面了" |
| gentle_remind | reminder_fired | "{reminder.title}, 别忘了哦" |
| comfort | mood_changed → sad | "我在的, 有空说说?" |

### 10.4 Score formula

```
soul.weights.score(c, persona, live) =
    base_weight[category]                       // 0.5
  + persona.tone_profile.proactivity * 0.2
  + mood_alignment(c.category, live.mood) * 0.2
  + same_category_24h_count * (-0.1)            // 重复惩罚
```

**MVP scope**:
- ✅ 4 trigger / 4 category / hard gate / Soul score / template+tone / log
- 🟡 P1+: LLM-driven candidate generation / contextual banter / user-defined trigger
- 🔴 不做: 持续观察类 reaction("看你打开了 X 5 分钟后评论") / proactive 多轮深入

---

## 11. Tool Capability Model (MVP)

> **本节给出的 path whitelist / capability model / audit schema 是 ADR-025 的 MVP 默认参考**。正式 ADR-025 文档需独立提交后审议。

### 11.1 MVP 3 个 read-only tool

| Tool | Schema | 输出 | 上限 |
|---|---|---|---|
| `glob` | `{pattern: string}` | `{matches: string[]}` | max 500 matches |
| `grep` | `{pattern: string, path?: string, regex?: bool}` | `{lines: {file, lineno, text}[]}` | max 200 matches / 1000 files |
| `read` | `{path: string, range?: [start,end]}` | `{content, lineCount}` | max 1MB / 10K lines / range default last 2K lines |

### 11.2 Path Whitelist + Denylist

```
✅ 白名单:
  %APPDATA%\AIDesktopPet\personas\user\**     (用户写的 .soul.md)
  %APPDATA%\AIDesktopPet\file_drop\**         (用户拖入的文件)
  assets\game_scenes\**.yaml                  (游戏场景)
  assets\safety\prefix_v1.txt                 (允许 LLM self-reflection 读 prefix)

🔴 硬拒(白名单内也拒):
  %APPDATA%\AIDesktopPet\app.db                (数据库自己不能读)
  %APPDATA%\AIDesktopPet\secrets
  C:\Windows\ / C:\Program Files\
  **/secrets/** / **/.env*
```

### 11.3 Capability model（编译期 + 运行期组合）

```rust
pub struct ToolContext {
    pub persona_id: String,
    pub conv_id: String,
    pub user_grant: Grant,
}
pub enum Grant {
    None,                          // 默认, 首次调用弹窗
    SessionScope(SessionId),       // 本 session 内不再问
    PersistentByTool(ToolId),      // 用户勾"以后不问", 写 config
}

ToolSub.execute(tool_id, args, ctx):
  1. lookup tool by id (unknown → reject)
  2. validate args vs ToolDefinition.parameters JSON Schema
  3. resolve path (canonicalize, 防 .. 跳出)
  4. check whitelist + denylist (硬拒早返)
  5. check grant (None → emit grant_request event; 阻塞等 ack)
  6. execute with bounded resources (size/time/count limit)
  7. write tool_audit_log + publish tool.executed
  8. return result (or ToolError::Denied / ToolError::Limit)
```

### 11.4 Audit log schema

```sql
CREATE TABLE tool_audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_id TEXT NOT NULL,
    persona_id TEXT NOT NULL,
    conv_id TEXT,
    args_hash TEXT NOT NULL,           -- SHA256 of args JSON
    paths TEXT NOT NULL DEFAULT '[]',  -- JSON 实际访问 path(供 review)
    status TEXT NOT NULL,              -- 'ok'|'denied:whitelist'|'denied:user'|'error'
    latency_ms INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    grant_kind TEXT NOT NULL           -- 'session'|'persistent'|'once'
);
CREATE INDEX idx_audit_persona ON tool_audit_log(persona_id, started_at DESC);
```

### 11.5 Tool 调用的 hot path 集成

```
ConversationSub.handle_user_message (Conversing sub-state)
  ↓ LLM emits tool_call delta
ConversationSub → state transition → Toolusing
  ↓
ToolSub.execute(tool_id, args, ctx)              // hot sync < 200ms
  ↓
  result inject as ChatMessage(role=tool, content=result_json)
  ↓
ConversationSub → state transition → Conversing
  ↓
  next LLM iteration with tool result in messages
```

**MVP scope**:
- ✅ 3 read-only tool / whitelist + denylist / capability + grant UX / audit log / hot path 集成
- 🟡 P1+: ADR-025 完整决议 / Edit/Write/Bash + 撤销机制 / WebFetch+WebSearch / 用户自定义 path
- 🔴 不做: MCP server bridge / 自定义 tool / tool chaining / background tool

---

## 12. Migration Path from AIPET

### 12.1 现有 service → subsystem 映射

| AIPET 现有文件 | 目标位置 | 改造方式 | 工作量 |
|---|---|---|---|
| `services/chat/service.rs` | `subsystems/conversation/service.rs` | 实现 `ConversationSubsystem` trait;hot path 接 SafetyGuard wrap/scan;tool_call 处理改转 ToolSub | ~1 周 |
| `services/chat/prompt.rs` | `subsystems/conversation/prompt.rs` + `soul/prompt_builder.rs` | 拆: 拼装机制留 conversation; persona 内容生成移 soul | ~3 天 |
| `services/chat/conversation.rs` | `subsystems/conversation/store.rs` | 几乎不动 | ~1 天 |
| `services/persona.rs` | `subsystems/persona/service.rs` | 实现 `PersonaSubsystem` trait;activate 时 publish | ~3 天 |
| `services/memory.rs` | `subsystems/memory/legacy.rs` + new `subsystems/memory/episodic.rs` | 现有 KV 含义不变, 新增 episodic+FTS5+working | ~1.5 周 |
| `services/living_pet.rs` | `subsystems/living/service.rs` | tick → lazy aging; mood_changed → EventBus publish | ~5 天 |
| (规划) `services/proactive_care.rs` | `subsystems/initiative/service.rs` | 落地 + EventBus subscribe + Soul score | ~5 天 |
| `services/llm/` | 不动 (L1 Provider 抽象, 不是 subsystem) | 0% | 0 |
| `services/llm_providers.rs` | 不动 | 0% | 0 |
| `services/scheduler.rs` | `kernel/scheduler.rs` | 现有是空骨架, 实现完整 4 触发类型 | ~3 天 |
| `services/db.rs` / `migration.rs` | `kernel/state_store.rs` 包装 | 加 capability token 层 | ~3 天 |
| `services/consent.rs` / `consent_gate.rs` | `kernel/lifecycle_manager.rs` gate 函数 | 整合到 Boot.7 gate | ~2 天 |
| `services/onboarding.rs` | 不动 | 0% | 0 |
| `services/reminder.rs` / `pomodoro.rs` / `todo.rs` | 保留独立 (TaskService) | 加 EventBus emit `task.reminder_fired` | ~1 天 |
| `services/snap.rs` / `window_*` / `tray.rs` / `shortcuts.rs` | 保留独立 (Surfaces 工具) | 0% | 0 |
| `services/avatars.rs` / `preferences.rs` / `nickname.rs` | `subsystems/persona/` 或 保留 | nickname 归 PersonaSub | ~1 天 |

### 12.2 SQLite migration plan

```
migrations/004_runtime_v1.sql
  -- 修改
  ALTER TABLE messages ADD COLUMN token_count INTEGER DEFAULT NULL;
  ALTER TABLE messages ADD COLUMN safety_scan_status TEXT DEFAULT 'pending';
  ALTER TABLE pet_runtime_state ADD COLUMN last_mood_event_at TEXT;
  ALTER TABLE pet_runtime_state ADD COLUMN last_energy_event_at TEXT;

  -- 新增
  CREATE TABLE episodic_memory (...);
  CREATE VIRTUAL TABLE episodic_memory_fts USING fts5(...);
  CREATE TRIGGER episodic_ai ...;
  CREATE TRIGGER episodic_ad ...;
  CREATE TRIGGER episodic_au ...;
  CREATE TABLE event_log (...);
  CREATE TABLE tool_audit_log (...);
  CREATE INDEX idx_audit_persona ON tool_audit_log(persona_id, started_at DESC);
```

迁移前自动 backup db (现有 MigrationService 已支持)。

### 12.3 现有 264 cargo test 适配

| 测试位置 | 适配方式 | 风险 |
|---|---|---|
| `services/chat/service.rs::tests` | 改 import 路径 + trait dispatch 实例化, 断言不变 | 低 |
| `services/chat/prompt.rs::tests` | 同上, 加 SafetyGuard mock | 低 |
| `services/chat/conversation.rs::tests` | 改 import 路径 | 极低 |
| `services/persona.rs::tests` | trait 实例化 | 低 |
| `services/living_pet.rs::tests` | **可能需要重写**(tick → lazy aging 的测试断言不同) | 中 |
| `services/llm/*::tests` | 不动 | 0 |
| 其他 | 不动 | 0 |

**估计**: 264 test 中 ~30-40 个需要小幅调整, ~5-10 个 living_pet 测试需要重写。整体测试套件保持 ≥ 250 pass。

### 12.4 Phase 排期（6-8 周）

```
Phase A: ADR-025 沙盒规则撰写 + review        ~0.5 周 (单独 issue, 阻塞 Tool sub)
Phase B: kernel/ 5 件套实施                    ~2 周
  ├ B.1 state_store + capability token         3 天
  ├ B.2 event_bus + event_log                  3 天
  ├ B.3 scheduler (4 触发类型)                 2 天
  ├ B.4 safety_guard (ADR-006 真注入)          3 天
  └ B.5 lifecycle_manager + FSM                3 天
Phase C: subsystem/ 6 件套改造                 ~3 周
  ├ C.1 PersonaSub (复用 services/persona)     3 天
  ├ C.2 ConversationSub (升级 services/chat)   1 周
  ├ C.3 LivingSub (lazy aging 重做)            5 天
  ├ C.4 MemorySub (FTS5 + 摘要)                1.5 周
  ├ C.5 InitiativeSub (新建)                   5 天
  └ C.6 ToolSub (3 read-only + sandbox)        1 周
Phase D: soul/ 4 件套实施                      ~1 周
  ├ D.1 PersonaPromptBuilder                   2 天
  ├ D.2 ToneShaper                             2 天
  ├ D.3 InitiativeWeights                      1 天
  └ D.4 RetrievalRanker                        2 天
Phase E: surface 集成 + 端到端测试             ~1 周
  ├ Tauri command 改走 subsystem trait
  ├ multi-surface event broadcast
  └ 264 cargo test 适配 + 新单测

合计: ~7.5 周, 与 MVP 时间窗对齐
```

---

## 13. ADR 增量

### 13.1 新增 ADR

| ADR | 主题 | 内容摘要 | 状态 |
|---|---|---|---|
| **ADR-025** | Agent 工具沙盒规则 | Path whitelist + denylist + Capability + Grant UX + Audit log | 草案 (§11 提供 MVP 默认), 独立 ADR 文档需撰写 |
| **ADR-026** | Companion Agent Runtime 顶层架构 | 本 spec 摘要(3-5 段) | 本 spec 通过后归档为 ADR-026 |
| **ADR-027** | Memory 三层架构 + FTS5 retrieval | working / episodic / semantic + 三因子排序 | §9 提供完整草案, 独立 ADR 撰写 |

### 13.2 Updated ADR

| ADR | 原状态 | Updated 内容 |
|---|---|---|
| **ADR-006** | "安全前缀 v1.0" | Updated 2026-05-24: prefix 注入路径明确为 SafetyGuard kernel trait, subsystem 无法 bypass, 编译期约束 |
| **ADR-015** | "对话面板三形态架构" | Updated 2026-05-24: ConversationStore 升级为 ConversationSubsystem; multi-surface broadcast 走 EventBus |
| **ADR-018** | "LLM 三层抽象 + AgentService 工具调用框架" | Updated 2026-05-24: Layer 2 → ConversationSubsystem; Layer 3 → ToolSubsystem; MVP 仅 read-only 3 件; ADR-025 拍板沙盒细则 |

### 13.3 Superseded ADR

无。本 spec 是增量,不推翻已有 24 项 ADR。

---

## 14. MVP scope summary

### 14.1 ✅ MVP 必做清单

**Kernel** (5 件):
- SafetyGuard (ADR-006 真注入 + 输出二次扫描)
- LifecycleManager (5 状态 FSM)
- EventBus (9 event + event_log)
- Scheduler (4 触发类型统一)
- StateStore (capability token + 事务)

**Subsystems** (6 件 implementation):
- PersonaSub (改造现有 PersonaService)
- ConversationSub (改造现有 ChatService)
- MemorySub (新建, 3 层 + FTS5)
- InitiativeSub (新建, 4 trigger + hard gate)
- ToolSub (新建, 3 read-only tool + sandbox)
- LivingSub (改造现有 LivingPetService, tick → lazy)

**Soul Overlay** (4 件):
- PersonaPromptBuilder
- ToneShaper
- InitiativeWeights
- RetrievalRanker

**SQLite**:
- 2 表 ALTER (messages, pet_runtime_state)
- 4 表新增 (episodic_memory, episodic_memory_fts, event_log, tool_audit_log)

**ADR**:
- 3 ADR 新增 (025, 026, 027)
- 3 ADR Updated (006, 015, 018)

### 14.2 🟡 P1+ 推迟清单

- Writable tool (Edit/Write/Bash) + 撤销机制
- Semantic memory + embeddings + vector store
- Multi-agent / subagent dispatch
- Long-running task (跨会话任务跟进)
- LLM-driven proactive candidate generation
- Memory 跨 conversation entity 链接
- Multi-LLM-provider routing (Anthropic / Gemini)
- 多模态消息持久化 (image/audio/file 入 messages)
- MCP server bridge
- 用户自定义 tool / 自定义 path whitelist
- Event sourcing replay / dynamic subscribe
- Wake 时 LLM-driven "重逢问候"

### 14.3 🔴 不做清单（MVP + P1 均不做）

- 跨进程 EventBus / 分布式 runtime
- Live config hot reload
- 进程级 hot reload
- Background long-running tool
- Network tool 在沙盒外执行 (WebFetch 暂不做, 因隐私边界考量)
- Persona 自动演化 (持续学习改 .soul.md, 隐私 + 用户自主权风险)

### 14.4 6-8 周 timeline

见 §12.4 Phase 排期表。

---

## 15. 风险与未决

### 15.1 已知技术风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| Capability token 在 Rust trait 上的表达可能受 lifetime / generic 限制困扰 | trait 设计需调整 | Phase B.1 (state_store) spike 验证 first;若复杂可降级为 runtime check + lint rule (放弃编译期保证) |
| FTS5 中文分词效果 | Memory retrieval 精度低 | MVP 沿用 SQLite FTS5 默认 unicode61 tokenizer + 关键词重叠;P1 评估 jieba-rs 集成 |
| LLM 摘要任务的 token 成本 | 用户账单 | MVP 仅在 chat session 静默 30min 后触发;限制摘要 prompt token + max_tokens output;后期评估 sliding window + skip if budget exceeded |
| Living tick → lazy aging 改造引入 mood/energy 不连续感 | 用户体验 | lazy formula 设计要保证 "查询时算出来的值" ≈ "tick-based 算出来的值";写充分的 property test |
| event_log 增长 | 数据库膨胀 | 30 天 retention + 每次启动 GC + 索引 created_at |
| Soul 编译期 `use` 黑名单的执行方式 | 实施复杂 | 用 rustc lint plugin 或 wasm-bindgen-style proc macro;实施前评估 ROI, 若复杂改 review-time check |

### 15.2 ADR-025 阻塞

ToolSubsystem 不能在 ADR-025 完整决议前开工。本 spec §11 提供 MVP 默认作为 ADR-025 起点, 但正式 ADR-025 必须独立提交 + 用户/审核者签字。建议 Phase A 优先做。

### 15.3 性能风险

- **memory retrieval p50 < 50ms**: FTS5 查询 + 100 行排序 + Soul re-rank, 单测验证 1000 episodic 时 < 50ms。
- **mood/energy recompute < 5ms**: 纯算数, 不查 DB (依赖 cached pet_runtime_state)。
- **多 surface broadcast < 50ms**: Tauri emit 已知性能 < 10ms per surface, 4 surface 序列化 + emit 总和 < 50ms。

### 15.4 安全风险

- **Soul 越权风险**: 编译期 `use` 黑名单是主要防线;如果黑名单失效, Soul 可读 write trait 但仍受 capability token 阻拦 (runtime panic);双层保护。
- **Tool 沙盒逃逸**: ADR-025 path whitelist + denylist + canonicalize 防 `..` 跳出;但 symlink 跨越白名单边界需运行时检测 (MVP 拒绝跟随 symlink)。
- **API Key 明文**: M1 期 config 表明文是已知技术债, 本 spec 不解决, 推 M3 G CryptoService。**任何对外分发版本必须先修这个**。

---

## 16. Open Questions（审核者可针对这些发问）

> 本 spec 已锁定的内容是核心架构。以下是审核者最可能挑战的 6 个点, 列出来便于讨论:

1. **Capability token 在 Rust 上的可行性**: `WriterCap<T: Owned>` + private `KernelSecret` 能在 trait method 签名上稳定表达吗?lifetime / generic 复杂度评估如何?
   - 作者预设(待审核): 应该可以,参考 `typed-builder` / `state-machine-future` 等 crate 的同类技巧。Phase B.1 spike 验证。

2. **Soul Overlay stateless 是否够用**: 实际产品中 Soul 可能需要"短期 reaction state"(刚说过的话风格延续 / 情绪累积) — 但本设计 Soul 无 state。
   - 作者预设(待审核): 短期 reaction state 借由 LivingSub.read_current() 注入(mood/energy 已是 short-term);跨多轮的"语气延续"通过 conversation history 自然实现。如果发现确实需要 Soul state,违反 First Principle,需要重 spec。

3. **Memory 三层是否冗余**: working_memory 是 in-mem,与 messages 表的最近 N 条等价吗?
   - 作者预设(待审核): 不等价。working_memory 包含 in-mem 的 token_count / safety_scan 中间状态等,fallback to messages 表 query 时 latency 不同;但确实可以考虑 MVP 阶段简化为"messages 表 cache" 推到 P1 再细分。

4. **Initiative quota=4/day 是否合理**: ADR-006 现状是这个数, 但 Companion 哲学下,主动陪伴上限可能要可配置。
   - 作者预设(待审核): settings 已暴露,用户可调;default 4 是 ADR 默认。

5. **9 个 event 是否够**: 真实生产 Agent Runtime 通常有 20-30 个 event。
   - 作者预设(待审核): 9 个是 MVP 启动集;Constitution #8 上限 15,留 6 个空位。P1 接入 wardrobe / voice / 多模态 / agent loop 状态变化时再加。

6. **6 subsystem 是否够**: Karpathy LLM-OS 提议中有 perception (vision/audio) 和 actuator (controls) 等额外组件。
   - 作者预设(待审核): AIPET MVP scope 不含 vision / voice in / controls;装扮/声音/游戏走 TaskService / 独立 surface,不需要独立 subsystem。P1+ 视需求再加(Constitution #8 上限 = 6,违反需新决策)。

---

## 17. Glossary（术语表,审核者必读）

| 术语 | 定义 | 来源 |
|---|---|---|
| **AIPET** | 本项目代号,"AI Desktop Pet" | 内部 |
| **Companion Agent Runtime** | 本 spec 设计的运行时, AIPET 的内核 | 本 spec |
| **Core / Soul 双层** | rational vs expressive 分离 | First Principle |
| **Kernel** | 5 件套 hard-isolated 基础设施 | §4.2 |
| **Subsystem** | 6 件套可独立演进的功能模块 | §4.3 |
| **Soul Overlay** | 4 件套 stateless 包装层 | §4.4 |
| **Surface** | UI 表面(pet/chat/workspace/tray) | §4.5 |
| **Hot path** | user input → reply 全程的同步执行链 | §5.2 |
| **Capability token** | `WriterCap<T>` 编译期 trait, 实现 Single Writer | §8.1 |
| **Lazy aging** | mood/energy 等不 tick, 按时间衰减重算 | §6.6 |
| **Living tick** | (本 spec 中**不存在** —— Living 走 lazy) | (反面参照) |
| **Working / Episodic / Semantic memory** | 三层 memory(in-mem / SQLite+FTS5 / KV) | §9 |
| **`.soul.md`** | 用户编辑的角色定义文件 | docs/persona/persona-design.md |
| **OpenClaw** | 开源 AI agent 项目, 验证 `SOUL.md` 模式可行 | github.com/openclaw |
| **VRM** | 3D 模型标准(VRoid + UniVRM),AIPET 桌宠形象格式 | ADR-002 |
| **Tauri** | Rust + WebView2 桌面框架,体积小 | docs/architecture |
| **ADR** | Architecture Decision Record, 三句话决策记录 | docs/decisions.md |
| **FTS5** | SQLite 全文检索扩展 | sqlite.org |
| **DPAPI** | Windows Data Protection API, secrets 加密 | architecture §8 |
| **LLM-OS** | Karpathy 提议的"LLM 即操作系统"架构观 | Karpathy 2023 |

---

> **End of spec**. 6-8 周后形成第一可工作版本。各子系统内部细节 spec 后续单独提交。
