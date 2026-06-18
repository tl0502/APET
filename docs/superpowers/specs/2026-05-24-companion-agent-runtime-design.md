---
title: Companion Agent Runtime v3 — Phase A0/A1/A2 + Persona Rebinding Rules (post review-2)
updated: 2026-05-24
related:
  - ../../decisions.md
  - ../../architecture/system-architecture.md
  - ../../persona/persona-design.md
  - ../../requirements/prd.md
  - ../../roadmap/development-roadmap.md
---

# Companion Agent Runtime v3 — Revised MVP Architecture Spec

> **Supersession note (2026-06-18)**: Agent Runtime hot path, prompt material contract, SafetyPolicy defaults, and Persona source-format boundaries are superseded by [2026-06-18-agent-runtime-contract-design.md](2026-06-18-agent-runtime-contract-design.md). This v3 spec remains useful for historical context and broader subsystem mapping, but active implementation must follow the 2026-06-18 runtime contract when conflicts exist.
>
> **第三方审核 v2 verdict (2026-05-24)**: v2 架构方向通过, 10 项收紧后出 v3 进入 **Phase A0 开发就绪**。
> **Audience**: 项目核心开发 + 第三方架构审核。本文档**自包含**。
> **Status**: Brainstorm v3, 等 third review (若需);通过后进入 implementation plan 阶段。
> **Spec scope**: 顶层 Runtime 架构 + 7 件 kernel + 6 件 subsystem 契约 + **四阶段** MVP (Phase A0/A1/A2/B/C)。

## v2 → v3 主要变更（review-2 落地清单）

| # | 项 | v2 | v3 |
|---|---|---|---|
| 1 | Soul 术语统一 | `.soul.md` / `.soul/` / `.soulpack` 残留混用 | **Superseded 2026-06-18**: runtime only depends on `SoulRuntimeProfile`; `.soul/` is an undecided source format, not an active runtime requirement |
| 2 | Phase A 拆分 | 单 Phase A ~3w | **Phase A0 Safety & Secrets (~1w) / A1 Persona Snapshot & Soul Package (~1.5w) / A2 Conversation Memory & History Stability (~0.5-1w)** |
| 3 | Phase A DoD | (无) | **每个 sub-phase 独立 DoD** (§14.x) |
| 4 | 旧 conversation migration | 默认 `COALESCE 'momo'` silent backfill | **cascade**: existing persona_id → metadata/messages → `LegacyUnknownSnapshot`; SQLite 两步 ALTER |
| 5 | SafetyGuard Scan Scope | §6.6 提及无矩阵 | **Scan Scope Matrix 7 项** (user input / stream token / final text / memory KV / rolling_summary / tool result / context snapshot); 分 **SafetyPrefix vs SafetyScanRules** |
| 6 | Memory MVP rolling_summary | "可选" | **默认关闭 / deterministic placeholder**; 自动 LLM rolling summary 推 P1 (要求 provider check / token budget / SafetyGuard scan / fallback) |
| 7 | PermissionService Phase A | "stub" | **DenyOnly stub**: 绝不调 GetForegroundWindow/getUserMedia/MediaRecorder/BitBlt; request_context 默认 denied + 审计 |
| 8 | GrantBroker Phase A | "stub" | **trait + DenyAllGrantBroker / MockGrantBroker only**; 无 UI modal / 无 persistent cache / 不接 ToolSub |
| 9 | Repository transaction | 未详 | **raw `sqlx::Pool` 仅 kernel/db 可见 + RuntimeUnitOfWork 跨 owner tx + migration 唯一 raw SQL 例外** |
| 10 | `.soulpack` 安全导入 | 未详 | **10 项硬规则** (max size / 禁绝对路径 / 禁 ../ / 禁 symlink / 禁可执行 / 扩展 allowlist / manifest 在根 / temp dir 解 / validate 通过移动 / 失败清理) |
| 11 | Persona Rebinding Rules | 仅"active 切换不污染" | **完整 11 条 + 4 UI 状态 (UpToDate / SamePersonaOutdated / DifferentFromActivePersona / LegacyUnknownPersona) + 判断表 + Snapshot 冻结 vs 不冻结 carve-out + fork 默认 = 复制 history + 切到新 conv** |

---

## 0. 背景与项目信息（third-party review 必读）

### 0.1 项目简介

**AIPET**（AI Desktop Pet, 内部代号）是一个 Windows 桌面 AI 桌宠应用。10 周 MVP, 单人 vibecoding 项目。差异化定位三引擎:

1. **用户自主人格**: 运行时只依赖 `SoulRuntimeProfile` / `PersonaSnapshot`；`.soul/`、`.soul.md`、GUI schema、imported package 都只是候选 source format。`.soul/` 不再是已拍板的源格式，后续需独立 source-format spec 决定。
2. **主动陪伴**: 桌宠**默认**基于本地空闲信号(GetLastInputInfo)主动起话题。**默认不读**窗口标题/应用名/输入内容/麦克风/屏幕内容。这些能力作为**可选 Context Awareness 增强**仅在用户**显式授权**后开放(P1+, 见 ADR-029)。
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
2. **用户自主权**: `.soul/` 包 (含 `.soulpack` 导入导出 + `.soul.md` legacy 输入) / 装扮 / 设置完全归用户;用户能读、能改、能导出、能迁移。
3. **非养成原则**: 不引入流失/死亡/必须签到机制。
4. **隐私边界 (默认保守 + 可选授权)**: **默认不读**应用名/窗口标题/输入内容/麦克风/屏幕内容(CI 静态扫描黑名单含 `getUserMedia` / `MediaRecorder` / `GetForegroundWindow` / `GetWindowText`)。Context Awareness 作为 P1+ 增强能力, 仅在用户**显式授权**后通过 **PermissionService** 统一开闸, 可撤销, 可审计 (`context_access_log` 表)。Soul / Persona **不能**启用、扩大或绕过权限。
5. **安全护栏不可绕过**: 任何人格 / 游戏场景 / `.soul/` 自定义内容不能覆盖系统安全前缀;SoulValidator 拒绝 manifest 出现 permissions/tools/safety_prefix 字段 (Constitution #11)。

### 0.6 当前 AI 路径关键缺口（本 spec 解决的根本问题）

> 本节诚实列出当前(M2 W3)AI 路径的真实状态,为审核者提供判断基线。**第三方 review v2** 补充了 4 项 P0 隐藏缺口(标 🆕)。

| 缺口 | 当前状态 | 影响 |
|---|---|---|
| **🔴 安全前缀 = None** | `src-tauri/src/services/chat/prompt.rs:34 const SAFETY_PREFIX: Option<&str> = None;` | LLM 调用当前**完全没有 ADR-006 安全前缀**;任何对外分发版本前必须修复 |
| **🔴 SecurityGuard 模块不存在** | `grep -r SecurityGuard` 0 命中 | LLM 输出无二次扫描;命中违禁词不会替换 |
| **🔴 🆕 SafetyGuard 流式 vs 终态扫描状态机未设计** | v1 spec 仅写"wrap + scan",未定义流式 token 已发 → final scan 命中违禁的回滚 / UI 替换 / DB 状态 / event 生命周期 | 流式输出后违禁触发,UI 与 DB 状态会不一致;reviewer P0 阻塞 |
| **🔴 🆕 Conversation 没有 persona_snapshot 绑定** | `conversations` 表无 `persona_snapshot_id`;hot path 读 active persona | 用户切换人格→旧对话 system prompt 也跟换;人格编辑→历史会话风格漂移;reviewer P0 阻塞 |
| **🟠 🆕 `.soul.md` 单文件成多消费方解析瓶颈** | 现有 3 内置人格皆 `.soul.md`;PromptBuilder / ToneShaper / InitiativeWeights / MemoryPolicy 都要从自由文本解析 | 维护性差,无法做 schema 演进 / persona snapshot / 高级编辑器;reviewer 建议改 `.soul/` 多文件包 |
| **🟠 🆕 ContextProvider / PermissionService 模块不存在** | 当前隐私承诺"永不读" hardcode 在文档,无统一权限网关 | P1+ 接入 Context Awareness 时, Soul / Tool / Initiative 各自申请上下文会导致权限扩散;必须统一收口 |
| **🟠 Memory 接入 prompt 缺失** | `chat/prompt.rs:208 build_system_message` 只注入 nickname bullet | 桌宠"记得用户偏好"的设计承诺当前失效;`memory` 表中的 KV 不进 system message |
| **🟠 history 硬切 N=10** | `chat/service.rs:59 const HISTORY_LIMIT: u32 = 10;` | 长对话丢上下文;无 token-aware window / 摘要压缩机制 |
| **🟠 AgentService 不存在** | `grep -r AgentService` 0 命中 | LLM 即使 emit tool_call 也被 ConversationSubsystem 主动吞掉 (`service.rs:338 注释 "M1 不接 tools,不会触发;忽略"`) |
| **🟠 ToolRegistry / GrantBroker 不存在** | 0 文件 | 6 起步 tool 全无实现; 即使 v2 仅 MVP 3 件 read-only, 也需 GrantBroker 同步授权握手 |
| **🔴 ADR-025 沙盒未拍板** | ADR-018 明示"具体路径白名单/命令沙盒细则待 **ADR-019** 决议",但 ADR-019 是 Onboarding 续接,不是沙盒;沙盒 ADR 编号未分配、内容未写 | Layer 3 AgentService / ToolSub 完全 block |
| **🟠 API Key 明文** | M1 期 `config` 表 KV;`secrets` 表 + DPAPI 未上 | M3 G `CryptoService` 落地前不能分发 |
| **🟠 ProactiveCare 未实现** | 规划在 M3 W5-6;`IdleDetector / ProactiveCareService` 文件不存在 | 当前桌宠纯 reactive |
| **🟢 流式 + cancel + 4 分支收尾** | ChatService 已扎实落地 | 已是 production-ready 水平,**本 spec 保留并升级为 ConversationSubsystem** |
| **🟢 typed 多模态 + tool 接口** | LLM types.rs 已 typed 完整 | ContentPart 5 variant + ToolCall/Definition/Choice/StreamDelta 全 typed,**本 spec 不重新设计** |

### 0.7 本 spec 解决什么 / 不解决什么

**解决** (顶层契约 + 7 件 kernel + 6 件 subsystem 边界):
- ✅ Runtime 总体架构 (Kernel 7 + Subsystems 6 + Soul Overlay 4)
- ✅ State ownership (谁拥有什么表 / 谁能写 / 谁能读) + MVP Repository pattern
- ✅ Lifecycle 状态机 (Boot/Live/Suspend/Wake/Shutdown) + SafetyGuard 8-state FSM（含 disabled）
- ✅ Event 模型 (10 event + sync/async 边界 + 失败分级)
- ✅ 17 trait 完整签名 (Kernel 7 + Subsystem 6 + Soul 4)
- ✅ Memory **Phase A MVP**: KV 注入 + token-aware history window + 可选 rolling_summary
- ✅ Initiative **Phase B MVP**: 4 trigger + hard gate + idle-only 默认
- ✅ Tool **Phase C P1**: 3 read-only + GrantBroker sync + whitelist + audit
- ✅ `SoulRuntimeProfile` / `PersonaSnapshot` runtime contract（source format 由 2026-06-18 contract 降级为未决）
- ✅ Persona Snapshot binding for Conversation
- ✅ Context Awareness 权限模型 (默认关闭, 可选授权增强)
- ✅ AIPET 现有 service → Runtime subsystem 三阶段迁移路径
- ✅ ADR 增量清单 (新增 5 + Updated 4)

**不解决** (后续独立 spec / ADR):
- ❌ 各子系统**内部**详细实现 (例如 PromptBuilder 具体怎么拼、ToneShaper 具体怎么改写) → 各子系统独立 spec 跟进
- ❌ ADR-025 沙盒规则的**完整决议** (本 spec §11 给 MVP 默认作为审核参考;正式 ADR-025 文档独立提交)
- ❌ ADR-028 Soul Package / ADR-029 Context Awareness 的完整 ADR 撰写 (本 spec §13 提供 MVP 默认, 独立 ADR 文档独立提交)
- ❌ Writable tool (Edit/Write/Bash) / 长期任务 / multi-agent / semantic memory + embeddings → P1+
- ❌ Episodic memory + FTS5 + LLM 摘要压缩 + RetrievalRanker → **Phase C P1**
- ❌ Context Awareness 实际实施 (app/window/selected text/mic) → P1+ 用户授权后
- ❌ UI 设计 / 装扮系统 / 声音系统 (与 AI runtime 正交;沿用现有 PRD/architecture)
- ❌ MCP server bridge (P1+,本 spec 不留专用 slot,通过 ToolRegistry 通用机制接入)

---

## 1. Scope & Impact（third-party review 必读）

> 列出本 spec 实施时**会动到的具体代码 / 表 / ADR / milestone**。审核者据此判断 blast radius。
> **v2 重要变更**: 本节按 **Phase A (P0 必做) / Phase B (MVP nice-to-have) / Phase C (P1 deferred)** 三阶段分级标注;审核者可分别评估每阶段 ROI。

### 1.1 影响的代码文件

**Phase A (P0 必做, ~3 周, 单人窗口可承受) — 需要新建**:

```
src-tauri/src/kernel/                     新建 kernel 层 (P0 子集)
├── mod.rs                                ← trait 聚合 export
├── safety_guard.rs                       🟢 P0 SafetyGuard path + SafetyPolicy + 8-state FSM
├── permission_service.rs                 🟢 P0 stub + audit 表 (Context Awareness 默认全 deny)
├── grant_broker.rs                       🟢 P0 sync request/response stub (Phase C 真接 ToolSub)
├── state_store.rs                        🟢 P0 Repository pattern 包 sqlx pool
└── lifecycle_manager.rs                  🟢 P0 FSM (Section 6) — 5 顶层 state + Live 4 sub-state

src-tauri/src/subsystems/persona/         🟢 P0
├── service.rs                            ← 改造现有 services/persona.rs
├── soul_compiler.rs                      🟢 P0 .soul/ 包加载 + validate + compile
├── snapshot.rs                           🟢 P0 PersonaSnapshot 写入 persona_snapshot_profiles
└── prompt_builder.rs                     🟢 P0 build_runtime_profile → system message

src-tauri/src/subsystems/conversation/    🟢 P0
├── service.rs                            ← 改造 services/chat/service.rs
├── store.rs                              ← 保留 services/chat/conversation.rs
└── repo.rs                               🟢 P0 ConversationRepo (Repository pattern)

src-tauri/src/subsystems/memory/          🟢 P0 (MVP 子集)
├── service.rs                            🟢 P0 set_fact / get_facts / retrieve_kv
├── prompt_inject.rs                      🟢 P0 KV → memory_bullets
└── history_window.rs                     🟢 P0 token-aware recent N + rolling_summary
```

**Phase B (MVP nice-to-have, ~2 周, 看时间窗) — 需要新建**:

```
src-tauri/src/kernel/
├── event_bus.rs                          🟡 Phase B 真正接入 (Phase A 仅 stub)
└── scheduler.rs                          🟡 Phase B cron / idle / one-shot / periodic

src-tauri/src/subsystems/initiative/      🟡 Phase B
└── service.rs                            🟡 4 trigger + hard gate + idle-only 默认

src-tauri/src/subsystems/living/          🟡 Phase B
└── service.rs                            🟡 tick → lazy aging 改造

src-tauri/src/soul/                       🟡 Phase B (Phase A 仅 PromptBuilder)
├── tone_shaper.rs                        🟡
├── initiative_weights.rs                 🟡
└── retrieval_ranker.rs                   🔴 Phase C (依赖 episodic memory)
```

**Phase C (P1 deferred, scope 看 M4-M5 节奏) — 推迟**:

```
src-tauri/src/subsystems/tool/            🔴 P1 (依赖 ADR-025 完整决议)
├── service.rs                            🔴 ToolSub.execute hot path
├── glob.rs / grep.rs / read.rs           🔴 3 read-only tool
├── whitelist.rs                          🔴 path canonicalize + denylist
└── audit.rs                              🔴 tool_audit_log writer

src-tauri/src/subsystems/memory/episodic.rs    🔴 P1 episodic_memory + FTS5
src-tauri/src/subsystems/memory/ranker.rs      🔴 P1 三因子排序 + LLM 摘要压缩
```

**需要改造** (现有文件,改造非重写):

| 现有文件 | Phase | 改造内容 | 改造规模 |
|---|---|---|---|
| `services/chat/service.rs` | A | 提取 ConversationSubsystem trait;hot path 集成 SafetyGuard wrap_messages + scan_token/scan_final FSM;tool_call 处理在 Phase A **保持忽略**(`service.rs:338` 注释不变), Phase C 才转 ToolSub | ~25% Phase A |
| `services/chat/prompt.rs` | A | `SAFETY_PREFIX = None` → 从 SafetyGuard 注入;build_system_message 接入 MemorySub.retrieve_kv (Phase A 仅 KV);history token window | ~30% Phase A |
| `services/chat/conversation.rs` | A | 加 `persona_id` + `persona_snapshot_id` + `rolling_summary` 三字段读写 | ~15% |
| `services/persona.rs` | A | 实现 PersonaSubsystem trait;`.soul/` 加载 → SoulCompiler;active 切换不影响已有 conv (Persona Snapshot 绑定) | ~30% |
| `services/living_pet.rs` | B | tick-based 改 lazy aging;实现 LivingSubsystem trait;mood_changed 改 EventBus publish | ~40% |
| `services/memory.rs` | A | 含义改:从"nickname/preference"扩为统一 KV + rolling_summary | ~20% Phase A |
| `services/scheduler.rs` | B | 现有是空骨架,Phase B 完整实现 | ~80% 新写 |
| `services/llm/openai.rs` | — | 不改 (L1 已成熟) | 0% |
| `services/llm/types.rs` | — | 不改 (已 typed 完整) | 0% |
| `lib.rs setup` | A | 启动序列重组 (Section 6.B 8 步) Phase A 5 步基础, Phase B 补 Scheduler+EventBus | ~40% A + 续 |

**Phase A 不动 (持续保留)**:
- `services/llm/openai.rs` / `error.rs` / `probe.rs` / `services/llm_providers.rs`
- `services/db.rs` / `migration.rs` / `config.rs`
- `services/consent.rs` / `consent_gate.rs` / `onboarding.rs`
- `services/reminder.rs` / `pomodoro.rs` / `todo.rs` (TaskService 独立)
- `services/snap.rs` (磁吸窗口系统)
- `services/window_state.rs` / `window_actions.rs` / `shortcuts.rs` / `tray.rs`
- `services/avatars.rs` / `preferences.rs`

### 1.2 影响的 SQLite 表

> 现有 27 表 (`docs/architecture/system-architecture.md` §4 详列);本 spec 按 **Phase A / B / C** 分级。

**Phase A (P0 必做) — 修改 schema**:

| 表 | 改动 | 迁移方式 |
|---|---|---|
| `conversations` | 加 `persona_id TEXT NOT NULL` + `persona_snapshot_id TEXT NOT NULL` (新会话强绑定) + `rolling_summary TEXT DEFAULT NULL` | `ALTER TABLE` + backfill 旧会话用 active persona snapshot |
| `messages` | 加 `token_count INTEGER DEFAULT NULL` + `safety_scan_status TEXT DEFAULT 'pending'` (8-state 枚举,含 `disabled`) | `ALTER TABLE` + default 兼容 |
| `persona_snapshots` | 加 `source_hash TEXT` + `compiled_profile_json TEXT NOT NULL` + `schema_version TEXT NOT NULL` (现有 schema 不足时拆 `persona_snapshot_profiles` 子表) | `ALTER TABLE` 或新建子表 |

**Phase A (P0 必做) — 新增 schema**:

| 表 | 用途 | Owner |
|---|---|---|
| `persona_snapshot_profiles` (可选, 若 persona_snapshots 不扩展) | Soul 编译后的运行时 profile JSON | PersonaSub |
| `context_access_log` | Context Awareness 权限授权 + 使用审计 | Kernel / PermissionService |
| `error_logs` (若不存在) | EventBus / SafetyGuard / tool 失败降级写入处 | Kernel |

**Phase B (MVP nice-to-have) — 修改 / 新增**:

| 表 | 改动 / 用途 |
|---|---|
| `pet_runtime_state` | 加 `last_mood_event_at TEXT` + `last_energy_event_at TEXT` 支持 lazy aging |
| `proactive_care_log` | 加 `trigger_kind TEXT` + `gate_result TEXT` + `context_scopes_used TEXT DEFAULT '[]'` + `score_breakdown TEXT DEFAULT NULL` |
| 🆕 `event_log` | EventBus 持久化关键事件 (Kernel owner, 30d retention) |

**Phase C (P1 deferred) — 新增 schema**:

| 表 | 用途 | Owner |
|---|---|---|
| 🆕 `episodic_memory` | 压缩后的 episode (LLM 摘要) | MemorySub |
| 🆕 `episodic_memory_fts` | FTS5 virtual table + 3 trigger;**注: `content_rowid='rowid'` (INTEGER) 而非 v1 的 'id' (TEXT)** | MemorySub (内部) |
| 🆕 `tool_audit_log` | tool 执行审计 | ToolSub |

**不动**:
- Phase A 不动 22 张表(personas / nicknames / memory / config / secrets / consent / schema_version / reminders / reminder_history / todos / pomodoro_sessions / milestones / user_anniversaries / accessories_inventory / wardrobe_decisions / voice_packs / voice_settings / game_sessions / game_session_events / diary_drafts / telemetry_queue 等)
- Phase B/C 视实施时再评估

### 1.3 影响的 ADR

**新增 ADR** (5 条):

| ADR | 主题 | Phase | 由本 spec 落地 |
|---|---|---|---|
| **ADR-025** | Agent 工具沙盒规则 (path whitelist + denylist + capability + grant UX) | C | Section 11 提供 MVP 默认作为参考,完整 ADR 独立提交 (Phase C 阻塞前置) |
| **ADR-026** | Companion Agent Runtime 顶层架构 + MVP Phasing (本 spec 摘要) | A | 本 spec 通过后归档为 ADR-026 |
| **ADR-027** | Memory MVP vs P1 (Phase A KV+window+rolling 与 Phase C episodic+FTS5 的分界) | A/C | Section 9 提供完整草案, 独立 ADR 撰写 |
| **🆕 ADR-028** | Persona source format（候选 `.soul/` / `.soul.md` / GUI schema / imported package）+ SoulRuntimeProfile + PersonaSnapshot | A | **Superseded 2026-06-18**: source format 未决；运行时只拍 `SoulRuntimeProfile` / `PersonaSnapshot` |
| **🆕 ADR-029** | Context Awareness Permission (默认全 deny + 显式授权 + ContextProvider + 审计) | A (框架) + P1 (实施) | Section 11 + Section 2.4 提供权限模型, 独立 ADR 撰写 |

**Updated ADR** (4 条):

| ADR | 现状 | 本 spec 影响 |
|---|---|---|
| **ADR-003** | "用户角色定义 `.soul.md` schema v2 + 3 内置人格 + 安全前缀拼装" | Updated: Conversation 必须绑定 PersonaSnapshot；source format 由 2026-06-18 runtime contract 降级为未决，运行时只依赖 `SoulRuntimeProfile` |
| **ADR-006** | "安全前缀 v1.0,通用核心 + 地区补充" | Updated: prefix / scan 路径由 SafetyGuard + SafetyPolicy 控制；4 scope 出厂全 OFF；FSM 为 8-state（含 `disabled`） |
| **ADR-015** | "对话面板三形态架构" | Updated: ConversationStore 升级为 ConversationSubsystem;三 surface(pet/chat/workspace)共享 data layer 通过 EventBus 多 surface broadcast;**新增 `persona_snapshot_id` 强绑定** |
| **ADR-018** | "LLM 三层抽象 + AgentService 工具调用框架" | Updated: Layer 2 ChatService → ConversationSubsystem;Layer 3 AgentService → ToolSubsystem (本 spec **Phase C P1** 才接入);Phase A 保留现有 `service.rs:338` 主动忽略 tool_call 行为不变;沙盒细则推到 ADR-025 |

**Superseded ADR**: 无 (本 spec 是增量,不推翻已有决策)

### 1.4 影响的 milestone

| Milestone | 原计划 | 本 spec 影响 |
|---|---|---|
| **M2 W4** (当前) | 物理交互 + 心情/精力 + 摸鱼 (#23) | LivingPetService tick → lazy 改造与 Phase B 对齐 (#23 实施时同步) |
| **M3 W5-6** | LLM Provider + SecurityGuard + MigrationService + UpdaterService + IdleDetector + ProactiveCareService + FileDropHandler + MilestoneService + LivingPetService 日常时段表 | **Phase A = M3 主线**: SafetyGuard 真注入 + DPAPI + Persona Snapshot + Memory KV + token window (~3 周);**Phase B 跟进**: IdleDetector / ProactiveCareService / LivingPet lazy 改造 (~2 周) |
| **M4 W7-8** | WardrobeService + VoiceEffectPlayer + 用户纪念日 + 装扮工坊 | 无直接影响 |
| **M5 W9-10** | GameEngine + 5 游戏 + 自测 + 可发布版 | LLMGameRunner 走 ConversationSubsystem 接口;**Phase C ToolSub 与 episodic memory 推到 M5 后或 P1** (10 周窗口承不住) |

**v2 调整**: v1 原建议"M3 W5 前加一周 Runtime Foundation"。v2 改为: **Phase A 嵌入 M3 W5-W7 (~3 周)**, **Phase B 嵌入 M3 W7-W8 (~2 周)**, **Phase C 推到 M5 末或 P1**。Phase A 是任何对外分发版本的 P0 阻塞(SafetyGuard / DPAPI)。

### 1.5 不影响的承诺

本 spec **不动**:
- ✅ PRD §1-§4 业务范围 / 用户故事 / 验收口径
- ✅ ADR-001 到 ADR-024 中除 ADR-003/006/015/018 之外的 20 项 Accepted ADR
- ✅ 装扮系统(模块 O) / 声音系统(模块 P) / 游戏系统(模块 Q) / 物理交互(模块 N)的产品设计
- ✅ VRM 渲染管线 + 配饰挂载点(ADR-002)
- ✅ Onboarding 续接(ADR-019)
- ✅ 磁吸窗口系统(ADR-020)
- ✅ Workspace 单窗壳(ADR-021)
- ✅ 现有 264 cargo test + 293 vitest test 的语义(测试可能需要小幅适配新 trait 接口,但断言不变)
- ✅ 性能预算: 250MB 内存 / 5% CPU / 5s 冷启 / 1.5s 首 token
- ✅ 安装包目标 ≤ 80MB
- ✅ Tauri 2.x + Vue 3 + Rust 技术栈

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
- **Kernel (7 件套, 极小, 不可被绕过)**: SafetyGuard / LifecycleManager / EventBus / Scheduler / StateStore / **PermissionService** / **GrantBroker**
- **Subsystems (6 件套, 可独立演进)**: PersonaSub / MemorySub / ConversationSub / InitiativeSub / ToolSub / LivingSub
- **Soul Overlay (4 件套, stateless function)**: PersonaPromptBuilder / ToneShaper / InitiativeWeights / RetrievalRanker

**为什么不选 actor / hexagonal** (审核者必读):
- **Actor (Akka/Orleans 风格)**: Rust 无原生 framework, `actix` 维护放缓;AIPET 现有 264 test 全在 service 层,actor 模型要全部重写,ROI 不匹配。
- **Hexagonal (Cockburn ports & adapters)**: 与 ADR-018 三层抽象同构,但**不是天然 event-driven**(要额外加 EventBus),且**不强制隔离靠纪律**;Companion runtime 安全要求需编译期 / 类型期 hard isolation。
- **Kernel + Subsystems**: 与 Core/Soul 双层完美映射(kernel = core 最硬部分, subsystem = core 中层, soul = overlay);AIPET 现有 service 1:1 映射到 subsystem,改造成本最低;safety/lifecycle/permission 在 kernel = 类型期约束不可被越权。

### 2.4 Privacy by Default + Opt-in Context Awareness

> v2 新增 First Principle, 由 reviewer P0 推动。**默认隐私保守, 可选授权增强**。

**默认状态** (开箱即用 / 任何用户首次启动):
- Initiative / Soul / Tool **均不**读取应用名 / 窗口标题 / 选中文本 / 输入内容 / 屏幕内容 / 麦克风
- 主动陪伴信号源**仅限**: idle_ms / quiet_hours / daily_quota / cooldown / last_interaction_at / wake.completed / task.reminder_fired / living.mood / living.energy / user_initiative_settings
- LivingPet 心情/精力计算**不依赖**外部应用上下文

**可选授权增强** (P1+, 默认全 deny):
- Context Awareness 能力分 scope: `foreground_app_name` / `window_title` / `selected_text` / `microphone_audio` / `screen_text` 等独立维度
- 每个 scope 必须用户**显式勾选授权**, 默认 false, **可随时撤销** (无副作用)
- 统一通过 **PermissionService** trait 收口 (Kernel 7 件之一);Tool / Initiative / Soul 任何一方调用都走 PermissionService, 不能旁路
- `context_access_log` 表记录: scope / granted_at / used_for / surface_id / retention_policy / actor (即调用方 subsystem)
- Soul **不能**启用 scope, **不能**通过 prompt 间接请求 LLM 调用工具读取这些 scope (与 Tool 权限域隔离)

**为什么**: 桌宠商品定位与"屏幕监听 Agent"差异本质在隐私态度。审核者指出 v1 "永不读取" 一句话会与 P1+ 路线自相矛盾;但若直接改成"会读取"又损害定位。正解是 **default off + 显式 opt-in + 三权限域分离**。

### 2.5 Session Persona Stability + Rebinding Rules

> v2 新增 First Principle, reviewer P0;v3 review-2 补完整 **Persona Rebinding Rules 11 条** + UI 状态 + 判断表 + Snapshot 冻结边界。

**核心规则**:
- `conversations` 表必须有 `persona_id` + `persona_snapshot_id` 双字段; **创建会话时绑定**, 之后**不会被 runtime 静默修改**
- 用户 active persona 切换 (设置面板 / 试聊沙盒 / `persona.activate`):
  - **仅影响**: 新会话默认人格 / 桌宠 idle / proactive 表现 / 未绑定会话的轻量气泡
  - **不影响**: 已存在的 conversation system prompt / 已发送消息的语气包装

**Hot path 强制路径**:

```
v1 (错):  ConversationSub.handle_user_message → PersonaSub.read_active() → build_prompt
v2/v3:    ConversationSub.handle_user_message(conv_id)
          → ConversationStore.read(conv_id) → persona_snapshot_id
          → PersonaSub.read_snapshot(persona_snapshot_id)
          → PromptBuilder.build(runtime_profile, ...)
```

#### 2.5.1 Conversation Persona Rebinding Rules (v3 新增, 11 条)

| # | 规则 | 实施 |
|---|---|---|
| 1 | `active_persona_id` 切换不会自动修改任何 existing conversation 的 `persona_snapshot_id` | ConversationSub 严禁监听 `persona.activated` 事件去 update conv;Constitution #10 加强 |
| 2 | 修改 `.soul/` 文件 (或导入新 `.soulpack`) 后, PersonaSub 必须**生成新 PersonaSnapshot** | source_hash 改变 → 新 row in `persona_snapshot_profiles`;旧 snapshot 保留供历史会话使用 |
| 3 | 新 conversation 默认使用该 persona 的 **latest snapshot** | `ConversationSub.create_conversation(persona_id)` 内部 `PersonaSub.read_latest_snapshot(persona_id)` |
| 4 | 旧 conversation 继续使用**原 `persona_snapshot_id`**, 与 latest snapshot 解耦 | hot path 强制 read by snapshot_id (Constitution #10) |
| 5 | 若旧 conversation 的 `persona_id` **与 latest snapshot persona_id 相同**, 用户可在 UI 选择"切换到最新版" → **原地 rebind**, 更新从下一条消息开始生效 | UI 状态 `SamePersonaOutdated`;触发 `ConversationSub.rebind_persona_snapshot(conv_id, latest_snapshot_id)` |
| 6 | 若 `active_persona_id` **与 conversation.persona_id 不同**, 用户选择"用当前人格继续"时, **默认 fork conversation** (复制全部 history + 用 active persona snapshot + 切到新 conv;原 conv 保留, 仍可继续用原 persona) | UI 状态 `DifferentFromActivePersona`;`ConversationSub.fork_conversation(source_conv_id, target_persona_snapshot_id)` |
| 7 | 所有 rebind / fork 都**必须可审计**, 记录 `from_snapshot` / `to_snapshot` / `reason` / `created_at` / `actor` | 新表 `persona_rebind_audit` (Phase A1 新增) |
| 8 | **Silent rebinding forbidden**: Runtime 不得因为 active persona 切换或 `.soul/` 文件变化而静默重绑 conversation.persona_snapshot_id | Constitution #10 加 sub-rule;CI 静态扫描禁止 `conversations.persona_snapshot_id` 在 ConversationSub 之外被 UPDATE |
| 9 | UI 状态有 4 种 (见 §2.5.2 表) | `ConversationSub.get_persona_status(conv_id)` 返枚举 |
| 10 | 判断表 (见 §2.5.3) 决定每个 UI 状态下的允许动作 | 实现在 UI 层 + ConversationSub trait method |
| 11 | PersonaSnapshot **只冻结人格定义**, 不冻结 SafetyGuard / Permission / Tool whitelist / nickname / user preference / mood/energy 等 Core/User/Runtime state (见 §2.5.4 carve-out 表) | SoulRuntimeProfile schema 严格限定字段 |

#### 2.5.2 UI 状态枚举 (4 种)

```rust
pub enum ConvPersonaStatus {
    /// conversation.persona_snapshot_id == latest_snapshot_of(conversation.persona_id)
    /// AND conversation.persona_id == active_persona_id
    UpToDate,

    /// conversation.persona_snapshot_id != latest_snapshot_of(conversation.persona_id)
    /// 即同人格但 .soul/ 已被编辑过, 生成了新 snapshot
    /// (active persona 是否 = conv persona 不影响此状态)
    SamePersonaOutdated { latest_snapshot_id: SnapshotId },

    /// active_persona_id != conversation.persona_id
    /// 用户切了 active persona, 但本会话绑定的是另一个人格
    DifferentFromActivePersona { active_persona_id: PersonaId, active_snapshot_id: SnapshotId },

    /// conversation.persona_snapshot_id 指向 LegacyUnknownSnapshot
    /// (migration 时无法推断, 用兜底 snapshot 包住; 详见 §12.2 migration cascade)
    LegacyUnknownPersona,
}
```

#### 2.5.3 判断表 (UI 状态 → 允许动作)

| UI 状态 | 允许动作 | 默认 |
|---|---|---|
| `UpToDate` | (无 rebind 需要) | 直接对话 |
| `SamePersonaOutdated` | ① 保留原 snapshot 继续对话 / ② 显式"切换到最新版"原地 rebind | 保留 (用户主动点才 rebind) |
| `DifferentFromActivePersona` | ① 保留原 conv 用原 persona / ② 显式"用当前人格继续" → **fork** 到新 conv (复制 history) | 保留 (用户主动点才 fork) |
| `LegacyUnknownPersona` | 保持 LegacyUnknownSnapshot (兜底 prompt) / 用户可显式绑定到某 active persona (fork OR 原地 rebind 由用户选) | 保持迁移人格 (兜底), 允许显式绑定或 fork |

#### 2.5.4 PersonaSnapshot 冻结边界 (carve-out)

| **在 Snapshot 内 (会被冻结)** | **不在 Snapshot 内 (Conversation 始终读最新)** |
|---|---|
| `identity_prompt` | safety prefix (kernel-level, SafetyGuard.load 时全局加载) |
| `style_prompt` | safety scan rules (kernel-level) |
| `initiative_config` (proactivity / quiet_hours 默认) | tool whitelist / denylist (kernel-level, ADR-025) |
| `memory_policy` (KV scope) | PermissionService grants (用户全局, 跨人格共享) |
| `examples` (few-shot 对话样本) | nickname (用户属性, persona-scoped KV 但不冻结) |
| `ui_metadata` (头像 / 主题色) | user_preference (用户全局) |
| `source_hash` (`.soul/` 树的 hash) | mood / energy (Living state, lazy aging) |
|   | rolling_summary (Conversation state, 不冻结) |
|   | conversations.archived / title (Conversation state) |

**为什么 carve-out**: 用户改昵称 / 调 safety prefix / 改隐私设置不应被人格 snapshot 锁住。否则用户每改一次设置就要 fork 所有 conversation, 不可接受。Snapshot 锚定**人格定义**, 不锚定 Core/User/Runtime 状态。

**为什么**: 陪伴产品中 persona 不是 UI theme, 是**关系上下文**。用户对 momo 说过的私密话不该因为切到 joker 就被"出戏"地引用。snapshot 是稳定锚, 也是 ToolSub / MemorySub audit 绑定 actor 的稳定 id。Rebinding/fork 的语义清晰 + 用户主动 = 产品信任与工程稳定的双底线。

### 2.6 Soul Package Compile Boundary

> v2 新增 First Principle, reviewer P0。**Soul 是多消费方资源, 必须 compile-once-consume-many**。

**问题**: `.soul.md` 单文件 markdown 被 PromptBuilder / ToneShaper / InitiativeWeights / MemoryPolicy / UI 编辑器五方各自解析 → 长期不可维护。

**v2/v3 解法**:

```
persona source format (未决; examples: .soul/ / .soul.md / GUI schema / imported package)
         ↓ PersonaSub-owned parser / compiler

SoulRuntimeProfile { identity_prompt, style_prompt, initiative_config,
                     memory_policy, examples, ui_metadata, source_hash }

         ↓ 写入 persona_snapshot_profiles 表
         ↓ 生成稳定 persona_snapshot_id

PersonaSnapshot (运行时锚, conversation 绑定, audit 锚)
```

**消费方**:
- PromptBuilder 读 `identity_prompt` / `style_prompt` / `examples`
- ToneShaper 读 `style_prompt`
- InitiativeWeights 读 `initiative_config`
- MemorySub 读 `memory_policy`
- UI 读 `ui_metadata` + source-format specific editor data（source format 仍未决）

#### 2.6.1 Soul Package Terminology (v3 新增, 统一术语)

| 术语 | 定义 | 状态 | 谁产 / 谁消费 |
|---|---|---|---|
| **Persona source format** | 未决源格式；可能是 `.soul/`、`.soul.md`、GUI schema、imported package 或其他格式 | 未决 | PersonaSub 解析 / 编译 |
| **`.soul/` 目录** | 候选源格式之一；不再作为 2026-06-18 后的运行时前提 | Candidate | 后续 source-format spec 再定 |
| **`.soul.md` 单文件** | 候选源格式之一；可作为 simple / legacy 输入保留 | Candidate | 后续 source-format spec 再定 |
| **Imported package** | 候选分发 / 导入格式；是否继续使用 `.soulpack` 后续再定 | Candidate | 后续 source-format spec 再定 |
| **SoulRuntimeProfile** | Source format 编译后的运行时 profile，PromptBuilder / ToneShaper / InitiativeWeights / MemorySub 消费它 | Active runtime contract | PersonaSub 产 / runtime 消费 |
| **PersonaSnapshot** | 写入 `persona_snapshot_profiles` 表的稳定 row, 含 snapshot_id + SoulRuntimeProfile + created_at | 内部 (PersonaSub) | PersonaSub.activate / load_soul_package 产 / ConversationSub 强绑 (Constitution #10) |

**v3 关键不混用**:
- 严禁让 PromptBuilder 解析 source files；PromptBuilder 只消费 `SoulRuntimeProfile`
- 严禁让 source format 授予 permissions / tools / safety_prefix 控制权

#### 2.6.2 `.soulpack` 安全导入规则 (v3 新增, P0)

> 用户分享的 `.soulpack` 是真实攻击面 (path traversal / zip bomb / symlink / 可执行文件 / 篡改安全约束)。Phase A1 SoulPackageImporter 必须满足以下 10 条:

| # | 规则 | 实施 |
|---|---|---|
| 1 | **Max size 限制**: `.soulpack` ≤ 10 MB (含 assets), 单个内部文件 ≤ 2 MB | unzip 前先读 zip header 累加 uncompressed 大小, 超限早返 `ImportError::SizeLimit` |
| 2 | **禁止绝对路径**: zip 内 entry 路径必须相对 | 任一 entry 以 `/` 或 `C:\` 等开头 → reject |
| 3 | **禁止 `../`**: 防 path traversal | entry 路径 normalize 后检查不含 `..` 段 |
| 4 | **禁止 symlink**: 不跟随任何 symlink entry | zip entry attribute 检查 symlink flag, 命中 → reject |
| 5 | **禁止可执行文件**: 任何 `*.exe / *.dll / *.bat / *.ps1 / *.sh / *.cmd / *.scr / *.com` | 扩展黑名单检查;Phase A1 仅允许扩展 allowlist (第 6 条) |
| 6 | **文件扩展 allowlist**: `.toml / .md / .json / .txt / .png / .jpg / .jpeg / .gif / .webp / .wav / .mp3 / .ogg` (Phase A1 仅 toml + md;音频/图片 P1+) | unzip 时按扩展过滤, 非 allowlist → reject 整个包 |
| 7 | **manifest.toml 必须在 zip 根** | 解压后必须存在 `<root>/manifest.toml`, 否则 reject |
| 8 | **解包到 temp dir**: 不直接落到 `%APPDATA%\AIDesktopPet\personas\user\` | `std::env::temp_dir().join(uuid)`, 避免污染目标目录 |
| 9 | **validate 通过后移动**: SoulValidator.validate 通过 (含 manifest schema_version 校验 + permissions/tools/safety 字段 deny) 才移动到目标目录 | atomic rename;移动失败回滚到 temp |
| 10 | **失败清理**: 任一步失败必须 rm -rf temp dir | RAII `TempDir` guard;Drop 时自动清理 |

```rust
pub trait SoulPackageImporter: Send + Sync {
    /// 安全 unzip + validate + 移动到 personas/user/{persona_id}
    /// 全程在 temp dir 操作, validate 通过才落到目标目录
    async fn import_soulpack(&self, source: &Path) -> Result<PersonaId, ImportError>;
}

pub enum ImportError {
    SizeLimit { actual: u64, max: u64 },
    AbsolutePath(PathBuf),
    PathTraversal(PathBuf),
    Symlink(PathBuf),
    ExecutableForbidden(PathBuf),
    ExtensionNotAllowed { path: PathBuf, ext: String },
    ManifestNotInRoot,
    ValidationFailed(SoulValidationError),
    IoError(io::Error),
}
```

**分发格式**: `.soulpack` = `.soul/` 目录的 zip 归档, 经 SoulPackageImporter 通过 10 条规则后落到 `personas/user/{persona_id}/`, 之后走相同 SoulCompiler 路径。

**为什么**: 单文件 markdown 多方解析 = schema 演进瘫痪。多文件包 + compile 一次 + snapshot 锚定 = 编辑安全 / 运行高效 / 审计稳定 / 高级编辑器友好。`.soul.md` 简单模式作为 SoulCompiler 输入的特殊形式 (单文件 → 默认 `.soul/` 布局), 但 MVP 起点是 `.soul/` 目录。`.soulpack` 安全规则防真实分发攻击面。

### 2.7 Tool Grant Is Synchronous Request/Response

> v2 新增 First Principle, reviewer 纠正 v1 概念混淆。

**问题**: v1 把 Tool 用户授权混入 EventBus 异步通知 → 错误。

**v2 正解**:
- EventBus = post-hoc 通知 (持久化 / 广播 / telemetry / proactive evaluate), **不阻塞** publisher
- **Tool grant = hot path request/response**: LLM emit tool_call → ConversationSub 必须等 ToolSub 返回 ToolResult 才能继续 agent loop → 若需要用户授权, 这是同步 request/response, 不是 fire-and-forget

**因此**:
- **GrantBroker** 是独立 Kernel trait (7 件之一);async 但**调用方等待返回**
- ToolSub.execute 调用 `grant_broker.request_tool_grant(surface, tool_id, args_summary, paths, reason).await?`
- 用户在 UI 看到 modal, 同意/拒绝/"本次会话内不再问"/"以后不问" → GrantBroker 返回 `GrantDecision`
- 完成后 ToolSub 才执行 tool 实体

**为什么**: 让"等待用户输入"误用 EventBus 会导致 agent loop 状态不可恢复 / publisher 不知道 subscriber 何时回 / 死锁风险。`request_tool_grant` 必须是 async fn 直接返回。

---

## 3. Constitution（14 条不变量,违反 = build fail 或 panic / 安全降级）

> v1 8 条;v2 后 6 条由 reviewer 推动新增, 全部 P0 级。

| # | 不变量 | 工程落地 |
|---|---|---|
| **1. Safety Configurable** (Updated 2026-05-26, 原名 "Safety Sovereignty") | SafetyGuard 路径必经，是否真注入/扫描由 **SafetyPolicy** 决定 4 scope 各自启用 | `safety_prefix` 由 kernel 强制经 `SafetyGuard.wrap_messages` 走，subsystem 不得 bypass 自建路径; SafetyPolicy 4 KV (`safety:prefix_enabled` / `safety:scan_user_input_enabled` / `safety:scan_token_enabled` / `safety:scan_final_enabled`) 出厂全 OFF，用户经 IPC + workspace popup UI 配置；任何 LLM stream finish 必经 `SafetyGuard.scan_final` 路径 (off 时返 always-pass);**8-state FSM 跨越流式/终态(见 §6.6, 新增 `disabled` 终态)** |
| **2. Single Writer per Table** | 每张 SQLite 表恰好 1 个 owner | **MVP: Repository pattern** — 每个 owner table 一个 repo, subsystem 只拿自己的 repo, repo 只暴露强类型写方法;raw `sqlx::Pool` 仅 kernel/db module 内可见;migration 是唯一例外。**P1 hardening: `WriterCap<T>` 类型期 token** (v1 设计推到 P1, MVP 不过度承诺类型期隔离) |
| **3. Event-or-Direct** | subsystem 间通信只有两路 | (a) 同步调 owner read trait(仅 read);(b) 经 EventBus publish(异步通知,owner 自己 subscribe + write)。**禁止**第三种"直接调 owner write 方法"。**EventBus 失败分级** (见 §7.3): 关键表 fatal, observability 表 degrade |
| **4. No Self-Ticking** | subsystem 不自封 timer | 仅 Scheduler 持有 tokio runtime handle;subsystem 实现 `on_scheduled_tick(reason)` callback |
| **5. Soul Boundary** | Soul 单向 + 不扩权 | `soul/` 模块 `use` 黑名单含所有 write trait + EventBus publish + Scheduler/StateStore mut + **PermissionService.grant** + **GrantBroker.request**;Soul 只能 read trait + 返 prompt 文本 / score 数值 |
| **6. Lazy First** | 衰减类计算 lazy | mood / energy / wandering = store source + recompute on read;tick 仅在 Scheduler 三种触发(cron / idle-cross / one-shot / periodic) |
| **7. Hot Path Sync** | user → reply 全程同步 | EventBus 仅用于 post-hoc(persist / broadcast / telemetry / proactive evaluate);不在 hot path 上。**GrantBroker.request 是同步 await, 不走 EventBus** (Constitution #13) |
| **8. MVP First / Trait Cap** | 新概念必经 MVP 必要性证明 | trait ≤ **17** (kernel 7 + subsystem 6 + soul 4);event ≤ 15;subsystem = 6 封顶,新功能挂到现有 subsystem |
| **🆕 9. Privacy by Default** | 默认主动陪伴只用 idle + 本地 runtime state | app/window/selected text/mic/screen 必须经 PermissionService 显式授权;默认 deny;每次访问写 `context_access_log`;Soul / Initiative / Tool 任何越过 PermissionService 直接读 OS 上下文 = build fail (CI 静态扫描黑名单 `getUserMedia` / `MediaRecorder` / `GetForegroundWindow` / `GetWindowText` / `BitBlt` 等) |
| **🆕 10. Session Persona Stability + No Silent Rebinding** | active persona 切换不污染历史会话 / runtime 不静默重绑 | `conversations.persona_snapshot_id NOT NULL`;hot path 必须 `ConversationStore.read(conv_id).persona_snapshot_id` → `PersonaSub.read_snapshot(id)`, 严禁 fallback 到 `PersonaSub.read_active()`;persona 编辑 → 旧会话 snapshot 不变 (要换需显式用户操作);**Runtime 不得因为 active persona 切换或 `.soul/` 文件变化而 UPDATE conversations.persona_snapshot_id** (CI 静态扫描禁止此 UPDATE 出现在 ConversationSub.rebind_persona_snapshot / fork_conversation 之外的任何代码) |
| **🆕 11. Soul Cannot Grant Permissions** | Soul 文件不能扩大权限 | SoulValidator 拒绝 manifest 出现 `permissions: [...]` / `context_scopes: [...]` / `tools: [...]` 字段;Soul 输出仅 prompt 文本 + score 数值;Soul 不能让 LLM emit 触发 PermissionService 改设置的 tool_call (Tool denylist) |
| **🆕 12. Soul Package Compile Boundary** | 运行时不散读 `.soul/` 文件 | PersonaSub.activate() 必经 SoulCompiler.compile() → PersonaSnapshot 持久化 → 之后所有 hot path 读 snapshot, 不读源文件;`.soul/` 源文件变化不会即时影响运行时, 必须显式 recompile |
| **🆕 13. Tool Grant Is Synchronous** | GrantBroker 是 hot path | `request_tool_grant` 是 async fn 直接 await 返回 GrantDecision;**严禁**通过 EventBus 发 `tool.grant_request` 让 UI subscribe 再 publish `tool.grant_response` 这种 fake-sync 模式;ToolSub.execute 必须能在用户拒绝 / 超时后干净返 `ToolError::Denied` |
| **🆕 14. Memory MVP First / Graceful Degradation** | MVP 不上 episodic / FTS5 / **自动 LLM rolling summary** | Phase A2 MUST: semantic KV → prompt + recent message token window;**rolling_summary 默认关闭 (deterministic placeholder, 不调 LLM)**, 自动 LLM rolling summary 推 P1 (启动前必须满足 4 项前置: provider check / token budget / SafetyGuard scan / 失败 fallback);episodic + FTS5 + RetrievalRanker + LLM 压缩推 Phase C P1;EventBus publish 失败分级 (见 §7.3): 关键 owner 表写失败 fatal, observability event_log 失败 degrade 到 `error_logs` 不 panic |

---

## 4. Architectural Map

### 4.1 全图

```
┌─────────────────── 4 SURFACES (UI 表面) ──────────────────────────────────┐
│   [Pet Window]   [Chat Panel]   [Workspace]   [Tray + Notification]       │
│        │              │              │                 │                   │
│        └──────────────┴──────────────┴─────────────────┘                   │
│                            ↑ event subscriptions (read-only on state)      │
│                            ↑ GrantBroker UI modal (sync request/response)  │
└───────────────────────────│───────────────────────────────────────────────┘
                            │
┌──────────────── SOUL OVERLAY (expressive, stateless) ─────────────────────┐
│  PersonaPromptBuilder | ToneShaper | InitiativeWeights | RetrievalRanker  │
│  仅两条通道:                                                               │
│  ① system message 注入  ② decision weight 加权                            │
│  Soul use-blacklist 含 PermissionService / GrantBroker (Constitution #11) │
└──────────────────────────────│────────────────────────────────────────────┘
                               │ 单向调用 (Soul → Core, 反向禁止)
┌─────────────────────── 6 SUBSYSTEMS (rational core) ──────────────────────┐
│                                                                            │
│  ConversationSub | MemorySub | InitiativeSub                               │
│  (chat agent loop)| (Phase A: KV+window  | (proactive trigger eval)        │
│   ← ChatService   |  Phase C: episodic)  | ← ProactiveCare 规划落地        │
│                   |                       |                                │
│  PersonaSub      | ToolSub (Phase C)    | LivingSub (Phase B)             │
│  (.soul/ package)| (read-only:           | (mood/energy/wandering          │
│  + SoulCompiler  |  Glob/Grep/Read       | lazy aging)                     │
│  + Snapshot      |  via GrantBroker)     | ← LivingPetService 改造         │
│  ← PersonaService| ← 新建                |                                 │
└──────────────────────────────│────────────────────────────────────────────┘
                               │ subsystem 之间禁止直接调用, 必须经 kernel
┌─────────────── KERNEL (7 件套, hard, never bypassable) ───────────────────┐
│  SafetyGuard 8-state FSM | LifecycleManager | EventBus | Scheduler        │
│  StateStore (Repository) | PermissionService 🆕 | GrantBroker 🆕          │
│  (ADR-006 真注入)        | (FSM 5 states)   | (typed pub/sub + 失败分级)  │
│                          | (cron+idle      | (默认 deny Context Awareness │
│                          | +one-shot       |  audit -> context_access_log)│
│                          | +periodic)      | (sync tool grant req/resp)   │
└────────────────────────────────────────────────────────────────────────────┘
                               │
                       [LLMProvider]    ← capability, 不在 kernel
                       [Tauri IPC]      ← platform adapter, 不在 kernel
```

### 4.2 Kernel 7 件套（hard, 永不可被 subsystem 越权）

| Kernel 组件 | 责任 | 不变量 | Phase |
|---|---|---|---|
| **SafetyGuard** | ADR-006 prompt prefix 注入 (policy-gated) + LLM 流式 token 增量扫描 + 终态全文扫描 + **8-state FSM (含 `disabled` 终态)** + 拒答降级链 | SafetyGuard 路径必经，是否真注入/扫描由 SafetyPolicy 决定（4 scope toggle 出厂全 OFF）；subsystem 不得 bypass SafetyGuard 自建路径 | A |
| **LifecycleManager** | Boot/Live/Suspend/Wake/Shutdown FSM + 启动顺序 + dependency wiring | 同一时刻 Runtime 状态唯一;Wake 后 state 一致性自检 | A |
| **EventBus** | 类型化 pub/sub + 同步派发 + 持久化关键事件到 `event_log` + **失败分级降级 (§7.3)** | 任一 publish 都至少持久化 schema_version + payload;关键 owner 写失败 fatal-for-conv, observability 失败 degrade 到 error_logs | A 占位 / B 真接入 |
| **Scheduler** | cron + idle-threshold + one-shot + periodic 4 种触发统一调度 | 单实例 tokio runtime;不允许 subsystem 自起 timer | B |
| **StateStore** | SQLite + config KV + secrets(DPAPI) 抽象;**Repository pattern** + 事务边界 | 写入必经 owner 自家 repo (raw `Pool` 仅 kernel/db module 可见);secrets 必经 CryptoService | A |
| **🆕 PermissionService** | Context Awareness 权限网关 + audit 日志 + 默认 deny 策略 + 用户授权 / 撤销 UI 入口 | 任何 OS 上下文读取必经此 trait;Soul / Tool / Initiative 不可旁路;`context_access_log` 写入是原子的 | A 框架 stub / P1 实施 |
| **🆕 GrantBroker** | Tool 调用的同步用户授权 request/response + Grant 决策缓存 (None/SessionScope/PersistentByTool) | hot path 上 async fn 直接 await 返回 GrantDecision;不走 EventBus (Constitution #13) | A stub / C 真接 ToolSub |

### 4.3 6 Subsystems（rational, 实现可独立演进）

| Subsystem | 责任 | 与 AIPET 现有 service 映射 | Phase 与改造工作量 |
|---|---|---|---|
| **ConversationSubsystem** | chat agent loop;消息流;persona_snapshot 绑定;tool_call 在 Phase A **保持忽略**, Phase C 转 ToolSub;多模态接口预留 (MVP 不实现, LLM types 已 typed) | `services/chat/*` 1:1 升级 | A: ~1 周 (接 SafetyGuard FSM + persona_snapshot binding + KV inject + token window) |
| **PersonaSubsystem** | `.soul/` 多文件包加载 / SoulCompiler / PersonaSnapshot / 试聊沙盒 / activate (不污染历史会话) | `services/persona.rs` 改造 | A: ~1 周 (SoulCompiler + Snapshot 是新; 90% legacy logic 保留) |
| **MemorySubsystem** | Phase A: KV → prompt + history token window + 可选 rolling_summary;Phase C: episodic + FTS5 检索 + LLM 摘要压缩 + RetrievalRanker | **Phase A 改造**现有 `services/memory.rs` (扩 KV 含义);**Phase C 新建** episodic 模块 | A: ~3 天 / C: ~2 周 |
| **InitiativeSubsystem** | proactive trigger 评估(idle / mood / quota / quiet hours);**默认 idle-only**, opt-in context awareness | `services/proactive_care.rs`(规划未实施)落地;`living_pet.rs` 主动起话题逻辑分离 | B: ~1 周 |
| **LivingSubsystem** | mood / energy / wandering 的 lazy aging function + 触发条件 | `services/living_pet.rs` 改造(去 tick, 改 lazy + event-driven) | B: ~1 周 |
| **ToolSubsystem** | Tool Registry + 路径白名单 + GrantBroker 同步授权 + 执行 + 审计(MVP: Glob / Grep / Read) | **新建**(ADR-025 阻塞前置) | C: ~1.5 周 (ADR-025 写 ~0.5 周 + 实现 ~1 周) |

### 4.4 Soul Overlay 4 件套（stateless function, 无 state）

| Component | 输入 | 输出 | 钩入点 | Phase |
|---|---|---|---|---|
| **PersonaPromptBuilder** | PersonaSnapshot.runtime_profile / nicknames / mood / context | system message 文本 | ConversationSubsystem `build_messages` 之前 | A |
| **ToneShaper** | core 准备发出的 raw text + snapshot.style_prompt + live state | tone 包装后的 text | LLM 调用前 / proactive 文案最后一步 | B |
| **InitiativeWeights** | candidates + snapshot.initiative_config + live | weighted score | InitiativeSubsystem `select_candidate` 排序时 | B |
| **RetrievalRanker** | episodic memory 候选 + 当前 context + emotional relevance | re-ranked list | MemorySubsystem `search` 后处理 | C (依赖 episodic) |

**Soul 不变量**(类型期 + lint 约束):
- Soul 不读写 StateStore / 不直接调 ToolSubsystem / 不改 Scheduler / 不绕 SafetyGuard / **不调 PermissionService 改设置** / **不调 GrantBroker** (Constitution #11)
- Soul 是 stateless function — 输入相同则输出相同(除 LLM 本身非确定性)
- `soul/` 模块 `use` 黑名单含所有 write trait + 权限相关 kernel traits
- 消费 PersonaSnapshot 时只读 `runtime_profile`, 不读源 `.soul/` 文件 (Constitution #12)

### 4.5 Surfaces 4 件套（UI 表面, 仅 read state 经 IPC）

| Surface | 内容 | 状态来源 |
|---|---|---|
| Pet Window | 透明置顶 VRM 桌宠 | ConversationSubsystem.read + LivingSubsystem.read_current |
| Chat Panel | 磁吸浮窗/嵌入式 chat UI | ConversationSubsystem (流式 Channel) + GrantBroker modal |
| Workspace | 单窗多 panel 壳 (ADR-021) | 全部 subsystem read trait |
| Tray + Notification | OS 托盘 + 系统通知 | EventBus subscribe (broadcast) |

---

## 5. State Ownership Map

### 5.1 表 Ownership（v2 含新增 + Phase 分级）

> v1: 4 新增。v2: Phase A 新增 2 (persona_snapshot_profiles, context_access_log), Phase B 新增 1 (event_log), Phase C 新增 3 (episodic, episodic_fts, tool_audit_log)。

| 表名 | Owner (writer) | Readers | Phase | Lifecycle |
|---|---|---|---|---|
| `conversations` | ConversationSub (via repo) | InitiativeSub(last_activity) / MemorySub(摘要时) / PersonaSub(snapshot binding 校验) | A 改 schema | persistent (+ `persona_id` + `persona_snapshot_id` + `rolling_summary`) |
| `messages` | ConversationSub | MemorySub(扫描转 episodic, Phase C) | A 改 schema | persistent(+ `token_count` / `safety_scan_status` 8-state 枚举) |
| `personas` | PersonaSub | ConversationSub / InitiativeSub | — | persistent |
| `persona_snapshots` | PersonaSub | ConversationSub (hot path read by snapshot_id) | A 改 schema (+ source_hash + compiled_profile_json + schema_version) 或拆 `persona_snapshot_profiles` 子表 | persistent |
| 🆕 `persona_snapshot_profiles` (可选, 若 persona_snapshots 不扩展) | PersonaSub | ConversationSub / PromptBuilder / ToneShaper / InitiativeWeights | **A 新增** | persistent |
| `memory` | MemorySub | ConversationSub(prompt 注入) | — (Phase A 含义扩展, schema 不变) | persistent(扩为 KV 偏好 + rolling_summary head pointer) |
| `nicknames` | PersonaSub | ConversationSub / InitiativeSub | — | persistent |
| `pet_runtime_state` | LivingSub | InitiativeSub | B 改 schema | persistent(+ `last_mood_event_at` / `last_energy_event_at`,改 lazy aging) |
| `proactive_care_log` | InitiativeSub | — | B 改 schema (+ trigger_kind / gate_result / context_scopes_used / score_breakdown) | persistent (7d retention) |
| `config` / `secrets` / `consent` / `schema_version` | **Kernel** (StateStore) | 全部 subsystem | — | persistent |
| `reminders` / `pomodoro_sessions` / `todos` | **TaskService**(保留独立) | — | — | persistent;与 6 subsystem 正交,经 EventBus 通知 LivingSub |
| 🆕 `context_access_log` | **Kernel** (PermissionService) | observability / 用户隐私设置 UI | **A 新增** (默认 deny 期间写入 reject 记录) | persistent |
| 🆕 `event_log` | **Kernel** (EventBus) | observability / debug | **B 新增** | persistent (30d retention) |
| 🆕 `episodic_memory` | MemorySub | ConversationSub(retrieve) | **C P1 新增** | persistent + FTS5 |
| 🆕 `episodic_memory_fts` | MemorySub (FTS5 internal) | MemorySub only | **C P1 新增** | persistent (content_rowid='rowid' INTEGER) |
| 🆕 `tool_audit_log` | ToolSub | safety review | **C P1 新增** | persistent |
| 🆕 `working_memory` (in-mem) | ConversationSub | — | A 含义抽象 (Phase A 不必独立存储, 即 messages 表最近 N 条 + token cache) | **transient (in-mem)** + persist on shutdown via messages 表 |

### 5.2 Sync vs Async 分类（防止 hot path 误经 EventBus）

**Sync hot path** (直接调用链, 禁经 EventBus):
```
user_input → ConversationSub.handle_user_message(conv_id)
  → ConversationStore.read(conv_id)            (sync)
  → PersonaSub.read_snapshot(snapshot_id)      (sync, Constitution #10)
  → MemorySub.retrieve_kv(persona, scope)      (sync, Phase A: KV bullets)
  → Soul.PromptBuilder.build(...)              (stateless)
  → SafetyGuard.wrap_messages (kernel)         (sync)
  → LLMProvider.chat_stream
  → SafetyGuard.scan_token (FSM step, hot)     (sync, 流式)
  → SafetyGuard.scan_final (FSM step, 终态)    (sync)
  → ToolSub.execute (Phase C 启用; Phase A 主动忽略)
    └ GrantBroker.request_tool_grant            (async await, sync 语义)
  → Soul.tone_shape (stateless, optional)
  → multi-surface emit (此处转 async EventBus)
```

**Async (EventBus, post-hoc)**:
- `chat.message_done` → MemorySub.persist (Phase A: rolling_summary update; Phase C: episodic) / surface_broadcast
- `persona.activated` → InitiativeSub.refresh_weights (Phase B) / surface_broadcast
- `scheduler.idle_threshold_crossed` → InitiativeSub.evaluate_proactive (Phase B)
- `living.mood_changed` → InitiativeSub.rescore (Phase B)
- `tool.executed` → tool_audit_log + telemetry (Phase C)
- `safety.violation` → log + telemetry (Phase A)
- `task.reminder_fired` → LivingSub.maybe_react (Phase B)
- `consent.changed` → LifecycleManager.gate_recheck
- `wake.completed` → InitiativeSub.missed-job evaluate (Phase B)
- 🆕 `context.permission_changed` → 各 sub gate_recheck (P1 用户授权 / 撤销)

**事件总数 10 个** (< 15 上限 ✓)

**v2 明确**: Tool grant **不是** event!  `request_tool_grant` 是 GrantBroker async fn, 直接 await (Constitution #13)。

### 5.3 Persistent / Transient / Lazy

| 类别 | 内容 |
|---|---|
| **Persistent** | 全部 owner 表 |
| **Transient (in-memory)** | `active_streams` CancellationToken map / cached `PersonaSnapshot` (从 persona_snapshot_profiles 解出来的 runtime profile) / SchedulerJobs handles / GrantBroker session-scope cache |
| **Lazy-computed** (store source + recompute on read) | `mood = decay(last_event, base, persona_modifier)` (Phase B) / `energy = decay(last_event, recent_activities)` (Phase B) / `wandering_target` / `memory_retrieval_score = recency + fts_match + emotional_weight` (Phase C) |

### 5.4 Hot Path 延迟预算

| 路径 | 类型 | 预算 |
|---|---|---|
| user input → first token | hot sync | **p50 ≤ 1.5s** (架构 §10.2 现有口径) |
| persona switch → new reply | hot sync | < 500ms (新会话, 旧会话不受影响) |
| persona snapshot load | hot sync | < 10ms (in-mem cache; cold load < 50ms) |
| GrantBroker request → response (auto-approve session-scope) | hot sync | < 20ms |
| GrantBroker request → response (用户手动审批) | hot, **取决于用户** | UX 期望 < 5s, 超时降级 ToolError::Denied |
| tool exec (Read 文件) → result | hot sync | < 200ms (含沙盒 check) |
| mood / energy recompute on read | lazy | < 5ms |
| memory retrieval (Phase A KV) | hot sync | < 5ms |
| memory retrieval (Phase C FTS5) | hot sync | < 50ms |
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

> v2 改 v1 8 步 → 10 步, 加 PermissionService / GrantBroker。

```
1. MigrationService.run()         schema check + 备份 + 升级 (含 v2 新增 ALTER + 新表)
2. StateStore.open()              sqlx pool + WAL + PRAGMA fk + Repository registry
3. SafetyGuard.load()             prefix_v1.txt + regional + FSM scanner ready
4. 🆕 PermissionService.init()    load context_access_log; 默认全 deny
5. 🆕 GrantBroker.init()          load Grant cache (PersistentByTool from config)
6. EventBus.init()                typed pub/sub + event_log writer (Phase B 真接入)
7. Scheduler.start()              tokio runtime + cron tab 空启 (Phase B)
8. 6 subsystems.init(handles)     每个拿到 (StateStore repo, EventBus, Scheduler, SafetyGuard,
                                                 PermissionService, GrantBroker — 仅 ToolSub 用)
   ├ PersonaSub: SoulCompiler 编译 active persona → PersonaSnapshot; load_active
   ├ MemorySub: 打开 KV scope + (Phase C: open episodic + alloc working)
   ├ ConversationSub: cancel_token map + ConversationRepo
   ├ ToolSub: Phase C 启用; Phase A 仅留 stub (load whitelist + 注册 3 tool)
   ├ InitiativeSub: read quiet_hours/quota (Phase B)
   └ LivingSub: restore last mood/energy (Phase B)
9. LifecycleManager.gate(consent) onboarding 续接 / consent.version
10. → Live (sub-state = Idle)
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
                          ↕
                  AwaitingGrant     🆕 v2 子态
                  (within Toolusing, GrantBroker 等待用户审批)
```

| Sub-state | 入条件 | 出条件 | 主要动作 |
|---|---|---|---|
| **Idle** | Live default | user_input / proactive fire | mood/energy lazy aging accumulate;surface 静态/微动 |
| **Conversing** | user_input from any surface | message_done | hot path sync(见 §5.2) |
| **Toolusing** (Phase C) | LLM stream emit tool_call | tool result inject / Denied / Limit | ToolSub safety check → GrantBroker.request → exec → 回 Conversing |
| 🆕 **AwaitingGrant** (Phase C, sub-of Toolusing) | tool 调用首次访问 path / 用户启用"每次问" | 用户决定 (allow/deny/once/session/persistent) OR 超时 5s 默认 deny | UI modal 显示 tool/path/reason;**不阻塞**其他 surface 但阻塞当前 agent loop |
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

### 6.6 SafetyGuard 8-state FSM（Updated 2026-05-26：从 7-state 扩，新增 disabled 终态）

> v1 缺口: SafetyGuard 只描述"wrap + scan"两动作, 没有定义流式 token 已发 → final scan 命中违禁的回滚 / UI 替换 / DB 状态 / event 生命周期。本节是 Constitution #1 工程落地的核心。

#### 6.6.0 SafetyPolicy 与 SafetyGuard 协作（Updated 2026-05-26）

SafetyPolicy 是 kernel-owned trait（不是 Kernel 第 8 件套，仍 7 件套；SafetyPolicy 作为 SafetyGuardImpl 的依赖注入）。详 spec [`2026-05-26-safety-policy-configurable-design.md`](2026-05-26-safety-policy-configurable-design.md)。

```rust
pub enum SafetyScope { PrefixInjection, UserInput, StreamToken, FinalOutput }

pub trait SafetyPolicy: Send + Sync {
    fn is_enabled(&self, scope: SafetyScope) -> bool;
    async fn set_enabled(&self, scope: SafetyScope, enabled: bool) -> Result<(), PolicyError>;
}

pub trait SafetyGuard {
    fn is_enabled(&self, scope: SafetyScope) -> bool;  // 转发 policy
    // ... 其余 4 方法 noop-when-disabled
}
```

4 个 config KV（出厂全 OFF）：

| Key | Default | 控制 |
|---|---|---|
| `safety:prefix_enabled` | `false` | wrap_messages 注入 ADR-006 prefix |
| `safety:scan_user_input_enabled` | `false` | scan_user_input (Scope #1) |
| `safety:scan_token_enabled` | `false` | scan_token (Scope #2) |
| `safety:scan_final_enabled` | `false` | scan_final (Scope #3) |

ConfigKvSafetyPolicy 持 4 个 `Arc<AtomicBool>`，boot 时同步读 KV 加载，运行期 atomic 读不 hit DB；`set_enabled` 写 DB 成功后才同步更新内存 AtomicBool（保持 DB 与内存一致）。

**针对对象**: assistant message (LLM 返回的每一条消息);user input / tool result / memory summary 在 P1 评估是否走同一 FSM。

**8 个状态** (`messages.safety_scan_status` 枚举值, Updated 2026-05-26 加 `disabled`):

```
                          ┌─────────────┐
                          │   pending   │ 消息创建, 尚未开始 stream
                          └──────┬──────┘
                                 │ run_stream 启动
            ┌────────────────────┼───────────────────┐
            │ policy.scan_final  │ policy.scan_final │
            │ OFF                │ ON                │
            ↓                    ↓                   │
       ┌──────────┐         ┌─────────────┐          │
       │ disabled │ (终态)  │  streaming  │          │
       │          │         └──────┬──────┘          │
       └──────────┘                │ scan_token chunk
                                   │ (policy.scan_token ON)
              ┌────────────────────┼────────────────────┐
              │ soft hit           │ 终态                │ 流式中断
              ↓                    ↓                     ↓
     ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
     │stream_soft_block │  │  scan_final 调用 │  │   scan_failed    │
     │ (插占位 "[审核中]│  │                   │  │ (网络断 / panic) │
     │  替换最近 N token)│  └────────┬─────────┘  │  保守降级 = redact│
     └────────┬─────────┘           │             └────────┬─────────┘
              │                     │                       │
              │             ┌───────┼────────┐              │
              │ scan_final  │ ok    │ block  │ redact       │
              │ 重判       ↓       ↓        ↓              │
              │     ┌──────────┐ ┌──────────┐ ┌────────────┐│
              └────→│ final_ok │ │  final_  │ │   final_   ││
                    │          │ │  blocked │ │   redacted │←┘
                    └──────────┘ └────┬─────┘ └────────────┘
                                      │
                               publish safety.violation
```

| 状态 | 含义 | DB.messages 内容 |
|---|---|---|
| `pending` | INSERT placeholder | content="" |
| `streaming` | scan_token ON 流中 | content = partial |
| `stream_soft_blocked` | scan_token chunk soft hit | content = partial 含 redact 标记 |
| `final_ok` | scan_final ON + 全文通过 | 全文 |
| `final_redacted` | scan_final ON + soft 命中 | redacted 全文 |
| `final_blocked` | scan_final ON + hard 命中（含 scan_token hard hit 强制终态） | fallback 文案 |
| `scan_failed` | scan 自身崩 | partial + warning |
| **`disabled`** (新, Updated 2026-05-26) | **scan_final OFF**, ChatService 流末显式写入 | LLM 原文 |

**关键转移规则**:

1. **streaming → final_***: stream finish (`FinishReason::Stop / Length / ContentFilter / Error / Unknown`) enters the `SafetyGuard.scan_final` path; if `SafetyPolicy.FinalOutput` is OFF, the path returns noop / always-pass and final status follows the 2026-06-18 priority table.
2. **stream_soft_blocked → final_blocked**: 若 soft block 累积 ≥ 3 次同一规则, 终态升级 blocked
3. **scan_failed**: 保守降级 = 视作 blocked (用户看到 fallback);**绝不**因 scan_failed 就放过原文
4. **UI 替换协议**: ConversationSub emit 新 `StreamEvent::ReplaceMessage { msg_id, new_content, reason }` (Tauri Channel), 前端按 msg_id 覆盖现有显示。Phase A 已落地的 4 分支收尾不变, 新增 5th 分支 `safety_replace`。
5. **流式 token 已发 vs DB 终态不一致**: 后端必须以 `safety_scan_status` 为 source of truth, 前端 reconnect 时按 DB 拉最新覆盖本地 buffer。

**v2 同 ChatService 现有 4 分支收尾的关系**:

```
v1 4 分支:  success / cancel / network_error / provider_error
v2 7-state vs 4 分支:
  - final_ok                   = success
  - final_redacted             = success + replace UI
  - final_blocked              = success + replace UI fallback
  - scan_failed                = success + replace UI + warning banner
  - cancel (用户)              = pending → cancel (不进 FSM)
  - network_error / provider_  = streaming → scan_failed (保守降级)
```

**消费范围** (Phase A MUST):

| 输入 | scan_token | scan_final | 备注 |
|---|---|---|---|
| LLM assistant message | ✅ | ✅ | 本节主体 |
| user input | ❌ | ✅ (Phase A 简单黑词扫) | 防 prompt injection 用户尝试改 prefix |
| tool result content (P1) | ❌ | ✅ | Phase C, tool 输出可能含 LLM 不该 verbatim 输出的内容 |
| memory summary content (P1) | ❌ | ✅ | Phase C, 摘要 LLM 产物也走扫 |

#### 6.6.1 SafetyPrefix vs SafetyScanRules (v3 review-2 概念分离)

| 名词 | 作用方向 | 内容 | 应用时机 |
|---|---|---|---|
| **SafetyPrefix** | **出**: prompt → LLM | ADR-006 安全前缀文本 (通用核心 + 地区补充) | `SafetyGuard.wrap_messages` 在 system message 第一位拼入 |
| **SafetyScanRules** | **入**: 各种 content → Runtime | 黑词表 + regex + (P1) classifier | `scan_token` / `scan_final` / `scan_user_input` 分类调用 |

**关键不变量**:
- SafetyPrefix **不扫**任何东西, 它是出去的; SafetyScanRules **不拼**任何东西, 它是检查进来的
- 两者都是 kernel-owned, subsystem / Soul 都不能修改
- SoulValidator 拒绝 `.soul/` manifest 出现 `safety_prefix` / `safety_scan_rules` 字段 (Constitution #11)

#### 6.6.2 Scan Scope Matrix (v3 review-2, 7 项)

> reviewer P0: 必须明确"哪些内容在哪个阶段被扫"。下表是 SafetyScanRules 的完整 scope 矩阵。

| # | Scope (被扫内容) | 来源 | scan_user_input | scan_token | scan_final | **Phase / default enabled (Updated 2026-05-26)** | 命中处理 |
|---|---|---|---|---|---|---|---|
| 1 | **user input** | 任一 surface 用户输入 | ✅ | — | — | A0 / **OFF (KV `safety:scan_user_input_enabled`)** | hit → ChatError::UnsafeInput, ConversationSub 拒发 LLM, UI 显示拒绝原因 |
| 2 | **assistant stream token** | LLM 流式 token chunk | — | ✅ | — | A0 / **OFF (KV `safety:scan_token_enabled`)** | soft hit → stream_soft_blocked (替换最近 N token 为 `[审核中…]`);hard hit → 强制 finish + scan_final |
| 3 | **assistant final text** | LLM 流式终态全文 | — | — | ✅ | A0 / **OFF (KV `safety:scan_final_enabled`)** | 决定 final_ok / final_redacted / final_blocked / scan_failed (§6.6 8-state FSM) |
| 4 | **memory KV** (从 `memory` 表读出, 拼入 system message 前) | MemorySub.retrieve_kv 输出 | — | — | ✅ (Phase A2 起) | A2 | hit → 该 KV bullet 不拼入 prompt + 写 error_logs (用户曾输入违禁内容到 KV 时防再次出现) |
| 5 | **rolling_summary** (Phase A2 占位不扫;P1 真接入时走 SafetyGuard path) | MemorySub.maybe_roll_summary 输出 | — | — | ✅ (P1 起, policy-gated) | P1 | hit → 整条摘要丢弃, conversations.rolling_summary 保持 NULL / 占位 + 写 safety.violation |
| 6 | **tool result content** | ToolSub.execute 返回 (read 文件 / grep 命中行 etc.) | — | — | ✅ | C | hit → ToolError::ResultUnsafe + 不 inject 到 LLM messages + audit log 标记 |
| 7 | **context snapshot** (Phase A0 全 deny, P1 启用后 OS context 读取的内容) | PermissionService.read_context 输出 | — | — | ✅ (P1 起) | P1 | hit → ContextValue::Redacted + 写 context_access_log + 不 inject 到 prompt |

**矩阵 hard rules**:
- 任一 Scope 出 hit 都不允许 silent pass — 必须有显式拒绝路径
- LLM 产物 (scope 2, 3, 5) hit 必须 publish `safety.violation` event
- 用户输入 (scope 1) hit 仅 UI 告知, 不 publish safety.violation (避免误报骚扰)
- Phase A0 **接通**路径 scope 1+2+3 + 默认 OFF + 用户经 KV/UI 可启用；Phase A2 加 scope 4；Phase C 加 scope 6；P1+ 加 scope 5+7
- `scan_final` 是 entry point, 内部根据 source 应用不同 SafetyScanRules 子集 (用户输入 vs LLM 产物的规则可不同)

#### 6.6.3 Cross-scope 互动表（Updated 2026-05-26）

`PrefixInjection` 与 scan 系列**独立**；`UserInput` 与 assistant message 的 safety_scan_status **无关**。

`scan_token` × `scan_final` 4 组合：

| `scan_token` | `scan_final` | 流末状态写入 |
|---|---|---|
| OFF | OFF | `disabled` |
| OFF | ON | `final_ok` / `final_redacted` / `final_blocked` |
| ON | ON | 完整 8-state FSM |
| ON | OFF | mid-stream hit → `stream_soft_blocked` / `final_blocked`（hard hit 强制终态）；无命中 → `disabled` |

### 6.7 Lazy 计算（read-time compute, 不持久化中间值）

| Lazy 对象 | source | recompute formula | trigger |
|---|---|---|---|
| `current_mood` | `pet_runtime_state.mood + last_mood_event_at` | `clamp(base + Σ event_deltas · decay(elapsed), 0, 100)` | InitiativeSub.evaluate / Soul.build_prompt / surface render |
| `current_energy` | `state.energy + last_event + recent_activities` | `clamp(base + recovery(elapsed) - cost(activities), 0, 100)` | 同上 |
| `proactive_score(candidate)` | candidate triggers + persona + mood + quota | weighted sum(见 §10) | InitiativeSub.select_candidate |
| `memory_relevance(item, query)` | FTS5 match + recency + emotional_weight | `0.5·fts + 0.3·recency + 0.2·emotion` | MemorySub.retrieve |

### 6.8 状态迁移决策表

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

### 7.1 Event Catalog（10 个, Constitution #8 上限 15 内）

> v1: 9 个;v2 加 `context.permission_changed` (P1 用户授权变更 broadcast)。
> **v2 重要**: `tool.grant_request` / `tool.grant_response` 等 fake-sync 命名**不存在**;Tool grant 是 GrantBroker async fn 同步 await, **不**走 EventBus (Constitution #13)。

| # | Event | Publisher | Subscribers | Payload | Persist | Phase |
|---|---|---|---|---|---|---|
| 1 | `chat.message_done` | ConversationSub | MemorySub(persist) / surface_broadcast | `{conv_id, msg_id, role, content, mode, token_count, finish_reason, safety_scan_status}` | yes(event_log)+ messages 表 | A |
| 2 | `persona.activated` | PersonaSub | InitiativeSub(refresh_weights) / surface_broadcast | `{persona_id, name, previous_id, snapshot_id}` | yes | A |
| 3 | `scheduler.idle_threshold_crossed` | Scheduler | InitiativeSub(evaluate_proactive) | `{idle_ms, threshold_ms, since}` | no(transient) | B |
| 4 | `living.mood_changed` | LivingSub | InitiativeSub(rescore) / surface_broadcast | `{from, to, transient: bool, trigger, source_event_id}` | yes(7d) | B |
| 5 | `tool.executed` | ToolSub | tool_audit_log writer / telemetry | `{tool_id, args_hash, status, latency_ms, exit_code, persona_snapshot_id, grant_kind}` | yes(persistent) | C |
| 6 | `safety.violation` | SafetyGuard | telemetry / error_logs | `{violation_kind, rule_id, snippet_hash, persona_snapshot_id, action_taken: redact\|block\|scan_failed}` | yes | A |
| 7 | `task.reminder_fired` | TaskService(reminders 独立) | LivingSub(maybe_react) / surface(notification) | `{reminder_id, title, priority}` | already in reminder_history | B |
| 8 | `consent.changed` | LifecycleManager | gate_recheck (all subsystems via subscribe) | `{version_old, version_new, granted, method}` | yes | A |
| 9 | `wake.completed` | LifecycleManager | InitiativeSub(missed-job evaluate) | `{suspended_at, woke_at, elapsed_secs}` | yes | B |
| 🆕 10 | `context.permission_changed` | PermissionService | InitiativeSub / ToolSub / surface_broadcast | `{scope, granted: bool, by_user_action, changed_at}` | yes | A 框架 / P1 实际 fire |

**故意不做的 event**(防止泛滥):
- ❌ `chat.token` — hot path, 走 Tauri Channel 不入 EventBus
- ❌ `tool.started` / `tool.grant_request` / `tool.grant_response` — `tool.executed` 已含 status + grant_kind;**grant 是同步 request/response, 不是 event** (Constitution #13)
- ❌ `pet.energy_changed` — Living 内部, 通过 surface read mood 即可
- ❌ `nickname.changed` / `persona.edited` — 走 Tauri emit, 不影响 runtime
- ❌ `wardrobe.changed` — 装扮系统不在 6 subsystem 内
- ❌ `pomodoro.tick` — 高频, 走 Tauri emit
- ❌ 🆕 `context.access_attempted` — PermissionService 写 `context_access_log` 表内部, 不广播

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

### 7.3 Publish 语义（明确 sync / async 边界 + v2 失败分级）

```
publisher 调 .publish(event)         [sync, ≤ 5ms]
  ↓
  ├ if PERSIST: INSERT event_log (sync, fallible)
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
- subscriber 内 publish **同 KIND** event → Boot 期 wire 时静态校验拒绝(防环)

**🆕 v2 失败分级** (reviewer P0, 替换 v1 的 "publish 失败 → panic"):

| 写入对象 | 失败处理 | 理由 |
|---|---|---|
| owner 表 `messages` 写入 | **fatal-for-conversation** — 当前会话标 error, 不影响其他会话 / 不 panic 进程 | messages 是 source of truth, 写不进等于消息丢, 必须用户感知;但单条对话失败不应拉垮整个 runtime |
| `config` / `secrets` / `consent` 写入 | **fatal or block feature** — 撤回该 feature 的可用性 (e.g. 切人格失败时不让切;授权失败时不让开 Context Awareness) | 这些是关键路径, 半成功的状态对用户更糟糕 |
| `tool_audit_log` 写入失败 (Phase C) | **禁用 ToolSub, 安全降级** — runtime 仍可对话, 但所有后续 tool_call 返 `ToolError::AuditFailed`;publish `safety.violation` 通知 | 工具执行不留痕 = 安全审计破产, 宁可丢功能不丢审计 |
| `safety.violation` 写入失败 | 尝试写 `error_logs`;再失败则**保守降级**: SafetyGuard 进入 `scan_failed` 模式 (后续所有 LLM 输出按 fallback 拒答) | 安全 event 写不进 = 监管能力破产, 必须降级 |
| `context_access_log` 写入失败 | **禁用 PermissionService, 拒绝所有 grant 申请** | 隐私审计破产 = 必须拒绝授权 |
| `event_log` (observability) 写入失败 | **降级到 `error_logs` / in-memory ring buffer, 不 panic** | 这是 observability, 不是 source of truth |
| `living.mood_changed` event 派发失败 | **可丢弃** — 下次 lazy recompute 时自然恢复 | 减衰类状态本来就 lazy, 丢一次 broadcast 不影响计算 |
| 其他 transient event (`scheduler.idle_threshold_crossed`) | **可丢弃** | trigger 自然会再来 |

**核心原则**: kernel-level invariant 破坏 (DB lock 持死 / WAL 损坏 / 磁盘满) 才走 panic → Suspend。事件类失败按"是否影响 source of truth"区分: 是 → 局部 fatal, 否 → degrade。

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

### 8.1 Repository Pattern（MVP, Constitution #2 工程落地）

> v1 用 `WriterCap<T>` capability token 编译期保护;reviewer 指出: 类型系统只能约束**你暴露的 API**, 如果暴露 raw `Pool` / `Tx`, 承诺就破。v2 改 **Repository Pattern**, WriterCap 推 P1 hardening。

```rust
/// kernel 内部唯一持有 sqlx::Pool 的模块
mod kernel::db {
    pub(crate) fn pool() -> &'static SqlitePool { ... }
    pub(crate) async fn run_migration(...) -> Result<()> { ... }   // 唯一例外
}

/// 每个 owner table 有自己的 Repository
pub struct ConversationRepo {
    store: Arc<StateStore>,
}

impl ConversationRepo {
    /// 强类型写方法; subsystem 拿到的 repo 只能调这些
    pub async fn insert_conversation(&self, draft: NewConversation) -> Result<ConvId>;
    pub async fn insert_message(&self, msg: NewMessage) -> Result<MessageId>;
    pub async fn update_safety_status(&self, message_id: MessageId, status: SafetyScanStatus) -> Result<()>;
    pub async fn append_rolling_summary(&self, conv_id: &str, summary: &str) -> Result<()>;
    pub async fn set_persona_snapshot(&self, conv_id: &str, snapshot_id: &str) -> Result<()>;
    pub async fn archive(&self, conv_id: &str) -> Result<()>;
    pub async fn delete(&self, conv_id: &str) -> Result<()>;
    pub async fn read_conversation(&self, conv_id: &str) -> Result<ConversationRecord>;
    pub async fn read_recent_messages(&self, conv_id: &str, limit: u32) -> Result<Vec<MessageRecord>>;
}

pub struct PersonaRepo { ... }
pub struct MemoryRepo { ... }
pub struct LivingRepo { ... }
pub struct ProactiveRepo { ... }
pub struct ToolAuditRepo { ... }   // Phase C
pub struct EpisodicRepo { ... }    // Phase C
pub struct EventLogRepo { ... }    // kernel-internal
pub struct PermissionRepo { ... }  // kernel-internal
```

**Boot 期 wire**:

```rust
// kernel/state_store.rs
pub struct StateStore { /* sqlx::Pool privately */ }

impl StateStore {
    pub fn conversation_repo(&self) -> ConversationRepo { ... }
    pub fn persona_repo(&self) -> PersonaRepo { ... }
    pub fn memory_repo(&self) -> MemoryRepo { ... }
    pub fn living_repo(&self) -> LivingRepo { ... }
    pub fn proactive_repo(&self) -> ProactiveRepo { ... }
    pub fn tool_audit_repo(&self) -> ToolAuditRepo { ... }     // Phase C
    pub fn episodic_repo(&self) -> EpisodicRepo { ... }        // Phase C
    pub(crate) fn event_log_repo(&self) -> EventLogRepo { ... } // kernel only
    pub(crate) fn permission_repo(&self) -> PermissionRepo { ... } // kernel only
}

// subsystems/conversation/service.rs
pub struct ConversationService {
    repo: ConversationRepo,   // owner repo
    persona: Arc<dyn PersonaSubsystem>,  // read-only trait
    memory: Arc<dyn MemorySubsystem>,    // read-only trait
    // 拿不到 PersonaRepo / MemoryRepo, 编译期保护
}
```

**P1 hardening** (推迟):

`WriterCap<T: Owned>` + private `KernelSecret`:
- 在 Repository pattern 之上加类型期约束 (Repo 只能用对应 cap 调用)
- 防止"错误地从 kernel 拿到其他 owner 的 repo handle"
- Phase A 不上, 因 Repository pattern 已经把 raw `Pool` 关在 kernel 内, 跨 ownership write 在 IDE/code review 阶段就显眼可见。

#### 8.1.1 Transaction Policy (v3 review-2 新增, P0)

> reviewer P0: Repository pattern 没说清 cross-owner transaction 怎么办。下面是完整契约。

**4 条铁律**:

1. **raw `sqlx::Pool` 仅 `kernel/db` module 可见**: 模块外不能 `use` 到 `Pool` / `Acquire` / `Transaction` 类型 (cargo deny lint + visibility 双重防护)
2. **subsystem 不持 Pool**: subsystem 构造时拿到的是 `Arc<{Owner}Repo>` 自己那一份 + 跨 sub read 通过别人的 `Arc<dyn SomeSubsystem>` trait — 全程无 Pool / Tx
3. **repo 只暴露强类型写方法**: `insert_message(NewMessage)` / `update_safety_status(MessageId, SafetyScanStatus)` 等;**不暴露**通用 `execute(sql: &str)` / `query<T>(sql)`
4. **跨 owner transaction 只能经 `RuntimeUnitOfWork`** (kernel-internal):
   - 用 case: ConversationSub fork_conversation 要同时 INSERT 新 conversation + 复制 messages + UPDATE persona_rebind_audit (跨 3 owner)
   - subsystem 无法获得 `Transaction` 对象, 通过描述性 API 表达意图: `unit_of_work.fork_conversation(source_id, target_snapshot_id) -> Result<NewConvId>`
   - UoW 内部用 raw `sqlx::Transaction`, 在 kernel/db 内完成

**migration 是唯一 raw SQL 例外**: `kernel::db::run_migration` 持有 raw Pool 可执行任意 SQL, 但仅在 Boot.1 时跑, 之后不可用。

```rust
// kernel/db.rs (only place that touches raw Pool)
mod kernel::db {
    pub(crate) fn pool() -> &'static SqlitePool { ... }
    pub(crate) async fn run_migration(...) -> Result<()> { ... }
}

// kernel/state_store.rs
pub trait StateStore: Send + Sync {
    // 各 owner repo (返 Arc, repo 内部用 kernel::db::pool)
    fn conversation_repo(&self) -> Arc<ConversationRepo>;
    fn persona_repo(&self) -> Arc<PersonaRepo>;
    // ... 其他 owner repo ...

    /// 跨 owner transaction 的唯一入口
    fn unit_of_work(&self) -> Arc<dyn RuntimeUnitOfWork>;
}

/// 跨 owner transaction 的描述性 API
/// 每个方法是一个 atomic operation, 内部用 raw sqlx::Transaction
/// subsystem 不能直接构造 Transaction
pub trait RuntimeUnitOfWork: Send + Sync {
    /// Phase A1: fork conversation 跨 conversations + messages + persona_rebind_audit
    async fn fork_conversation(
        &self,
        source_conv_id: &str,
        target_persona_snapshot_id: &str,
        reason: RebindReason,
        actor: ActorId,
    ) -> Result<ConvId, UowError>;

    /// Phase A1: 显式 rebind (SamePersonaOutdated 切到最新版)
    async fn rebind_persona_snapshot(
        &self,
        conv_id: &str,
        new_snapshot_id: &str,
        reason: RebindReason,
        actor: ActorId,
    ) -> Result<(), UowError>;

    /// Phase A1: 加载新 .soulpack 后 commit (persona_snapshot_profiles + personas + persona_snapshot_audit)
    async fn commit_new_snapshot(
        &self,
        persona_id: &str,
        compiled: SoulRuntimeProfile,
        actor: ActorId,
    ) -> Result<SnapshotId, UowError>;

    /// Phase C: tool 执行原子写 (tool_audit_log + tool.executed event_log)
    #[cfg(phase_c)]
    async fn record_tool_execution(
        &self,
        record: ToolExecutionRecord,
    ) -> Result<(), UowError>;
}
```

**为什么 UoW 不是直接暴露 Transaction**:
- 暴露 Tx → subsystem 拿到后能 INSERT 任何表 (即破坏 Single Writer)
- UoW 描述性 method → subsystem 只能调有限几个 atomic operation, 跨 owner 写在 kernel 内部完成
- 每个 UoW method 在 kernel 内部就是 audit / single writer 边界审查点 (新增跨 owner operation 时强制 review)

**Phase 落地节奏**:
- A0: StateStore + ConversationRepo / PersonaRepo / MemoryRepo / KernelConfigKv (无 UoW 即可, 单 owner 写)
- A1: 加 UoW + `fork_conversation` / `rebind_persona_snapshot` / `commit_new_snapshot` 三个 method
- A2: 沿用 A1 UoW, 无新 method
- C: UoW 加 `record_tool_execution`

### 8.2 Kernel 7 Traits（v2 +2 traits）

```rust
//═══════════════════════════════════════════════════════════════════
// 1. SafetyGuard — Constitution #1 + 8-state FSM (§6.6)
//═══════════════════════════════════════════════════════════════════
pub trait SafetyGuard: Send + Sync {
    /// LLM 调用前包装(prefix 强制第一位 + 地区补充); subsystem 拿到的已是包装后
    fn wrap_messages(&self, messages: Vec<ChatMessage>, locale: Locale) -> Vec<ChatMessage>;

    /// 流式增量扫描(快速); 命中违禁 → ScanTokenResult { action: Pass/SoftBlock/HardEnd }
    fn scan_token(&self, partial: &str, accumulated: &str, finished: bool) -> ScanTokenResult;

    /// 流终态全文扫描路径; 8-state FSM 终态决策, actual scan is SafetyPolicy-gated
    fn scan_final(&self, full_text: &str, persona_snapshot_id: &str) -> ScanFinalResult;

    /// 用户输入扫描 (Phase A 简单黑词, 防 prompt injection)
    fn scan_user_input(&self, text: &str) -> ScanFinalResult;
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
// 3. EventBus — §7 已定义
//═══════════════════════════════════════════════════════════════════
pub trait EventBus: Send + Sync {
    fn publish<E: Event>(&self, event: E) -> Result<EventId, EventError>;
    fn subscribe<E: Event>(&self, sub_id: SubscriberId, h: EventHandler<E>);
}

//═══════════════════════════════════════════════════════════════════
// 4. Scheduler — Constitution #4
//═══════════════════════════════════════════════════════════════════
pub trait Scheduler: Send + Sync {
    fn register_cron(&self, spec: CronSpec, h: TickHandler) -> JobId;
    fn register_idle_threshold(&self, ms: u64, h: TickHandler) -> JobId;
    fn register_one_shot(&self, when: Instant, h: TickHandler) -> JobId;
    fn register_periodic(&self, every: Duration, h: TickHandler) -> JobId;
    fn cancel(&self, job: JobId);
    fn pause_all(&self);
    fn resume_all(&self);
}

//═══════════════════════════════════════════════════════════════════
// 5. StateStore — Repository pattern (v2)
//═══════════════════════════════════════════════════════════════════
pub trait StateStore: Send + Sync {
    fn conversation_repo(&self) -> Arc<ConversationRepo>;
    fn persona_repo(&self) -> Arc<PersonaRepo>;
    fn memory_repo(&self) -> Arc<MemoryRepo>;
    fn living_repo(&self) -> Arc<LivingRepo>;
    fn proactive_repo(&self) -> Arc<ProactiveRepo>;
    fn tool_audit_repo(&self) -> Arc<ToolAuditRepo>;   // Phase C
    fn episodic_repo(&self) -> Arc<EpisodicRepo>;       // Phase C
    fn kernel_config(&self) -> &KernelConfigKv;        // config / secrets / consent
}

//═══════════════════════════════════════════════════════════════════
// 6. PermissionService — 🆕 v2 Context Awareness 网关
//    v3 review-2 明确: Phase A0 = DenyOnly stub, 不调任何 OS API
//═══════════════════════════════════════════════════════════════════
pub trait PermissionService: Send + Sync {
    /// 查询 scope 是否已授权; hot path 调用前先 check
    /// Phase A0 实现 (DenyOnly) 永远返 false
    fn is_granted(&self, scope: ContextScope) -> bool;

    /// 用户授权 (设置面板触发 / GrantBroker 升级路径)
    /// Phase A0 实现拒绝所有 grant 请求 (Err::FeatureDisabled), 设置面板显示"未启用"
    async fn grant(&self, scope: ContextScope, by_action: GrantSource) -> Result<()>;
    async fn revoke(&self, scope: ContextScope, by_action: GrantSource) -> Result<()>;

    /// 实际读取上下文
    /// Phase A0 实现永远返 Err::Denied + 写 context_access_log (granted=0)
    /// P1+ 才有真实 OS API 调用 (GetForegroundWindow / getUserMedia / etc.)
    async fn read_context(&self, scope: ContextScope, used_for: &str, actor: SubsystemId)
        -> Result<Option<ContextValue>>;

    fn list_scopes(&self) -> Vec<ContextScopeDescriptor>;
    fn audit_query(&self, since: DateTime, scope_filter: Option<ContextScope>) -> Vec<AuditRecord>;
}

/// Phase A0 唯一实现, 永远拒绝
pub struct DenyOnlyPermissionService {
    audit_repo: Arc<PermissionRepo>,   // 写 context_access_log
}
impl PermissionService for DenyOnlyPermissionService {
    fn is_granted(&self, _: ContextScope) -> bool { false }
    async fn grant(&self, _: ContextScope, _: GrantSource) -> Result<()> {
        Err(PermissionError::FeatureDisabled)
    }
    async fn read_context(&self, scope, used_for, actor) -> Result<Option<ContextValue>> {
        self.audit_repo.append_denied(scope, used_for, actor).await?;
        Err(PermissionError::Denied { scope, reason: "Phase A0: DenyOnly".into() })
    }
    // ...
}

/// CI 静态扫描强制: 任何 PermissionService 实现都不能 import 下列 OS API
/// (cargo deny + grep ban-list 双重防护)
///   ❌ winapi::um::winuser::GetForegroundWindow
///   ❌ winapi::um::winuser::GetWindowTextW
///   ❌ winapi::um::winbase::GetUserGeoID  (用于地区上下文)
///   ❌ web_sys::Navigator::media_devices  (getUserMedia, MediaRecorder)
///   ❌ winapi::um::wingdi::BitBlt          (屏幕截图)
///   ❌ tauri::clipboard 任何 read 操作
/// Phase A0 这些 API 在 src-tauri 整个 crate 都不应出现 (除非 P1+ 启用并隔离到 PermissionService 内部模块)

pub enum ContextScope {
    ForegroundAppName,
    WindowTitle,
    SelectedText,
    MicrophoneAudio,
    ScreenText,
    // P1+ 扩展前必经新决策, 默认全 deny
}

pub enum GrantSource {
    UserSettingsToggle,
    OnboardingFlow,
    GrantBrokerUpgrade,
    SystemDefault,        // 仅 deny 一种
}

//═══════════════════════════════════════════════════════════════════
// 7. GrantBroker — 🆕 v2 Tool 同步授权 request/response (Constitution #13)
//    v3 review-2 明确: Phase A0 = trait + DenyAllGrantBroker / MockGrantBroker only
//                       无 UI modal, 无 persistent cache, 不接 ToolSub
//═══════════════════════════════════════════════════════════════════
pub trait GrantBroker: Send + Sync {
    /// hot path async fn; 必须等用户决定才返回 (UI modal); 超时降级 Denied
    async fn request_tool_grant(
        &self,
        surface: SurfaceId,
        tool_id: &str,
        args_summary: ToolArgsSummary,
        paths: Vec<PathBuf>,
        reason: GrantReason,
        persona_snapshot_id: &str,
    ) -> Result<GrantDecision, GrantError>;

    /// Grant 缓存查询 (Session-scope / PersistentByTool); 命中则免 UI
    /// Phase A0 实现永远返 None
    fn check_cached(&self, tool_id: &str, args_hash: &str) -> Option<GrantDecision>;
}

pub enum GrantDecision {
    AllowOnce,
    AllowSession(SessionId),                  // 本 session 内不再问
    AllowPersistent(ToolId, ScopeNarrowing),  // 写 config; 含 path prefix 限定
    Deny,
    DenyAndDisable,                           // 用户勾"以后这个 tool 不再问 = 永久 deny"
}

pub enum GrantError {
    Timeout(Duration),
    UserDismissed,
    SurfaceUnavailable,  // UI 不在前台无法弹 modal
    FeatureDisabled,     // Phase A0/A1/A2 用 DenyAllGrantBroker 时返此
}

/// Phase A 起 (A0/A1/A2/B): 默认安装的 GrantBroker
/// ToolSub 不存在时永远不会被调用; 即使被调也立刻拒绝
pub struct DenyAllGrantBroker;
impl GrantBroker for DenyAllGrantBroker {
    async fn request_tool_grant(&self, ...) -> Result<GrantDecision, GrantError> {
        Err(GrantError::FeatureDisabled)
    }
    fn check_cached(&self, _: &str, _: &str) -> Option<GrantDecision> { None }
}

/// 测试用: ConversationSub Phase A 测试 / ToolSub Phase C 单测时注入
/// 可预设固定 GrantDecision 序列
pub struct MockGrantBroker {
    decisions: Mutex<VecDeque<GrantDecision>>,
}
impl MockGrantBroker {
    pub fn new(decisions: Vec<GrantDecision>) -> Self { ... }
}
impl GrantBroker for MockGrantBroker {
    async fn request_tool_grant(&self, ...) -> Result<GrantDecision, GrantError> {
        self.decisions.lock().pop_front()
            .map(Ok)
            .unwrap_or(Err(GrantError::FeatureDisabled))
    }
    fn check_cached(&self, _: &str, _: &str) -> Option<GrantDecision> { None }
}

/// Phase C 才实现 (RealGrantBroker), 含 UI modal + persistent cache + 接 ToolSub
```

### 8.3 Subsystem 6 Traits（v2 关键: PersonaSub + ConversationSub snapshot binding）

```rust
//─── 1. PersonaSubsystem (🆕 v2: SoulCompiler + Snapshot binding) ──────────
pub trait PersonaSubsystem: Send + Sync {
    /// 仅用于"新会话默认人格"等;hot path conversation 必须 read_snapshot
    async fn read_active(&self) -> Result<PersonaSummary>;

    /// hot path: conversation 用 persona_snapshot_id 读出来的 runtime profile
    async fn read_snapshot(&self, snapshot_id: &str) -> Result<PersonaSnapshot>;

    /// 从 `.soul/` 目录或 `.soulpack` zip 加载 → SoulCompiler → 写 persona_snapshot_profiles
    async fn load_soul_package(&self, source: SoulPackageSource) -> Result<(PersonaId, SnapshotId)>;

    /// activate: 切换 active persona (新会话默认); **不**改变已有 conversation 绑定
    async fn activate(&self, persona_id: &str) -> Result<SnapshotId>;

    async fn list_all(&self) -> Result<Vec<PersonaMeta>>;
    async fn save(&self, draft: PersonaDraft) -> Result<(PersonaId, Version)>;
    async fn sandbox_chat(&self, draft_soul: SoulPackageSource, input: &str) -> Result<String>;
}

pub enum SoulPackageSource {
    Directory(PathBuf),       // `.soul/`
    Pack(PathBuf),            // `.soulpack` (zip)
    BuiltIn(&'static str),    // "momo" / "joker" / "coach"
    LegacyMd(String),         // P1 兼容: 单 .soul.md → 默认 .soul/ 布局
}

//─── SoulCompiler (kernel-internal, 不暴露独立 trait, 是 PersonaSub 内部模块) ─
pub struct SoulCompiler;
impl SoulCompiler {
    pub fn parse(source: &SoulPackageSource) -> Result<SoulPackage>;
    pub fn validate(pkg: &SoulPackage) -> Result<()>;   // 拒绝危险字段 (permissions/tools)
    pub fn compile(pkg: SoulPackage) -> Result<SoulRuntimeProfile>;
}

pub struct SoulRuntimeProfile {
    pub persona_id: String,
    pub display_name: String,
    pub identity_prompt: String,        // PromptBuilder 用
    pub style_prompt: String,           // ToneShaper 用
    pub initiative_config: InitiativeSoulConfig,  // InitiativeWeights 用
    pub memory_policy: SoulMemoryPolicy, // MemorySub 用
    pub examples: Vec<DialogueExample>,
    pub ui_metadata: SoulUiMetadata,
    pub source_hash: String,             // 源 .soul/ 文件树的 hash
}

pub struct PersonaSnapshot {
    pub id: String,                      // snapshot_id, conversation 绑定锚
    pub persona_id: String,
    pub schema_version: String,
    pub runtime_profile: SoulRuntimeProfile,
    pub created_at: DateTime,
}

//─── 2. MemorySubsystem (v2: Phase A 简化为 KV + window) ────────────────────
pub trait MemorySubsystem: Send + Sync {
    /// Phase A hot path: prompt 拼装时调用; < 5ms; 仅 KV bullets
    async fn retrieve_kv(&self, persona_snapshot_id: &str, scope: KvScope) -> Result<Vec<KvBullet>>;

    /// Phase A: 维护 rolling_summary (可选, LLM 单 call); periodic
    async fn maybe_roll_summary(&self, conv_id: &str) -> Result<Option<String>>;

    /// Phase A: token-aware history 切片
    async fn read_history_window(&self, conv_id: &str, token_budget: u32)
        -> Result<Vec<MessageRecord>>;

    /// Phase A: KV 偏好
    async fn set_fact(&self, persona_snapshot_id: &str, key: &str, value: &str, source: FactSource)
        -> Result<()>;
    async fn get_facts(&self, persona_snapshot_id: &str) -> Result<Vec<KvBullet>>;

    /// Phase C: episodic memory retrieve (FTS5); 不在 Phase A
    #[cfg(phase_c)]
    async fn retrieve_episodic(&self, ctx: RetrievalContext) -> Result<RetrievedMemory>;

    /// Phase C: working → episodic 压缩 (5min periodic tick)
    #[cfg(phase_c)]
    async fn compress_to_episodic(&self) -> Result<u32>;
}

//─── 3. InitiativeSubsystem (Phase B) ─────────────────────────────
pub trait InitiativeSubsystem: Send + Sync {
    /// scheduler.idle_threshold_crossed handler 内
    async fn evaluate_proactive(&self, trigger: ProactiveTrigger) -> Result<EvalResult>;

    /// persona.activated / living.mood_changed handler 内(in-memory 重排)
    fn rescore_candidates(&self);

    async fn record_response(&self, log_id: i64, response: UserResponse) -> Result<()>;
    async fn get_settings(&self) -> Result<InitiativeSettings>;
    async fn set_settings(&self, s: InitiativeSettings) -> Result<()>;
}

//─── 4. ConversationSubsystem (🆕 v2: persona_snapshot binding) ──────────────
pub trait ConversationSubsystem: Send + Sync {
    /// hot path entry: 任一 surface 输入触发
    /// v2 关键: conv_id 必传; 内部读 ConversationStore 拿 persona_snapshot_id
    async fn handle_user_message(
        &self,
        surface: SurfaceId,
        input: String,
        conv_id: String,                      // v2 强制必传
        channel: Channel<StreamEvent>,
    ) -> Result<SendResult>;

    fn cancel(&self, message_id: &str);
    async fn history(&self, conv_id: &str, limit: u32) -> Result<Vec<MessageRecord>>;

    // conversation 管理 (v2: create 必绑 snapshot)
    async fn list_conversations(&self, limit: u32) -> Result<Vec<ConvSummary>>;
    async fn create_conversation(&self, persona_id: &str) -> Result<ConvId>;  // 内部解出 active snapshot
    async fn rebind_persona_snapshot(&self, conv_id: &str, snapshot_id: &str) -> Result<()>;  // 显式重绑
    async fn rename(&self, conv_id: &str, title: &str) -> Result<()>;
    async fn archive(&self, conv_id: &str) -> Result<()>;
    async fn delete(&self, conv_id: &str) -> Result<()>;
    async fn set_active(&self, conv_id: &str) -> Result<()>;
}

//─── 5. ToolSubsystem (Phase C, GrantBroker 接入) ────────────────────────────
pub trait ToolSubsystem: Send + Sync {
    /// hot path: ConversationSub 在 LLM emit tool_call 后调用 (Phase C)
    async fn execute(
        &self,
        tool_id: &str,
        args: ToolArgs,
        ctx: ToolContext,   // 含 persona_snapshot_id / conv_id / surface
    ) -> Result<ToolResult, ToolError>;

    fn list_available(&self) -> Vec<ToolDefinition>;
    fn whitelist(&self) -> &PathWhitelist;
}

// ToolSub.execute 内部:
//   1. lookup tool by id
//   2. validate args via JSON Schema
//   3. resolve path (canonicalize, 防 ..)
//   4. check whitelist + denylist (硬拒早返 ToolError::Denied)
//   5. GrantBroker.check_cached → 命中跳到 7
//   6. GrantBroker.request_tool_grant(...).await → 若 Deny 返 ToolError::Denied
//   7. execute with bounded resources
//   8. write tool_audit_repo + publish tool.executed
//   9. return ToolResult

//─── 6. LivingSubsystem (Phase B) ─────────────────────────────
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
         │                KERNEL (Boot 1-7)                     │
         │  Migration → StateStore → SafetyGuard →              │
         │  PermissionService → GrantBroker →                   │
         │  EventBus → Scheduler → LifecycleManager             │
         └─┬──────────┬──────────┬──────────┬──────────┬───────┘
           │ all sub  │ Conver-  │ all sub  │ Init/Mem │ all sub
           │ get repo │ sation   │ subscribe│ schedule │ check state
           │ + handle │ + Tool   │ events   │ tick     │
           ↓          ↓          ↓          ↓          ↓
   ┌─────────────────────────────────────────────────────────┐
   │            SUBSYSTEMS (Boot 8, parallel init)            │
   │                                                          │
   │  PersonaSub  ─→ (read snapshot) ─→ ConversationSub ←ToolSub
   │  + SoulCompiler                       │  ↓                │
   │       ↑                               │  ↓ (call execute) │
   │  InitiativeSub ←(read mood)─ LivingSub                   │
   │       ↑                                                   │
   │   MemorySub (Phase A: KV+window; Phase C: episodic+FTS5) │
   │                                                           │
   │  ToolSub ─→ GrantBroker (sync req/resp) ─→ user UI       │
   │                                                           │
   │  ※ 箭头 = sync read trait 调用方向                        │
   │  ※ 跨 sub write 全部经 EventBus(图未画, 见 §7)          │
   └──────────────────────────────────────────────────────────┘
              ↑                                  ↑
              │ Soul read 6 subsystem            │ Surfaces read 6 subsystem
              │ + write nothing                  │ + 通过 IPC 调 trait
              │ + 黑名单 PermissionService /     │ + GrantBroker modal
              │   GrantBroker (Constitution #11)│
              │                                  │
   ┌──────────────────────────┐   ┌──────────────────────────┐
   │       SOUL OVERLAY        │   │      SURFACES (Tauri)     │
   │  PromptBuilder            │   │  Pet / Chat / Workspace   │
   │  ToneShaper               │   │  / Tray + Notification    │
   │  InitiativeWeights        │   │                            │
   │  RetrievalRanker (C)      │   │                            │
   └──────────────────────────┘   └──────────────────────────┘
```

### 8.6 Trait 总额清算 (v2)

| 层 | trait 数 | method 总数 |
|---|---|---|
| Kernel | **7** (+2: PermissionService, GrantBroker) | ~30 |
| Subsystem | 6 | ~33 (PersonaSub + ConversationSub 各 +2) |
| Soul | 4 | 4 |
| **总计** | **17** (Constitution #8 上限 17) | **~67** |

每个 method 都是 hot path / async event handler / settings UI 三类之一, 无装饰性方法。

**v2 增量明细**:
- Kernel + PermissionService (6 methods)
- Kernel + GrantBroker (2 methods)
- PersonaSub + read_snapshot / load_soul_package (2 methods)
- ConversationSub + rebind_persona_snapshot (1 method)
- ToolSub.execute 内部加 GrantBroker.check_cached + request (无新 trait method)
- 不新增 trait 但新增 kernel-internal struct: SoulCompiler (静态 fn, 不算 trait)

---

## 9. Memory Schema

> v2 重大变更: 拆 **9.A MVP (Phase A 必做)** vs **9.B P1 (Phase C 推迟)**。reviewer P0: episodic+FTS5+LLM 摘要在 MVP 阶段过重 (provider 未配置 / Ollama 慢 / 摘要失败 / 隐私 / persona scope / summary 安全扫描等 5 个未解风险), Phase A 先解决真实缺口: KV 不进 prompt + history 硬切 N=10。

### 9.A Memory MVP (Phase A2 必做)

**目标**: 用最小开销实现"桌宠记得你"的功能承诺, 同时支持 token-aware history。

**3 个组件 (Phase A2 MUST 仅前两个; 第三 default-off)**:

| 组件 | 介质 | 持久性 | Phase A2 状态 |
|---|---|---|---|
| **Semantic KV** | SQLite `memory` 表 (已存在, 扩展含义) | persistent | **MUST**, 用户主动 set / Soul.memory_policy 提取 long-term fact (Phase A 用户手动为主) |
| **Recent message window (token-aware)** | SQLite `messages` 表 (已存在) | persistent | **MUST**, hot path 时 token-aware 切片 |
| **Rolling summary** | `conversations.rolling_summary` (Phase A2 新加字段) | persistent | **默认关闭 (deterministic placeholder, 不调 LLM)**;自动 LLM rolling summary 推 P1 (见下方"P1 前置门槛") |

**Phase A2 rolling_summary 默认行为**:
- column 存在 (schema 已落), 值为 `NULL` 或 deterministic 占位 `"(rolling_summary disabled in Phase A2)"`
- PromptBuilder 拿到 `NULL` / 占位字符串 → **不拼入 prompt**, 等价于"该会话无 rolling_summary"
- 不调 LLM, 不触发任何 provider request
- Phase A2 不实现 `MemorySub.maybe_roll_summary` 真逻辑, 只留 trait 占位返 `Ok(None)`

**自动 LLM rolling summary 推 P1 的前置门槛 (Constitution #14)**:

| # | 前置 | 实施 |
|---|---|---|
| 1 | **Provider check**: 调用前必须确认 LLM provider 已配置且可用 | LLMProvider.probe() 失败 → 整个 rolling summary feature flag off |
| 2 | **Token budget**: 限制 prompt token + max_tokens output, 不能因摘要拉爆用户账单 | hard cap: prompt ≤ 2000 token / output ≤ 300 token;超出 skip |
| 3 | **SafetyGuard scan**: 摘要 LLM 产物必经 `scan_final` (Constitution #1, 摘要是 LLM 产物可能含违禁) | `SafetyGuard.scan_final(summary, snapshot_id)` 命中 → 整条摘要丢弃, 保留原占位 |
| 4 | **失败 fallback**: 摘要失败 (provider 错 / scan 命中 / 网络断) 静默写 error_logs, **不影响**正常对话 | `Result<Option<String>>` 形式;Err 不传染 hot path |

**Hot path 集成** (Phase A2):

```
ConversationSub.handle_user_message(conv_id, input)
  1. ConversationStore.read(conv_id) → persona_snapshot_id
  2. PersonaSub.read_snapshot(snapshot_id) → SoulRuntimeProfile
  3. MemorySub.retrieve_kv(snapshot_id, scope=current_persona) → KvBullet[]
     例: [{key:"user_pet_name", val:"咪咪"}, {key:"user_job", val:"程序员"}]
  4. MemorySub.read_history_window(conv_id, token_budget=4000) → MessageRecord[]
     - 从最近向前累加, 直到 token_count 触顶
     - Phase A2: rolling_summary 默认 NULL / 占位, 不拼入
     - P1+: 若 rolling_summary 真实存在 (P1 落地后), 拼接为最早一条 system message
  5. PromptBuilder.build(snapshot.identity_prompt, kv_bullets, history) — Phase A2 不拼 summary
     → SafetyGuard.wrap_messages → LLMProvider
  6. (post chat) Phase A2: 无任何摘要任务;P1+: async MemorySub.maybe_roll_summary(conv_id)
```

**KV scope 设计**:

```
key 前缀作隔离:
  global.*           跨人格通用 (e.g. user_nickname, user_locale)
  persona.{id}.*     仅当前人格可见 (e.g. persona.momo.private_topic)
```

Soul `memory_policy.toml` 控制人格能 set 哪些 prefix; PromptBuilder 注入时只读当前 snapshot 可见的 KV。

**MVP scope (Phase A2)**:
- ✅ KV 注入 prompt / token-aware history window
- ✅ persona-scoped KV (人格隔离 user 信息)
- ✅ `rolling_summary` column 落 (schema), 值默认 NULL / deterministic 占位
- ❌ 自动 LLM rolling_summary → 推 P1 (Constitution #14 4 项前置门槛)
- ❌ episodic memory / FTS5 / RetrievalRanker → 推 9.B (Phase C P1)
- ❌ LLM 自动提炼 fact (Phase A2 用户手动 set 为主)
- ❌ embeddings / vector store

### 9.B Memory P1 (Phase C 推迟)

> Phase A 完成后, 若 M3-M5 时间窗有余, 评估是否上 episodic + FTS5。

**3 层 memory 完整设计**:

| 层 | 介质 | 持久性 |
|---|---|---|
| **working_memory** | in-memory `VecDeque<MessageRef>` per conv | transient + flush on shutdown |
| **episodic_memory** | SQLite + FTS5 | persistent |
| **semantic_memory** | SQLite `memory` 表 KV | persistent (Phase A 已上, Phase C 加 LLM 自动提炼) |

**Episodic Schema** (v2 修正 reviewer 指出的 FTS5 rowid 问题):

```sql
CREATE TABLE episodic_memory (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,    -- 🆕 v2: INTEGER 做 FTS5 content_rowid
    id TEXT NOT NULL UNIQUE,                    -- ULID (业务 ID)
    conversation_id TEXT,                       -- 可空 (proactive episode 无 conv)
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL,
    summary TEXT NOT NULL,                      -- 100-300 字 LLM 摘要
    entities TEXT NOT NULL DEFAULT '[]',        -- JSON: ["项目X","猫",...]
    emotional_tags TEXT NOT NULL DEFAULT '[]',  -- JSON: ["happy","concerned",...]
    emotional_weight REAL NOT NULL DEFAULT 0.5, -- [0,1]
    source_message_ids TEXT NOT NULL,           -- JSON ULID[], 回溯原文
    persona_snapshot_id TEXT NOT NULL,          -- 🆕 v2: bind 到稳定 snapshot, 不是 persona_id
    created_at TEXT NOT NULL
);
CREATE INDEX idx_episodic_persona ON episodic_memory(persona_snapshot_id, ended_at DESC);
CREATE INDEX idx_episodic_emotion ON episodic_memory(emotional_weight DESC, ended_at DESC);
CREATE UNIQUE INDEX idx_episodic_id ON episodic_memory(id);

-- v2 修正: content_rowid='rowid' 用 INTEGER (v1 用 'id' 是 TEXT 不符合 FTS5 规范)
CREATE VIRTUAL TABLE episodic_memory_fts USING fts5(
    summary, entities, emotional_tags,
    content='episodic_memory', content_rowid='rowid'
);

CREATE TRIGGER episodic_ai AFTER INSERT ON episodic_memory BEGIN
    INSERT INTO episodic_memory_fts(rowid, summary, entities, emotional_tags)
    VALUES (new.rowid, new.summary, new.entities, new.emotional_tags);
END;
CREATE TRIGGER episodic_ad AFTER DELETE ON episodic_memory BEGIN
    INSERT INTO episodic_memory_fts(episodic_memory_fts, rowid, summary, entities, emotional_tags)
    VALUES('delete', old.rowid, old.summary, old.entities, old.emotional_tags);
END;
CREATE TRIGGER episodic_au AFTER UPDATE ON episodic_memory BEGIN
    INSERT INTO episodic_memory_fts(episodic_memory_fts, rowid, summary, entities, emotional_tags)
    VALUES('delete', old.rowid, old.summary, old.entities, old.emotional_tags);
    INSERT INTO episodic_memory_fts(rowid, summary, entities, emotional_tags)
    VALUES (new.rowid, new.summary, new.entities, new.emotional_tags);
END;
```

**Retrieval Pipeline (Phase C, hot path p50 < 50ms)**:

```
ConversationSub.handle_user_message → MemorySub.retrieve_episodic(ctx)
  1. FTS5 query: MATCH input_keywords + persona_snapshot_id filter, LIMIT 20
  2. score per item:
       fts        = rank()                    -- FTS5 内置 bm25
       recency    = exp(-elapsed_days / 30)   -- 半衰期 30 天
       emotion    = item.emotional_weight
       score      = 0.5·fts + 0.3·recency + 0.2·emotion
  3. Soul.RetrievalRanker.rank(items, ctx, snapshot) → re-ranked
  4. take top K=3 (MVP 固定)
  5. format memory_bullets:
       "我们之前聊过 {summary}(那时你 {emotional_tag})"
  6. return RetrievedMemory { bullets, debug_score }
```

**压缩流程 (Phase C, periodic_maintenance tick, 5min)**:

```
MemorySub.compress_working_to_episodic
  1. 扫所有 working_memory[conv_id], 找 last_activity > 30min 的 conv
  2. 取这段 5-10 turn 拼 raw_dialog
  3. 调 LLM 用 "摘要 persona"(不污染 active persona)生成:
       { summary, entities, emotional_tags, emotional_weight }
  4. SafetyGuard.scan_final(summary) path is required; actual scan is SafetyPolicy-gated.
  5. INSERT episodic_memory + source_message_ids
  6. working_memory[conv_id] compact (保留 last 3 turn for continuity)
```

**Phase C 进入门槛**:
- Phase A KV+window 落地稳定 ≥ 4 周
- 至少 2 个 LLM provider (OpenAI 兼容 + Ollama 本地) 经过 1 周生产观察, 摘要任务的 token 成本 / 延迟可接受
- SafetyGuard scan_final 对 LLM 自身产物 (摘要) 已经历测试
- ADR-027 决议中"用什么 persona 做摘要"已敲定

**P1+ (Phase C 后)**:
- embeddings / vector store
- 跨 conversation entity 链接
- 用户主动 forget
- 多模态 memory item
- memory diff / forget cascade

---

## 10. Initiative Pipeline (MVP, Phase B)

> v2 Phase B = MVP nice-to-have, ~2 周。**默认 idle-only**, 严格不读 OS 上下文 (Constitution #9 Privacy by Default)。

### 10.1 4 种 trigger (默认源, 不含 Context Awareness)

| Trigger | 来源 | 频率 | Privacy |
|---|---|---|---|
| `idle_threshold_crossed` | IdleDetector(GetLastInputInfo) → Scheduler | 跨阈值 1 次 | ✅ 仅本地空闲计数 |
| `living.mood_changed` | LivingSub → EventBus | mood 变化时 | ✅ 内部状态 |
| `task.reminder_fired` | TaskService → EventBus | 用户提醒触发 | ✅ 用户主动设置 |
| `wake.completed` | LifecycleManager → EventBus | wake 完成 | ✅ OS 信号 |

**🆕 v2: Context Awareness 增强 trigger (P1+, 默认全 deny)**:

> 仅在用户**显式授权**了对应 Context Scope 后, InitiativeSub 才能通过 PermissionService 获取这些信号。**默认完全关闭**, 不影响 MVP 主动陪伴功能。

| Trigger | 需要的 ContextScope | 状态 |
|---|---|---|
| `context.app_changed` | ForegroundAppName | P1+ 评估 |
| `context.idle_in_app` | ForegroundAppName + WindowTitle | P1+ 评估 |
| `context.selected_text` | SelectedText (用户右键 "问桌宠") | P1+ |

### 10.2 Evaluation pipeline（async event handler）

```
InitiativeSub.evaluate_proactive(trigger)

  ─── Step 0: hard gates (Constitution #4/6/9, 不可被 Soul 跳过) ─────────
  if in_quiet_hours()                  return Skip("quiet_hours")
  if proactive_today_count >= 4        return Skip("daily_quota")
  if last_fired_within(2.hours)        return Skip("cooldown")
  if user_disabled                     return Skip("user_disabled")
  🆕 if trigger.requires_context_scope:
       if !permission_service.is_granted(scope)
                                       return Skip("context_scope_not_granted")

  ─── Step 1: 生成 candidates ─────────────────────────────────────
  let candidates = match trigger {
      Idle         => [empathy, greeting, banter],
      MoodChanged  => [empathy, comfort],
      ReminderFired=> [gentle_remind],
      WakeCompleted=> [greeting],
  }
  // 🆕 P1+: context-aware triggers 走单独 category, 例 ContextAppChanged => [context_remark]

  ─── Step 2: Soul 加权(only score, not select) ───────────────────
  let snapshot = persona_sub.read_snapshot(active_snapshot_id)  // v2: snapshot not active
  let live     = living_sub.read_current()                       // lazy recompute
  let scored: Vec<(Candidate, f32)> =
      candidates.iter().map(|c| (c, soul.weights.score(c, &snapshot.runtime_profile, &live)))

  ─── Step 3: 选 top 1 + 抽人格模板 ──────────────────────────────
  let chosen   = scored.max_by_score()
  let template = persona_sub.read_offline_template(chosen.category, &snapshot)

  ─── Step 4: Soul ToneShape ─────────────────────────────────────
  let text = soul.tone_shaper.shape(&template, &snapshot.runtime_profile, &live)

  ─── Step 5: log + surface emit ─────────────────────────────────
  proactive_care_log.insert({
      trigger_kind: trigger.kind,
      gate_result: "fired",
      context_scopes_used: [],   // 🆕 默认空; P1 授权后会填
      score_breakdown: { ... },
      ...
  })
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
soul.weights.score(c, snapshot.initiative_config, live) =
    base_weight[category]                       // 0.5
  + snapshot.initiative_config.proactivity * 0.2
  + mood_alignment(c.category, live.mood) * 0.2
  + same_category_24h_count * (-0.1)            // 重复惩罚
```

**v2 MVP scope (Phase B)**:
- ✅ 4 trigger / 4 category / hard gate (含 quiet_hours + daily_quota + cooldown + user_disabled + 🆕 context_scope_not_granted) / Soul score / template+tone / log + context_scopes_used 字段
- 🟡 P1+: LLM-driven candidate generation / context-aware banter / user-defined trigger / context.app_changed 等增强 trigger
- 🔴 不做: 持续观察类 reaction("看你打开了 X 5 分钟后评论") / proactive 多轮深入 / 任何无授权读取 OS 上下文的行为

---

## 11. Tool Capability Model (Phase C P1)

> **v2 重大变更**:
> - 整个 ToolSub 推到 **Phase C P1** (reviewer 推动, 10 周 MVP 时间窗承不住完整 Tool + 沙盒)
> - **GrantBroker 同步 request/response** (不走 EventBus, Constitution #13)
> - **三权限域分离** (Tool / Context Awareness / Soul) — 关键概念修正
> - Phase A 期间 ConversationSub 保留现有 `service.rs:338` 主动忽略 tool_call 行为不变, Phase C 才接入

### 11.1 三权限域分离（v2 关键修正）

> reviewer 指出: 若把 window title / mic 做成 ToolSub 的普通 tool, LLM 可能通过 tool_call 主动请求, Soul/persona 间接扩大隐私权限。**三权限域必须独立**。

| 权限域 | 谁请求 | 谁批准 | 用什么 trait | 何时问 |
|---|---|---|---|---|
| **Tool Permission** | LLM emit tool_call | 用户经 GrantBroker UI modal | `GrantBroker.request_tool_grant` (sync) | 工具调用时 (按 path / 按 args) |
| **Context Awareness** | InitiativeSub / Soul / Surface 读 OS 上下文 | 用户设置面板 (持久授权) | `PermissionService.grant` (设置时) + `is_granted` (调用时) | 设置面板, 一次授权多次使用 |
| **Soul Permission** | (无) | (无) | (Soul 不能扩权 Constitution #11) | — Soul 无权限 |

**关键不变量**:
- Tool 不能调 PermissionService.grant (扩 Context 权限) — tool denylist
- Tool 不能要求 LLM 通过 prompt 引导用户去开 Context Awareness 设置 — Soul 不可越权
- Context Awareness 不能调 ToolSub.execute 反向 — PermissionService 仅产 ContextValue, 不调 tool
- Soul 既不能调 PermissionService.grant 也不能调 GrantBroker.request — `use` 黑名单

### 11.2 MVP 3 个 read-only tool (Phase C)

| Tool | Schema | 输出 | 上限 |
|---|---|---|---|
| `glob` | `{pattern: string}` | `{matches: string[]}` | max 500 matches |
| `grep` | `{pattern: string, path?: string, regex?: bool}` | `{lines: {file, lineno, text}[]}` | max 200 matches / 1000 files |
| `read` | `{path: string, range?: [start,end]}` | `{content, lineCount}` | max 1MB / 10K lines / range default last 2K lines |

**Phase C MUST NOT**:
- `edit` / `write` / `bash` / `shell` (任何 writable / 执行)
- `web_fetch` / `web_search` (隐私边界 + 沙盒外)
- `screenshot` / `read_clipboard` (走 Context Awareness 域, 不是 Tool)
- `get_active_window` (走 Context Awareness 域)

### 11.3 Path Whitelist + Denylist

```
✅ 白名单 (Phase C):
  %APPDATA%\AIDesktopPet\personas\user\**     (用户写的 .soul/ 包)
  %APPDATA%\AIDesktopPet\file_drop\**         (用户拖入的文件)
  assets\game_scenes\**.yaml                  (游戏场景)
  assets\safety\prefix_v1.txt                 (允许 LLM self-reflection 读 prefix)

🔴 硬拒 (白名单内也拒):
  %APPDATA%\AIDesktopPet\app.db                (数据库自己不能读)
  %APPDATA%\AIDesktopPet\secrets\**            (DPAPI 加密区)
  %APPDATA%\AIDesktopPet\persona_snapshot_profiles\**  (编译后的 PersonaSnapshot)
  C:\Windows\ / C:\Program Files\
  **/secrets/** / **/.env* / **/*credentials*
  symlink (不跟随)
  绝对 path 不在白名单根下 (canonicalize 后比对)
```

### 11.4 GrantBroker 同步授权流程（v2 关键）

```rust
ToolSub.execute(tool_id, args, ctx) {
  // Step 1-4: 同步 precheck
  let tool = registry.lookup(tool_id).ok_or(ToolError::UnknownTool)?;
  tool.validate_args(&args)?;
  let paths = tool.resolve_paths(&args)?.into_iter()
      .map(|p| p.canonicalize())
      .collect::<Result<Vec<_>>>()?;
  for p in &paths {
      whitelist.check(p)?;          // 早返 ToolError::Denied
      denylist.check(p)?;
  }

  // Step 5: Grant cache 查询 (Session-scope / PersistentByTool)
  let args_hash = sha256(&args);
  let cached = grant_broker.check_cached(tool_id, &args_hash);
  let decision = match cached {
      Some(d) => d,
      None => {
          // Step 6: 同步 request/response, 等用户决定 (5s timeout)
          grant_broker.request_tool_grant(
              ctx.surface,
              tool_id,
              ToolArgsSummary::from(&args),
              paths.clone(),
              GrantReason::FirstAccess,
              ctx.persona_snapshot_id,
          ).await?   // 返 GrantDecision, 失败 GrantError::Timeout → ToolError::Denied
      }
  };

  match decision {
      GrantDecision::Deny | GrantDecision::DenyAndDisable => {
          audit_log.record(tool_id, args_hash, "denied:user", paths, ...);
          return Err(ToolError::Denied);
      }
      _ => {}  // AllowOnce / AllowSession / AllowPersistent → 继续
  }

  // Step 7-9: execute + audit
  let started = Instant::now();
  let result = tool.execute_bounded(&args, &paths)?;
  audit_log.record(tool_id, args_hash, "ok", paths, started.elapsed());
  event_bus.publish(ToolExecuted { ... });
  Ok(result)
}
```

### 11.5 Audit log schema

```sql
CREATE TABLE tool_audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_id TEXT NOT NULL,
    persona_snapshot_id TEXT NOT NULL,        -- 🆕 v2: bind snapshot
    conv_id TEXT,
    args_hash TEXT NOT NULL,                  -- SHA256 of args JSON
    paths TEXT NOT NULL DEFAULT '[]',         -- JSON 实际访问 path(供 review)
    status TEXT NOT NULL,                     -- 'ok'|'denied:whitelist'|'denied:user'|'denied:timeout'|'error'
    latency_ms INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    grant_kind TEXT NOT NULL,                 -- 'session'|'persistent'|'once'|'cached'
    🆕 grant_decided_at TEXT NOT NULL,         -- 用户决定时刻 (cached 时 = started_at)
    🆕 surface_id TEXT NOT NULL                -- 哪个 surface 触发的 UI
);
CREATE INDEX idx_audit_persona ON tool_audit_log(persona_snapshot_id, started_at DESC);
```

### 11.6 Context Awareness audit log schema

```sql
-- 与 tool_audit_log 平行的独立审计 (P1 框架, Phase A 创建表)
CREATE TABLE context_access_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL,                      -- 'foreground_app_name' / 'window_title' / ...
    granted INTEGER NOT NULL,                  -- 0/1; Phase A 默认全 0 (deny)
    actor TEXT NOT NULL,                       -- 'InitiativeSub' / 'Surface' / 'Soul'
    used_for TEXT NOT NULL,                    -- 调用方说明
    surface_id TEXT,
    retention_policy TEXT NOT NULL DEFAULT 'transient', -- 'transient'|'30d'|'persistent'
    created_at TEXT NOT NULL,
    🆕 permission_granted_at TEXT,             -- 用户授权时刻 (deny 时 NULL)
    🆕 context_captured_at TEXT                -- 实际读取时刻 (deny 时 NULL)
);
CREATE INDEX idx_context_audit_scope ON context_access_log(scope, created_at DESC);
```

### 11.7 Tool 调用的 hot path 集成 (Phase C)

```
ConversationSub.handle_user_message (Conversing sub-state)
  ↓ LLM emits tool_call delta
ConversationSub → state transition → Toolusing
  ↓
ToolSub.execute(tool_id, args, ctx)
  ├ precheck whitelist/denylist (≤ 5ms)
  ├ GrantBroker.check_cached (≤ 1ms 命中) / request (≤ user time)
  │  └ 若需 modal, ConversationSub state → AwaitingGrant
  │  └ 用户决定后回到 Toolusing
  ├ execute (≤ 200ms 含 IO)
  ├ audit log + event publish
  └ return ToolResult or ToolError
  ↓
  result inject as ChatMessage(role=tool, content=result_json)
  ↓
ConversationSub → state transition → Conversing
  ↓
  next LLM iteration with tool result in messages
```

**Phase A 期间** (Phase C 未实施前): ConversationSub 保留现有 `service.rs:338` 注释 "M1 不接 tools, 不会触发; 忽略" 行为, **不**调用 ToolSub, **不**改变 LLM 调用参数 (tools=vec![] 不变)。Phase C 启动时增量改造。

**Phase C scope**:
- ✅ 3 read-only tool / whitelist + denylist / GrantBroker 同步授权 / cache / audit log / hot path 集成 / AwaitingGrant 子态
- 🟡 P1+: ADR-025 完整决议 / Edit/Write/Bash + 撤销机制 / WebFetch+WebSearch / 用户自定义 path
- 🔴 不做: MCP server bridge / 自定义 tool / tool chaining / background tool

---

## 12. Migration Path from AIPET (v2 三阶段重排)

> v1 估算 ~7.5 周完整 Runtime, reviewer 指出单人 6-8 周窗口承不住。v2 拆 **Phase A P0 (~3 周) / Phase B MVP nice (~2 周) / Phase C P1 (deferred)**。Phase A 是任何对外分发版本的 P0 阻塞;Phase B 是 MVP 完整体验;Phase C 视 M4-M5 节奏决定。

### 12.1 现有 service → subsystem 映射

| AIPET 现有文件 | 目标位置 | 改造方式 | Phase + 工作量 |
|---|---|---|---|
| `services/chat/service.rs` | `subsystems/conversation/service.rs` | 实现 ConversationSubsystem trait;hot path 接 SafetyGuard wrap_messages + scan_token/scan_final FSM;persona_snapshot_id 强制读取 + binding;Phase A 期间 tool_call 仍主动忽略 (Phase C 才改) | **A: ~1 周** |
| `services/chat/prompt.rs` | `subsystems/conversation/prompt.rs` + `subsystems/persona/prompt_builder.rs` | `SAFETY_PREFIX = None` → SafetyGuard 注入;build_system_message 接 MemorySub.retrieve_kv + token-aware history | **A: ~3 天** |
| `services/chat/conversation.rs` | `subsystems/conversation/store.rs` + `repo.rs` | 加 persona_id / persona_snapshot_id / rolling_summary 字段读写;backfill 旧会话 | **A: ~2 天** |
| `services/persona.rs` | `subsystems/persona/service.rs` + `soul_compiler.rs` + `snapshot.rs` | 实现 PersonaSubsystem trait;.soul/ 包加载 → SoulCompiler;activate 写 PersonaSnapshot;3 内置人格 momo/joker/coach 迁移到 `.soul/` 目录 | **A: ~1 周** (SoulCompiler + 内置人格迁移是大头) |
| `services/memory.rs` | `subsystems/memory/service.rs` + `prompt_inject.rs` + `history_window.rs` | 扩 KV 含义;实现 retrieve_kv / read_history_window / maybe_roll_summary | **A: ~3 天** |
| `services/db.rs` / `migration.rs` | `kernel/state_store.rs` + Repository 模块 | 加 Repository pattern; raw Pool 私有化 | **A: ~3 天** |
| `services/llm/` | 不动 (L1 Provider 抽象, 不是 subsystem) | 0% | — |
| `services/llm_providers.rs` | 不动 | 0% | — |
| `services/consent.rs` / `consent_gate.rs` | `kernel/lifecycle_manager.rs` gate 函数 | 整合到 Boot.9 gate | **A: ~1 天** |
| `services/onboarding.rs` | 不动 | 0% | — |
| `services/reminder.rs` / `pomodoro.rs` / `todo.rs` | 保留独立 (TaskService) | Phase B 加 EventBus emit `task.reminder_fired` | **B: ~1 天** |
| `services/scheduler.rs` | `kernel/scheduler.rs` | 现有是空骨架, Phase B 实现完整 4 触发类型 | **B: ~3 天** |
| `services/living_pet.rs` | `subsystems/living/service.rs` | tick → lazy aging; mood_changed → EventBus publish | **B: ~5 天** |
| (规划) `services/proactive_care.rs` | `subsystems/initiative/service.rs` | 落地 + EventBus subscribe + Soul score; **默认 idle-only** | **B: ~5 天** |
| (新建) `kernel/permission_service.rs` | — | Phase A stub (默认 deny 全部 Context Scope) + `context_access_log` 表 + UI 入口 (Phase A 显示"未启用") | **A: ~2 天** |
| (新建) `kernel/grant_broker.rs` | — | Phase A stub (无 Tool 时不会被调) + Grant cache 数据结构 | **A: ~1 天** |
| (新建) `subsystems/tool/` | — | 完整 ToolSub + 3 read-only tool + GrantBroker 真接入 + ADR-025 阻塞前置 | **C: ~1.5 周** |
| (新建) `subsystems/memory/episodic.rs` + `ranker.rs` | — | episodic_memory + FTS5 + 摘要 + RetrievalRanker | **C: ~2 周** |
| `services/snap.rs` / `window_*` / `tray.rs` / `shortcuts.rs` | 保留独立 (Surfaces 工具) | 0% | — |
| `services/avatars.rs` / `preferences.rs` / `nickname.rs` | nickname 归 PersonaSub | nickname 归 PersonaSub | **A: ~1 天** |

### 12.2 SQLite migration plan (v3 review-2: cascade + 两步 ALTER)

> reviewer P0 修正: v2 `COALESCE(persona_id, 'momo')` 是 silent backfill, 违反 Constitution #10。v3 用 cascade fallback + LegacyUnknownSnapshot 兜底。

**migration 004 (Phase A1 必走, conversations + persona_snapshot_id 绑定 + LegacyUnknownSnapshot 兜底)**:

```sql
-- ────── 步骤 1: 加 nullable column (SQLite NOT NULL ALTER 受限, 必须两步) ──────
ALTER TABLE conversations ADD COLUMN persona_id TEXT;
ALTER TABLE conversations ADD COLUMN persona_snapshot_id TEXT;
ALTER TABLE conversations ADD COLUMN rolling_summary TEXT DEFAULT NULL;

-- ────── 步骤 2: cascade backfill (按优先级 4 层兜底) ──────

-- ① 已有 persona_id 字段不动 (理论上不该有, 但保留兼容)
-- (skip)

-- ② 从 conversations.metadata JSON 提取 (若有)
UPDATE conversations
   SET persona_id = json_extract(metadata, '$.persona_id')
 WHERE persona_id IS NULL
   AND metadata IS NOT NULL
   AND json_extract(metadata, '$.persona_id') IS NOT NULL;

-- ③ 从该 conversation 的第一条 message.metadata 提取 (若有)
UPDATE conversations AS c
   SET persona_id = (
     SELECT json_extract(m.metadata, '$.persona_id')
       FROM messages m
      WHERE m.conversation_id = c.id
        AND json_extract(m.metadata, '$.persona_id') IS NOT NULL
      ORDER BY m.created_at ASC
      LIMIT 1
   )
 WHERE c.persona_id IS NULL;

-- ④ 仍为 NULL 的 conversation → 绑定 LegacyUnknownPersona
-- (LegacyUnknownPersona 是预置兜底人格, 由 PersonaSub Boot 时确保存在)
UPDATE conversations
   SET persona_id = 'legacy_unknown'
 WHERE persona_id IS NULL;

-- ────── 步骤 3: 解 persona_snapshot_id ──────
-- 对每个 conversation: persona_snapshot_id = 该 persona 在 conv.created_at 时刻的 latest snapshot
-- (或 LegacyUnknownPersona 永久预置 snapshot)
UPDATE conversations AS c
   SET persona_snapshot_id = (
     SELECT s.id
       FROM persona_snapshot_profiles s
      WHERE s.persona_id = c.persona_id
        AND s.created_at <= c.created_at
      ORDER BY s.created_at DESC
      LIMIT 1
   )
 WHERE persona_snapshot_id IS NULL;

-- 兜底: 仍无 snapshot (新装机用户老库) → 用 LegacyUnknownSnapshot
UPDATE conversations
   SET persona_snapshot_id = 'legacy_unknown_snapshot'
 WHERE persona_snapshot_id IS NULL;

-- ────── 步骤 4: 加 NOT NULL constraint (SQLite 路径: 重建表) ──────
-- SQLite ALTER TABLE 不支持原地加 NOT NULL, 需走 "create new + copy + drop + rename"
-- 此处不写完整 DDL (太冗长), 由 MigrationService 用 builder helper 实施

-- messages 表加 safety 状态 (无 backfill 需求)
ALTER TABLE messages ADD COLUMN token_count INTEGER DEFAULT NULL;
ALTER TABLE messages ADD COLUMN safety_scan_status TEXT NOT NULL DEFAULT 'pending';

-- persona_snapshot_profiles 新建 (Phase A1 PersonaSub 落地必需)
CREATE TABLE persona_snapshot_profiles (
    id TEXT PRIMARY KEY,                       -- snapshot_id, ULID
    persona_id TEXT NOT NULL,
    source_hash TEXT NOT NULL,                 -- .soul/ 树的 hash
    schema_version TEXT NOT NULL,
    compiled_profile_json TEXT NOT NULL,       -- SoulRuntimeProfile JSON
    created_at TEXT NOT NULL
);
CREATE INDEX idx_snapshot_persona ON persona_snapshot_profiles(persona_id, created_at DESC);

-- LegacyUnknownPersona 预置 (PersonaSub Boot 时插入若不存在)
-- (此处不写 INSERT, 由 PersonaSub 实施)

-- persona_rebind_audit 新建 (Phase A1 fork/rebind 审计)
CREATE TABLE persona_rebind_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conv_id TEXT NOT NULL,
    from_snapshot_id TEXT,                     -- 可空 (首次绑定无 from)
    to_snapshot_id TEXT NOT NULL,
    action TEXT NOT NULL,                       -- 'create'|'rebind'|'fork'
    reason TEXT,                                -- 用户输入或系统原因
    actor TEXT NOT NULL,                        -- 'user'|'migration'|'system'
    created_at TEXT NOT NULL,
    FOREIGN KEY (conv_id) REFERENCES conversations(id)
);
CREATE INDEX idx_rebind_conv ON persona_rebind_audit(conv_id, created_at DESC);

-- 新增 context_access_log (Phase A0 默认 deny 期间写 reject 记录)
CREATE TABLE context_access_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL,
    granted INTEGER NOT NULL,
    actor TEXT NOT NULL,
    used_for TEXT NOT NULL,
    surface_id TEXT,
    retention_policy TEXT NOT NULL DEFAULT 'transient',
    created_at TEXT NOT NULL,
    permission_granted_at TEXT,
    context_captured_at TEXT
);
CREATE INDEX idx_context_audit_scope ON context_access_log(scope, created_at DESC);

-- error_logs 若不存在, 创建
CREATE TABLE IF NOT EXISTS error_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    level TEXT NOT NULL,
    source TEXT NOT NULL,
    message TEXT NOT NULL,
    details TEXT,
    created_at TEXT NOT NULL
);
```

**LegacyUnknownPersona / LegacyUnknownSnapshot 兜底约定**:

| 项 | 值 |
|---|---|
| `personas.id` | `'legacy_unknown'` |
| `personas.name` | `'未知人格 (历史会话)'` |
| `persona_snapshot_profiles.id` | `'legacy_unknown_snapshot'` |
| identity_prompt | `"你是一个友善、礼貌、安全的助手。本会话来自历史数据, 人格信息无法恢复。"` |
| style_prompt | (中性, 不带特定语气) |
| initiative_config | proactivity=0 (不主动) |
| memory_policy | scope=global only (不访问 persona-scoped KV) |

**migration 005 (Phase B 必走)**:
```sql
ALTER TABLE pet_runtime_state ADD COLUMN last_mood_event_at TEXT;
ALTER TABLE pet_runtime_state ADD COLUMN last_energy_event_at TEXT;

ALTER TABLE proactive_care_log ADD COLUMN trigger_kind TEXT;
ALTER TABLE proactive_care_log ADD COLUMN gate_result TEXT;
ALTER TABLE proactive_care_log ADD COLUMN context_scopes_used TEXT DEFAULT '[]';
ALTER TABLE proactive_care_log ADD COLUMN score_breakdown TEXT DEFAULT NULL;

CREATE TABLE event_log (...);
```

**migration 006 (Phase C, P1 启动时)**:
```sql
-- episodic memory + FTS5 (v2: INTEGER rowid)
CREATE TABLE episodic_memory (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    ...
    persona_snapshot_id TEXT NOT NULL,
    ...
);
CREATE VIRTUAL TABLE episodic_memory_fts USING fts5(
    summary, entities, emotional_tags,
    content='episodic_memory', content_rowid='rowid'   -- 🆕 v2: INTEGER
);
-- 3 trigger ...

-- tool_audit_log
CREATE TABLE tool_audit_log (...);
```

迁移前自动 backup db (现有 MigrationService 已支持)。
迁移失败 (任一步报错) 立即恢复 backup, runtime 拒绝启动并报错 — silent partial migration forbidden。

### 12.3 现有 264 cargo test 适配

| 测试位置 | 适配方式 | 风险 |
|---|---|---|
| `services/chat/service.rs::tests` | 改 import 路径 + trait dispatch 实例化 + 加 SafetyGuard mock + persona_snapshot fixture, 断言**多数**不变 | 中 (mock 增多) |
| `services/chat/prompt.rs::tests` | 同上, 加 SafetyGuard.wrap_messages mock + MemorySub.retrieve_kv mock | 低 |
| `services/chat/conversation.rs::tests` | 改 import 路径 + 新字段 backfill | 极低 |
| `services/persona.rs::tests` | trait 实例化 + SoulCompiler fixture (3 内置人格 `.soul/` 目录) | 中 (SoulCompiler 是新代码) |
| `services/living_pet.rs::tests` | **可能需要重写** (tick → lazy aging) | 中 (Phase B 才动) |
| `services/llm/*::tests` | 不动 | 0 |
| 其他 | 不动 | 0 |

**估计**: 264 test 中 Phase A ~40 个需要小幅调整, Phase B ~10 个 living_pet 测试需要重写, Phase C ~20-30 个新 Tool/Memory 测试新增。整体测试套件保持 ≥ 250 pass。

### 12.4 Phase 排期（v3 review-2: Phase A 拆 A0/A1/A2, 共 ~3-3.5 周 MVP + B + C deferred）

```
┌─ Phase A0: Safety & Secrets (~1 周, 任何对外分发版本前 P0 阻塞) ─────────┐
│                                                                            │
│ A0.1 kernel/safety_guard.rs SafetyGuard path + 8-state FSM + Scan Scope │
│       Matrix scope 1+2+3 (user input / stream / final)        ~4 天        │
│ A0.2 kernel/state_store.rs Repository pattern + raw Pool 私有  ~2 天      │
│ A0.3 kernel/permission_service.rs DenyOnlyPermissionService              │
│       + context_access_log 表 + CI 黑名单 OS API           ~1 天          │
│ A0.4 kernel/grant_broker.rs trait + DenyAllGrantBroker                  │
│       + MockGrantBroker (测试用)                            ~0.5 天       │
│ A0.5 DPAPI secrets 表 + CryptoService                       ~2 天         │
│ A0.6 lib.rs Boot 1-7 改 (含新 PermissionService / GrantBroker) ~1 天    │
│ A0.7 ChatService → ConversationSub 接 SafetyGuard.wrap_messages         │
│       + scan FSM + StreamEvent::ReplaceMessage              ~3 天         │
│ A0.8 264 cargo test 适配 (~10 个改)                         ~1 天         │
│                                                                            │
│ A0 DoD (任何对外分发版本必满足):                                          │
│  ✅ SafetyGuard path completeness + SafetyPolicy 4 scope default OFF     │
│  ✅ 8-state FSM 单测覆盖 (8 状态 + disabled + scan_failed 降级)          │
│  ✅ StreamEvent::ReplaceMessage 协议前后端走通                            │
│  ✅ DPAPI secrets 表落地, API Key 不再明文 (CryptoService 实施)         │
│  ✅ 整个 src-tauri crate 不出现 GetForegroundWindow/getUserMedia/etc    │
│     (CI 静态扫描通过)                                                     │
│  ✅ DenyAllGrantBroker + DenyOnlyPermissionService 默认安装                │
│  ✅ 264 test ≥ 250 pass                                                   │
└──────────────────────────────────────────────────────────────────────────┘

┌─ Phase A1: Persona Snapshot & Soul Package (~1.5 周, A0 之上) ─────────┐
│                                                                            │
│ A1.1 ADR-028 Soul Package Format 撰写 + review              ~0.5 周      │
│ A1.2 subsystems/persona/soul_compiler.rs + .soul/ parse + validate      │
│       + compile → SoulRuntimeProfile                        ~3 天         │
│ A1.3 subsystems/persona/snapshot.rs + persona_snapshot_profiles 表 +    │
│       Boot 时确保 LegacyUnknownPersona+Snapshot 预置          ~2 天     │
│ A1.4 SoulPackageImporter (10 条安全规则) + .soulpack zip 解析  ~2 天    │
│ A1.5 内置 3 人格 momo/joker/coach 迁移到 .soul/ 目录布局      ~2 天      │
│ A1.6 ConversationSub.handle_user_message 强制 read_snapshot              │
│       (Constitution #10) + 写 hot path                       ~2 天        │
│ A1.7 ConversationSub.rebind_persona_snapshot + fork_conversation        │
│       + 4 UI 状态枚举 + 判断表 + UoW 三 method               ~3 天      │
│ A1.8 migration 004 cascade backfill + LegacyUnknownSnapshot 兜底       │
│       + persona_rebind_audit 表                              ~2 天        │
│ A1.9 PromptBuilder.build 接 SoulRuntimeProfile               ~1 天        │
│                                                                            │
│ A1 DoD:                                                                   │
│  ✅ persona_snapshot_id NOT NULL on conversations                         │
│  ✅ SoulCompiler 单测覆盖 (parse / validate / compile)                   │
│  ✅ 内置 3 人格已迁移 .soul/ 目录, 现有 conversation 用 LegacyUnknownSnapshot │
│  ✅ hot path 不读 active_persona (CI 静态扫描 + 集成测试)               │
│  ✅ 用户切 active persona / 编辑 .soul/ → 旧 conv snapshot 不变 (集成测试) │
│  ✅ rebind / fork 走 UoW, persona_rebind_audit 记录完整                   │
│  ✅ .soulpack 安全导入 10 条规则单测覆盖 (含 path traversal / symlink / 大小 等 attack case) │
└──────────────────────────────────────────────────────────────────────────┘

┌─ Phase A2: Conversation Memory & History Stability (~0.5-1 周) ────────┐
│                                                                            │
│ A2.1 subsystems/memory/prompt_inject.rs KV → memory_bullets  ~1 天       │
│ A2.2 subsystems/memory/history_window.rs token-aware 切片    ~2 天       │
│ A2.3 ChatService prompt.rs build_system_message 接 MemorySub.retrieve_kv │
│       + history_window;HISTORY_LIMIT N=10 → token budget    ~2 天       │
│ A2.4 conversations.rolling_summary column 落 + 默认 NULL/占位 ~0.5 天   │
│ A2.5 SafetyGuard Scan Scope Matrix scope 4 (memory KV)       ~1 天       │
│                                                                            │
│ A2 DoD:                                                                   │
│  ✅ 用户 set 的 KV 出现在 prompt system message                          │
│  ✅ 长对话不再硬切 N=10, 走 token-aware window                            │
│  ✅ rolling_summary 字段存在但默认关闭 (不调 LLM, prompt 中不出现)       │
│  ✅ KV scope 4 安全扫: KV 中违禁内容不进 prompt                          │
│  ✅ persona-scoped KV 隔离 (人格 A 看不到人格 B 的私密 KV)               │
└──────────────────────────────────────────────────────────────────────────┘

┌─ Phase B: MVP nice-to-have (主动陪伴闭环, ~2 周) ───────────────────────┐
│ (内容同 v2, 见 §14.2)                                                   │
└──────────────────────────────────────────────────────────────────────────┘

┌─ Phase C: P1 deferred (Tool + episodic memory) ─────────────────────────┐
│ (内容同 v2, 见 §14.3; ADR-025 阻塞前置)                                 │
└──────────────────────────────────────────────────────────────────────────┘

合计 MVP (Phase A0+A1+A2+B): ~5-5.5 周, 与 M3 W5-W8 对齐
Phase C: deferred, 视 M4-M5 节奏决定
```

**Phase A0/A1/A2 关键依赖**:
- A0 完全独立 (不依赖 A1/A2), 实施后可单独发布
- A1 依赖 A0 (用 Repository + UoW + SafetyGuard 已落)
- A2 依赖 A1 (用 persona_snapshot_id 取 SoulRuntimeProfile)

**Phase A0 是任何对外分发版本前的 hard gate** — 即使 A1/A2 推迟, A0 也必须先落（DPAPI secrets / SafetyGuard path completeness / SafetyPolicy default OFF + configurable scopes / 8-state FSM 是产品发布的 P0 条件）。

---

## 13. ADR 增量 (v2)

### 13.1 新增 ADR (5 条)

| ADR | 主题 | Phase | 状态 |
|---|---|---|---|
| **ADR-025** | Agent 工具沙盒规则 (path whitelist + denylist + capability + GrantBroker UX + Audit log) | C | 草案 (本 spec §11 提供 MVP 默认), 独立 ADR 文档需撰写; Phase C 阻塞前置 |
| **ADR-030** | Companion Agent Runtime 顶层架构 + MVP Phasing (本 spec v2 摘要 + 四阶段 + 14 Constitution) | A | 本 spec 通过后归档为 ADR-030（编号让位 2026-05-26: ADR-026 已用于 SafetyPolicy 可配置化, 详 [`2026-05-26-safety-policy-configurable-design.md`](2026-05-26-safety-policy-configurable-design.md)） |
| **ADR-027** | Memory MVP vs P1 (Phase A: KV+window+rolling_summary; Phase C: episodic+FTS5+RetrievalRanker+LLM 摘要) | A/C | 本 spec §9 提供完整草案 (含 Phase 分界 + 进入门槛), 独立 ADR 撰写 |
| **🆕 ADR-028** | Persona source format + SoulRuntimeProfile + PersonaSnapshot binding | A | Source format 已由 2026-06-18 contract 降级为未决；后续独立 source-format spec 再定 |
| **🆕 ADR-029** | Context Awareness Permission (默认全 deny + 显式授权 + PermissionService 收口 + context_access_log + 三权限域分离) | A 框架 / P1 实施 | 本 spec §2.4 + §11.1 + §11.6 提供权限模型, 独立 ADR 撰写; MVP 仅落 PermissionService stub + 审计表 + 设置面板 "未启用" 提示 |

### 13.2 Updated ADR (4 条)

| ADR | 原状态 | Updated 内容 |
|---|---|---|
| **ADR-003** | "用户角色定义 `.soul.md` schema v2 + 3 内置人格 + 安全前缀拼装" | Updated 2026-06-18: source format 未决；运行时只依赖 `SoulRuntimeProfile` / `PersonaSnapshot`;Conversation 必须强制绑定 PersonaSnapshot, 严禁 hot path 读 active persona |
| **ADR-006** | "安全前缀 v1.0,通用核心 + 地区补充" | Updated 2026-06-18: SafetyGuard path remains mandatory, while prefix injection and scan behavior are controlled by SafetyPolicy 4 scope toggles (default OFF). FSM is 8-state with `disabled`; `StreamEvent::ReplaceMessage` remains the UI replacement protocol. |
| **ADR-015** | "对话面板三形态架构" | Updated 2026-05-24: ConversationStore 升级为 ConversationSubsystem;三 surface(pet/chat/workspace)共享 data layer 通过 EventBus 多 surface broadcast;**新增 `persona_snapshot_id` 强绑定** — 用户切换 active persona 不污染历史会话, 历史会话风格稳定 |
| **ADR-018** | "LLM 三层抽象 + AgentService 工具调用框架" | Updated 2026-05-24: Layer 2 ChatService → ConversationSubsystem;Layer 3 AgentService → ToolSubsystem (本 spec **Phase C P1** 才接入, 不在 MVP);**Phase A 保留** 现有 `service.rs:338` 主动忽略 tool_call 行为不变 (LLM 调用 tools=vec![]);Phase C 启动时增量改造;沙盒细则推到 ADR-025 |

### 13.3 Superseded ADR

无。v2 是增量, 不推翻已有 24 项 ADR。**ADR-003 / 006 / 015 / 018 是 Updated, 不是 Superseded** (反映演化, 不否定原意)。

---

## 14. MVP scope summary (v3 四阶段)

### 14.1 Phase A0: Safety & Secrets（~1 周, 任何对外分发版本前 P0 阻塞）

**MUST**:
- SafetyGuard ADR-006 真注入 (`SAFETY_PREFIX = None` → kernel 注入)
- SafetyGuard 8-state FSM + Scan Scope Matrix scope 1+2+3 (user input + stream token + final text), actual scan policy-gated
- SafetyGuard 区分 SafetyPrefix vs SafetyScanRules (Constitution #1)
- StreamEvent::ReplaceMessage 协议 (前端按 msg_id 覆盖)
- DPAPI secrets 表 + CryptoService (API Key 不再明文)
- StateStore Repository pattern + raw Pool 私有 + KernelConfigKv
- PermissionService DenyOnlyPermissionService (default deny, 所有 ContextScope) + context_access_log 表 + 设置面板"未启用"
- GrantBroker trait + DenyAllGrantBroker + MockGrantBroker
- LifecycleManager FSM (5 顶层 state)
- Boot 1-7 序列 (含新 PermissionService / GrantBroker init)
- ConversationSub 接 SafetyGuard.wrap_messages + scan FSM
- **MUST (Updated 2026-05-26)**: SafetyPolicy trait + ConfigKvSafetyPolicy + 4 config KV (`safety:prefix_enabled` / `safety:scan_user_input_enabled` / `safety:scan_token_enabled` / `safety:scan_final_enabled`，出厂全 OFF)
- **MUST (Updated 2026-05-26)**: `messages.safety_scan_status` 列真接入 ChatService 主链路 (ConversationRepo 的 `update_safety_status` / `update_message_content_and_status` 真消费, 不再 dead code) — 修复 HIGH-2
- **MUST (Updated 2026-05-26)**: `scan_token` 真接入 ChatService::run_stream on_delta + trailing-window 优化 (N=64 chars, O(window) 替代 O(n²)) + rule_id dedupe HashSet 防震荡 — 修复 HIGH-1 + 合并 [#49](https://github.com/tl0502/APET/issues/49)
- **MUST (Updated 2026-05-26)**: workspace popup sidebar 加第 7 项 "Safety" 4-toggle UI

**SHOULD** (Phase A0 不强制, 但若时间允许可一起做):
- ~~SafetyGuard scan_user_input 简单黑词扫~~ (Updated 2026-05-26: 已上 MUST, 详上方; 规则保持现状 4 黑词 P1 评估扩)
- Boot 8 subsystems 初始 (Phase A1 才完整)

**MUST NOT** (Phase A0 严禁出现):
- ❌ 调用 `GetForegroundWindow` / `GetWindowText` / `getUserMedia` / `MediaRecorder` / `BitBlt` 任何 OS context API (CI 静态扫描)
- ❌ 真 GrantBroker UI modal (Phase C 才上)
- ❌ Tool 实施 (Phase C 才上)
- ❌ 用 EventBus 做 Tool grant request/response (Constitution #13 永禁)
- ❌ SoulCompiler 实施 (Phase A1 才上)
- ❌ episodic memory / FTS5 (Phase C 才上)

### 14.2 Phase A1: Persona Snapshot & Soul Package（~1.5 周, A0 之上）

**MUST**:
- ADR-028 Soul Package Format 撰写 + review 通过
- SoulCompiler (parse + validate + compile → SoulRuntimeProfile)
- SoulValidator 拒绝 manifest `permissions` / `tools` / `safety_prefix` 字段 (Constitution #11)
- SoulPackageImporter 10 条安全规则 (`.soulpack` 解析)
- 内置 3 人格 momo / joker / coach 迁移到 `.soul/` 目录布局
- persona_snapshot_profiles 表 + LegacyUnknownPersona / LegacyUnknownSnapshot 预置
- persona_rebind_audit 表
- ConversationSub.handle_user_message 强制 `read_snapshot` (Constitution #10)
- ConversationSub.rebind_persona_snapshot + fork_conversation (UoW)
- ConvPersonaStatus 4 状态枚举 + 判断表 UI 实施
- RuntimeUnitOfWork (Repository transaction policy)
- migration 004 cascade backfill (existing persona_id → metadata → message metadata → LegacyUnknownSnapshot)
- PromptBuilder.build 接 SoulRuntimeProfile

**SHOULD**:
- UI 显示 `SamePersonaOutdated` / `DifferentFromActivePersona` 提示
- 用户手动 trigger fork / rebind 的 UX

**MUST NOT**:
- ❌ Runtime 静默 UPDATE conversations.persona_snapshot_id (Constitution #10 sub-rule)
- ❌ hot path 读 active_persona (CI 静态扫描)
- ❌ `COALESCE(persona_id, 'momo')` 类 silent backfill (reviewer P0 修正)
- ❌ Snapshot 内冻结 nickname / safety prefix / tool whitelist / mood (carve-out 表)

### 14.3 Phase A2: Conversation Memory & History Stability（~0.5-1 周）

**MUST**:
- MemorySub.retrieve_kv (KV → memory_bullets, persona-scoped)
- MemorySub.read_history_window (token-aware 切片)
- ChatService prompt.rs HISTORY_LIMIT N=10 → token budget
- conversations.rolling_summary column 落 schema, 默认 NULL / deterministic 占位
- SafetyGuard Scan Scope Matrix scope 4 (memory KV)

**SHOULD**:
- KV scope 隔离 (人格 A 看不到 B 的私密 KV) 单测
- token_count 字段在 messages 表 backfill 计算

**MUST NOT**:
- ❌ 自动 LLM rolling summary (推 P1, 必须满足 Constitution #14 4 项前置)
- ❌ episodic memory / FTS5 (推 Phase C)
- ❌ embeddings / vector store

### 14.4 Phase B: MVP nice-to-have（主动陪伴闭环, ~2 周, 看时间窗）

**MUST**:
- EventBus 真接入 (10 event + 失败分级)
- Scheduler 4 触发类型 (cron / idle / one-shot / periodic)
- LivingSub tick → lazy aging 改造
- InitiativeSub 4 trigger + hard gate (含 user_disabled_context_scope) + **idle-only 默认**
- ToneShaper + InitiativeWeights (Soul Overlay)
- event_log 表
- pet_runtime_state + proactive_care_log schema 扩展
- ADR-029 Context Awareness Permission 撰写 (P1 框架, MVP 不实施)

**SHOULD**:
- 主动陪伴 UI 气泡 (Phase B 末)
- proactive_care_log 含完整审计字段

**MUST NOT**:
- ❌ Initiative 读取 OS context (Constitution #9)
- ❌ context_scopes_used 出现非空值 (Phase B 默认全 deny)

### 14.5 Phase C: P1 deferred（Tool + episodic memory）

**MUST** (Phase C 启动时):
- ADR-025 Tool Sandbox 完整决议 + ADR-027 Memory Phase C 决议
- ToolSub (3 read-only: Glob / Grep / Read)
- RealGrantBroker (含 UI modal + persistent cache + 接 ToolSub)
- ConversationSub.handle_user_message tool_call 处理 (`service.rs:338` 改写)
- tool_audit_log 表
- AwaitingGrant Live sub-state
- SafetyGuard Scan Scope Matrix scope 6 (tool result)
- episodic_memory + FTS5 (INTEGER rowid) + LLM 摘要压缩
- RetrievalRanker (Soul Overlay)

**SHOULD**:
- Phase C 启动门槛: Phase A + B 落地稳定 ≥ 4 周

**MUST NOT**:
- ❌ Edit / Write / Bash tool (P1+)
- ❌ WebFetch / WebSearch (隐私边界)
- ❌ Screenshot / clipboard (走 Context Awareness 域, 不是 Tool)

### 14.6 🟡 P1+ 推迟清单（v3 明确不进 MVP）

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
- **Context Awareness 实际启用** (foreground_app / window_title / selected_text / microphone / screen_text 任何一项的实际读取)
- **WriterCap<T> 类型期 token** (Repository pattern 已够, WriterCap 是 hardening)
- **自动 LLM rolling summary** (要 4 项前置满足才上)

### 14.7 🔴 不做清单（MVP + P1 均不做）

- 跨进程 EventBus / 分布式 runtime
- Live config hot reload
- 进程级 hot reload
- Background long-running tool
- Network tool 在沙盒外执行 (WebFetch 暂不做, 因隐私边界考量)
- Persona 自动演化 (持续学习改 `.soul/` 文件, 隐私 + 用户自主权风险)
- Soul 启用 Context Awareness 权限 (Constitution #11 永禁)
- Tool grant 走 EventBus (Constitution #13 永禁)
- **Runtime 静默 rebind conversation.persona_snapshot_id** (Constitution #10 永禁)
- **Snapshot 内冻结 nickname / safety / tool whitelist / mood** (carve-out 表 §2.5.4 永禁)

### 14.8 ~5-5.5 周 MVP timeline (Phase A0+A1+A2+B)

见 §12.4 Phase 排期表。Phase C 视实施期节奏单独决定, 不阻塞 MVP 发布。

---

## 15. 风险与未决 (v2)

### 15.1 已知技术风险

| 风险 | 影响 | 缓解 | Phase |
|---|---|---|---|
| Repository pattern 仍可绕 (subsystem 拿到自己 repo 后将其传给他人 / kernel 暴露 raw Pool) | Single Writer 实质保护变弱 | Phase A 严格 code review;P1 hardening 上 WriterCap 类型期 token | A |
| SoulCompiler schema 演进 (manifest.toml v1 → v2 兼容) | 用户分享 `.soulpack` 跨版本无法用 | SoulValidator 强制 schema_version 字段;PersonaSnapshot 持久 source_hash 不动 | A |
| 3 内置人格 momo/joker/coach 迁移 `.soul/` 目录 | 现有用户已有自定义 `.soul.md` 文件 | Phase A 提供 LegacyMd → `.soul/` 自动 compile 路径 (单 .md 落到默认布局);用户文件保留, runtime 用 compile 后的 PersonaSnapshot | A |
| SafetyGuard 8-state FSM 流式 UI 替换协议 (StreamEvent::ReplaceMessage) 前后端时序 | 用户看到一闪而过的命中内容然后被替换 | scan_token 在 chunk 命中时立即 SoftBlock (本地替换最近 N token, UI 看到 `[审核中…]`);final scan path policy-gated 再判 | A |
| GrantBroker UI modal 在 surface 不在前台时弹不出 | tool 调用阻塞或被 timeout 自动 deny | MVP timeout 5s, 超时返 GrantError::Timeout → ToolError::Denied;Surface 增强: pet window 跳出 modal 自带 focus 抢占 (P1) | C |
| FTS5 中文分词效果 | Memory retrieval 精度低 | Phase C 沿用 SQLite FTS5 默认 unicode61 tokenizer + 关键词重叠;P1 评估 jieba-rs 集成 | C |
| LLM 摘要任务的 token 成本 | 用户账单 | Phase A rolling_summary 仅在阈值触发, 限制 prompt token + max_tokens output;Phase C episodic 仅在 chat session 静默 30min 后触发 | A/C |
| Living tick → lazy aging 改造引入 mood/energy 不连续感 | 用户体验 | lazy formula 设计要保证 "查询时算出来的值" ≈ "tick-based 算出来的值";写充分的 property test | B |
| event_log 增长 | 数据库膨胀 | 30 天 retention + 每次启动 GC + 索引 created_at | B |
| Soul `use` 黑名单的执行方式 | 实施复杂 | MVP 用 cargo deny lint + manual code review;P1 评估 proc macro / rustc plugin | A |

### 15.2 ADR 阻塞依赖

| ADR | 阻塞 | 时机 |
|---|---|---|
| **ADR-025** Tool Sandbox | ToolSubsystem 实施 | **Phase C 阻塞前置** (不在 MVP 路径) |
| **ADR-027** Memory MVP vs P1 | episodic memory 实施 | Phase C 阻塞前置 |
| **ADR-028** Soul Package Format | PersonaSub SoulCompiler | **Phase A 阻塞前置** (~0.5 周提前撰写) |
| **ADR-029** Context Awareness | PermissionService 实施 | Phase A 框架不阻塞 (默认 deny);P1 实际启用前必须完整决议 |

### 15.3 性能风险

- **memory retrieval p50 < 50ms** (Phase C): FTS5 查询 + 100 行排序 + Soul re-rank, 单测验证 1000 episodic 时 < 50ms
- **memory KV inject p50 < 5ms** (Phase A): 单表索引查询, 测试覆盖
- **mood/energy recompute < 5ms**: 纯算数, 不查 DB (依赖 cached pet_runtime_state)
- **多 surface broadcast < 50ms**: Tauri emit 已知性能 < 10ms per surface, 4 surface 序列化 + emit 总和 < 50ms
- **GrantBroker request → response (auto-approve cached)**: < 20ms
- **SafetyGuard scan_token 单 chunk**: < 5ms (黑词表 + 简单 regex);**scan_final 全文**: < 100ms for 8K text
- **SoulCompiler compile**: < 50ms for 单个人格 (含 6 文件 parse + validate + JSON 编码)

### 15.4 安全风险

- **Soul 越权风险**: cargo deny lint + code review 是主要防线;Constitution #11 + #12 双重约束;若黑名单失效, runtime 仍可通过"PermissionService.grant 拒绝 from=Soul"再加一层
- **Tool 沙盒逃逸** (Phase C): ADR-025 path whitelist + denylist + canonicalize 防 `..` 跳出;**symlink 不跟随** (MVP 拒绝)
- **API Key 明文**: M1 期 config 表明文是已知技术债, **Phase A P0** 落地 DPAPI;任何对外分发版本必须先修
- **🆕 Context Awareness 权限扩散**: PermissionService 统一收口防 Soul/Tool 各自申请;`context_access_log` 表所有访问可审计;**默认 deny** 是最强防线
- **🆕 PersonaSnapshot 写入审计**: 用户编辑 persona source 可能含恶意 prompt injection（要求 LLM "忘记安全约束"）；SafetyGuard scan path is SafetyPolicy-gated; PersonaSub still rejects source fields that try to grant permissions/tools/safety_prefix control.

---

## 16. Open Questions（审核者可针对这些发问；v2 标注 reviewer 已答 / 仍待审）

> v1 6 个问题, reviewer 已答 5 个 (✅), v2 新 3 个 (🆕)。

1. ✅ **(已答) Capability token 在 Rust 上的可行性**: reviewer 指出 v1 `WriterCap<T>` 在暴露 raw Pool 时承诺破。
   - **v2 解**: MVP 用 Repository pattern, raw Pool 私有化;WriterCap 推 P1 hardening (Constitution #2 已 Updated)。

2. ✅ **(已答) Soul Overlay stateless 是否够用**: reviewer 未质疑此条;v2 通过 PersonaSnapshot 锚定 + LivingSub.read_current() 注入 short-term state 已够。
   - **v2 保留预设**: Soul stateless 充分;短期 reaction 走 snapshot.runtime_profile + live.

3. ✅ **(已答, 部分撤回) Memory 三层是否冗余**: reviewer 推动 Phase A 简化为 KV + window + rolling_summary, **episodic + working 推 P1**。
   - **v2 解**: §9.A MVP / §9.B P1 拆分。Phase A 不需要 working_memory 独立组件 (即 messages 表最近 N 条 + token cache 等价)。

4. ✅ **(已答, 保留) Initiative quota=4/day 是否合理**: reviewer 未质疑。
   - **v2 保留预设**: settings 暴露, 默认 4 来自 ADR-006。

5. ✅ **(已答, 保留 + 加 1) 9 个 event 是否够**: reviewer 未质疑数量, 但**纠正概念**: `tool.grant_request`/`tool.grant_response` 不应是 event (是 GrantBroker sync request/response)。
   - **v2 解**: event 数 9 → 10 (加 `context.permission_changed`);GrantBroker 是 trait 不是 event (Constitution #13)。

6. ✅ **(已答) 6 subsystem 是否够**: reviewer 未质疑数量, 推动 **Kernel 5 → 7** (加 PermissionService + GrantBroker)。
   - **v2 解**: Subsystem 仍 6 件;Kernel 扩到 7 件;trait cap 15 → 17。

### 🆕 v2 新 Open Questions (审核者重点关注)

7. 🆕 **Phase A 3 周窗是否够 PersonaSnapshot runtime contract + Repository pattern + SafetyGuard 8-state FSM 一次到位?**
   - 作者预设(待审核): SoulCompiler + 内置人格迁移是最大不确定项 (~1 周);若超时, 优先级排序: SafetyGuard FSM > DPAPI > persona_snapshot binding > Memory KV > SoulCompiler 落地。SoulCompiler 简化为 `.soul.md` 单文件 → 默认 `.soul/` 布局 (LegacyMd compile 路径), 内置 3 人格暂不显式拆多文件, P1 再做完整目录布局编辑器。

8. 🆕 **Phase C 完全 deferred 后, M5 W9-10 LLMGameRunner 游戏依赖 Tool 吗?**
   - 作者预设(待审核): M5 5 个游戏中"故事接龙"+"角色扮演"是纯 ConversationSub LLM 调用, 不依赖 Tool;3 个本地游戏与 Tool 无关。所以 Phase C deferred 不阻塞 M5。若 P1+ 加"LLM 写诗、画 ASCII"等需要 Tool 的游戏再启动 Phase C。

9. 🆕 **PermissionService 在 Phase A 全 deny 状态下, 用户隐私设置面板该不该显示这些 Context Scope?**
   - 作者预设(待审核): 应显示 (灰色 + "需在 Phase C/P1 启用" 提示), 让用户预期产品路线。若不显示, P1 启动时用户首次看到会被吓到。

---

## 17. Glossary（术语表,审核者必读）

> v2 +9 新术语标 🆕。

| 术语 | 定义 | 来源 |
|---|---|---|
| **AIPET** | 本项目代号,"AI Desktop Pet" | 内部 |
| **Companion Agent Runtime** | 本 spec 设计的运行时, AIPET 的内核 | 本 spec |
| **Core / Soul 双层** | rational vs expressive 分离 | First Principle (§2.1) |
| **Kernel** | 7 件套 hard-isolated 基础设施 (v2 +2: PermissionService, GrantBroker) | §4.2 |
| **Subsystem** | 6 件套可独立演进的功能模块 | §4.3 |
| **Soul Overlay** | 4 件套 stateless 包装层 | §4.4 |
| **Surface** | UI 表面(pet/chat/workspace/tray) | §4.5 |
| **Hot path** | user input → reply 全程的同步执行链 | §5.2 |
| **Lazy aging** | mood/energy 等不 tick, 按时间衰减重算 | §6.7 |
| **Living tick** | (本 spec 中**不存在** —— Living 走 lazy) | (反面参照) |
| **Working / Episodic / Semantic memory** | 三层 memory;v2 Phase A 仅 semantic KV + window, Phase C 才补 working/episodic | §9 |
| **`.soul.md`** | v1 用户编辑的角色定义文件;**v2 简化为 SoulCompiler 的 LegacyMd 输入** | docs/persona/persona-design.md |
| **OpenClaw** | 开源 AI agent 项目, 验证 `SOUL.md` 模式可行 | github.com/openclaw |
| **VRM** | 3D 模型标准(VRoid + UniVRM),AIPET 桌宠形象格式 | ADR-002 |
| **Tauri** | Rust + WebView2 桌面框架,体积小 | docs/architecture |
| **ADR** | Architecture Decision Record, 三句话决策记录 | docs/decisions.md |
| **FTS5** | SQLite 全文检索扩展 | sqlite.org |
| **DPAPI** | Windows Data Protection API, secrets 加密 | architecture §8 |
| **LLM-OS** | Karpathy 提议的"LLM 即操作系统"架构观 | Karpathy 2023 |
| **Repository pattern** | v2 替代 v1 WriterCap 的 MVP 数据访问模式 | §8.1 |
| **🆕 PermissionService** | Kernel 件 6: Context Awareness 权限网关 (默认 deny) | §2.4 / §8.2 |
| **🆕 GrantBroker** | Kernel 件 7: Tool 同步授权 request/response (非 EventBus) | §2.7 / §8.2 |
| **🆕 Context Awareness** | 读取 OS 上下文 (app/window/selected/mic/screen) 的能力总称, 默认全 deny | §2.4 / ADR-029 |
| **🆕 ContextScope** | Context Awareness 的独立维度 (ForegroundAppName / WindowTitle / SelectedText / MicrophoneAudio / ScreenText) | §8.2 |
| **🆕 Soul Package** | `.soul/` 多文件目录, manifest+identity+style+initiative+memory+examples | §2.6 / ADR-028 |
| **🆕 `.soulpack`** | Soul Package 的 zip 分发格式 | §2.6 |
| **🆕 SoulCompiler** | PersonaSub 内部模块: `.soul/` → SoulRuntimeProfile + PersonaSnapshot | §2.6 / §8.3 |
| **🆕 SoulRuntimeProfile** | SoulCompiler 输出, 含 identity_prompt + style_prompt + initiative_config + memory_policy + examples | §8.3 |
| **🆕 PersonaSnapshot** | Conversation 绑定的稳定锚, 包含 SoulRuntimeProfile + source_hash, 写入 persona_snapshot_profiles | §2.5 / §8.3 |
| **🆕 SafetyGuard 8-state FSM** | pending / streaming / stream_soft_blocked / final_ok / final_redacted / final_blocked / scan_failed / disabled | §6.6 |

---

> **End of spec**. 6-8 周后形成第一可工作版本。各子系统内部细节 spec 后续单独提交。
