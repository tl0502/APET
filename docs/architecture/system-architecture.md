---
title: AI 桌宠 系统架构设计
updated: 2026-05-06
related:
  - ../requirements/prd.md
  - ../requirements/flows.md
  - ../persona/persona-design.md
  - ../decisions.md
---

# AI 桌宠 系统架构设计 v1.1

- 适用阶段：MVP **实施期**（M1 起作为唯一权威架构源，与 PRD 基线对齐）。
- 关联：15 项 ADR（详见 [decisions.md](../decisions.md)）；本文档关键决策直接引用编号。

> **关于本版本**：v1.0 是 v0.1 → v0.4 + M0 14 项 ADR 决策结果的"压平基线"，所有"v0.X 沿用 / 新增"等增量话术已展开，文档以连续叙事呈现实施期完整架构；v1.1 是 v1.0 之上的章节级增量（ADR-015 决策落地），不再压平。历史版本演化见 [CHANGELOG.md](../CHANGELOG.md)。

## 变更摘要

### v1.2（2026-05-24）

实施期 M2 W3-W4 经 ADR-020 Updated 2026-05-18/20（磁吸 partial mesh + Rust solver）+ ADR-021 Accepted + Updated 2026-05-21（砍 dockview 改三栏手写）+ Updated 2026-05-22（L 型 chrome 框）+ ADR-022（PopupSidebar）+ ADR-024（dark / light surface 阶梯）后增量更新：

- §2.2 窗口模型整段重写：删 `hub` / `settings` / `workshop` 行（被 workspace shell 取代或迁入）；加 `workspace` / `pomodoro` 行；保留 `pet` / `chat` / `onboarding` / `game_room` / `tray-menu`
- §3.1 模块清单整段重写：删 ChatPanelView2 / ChatPanelView3 / HubChatTab 三 view 行（被 ADR-021 三栏壳取代）；新增 WorkspaceShell / ChatBody / ChatThreadPane / ConversationListPane / SnapService（Rust + 前端）/ TrayService / WindowActions+State / ConsentGate / PreferencesService / AvatarsService / OnboardingReminders 等工程演化产物
- §6.1 LLMProvider trait `impl Stream` 形态保留为字面参考，*Superseded by ADR-018*（已加跳转）

未变更：§0 / §1 / §2.1 / §2.3 / §3.2 / §4 / §5 / §6.2-§16 与 v1.1 一致。

### v1.1（2026-05-02）

实施期 M1 D3 经 ADR-015 Accepted 后增量更新：

- §2.2 窗口模型：加 `hub` 行（M4）；`chat` 行注释磁吸/断开两态；`pet` 行注释含控制按钮区子组件（M2）
- §3.1 模块清单：`ChatPanel` 拆为 ChatPanelView2 / ChatPanelView3 / HubChatTab 三 view 实现；新增 `ConversationStore` view-agnostic service
- §4 SQLite schema：`conversations` 表加 `title` / `archived` 字段（用户多 conversation 命名 + 归档，M3 B.3.d）
- §5.1 IPC：`chat.send` 强调 `conversation_id` 必填（原选填）；新增 `conversation.list / create / rename / archive / delete / activate` 6 命令
- §15 路线图：列出 B.3.a-f 跨 M1-M5 切片

未变更：§0 / §1 / §2.1 / §2.3 / §3.2 / §4 其余表 / §5.2 / §5.3 / §6-§14 与 v1.0 一致。

## 0. 关键技术栈速查(M1 第一天即用)

```
┌─ 主进程 (Rust)
│   - Tauri 2.x
│   - tokio (async runtime)
│   - rusqlite (SQLite + WAL + JSON1)
│   - reqwest (HTTP 客户端,LLM Provider)
│   - tauri-plugin-global-shortcut (摸鱼/对话快捷键)
│   - tauri-plugin-updater (M5 自测期不签名,见 ADR-013)
│   - windows crate (DPAPI / GetLastInputInfo / RAWINPUT)
│
├─ 前端 (WebView2)
│   - Vue 3.4+ + TypeScript 5+ (ADR-001)
│   - Pinia (状态管理)
│   - Vite (构建)
│   - 组件库:Naive UI 或 Element Plus(M1 第一天 spike 后定)
│   - Three.js + @pixiv/three-vrm (VRM 3D 桌宠渲染,ADR-002;原 Live2D Cubism 4 路线已 Superseded)
│   - HTML5 Audio (声音表情播放,ADR-010)
│
├─ 数据层
│   - SQLite + WAL
│   - schema_version: 3 (M5 后可升 4)
│   - Windows DPAPI 加密 secrets
│
└─ 资源
    - personas/_builtin/{momo,joker,coach}.soul.md (ADR-009)
    - assets/voice_packs/default/*.ogg (12-20 条,自录,ADR-010)
    - assets/accessories/*.png + manifest.json
    - assets/game_scenes/{story_relay,cafe_owner}.yaml (ADR-007)
    - assets/safety/prefix_v1.txt + 地区补充 (ADR-006)
    - assets/onboarding/soul_pledge_v1.txt (ADR-008)
    - assets/legal/data_policy_v1.md (ADR-008)
```

### 0.1 LLM Provider Preset 清单(ADR-005 落地)

```yaml
presets:
  - id: openai
    name: OpenAI
    base_url: https://api.openai.com/v1
    model_default: gpt-4o-mini
  - id: deepseek
    name: DeepSeek
    base_url: https://api.deepseek.com
    model_default: deepseek-chat
  - id: moonshot
    name: Moonshot (Kimi)
    base_url: https://api.moonshot.cn/v1
    model_default: moonshot-v1-8k
  - id: qwen
    name: 通义千问
    base_url: https://dashscope.aliyuncs.com/compatible-mode/v1
    model_default: qwen-turbo
  - id: ollama
    name: 本地 Ollama (P1-R3 推荐 qwen2.5:3b)
    base_url: http://localhost:11434/v1
    model_default: qwen2.5:3b
  - id: custom
    name: 自定义...
    base_url: ""
    model_default: ""
```

Onboarding Step 6 不强制要求 API Key(ADR-005);首次唤起对话失败时再引导。

### 0.2 GameRoom 窗口规格(ADR-012)

新增 Tauri 窗口承载所有 5 个游戏(本地 3 + LLM 2):

```rust
WindowBuilder::new(app, "game_room", WindowUrl::App("game.html".into()))
    .title("和我玩…")
    .inner_size(480.0, 600.0)
    .resizable(false)
    .visible(false)  // 默认隐藏,IPC `game.start` 后显示
    .transparent(false)
    .decorations(true)
    .build()?;
```

- 位置:首次显示居中,后续记忆上次位置(`game_room_position` 配置项)。
- 关闭行为:点 X 等价于 `game.end(saveAsDiary?)`(弹出确认)。
- 桌宠窗口在游戏期保持可见(IN_GAME 叠加态),mood 可受游戏内反馈影响。

## 1. 整体架构

```
┌──────────────────────────────────────────────────────────────────┐
│                       AI 桌宠 (Tauri 2.x)                          │
│                                                                    │
│   ┌──────────────────────────────────────────────────────────┐  │
│   │                  WebView 前端层 (Vue 3 + TS)               │  │
│   │  ┌──────────┐ ┌──────┐ ┌────┐ ┌────┐ ┌─────┐ ┌─────┐    │  │
│   │  │PetCanvas │ │ Chat │ │设置│ │工坊│ │装扮 │ │游戏 │    │  │
│   │  │+ hitbox  │ │      │ │    │ │    │ │工坊 │ │舱   │    │  │
│   │  │+ accessor│ │      │ │    │ │    │ │     │ │     │    │  │
│   │  └──────────┘ └──────┘ └────┘ └────┘ └─────┘ └─────┘    │  │
│   └────────────────────────┬─────────────────────────────────┘  │
│                            │ Tauri IPC (invoke / event)           │
│   ┌────────────────────────┴─────────────────────────────────┐  │
│   │              Rust 主进程 (Core Services)                    │  │
│   │                                                              │  │
│   │  核心服务:                                                  │  │
│   │  ┌──────┐┌──────┐┌────────┐┌──────┐┌──────────┐            │  │
│   │  │ Chat ││ Task ││Persona ││Memory││ Crypto   │            │  │
│   │  └──────┘└──────┘└────────┘└──────┘└──────────┘            │  │
│   │  ┌─────────┐┌──────────┐┌─────────┐┌──────────┐             │  │
│   │  │Telemetry││Migration ││ Updater ││NetProbe  │             │  │
│   │  └─────────┘└──────────┘└─────────┘└──────────┘             │  │
│   │                                                              │  │
│   │  生命感与陪伴:                                              │  │
│   │  ┌─────────────┐┌──────────────┐┌────────────────┐          │  │
│   │  │IdleDetector ││ LivingPet    ││ ProactiveCare  │          │  │
│   │  │(GetLastInp- ││ (mood/energy ││ (frequency cap │          │  │
│   │  │ut + RAWINPT)││ + wandering  ││ + tmpl pick)   │          │  │
│   │  │             ││ + DailySched)│└────────────────┘          │  │
│   │  └──────┬──────┘└──────┬───────┘                            │  │
│   │  ┌─────┴──────┐┌──────┴───────┐┌────────────────┐           │  │
│   │  │ BossKey    ││ FileDrop     ││ Milestone      │           │  │
│   │  │ (hide/show)││ (drop event  ││ (+UserAnniv)   │           │  │
│   │  │            ││ → bubbles)   ││                │           │  │
│   │  └────────────┘└──────────────┘└────────────────┘           │  │
│   │                                                              │  │
│   │  交互、装扮、游戏、声音:                                    │  │
│   │  ┌────────────────┐┌────────────────┐┌──────────────┐       │  │
│   │  │ Interaction    ││ VoiceEffect    ││ Wardrobe     │       │  │
│   │  │ Router         ││ Player         ││ Service      │       │  │
│   │  │ (hitbox→action ││ (本地 audio +  ││ (配饰/皮肤   │       │  │
│   │  │  → 反应)       ││  静音时段)     ││  +付费预埋)  │       │  │
│   │  └────────────────┘└────────────────┘└──────────────┘       │  │
│   │  ┌──────────────────────────────────────────────┐           │  │
│   │  │ GameEngine                                    │           │  │
│   │  │  ├ LocalGameRunner (RPS / GuessNumber /       │           │  │
│   │  │  │   WordChain)                               │           │  │
│   │  │  └ LLMGameRunner   (StoryRelay / RolePlay)    │           │  │
│   │  │  → 共用 SecurityGuard + token 上限            │           │  │
│   │  └──────────────────────────────────────────────┘           │  │
│   │  ┌─────────────────┐                                         │  │
│   │  │ Nickname        │  (扩展 MemoryService 的轻量 facade)     │  │
│   │  │ Service         │                                          │  │
│   │  └─────────────────┘                                         │  │
│   └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│        ┌─────────────────────┼─────────────────────┐               │
│        ↓                     ↓                     ↓               │
│   云模型 API            操作系统 API          自动更新服务           │
│  (OpenAI/兼容/...)    (托盘/通知/快捷键)     (tauri-updater)        │
└──────────────────────────────────────────────────────────────────┘
```

### 1.1 关键设计决策

| 决策 | 选择 | 理由 / ADR |
|---|---|---|
| 桌面框架 | Tauri 2.x | 体积小(~20MB)、内存占用低、Rust 主进程安全 |
| 前端框架 | Vue 3 + TS + Pinia + Vite | ADR-001 |
| 桌宠渲染 | Three.js + @pixiv/three-vrm(VRM 3D) | ADR-002(原 Live2D 已 Superseded,M0 末因 Cubism Core 6 不兼容切换) |
| 数据存储 | SQLite WAL + 文件系统 | 离线优先,无服务端依赖 |
| 进程模型 | 单进程多窗口 | 内存预算下唯一选择(GameRoom 例外) |
| LLM 调用 | 主进程发起,前端不直连 | API Key 不进 WebView |
| 加密 | Windows DPAPI(用户账户绑定) | 不需要二次密码 |
| 通信 | Tauri IPC(invoke + event) | 内置类型化 |
| 默认 Provider | 零默认 + 6 个 preset | ADR-005 |
| 安全前缀 | 通用核心 + 地区补充 v1.0 | ADR-006 |
| 配饰管线 | VRM humanoid bone attach(原 Live2D 插槽 Superseded) | ADR-003 |
| 物理动作 | 12 个核心动作 ID | ADR-004 |
| 装扮 schema | 结构化对象 + JSON 列存储 | ADR-011 |
| 游戏 UI | 独立游戏舱窗口 480×600 | ADR-012 |
| 灵魂宣誓 | 温暖叙述版 v1.0 | ADR-008 |
| 内置人格 | 默默 / 阿吉 / 教官 | ADR-009 |
| 音效来源 | 自录(产品配音) | ADR-010 |
| LLM 游戏场景 | 1+1(故事接龙 + 咖啡店老板) | ADR-007 |
| 代码签名 | M5 自测期不签名 + user education | ADR-013 |
| 本地小模型 | P1-R3 调用本地 Ollama | ADR-014 |

## 2. 进程与窗口模型

### 2.1 进程

- **主进程(Rust)**:所有 IO、网络、加密、数据库、调度。
- **WebView 进程(Edge WebView2)**:仅 UI 渲染。
- **不引入额外子进程**(本地小模型 P1-R3 引入时通过 Ollama HTTP 调用,不内嵌)。

### 2.2 窗口(v1.2 重写,ADR-021 后)

| 窗口 | 类型 | 默认状态 | 备注 |
|---|---|---|---|
| `pet` | 透明、置顶、无边框 | 启动后常驻 | 320×320;角色窗 + 控制按钮区子组件(M2 W3,模块 A 延伸);hitbox raycast(§7.1.3) |
| `chat` | 透明、无边框、可拖磁吸 | hidden 启动 + 快捷键唤起 | 形态 2 磁吸浮窗(ADR-020 partial mesh);默认 640×480,允许 [480..1000]×[400..800] 调;`useFocusAOT` 控制 AOT |
| `workspace` | 实色、无边框、自绘 L 型 chrome 框(ADR-021 P3) | hidden 启动 + 用户唤起 | 形态 1 主床 + 5+3 panel 内嵌(chat / settings 3 / tasks 3);1100×720 default / 800×520 min;BrandBar + MasterColumn + SashHandle + DetailColumn 三栏;Profile in-workspace popup(ADR-022);rect 跨重启持久化 |
| `pomodoro` | 实色、无边框、phase-driven AOT | hidden 默认;`pomodoro.start` 后显 | Pomotroid 型 360×480 frameless(#31);全屏 focus / 三入口 / 位置记忆 |
| `onboarding` | 实色、无边框 | hidden 启动;仅首启显 | 480×640;ADR-019 续接 + 5 view 流程(soul-pledge / persona-picker / shortcut-confirm / reminder-intents / summon-invite);完成销毁 |
| `game_room` | 普通、固定 480×600 | hidden 默认;`game.start` 后显示;M5 落地 | ADR-012 独立窗;workspace 游戏 launcher panel(M5)调 `game.start(id)` 启动 |
| `tray-menu` | 系统托盘(非窗口) | 常驻 | 左键双击 → 唤起 workspace;右键 → 菜单 |

**已删除窗口**(v1.1 → v1.2):`hub`(被 workspace 取代,ADR-021)、`settings` 独立窗(panel 迁入 workspace,ADR-021 phase E)、`workshop` 独立窗(panel 迁入 workspace 或留待 M4 重做)。

### 2.3 多显示器与 DPI

- 桌宠位置以**逻辑像素 + 屏幕标识**双键存储,重启或屏幕变化后正确还原。
- VRM 渲染按当前屏幕 DPI 计算 `renderer.setPixelRatio`,避免模糊。
- GameRoom 首次显示居中,后续记忆位置。

## 3. 模块边界

### 3.1 模块清单(v1.2 重写)

| 模块 | 责任 | 主要 API | 引入版本 |
|---|---|---|---|
| **PetRenderer**(前端) | 桌宠状态机渲染、动作、表情、配饰叠加;含控制按钮区子组件(M2 W3,模块 A 延伸) | `playMotion / setExpression / loadAccessories` | M1+ |
| **WorkspaceShell**(前端,v1.2,ADR-021) | 工作台主壳:BrandBar + MasterColumn + SashHandle + DetailColumn 三栏;L 型 chrome 框;rect 跨重启;5+3 panel 容纳(chat / settings 3 / tasks 3) | 通过 useWorkspaceLayoutStore | M2 W3-W4 |
| **ChatBody**(前端,v1.2) | chat 业务壳层:ConversationListPane + ChatThreadPane 双 pane 组装(磁吸窗用) | 共享 ConversationStore | M2 W3-W4 |
| **ChatThreadPane**(前端,v1.2) | chat thread 单 pane(content-header + messages + composer);workspace DetailColumn 与磁吸窗 ChatBody 同源 | 共享 ConversationStore | M2 W3-W4 |
| **ConversationListPane**(前端,v1.2) | chat 会话列表单 pane(sidebar + 删除二次确认);workspace MasterColumn 与磁吸窗 ChatBody 同源 | 共享 ConversationStore | M2 W3-W4 |
| **ChatBubbleView**(前端,占位) | 形态 3 漫画对话气泡 view(角色窗内子组件,沉浸式) | 共享 ConversationStore | M5(B.3.f) |
| **ConversationStore**(前端 pinia,v1.1) | view-agnostic 数据层,管理多 conversation + 当前活跃(ADR-015) | `conversation_list / create / rename / archive / delete / activate` | M1 D5 + M3 完整 UI |
| **PopupSidebar / UserProfilePopup**(前端,v1.2,ADR-022) | in-workspace popup 880×580 overlay + flat sidebar nav(profile / account / privacy / notifications / help / about) | 共享 useUserPopupStore | M2 W4 |
| **PetCanvas / VrmAvatarExporter / Cropperjs Avatar**(前端) | VRM 渲染 + 头像导出(#26)+ 用户头像上传裁剪(#25) | `avatars_upload / avatars_export_vrm` | M2 W3 |
| **PersonaWorkshop**(前端,占位) | 人格工坊 GUI(M4 视需求接入 workspace 或独立窗) | 通过 IPC 调 PersonaService | M2 W4 / M4 |
| **GameRoom**(前端,占位) | 游戏舱 UI(独立窗口,ADR-012;workspace launcher panel M5 接入) | 通过 IPC 调 GameEngine | M5 |
| **ChatService**(主进程) | 对话编排、prompt 拼装、流式回复(`tauri::ipc::Channel<StreamEvent>` 流式契约,ADR-018) | `chat_send / chat_cancel / chat_history` | M1+ |
| **PersonaService**(主进程) | `.soul.md` 读写、校验、热切换 | `persona_list / get / save / import / activate` | M1+ |
| **MemoryService**(主进程) | 用户偏好读写、增量更新 | `memory_list / set / delete` | M1+ |
| **NicknameService**(主进程) | 桌宠/用户昵称管理(MemoryService facade)+ 跨窗广播(`nickname:changed`) | `nickname_get / set_pet / set_user / restore` | M1 |
| **AvatarsService**(主进程,v1.2) | 用户头像 / VRM 头像导出(#25/#26);assetProtocol scope `$APPCONFIG/avatars/**` | `avatars_upload_user / avatars_export_vrm / avatars_list` | M2 W3 |
| **ReminderService**(主进程,M2) | 提醒 CRUD + Scheduler 5s polling + OS 通知 + 桌宠气泡 | `reminder_create / list / update / delete / snooze / complete` | M2 W3 |
| **PomodoroService**(主进程,M2) | 番茄 5 IPC + drift 校准 + Scheduler 1s + FOCUS 期协作(hard 打断 / soft 缓冲) | `pomodoro_start / pause / resume / stop / today_stats` | M2 W3 |
| **TodoService**(主进程,M2) | 待办 CRUD + 拖排序 + priority + 批量 + 搜索 + 最小日历(#29) | `todo_create / list / update / complete / breakdown_with_ai` | M2 W3-W4 |
| **OnboardingReminders**(主进程,v1.2) | onboarding step 4 提醒 intent → 实例化 reminders(tx 注入式,lesson §15;模板双写约束 §16) | 内部(由 setup 调) | M2 W3 |
| **OnboardingService / ConsentGate**(主进程,v1.2) | onboarding KV `onboarding:current_step` + 续接(ADR-019);consent.granted 守卫(IPC 前过滤) | `onboarding_complete / onboarding_get_step` / `consent_check_version` | M1+ |
| **Scheduler**(主进程,M2) | 定时器、并发优先级、休眠唤醒恢复(reminder 5s + pomodoro 1s 双引擎) | 内部 | M2 W3 |
| **SnapService**(Rust + 前端 lib,M2,ADR-020) | 磁吸 partial mesh + Forest-Walk Solver;前端 constraint 权威 + Rust 订阅 `WindowEvent::Moved` 批量 set_position | `snap_sync_constraints` + `useSnapWindow` + `useFocusAOT` composable | M2 W3 |
| **TrayService**(主进程,v1.2) | 系统托盘菜单 + 左键双击 toggle workspace + 右键菜单(显示/隐藏 / 设置 / 退出) | 内部(setup 注册) | M1+ |
| **WindowActions / WindowState**(主进程,v1.2) | 多窗显隐 + boss-key 全 hide + rect 跨重启持久化(pet / chat / pomodoro / workspace)+ `window:visibility-changed` 主动 emit(lesson §11) | `window_show / hide / toggle / get_rect / set_rect` | M1+(rect 跨重启 M2 W4) |
| **PreferencesService**(主进程,v1.2) | 通用配置 KV facade(config 表 27 表零迁移原则,lesson §2)+ theme / shortcut 等高频项 | `preferences_get / set / delete / list_by_prefix` | M1+ |
| **LLMProvider**(主进程) | 多供应商抽象、流式接口(callback 取代 Stream,ADR-018) | `provider.chat_stream / ping` | M1+ |
| **LLMProviderConfig**(主进程,v1.2) | 6 preset(OpenAI / DeepSeek / Moonshot / Qwen / Ollama / Custom)+ API Key DPAPI 加密(M3 G 前明文 KV,ADR-018) | `llm_providers_list / get / save / set_api_key / test` | M2 |
| **CryptoService**(主进程,占位) | DPAPI 包装、敏感字段加解密 | `crypto.protect / unprotect` | M3 G |
| **TelemetryService**(主进程,占位) | 埋点收集、本地缓存、补发 | `telemetry.record / flush` | M5 自测期 |
| **MigrationService**(主进程) | DB schema 升级、备份、回滚(plugin-sql migrations preload,lesson §3) | 启动时执行 | M1+ |
| **UpdaterService**(主进程,占位) | tauri-updater 集成 | `updater.check / install` | M3 |
| **NetworkProbe**(主进程,占位) | 网络状态探测、模式切换通知 | event: `network:changed` | M3 |
| **SecurityGuard**(主进程,占位) | 安全前缀注入、内容过滤(M1 占位,M3 G 真注入,ADR-006) | 内部,被 ChatService / LLMGameRunner 调用 | M3 G |
| **IdleDetector**(主进程,占位) | 键鼠空闲 + 键盘 burst 检测 | `current_idle_ms / subscribe` | M3(N.4 在 M2) |
| **LivingPetService**(主进程,M1+) | mood / energy / wandering / DailySchedule + reminder hook + FOCUS / REMIND 期 wander 跳过 | `living_pet_get_state / record_interaction / force_stop / set_feature_enabled` | M1+(mood / energy / DailySchedule 待 M2-M3) |
| **ProactiveCareService**(主进程,占位) | 主动关心调度 + 频率上限 + 文案选择 | `set_enabled / set_quiet_hours / set_idle_threshold / user_response` | M3 |
| **BossKeyService**(主进程,占位) | 摸鱼模式快捷键与窗口可见性(全 hide 含 workspace) | `toggle / rebind / is_hidden` | M2 W4 |
| **FileDropHandler**(主进程,占位) | 文件拖入 preflight + action | `preflight / handle_action` | M3 |
| **MilestoneService**(主进程,占位) | 跨日纪念日 + 用户纪念日检测 | `check_now / list_reached` | M3+(用户纪念日 M4) |
| **InteractionRouter**(主进程,占位) | 物理交互 hitbox → 反应路由 | `dispatch / record_drag_count / reset_drag_state` | M2 W4 |
| **VoiceEffectPlayer**(主进程,占位) | 本地音效播放 + 静音时段 | `play / set_global_mute / set_quiet_hours / set_volume / list_packs` | M4 |
| **WardrobeService**(主进程,占位) | 配饰库 + 节气推送 + 付费预埋 | `list_inventory / equip / unequip_all / current_equipped / check_seasonal` | M4 |
| **GameEngine**(主进程,占位) | 小游戏会话编排 + 安全前缀 + token 上限 | `start / submit / end / list_available` | M5 |

### 3.2 严格依赖方向

```
前端 → IPC → 主进程服务 → 持久化 / 外部
ChatService → SecurityGuard + PersonaService + MemoryService + NicknameService → LLMProvider
LLMGameRunner → SecurityGuard + PersonaService + LLMProvider(prompt 含 game_scenes/<id>.yaml)
PersonaService 不能调用 MemoryService / NicknameService / WardrobeService(防越权);由 ChatService / GameEngine 统一编排
TaskService 与 PersonaService 互不依赖
NicknameService 是 MemoryService 上的轻量 facade;桌宠"知道自己穿了什么"通过 system prompt 中的"当前装扮摘要"注入,而非人格直读 WardrobeService
```

## 4. 数据模型(SQLite Schema)

```sql
-- ==== 元信息 ====
CREATE TABLE schema_version (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);

-- ==== 配置(kv) ====
CREATE TABLE config (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- ==== 加密敏感数据 ====
CREATE TABLE secrets (
  key TEXT PRIMARY KEY,           -- e.g. 'openai.api_key'
  ciphertext BLOB NOT NULL,       -- DPAPI 输出
  updated_at TEXT NOT NULL
);

-- ==== 同意记录 ====
CREATE TABLE consent (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  granted INTEGER NOT NULL DEFAULT 0,
  method TEXT NOT NULL,            -- 'soul_pledge' | 'classic'
  version INTEGER NOT NULL,        -- safety prefix / data policy 版本号
  accepted_at TEXT NOT NULL
);

-- ==== 人格 ====
CREATE TABLE personas (
  id TEXT PRIMARY KEY,            -- slug
  name TEXT NOT NULL,
  version TEXT NOT NULL,
  source TEXT NOT NULL,           -- 'builtin' | 'user' | 'imported'
  file_path TEXT NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE persona_snapshots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  persona_id TEXT NOT NULL,
  version TEXT NOT NULL,
  content TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (persona_id) REFERENCES personas(id) ON DELETE CASCADE
);

-- ==== 记忆 + 昵称 ====
CREATE TABLE memory (
  key TEXT PRIMARY KEY,           -- e.g. 'username', 'wake_time'
  value TEXT NOT NULL,
  source TEXT NOT NULL,           -- 'user_set' | 'inferred'
  updated_at TEXT NOT NULL
);

CREATE TABLE nicknames (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  pet_nickname TEXT,              -- nullable,null 表示用 .soul.md 默认
  pet_nickname_previous TEXT,     -- 上次自定义值,用于"恢复"按钮
  user_nickname TEXT,
  updated_at TEXT NOT NULL
);

-- ==== 对话 ====
CREATE TABLE conversations (
  id TEXT PRIMARY KEY,            -- ULID
  persona_id TEXT NOT NULL,
  title TEXT,                     -- 用户自定义会话名(v1.1 / M3 B.3.d 加;NULL 时 UI 显示"未命名 + 时间")
  archived INTEGER NOT NULL DEFAULT 0,  -- 归档标记(v1.1 / M3 B.3.d 加)
  started_at TEXT NOT NULL,
  last_activity_at TEXT NOT NULL,
  is_sandbox INTEGER NOT NULL DEFAULT 0  -- 试聊沙盒标记
);
CREATE INDEX idx_conversations_active ON conversations(archived, last_activity_at DESC);  -- v1.1

CREATE TABLE messages (
  id TEXT PRIMARY KEY,            -- ULID
  conversation_id TEXT NOT NULL,
  role TEXT NOT NULL,             -- 'user' | 'assistant' | 'system'
  content TEXT NOT NULL,
  mode TEXT NOT NULL,             -- 'online' | 'offline_rule'
  created_at TEXT NOT NULL,
  FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);
CREATE INDEX idx_messages_conv ON messages(conversation_id, created_at);

-- ==== 任务三件套 ====
CREATE TABLE reminders (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  trigger_type TEXT NOT NULL,     -- 'once' | 'daily' | 'weekly' | 'cron'
  trigger_spec TEXT NOT NULL,     -- ISO8601 / cron 表达式
  priority TEXT NOT NULL DEFAULT 'soft',  -- 'soft' | 'hard'
  enabled INTEGER NOT NULL DEFAULT 1,
  snooze_count INTEGER NOT NULL DEFAULT 0,
  next_fire_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE reminder_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  reminder_id TEXT NOT NULL,
  fired_at TEXT NOT NULL,
  action TEXT NOT NULL,           -- 'completed' | 'snoozed' | 'ignored' | 'overdue'
  snooze_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_reminder_history_rid ON reminder_history(reminder_id);

CREATE TABLE todos (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  due_at TEXT,
  status TEXT NOT NULL DEFAULT 'open',  -- 'open' | 'done' | 'cancelled'
  source TEXT NOT NULL,                  -- 'manual' | 'ai_breakdown'
  parent_id TEXT,                        -- 拆解结果的父任务
  created_at TEXT NOT NULL,
  done_at TEXT
);

CREATE TABLE pomodoro_sessions (
  id TEXT PRIMARY KEY,
  focus_min INTEGER NOT NULL,
  rest_min INTEGER NOT NULL,
  status TEXT NOT NULL,           -- 'running' | 'paused' | 'completed' | 'cancelled'
  started_at TEXT NOT NULL,
  ended_at TEXT
);

-- ==== 生命感 ====
CREATE TABLE pet_runtime_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),  -- 单行表
  energy INTEGER NOT NULL DEFAULT 60,
  mood TEXT NOT NULL DEFAULT 'neutral',   -- 不存 transient mood
  last_interaction_at TEXT NOT NULL,
  disabled_features TEXT NOT NULL DEFAULT '[]',  -- JSON array
  updated_at TEXT NOT NULL
);

-- ==== 主动关心日志(频率控制 + KPI) ====
CREATE TABLE proactive_care_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  fired_at TEXT NOT NULL,
  trigger TEXT NOT NULL,          -- 'idle' | 'late_night' | 'no_pomodoro_long' | 'milestone' | 'wardrobe_suggest'
  category TEXT NOT NULL,         -- 'empathy' | 'greeting' | 'gentle_remind' | 'celebration' | 'wardrobe_suggest'
  persona_id TEXT NOT NULL,
  template_idx INTEGER NOT NULL,  -- 命中模板的池内索引
  user_response TEXT,             -- 'clicked' | 'replied' | 'dismissed' | null
  responded_at TEXT
);
CREATE INDEX idx_pcl_fired ON proactive_care_log(fired_at);
-- 7 天保留:每次启动清理 fired_at < now - 7d 的记录

-- ==== 里程碑 + 用户纪念日 ====
CREATE TABLE milestones (
  id TEXT PRIMARY KEY,            -- e.g. 'first_launch_7d' / 'anniversary_birthday_2027'
  category TEXT NOT NULL,         -- 'first_launch'|'streak'|'pomodoro_count'|'todo_count'|'user_anniversary'
  threshold INTEGER NOT NULL,
  reached_at TEXT NOT NULL,
  context TEXT                    -- JSON: 触达时的辅助信息
);

CREATE TABLE user_anniversaries (
  key TEXT PRIMARY KEY,           -- 'birthday' | 'work_start' | 'custom_<ulid>'
  display_name TEXT NOT NULL,
  date_md TEXT NOT NULL,          -- 'MM-DD',年度重复
  created_at TEXT NOT NULL
);

-- ==== 装扮 ====
CREATE TABLE accessories_inventory (
  id TEXT PRIMARY KEY,            -- accessory_id(与资源文件对应)
  unlocked_at TEXT NOT NULL,
  unlock_reason TEXT NOT NULL,    -- 'always'|'seasonal'|'milestone:xxx'|'purchase:sku'|'gift'|'user_upload'
  is_equipped INTEGER NOT NULL DEFAULT 0,
  metadata TEXT                   -- JSON: tier / unlock spec / anchor / etc(ADR-011)
);

CREATE TABLE wardrobe_decisions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  accessory_id TEXT NOT NULL,
  year INTEGER NOT NULL,
  decision TEXT NOT NULL,         -- 'accepted' | 'declined'
  decided_at TEXT NOT NULL,
  UNIQUE(accessory_id, year)
);

-- ==== 声音 ====
CREATE TABLE voice_packs (
  id TEXT PRIMARY KEY,            -- 'default' | 'cute_v1' | 'user_imported_xxx'
  name TEXT NOT NULL,
  source TEXT NOT NULL,           -- 'builtin' | 'user_imported'
  manifest_path TEXT NOT NULL,
  installed_at TEXT NOT NULL
);

CREATE TABLE voice_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  global_mute INTEGER NOT NULL DEFAULT 0,
  volume INTEGER NOT NULL DEFAULT 50,
  quiet_weekdays TEXT NOT NULL DEFAULT '[1,2,3,4,5]',  -- JSON array
  quiet_ranges TEXT NOT NULL DEFAULT '[["09:00","18:00"]]', -- JSON array
  updated_at TEXT NOT NULL
);

-- ==== 游戏 ====
CREATE TABLE game_sessions (
  id TEXT PRIMARY KEY,            -- ULID
  game_id TEXT NOT NULL,
  kind TEXT NOT NULL,             -- 'local' | 'llm'
  started_at TEXT NOT NULL,
  ended_at TEXT,
  result TEXT,                    -- JSON:游戏特定结果
  saved_as_diary INTEGER NOT NULL DEFAULT 0,
  total_tokens INTEGER NOT NULL DEFAULT 0  -- 仅 LLM 游戏
);

CREATE TABLE game_session_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  role TEXT NOT NULL,             -- 'user' | 'assistant' | 'system'
  content TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (session_id) REFERENCES game_sessions(id) ON DELETE CASCADE
);

-- 日记草稿(来自游戏"保留为日记片段",P1-R1 桌宠日记功能消费)
CREATE TABLE diary_drafts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source TEXT NOT NULL,           -- 'game:<game_id>' | 'manual' | ...
  content TEXT NOT NULL,
  created_at TEXT NOT NULL,
  consumed INTEGER NOT NULL DEFAULT 0
);

-- ==== 埋点 + 错误日志 ====
CREATE TABLE telemetry_queue (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_name TEXT NOT NULL,
  schema_version INTEGER NOT NULL,
  payload TEXT NOT NULL,          -- JSON
  created_at TEXT NOT NULL,
  flushed INTEGER NOT NULL DEFAULT 0,
  retry_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_telemetry_unflushed ON telemetry_queue(flushed) WHERE flushed = 0;

CREATE TABLE error_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  level TEXT NOT NULL,            -- 'warn' | 'error' | 'fatal'
  module TEXT NOT NULL,
  message TEXT NOT NULL,
  context TEXT,                   -- JSON
  created_at TEXT NOT NULL
);
```

### 4.1 设计要点

- **ULID 优先**于 UUID(按时间排序、可读性更好)。
- 时间戳统一 ISO8601 字符串。
- 软删除策略:仅对 `messages` 用 `is_deleted` 字段(90 天清理用),其他表硬删除。
- WAL 模式(`PRAGMA journal_mode=WAL`)提高并发与崩溃恢复。

### 4.2 schema 迁移

- `migrations/` 目录下顺序文件:`001_init.sql`(v0.1 schema_version=1) / `002_v0_5_living_pet.sql`(version=2) / `003_v0_6_interaction_extension.sql`(version=3)。M5 后可升 4。
- `MigrationService` 启动时检查 `schema_version`,按序执行未应用的迁移。
- 每次迁移前自动复制 db 到 `backup/db-<schema_v>-<timestamp>.bak`,保留最近 5 个。
- 迁移失败 → 回滚 + 错误上报 + 告知用户"使用前次版本启动"。

## 5. IPC 契约(Tauri Commands & Events)

### 5.1 Commands(前端 → 主进程)

> **命名约束**(Tauri 2.x runtime):IPC command name 仅允许 `[a-zA-Z0-9_]`(下划线),不允许 `.`。本表中点号写法 `chat.send` / `conversation.list` 等是**逻辑分组**形式,实际后端注册名为 snake_case(`chat_send` / `conversation_create` 等;参 `commands/chat.rs` + `commands/llm.rs` 已落地命名)。前端 binding 层(`src/ipc/`)直接调用 snake_case 名字,无需做映射。Event name 则可保留 `:` 分隔(见 §5.2)。

```ts
// ==== 对话 ====
chat.send(input: string, conversationId?: string): Promise<{ messageId: string }>
chat.cancel(messageId: string): Promise<void>
chat.history(conversationId: string, limit: number): Promise<Message[]>

// ==== Conversation 管理(v1.1 新增,ADR-015 ConversationStore)====
conversation.list(): Promise<Array<{ id, title, persona_id, started_at, last_activity_at, archived }>>
conversation.create(payload: { persona_id, title? }): Promise<{ id }>
conversation.rename(id: string, title: string): Promise<void>
conversation.archive(id: string, archived: boolean): Promise<void>
conversation.delete(id: string): Promise<void>      // 物理删除消息 + 会话(用户主动),走 ON DELETE CASCADE
conversation.activate(id: string): Promise<void>    // 设为当前活跃,持久化到 user_state.active_conversation_id

// ==== 人格 ====
persona.list(): Promise<PersonaMeta[]>
persona.get(id: string): Promise<PersonaFull>
persona.save(payload: PersonaSave): Promise<{ id, version }>
persona.import(filePath: string): Promise<{ id, conflict?: 'overwrite'|'rename' }>
persona.export(id: string, includeAssets: boolean): Promise<{ path }>
persona.activate(id: string): Promise<void>
persona.delete(id: string): Promise<void>
persona.sandbox_chat(payload: { draftMd, input }): Promise<string>

// ==== 记忆 + 昵称 ====
memory.list(): Promise<MemoryItem[]>
memory.set(key: string, value: string): Promise<void>
memory.delete(key: string): Promise<void>
memory.clear(): Promise<void>

nickname.get(): Promise<{ pet?: string; user?: string; pet_previous?: string }>
nickname.set_pet(nickname: string | null): Promise<void>      // null = 恢复 .soul.md 默认
nickname.set_user(nickname: string | null): Promise<void>
nickname.restore_pet_previous(): Promise<void>

// ==== 任务 ====
reminder.create / list / update / delete / snooze / complete
pomodoro.start / pause / resume / stop / today_stats
todo.create / list / update / complete / breakdown_with_ai

// ==== 生命感 ====
living_pet.get_state(): Promise<PetRuntimeState>
living_pet.set_feature_enabled(feature: 'wandering'|'mood_icon'|'energy'|'daily_schedule', enabled: boolean): Promise<void>

// ==== 主动关心 ====
proactive_care.get_settings(): Promise<ProactiveCareSettings>
proactive_care.set_enabled(enabled: boolean): Promise<void>
proactive_care.set_quiet_hours(ranges: TimeRange[]): Promise<void>
proactive_care.set_idle_threshold_min(n: number): Promise<void>
proactive_care.respond(logId: number, response: 'clicked'|'replied'|'dismissed'): Promise<void>

// ==== 摸鱼模式 ====
boss_key.toggle(): Promise<{ hidden: boolean }>
boss_key.rebind(shortcut: string): Promise<void>
boss_key.is_hidden(): Promise<boolean>

// ==== 文件拖入 ====
file_drop.preflight(paths: string[]): Promise<FileDropPreflight>
file_drop.handle_action(action: 'summarize'|'explain'|'rename', paths: string[]): Promise<{ messageId }>

// ==== 里程碑 + 用户纪念日 ====
milestone.list_reached(): Promise<Milestone[]>
milestone.check_now(): Promise<Milestone[]>
anniversary.list(): Promise<UserAnniversary[]>
anniversary.add(payload: { displayName, dateMd, key? }): Promise<void>
anniversary.remove(key: string): Promise<void>

// ==== 物理交互 ====
interaction.dispatch(evt: InteractionEvent): Promise<Reaction[]>
interaction.set_n4_enabled(enabled: boolean): Promise<void>

// ==== 声音 ====
voice.play(voiceId: string): Promise<void>            // persona_id 由主进程从当前激活人格自动注入
voice.set_global_mute(mute: boolean): Promise<void>
voice.set_quiet_hours(ranges: TimeRange[], weekdays: Weekday[]): Promise<void>
voice.set_volume(vol: number): Promise<void>
voice.list_packs(): Promise<VoicePackMeta[]>

// ==== 装扮 ====
wardrobe.list_inventory(): Promise<AccessoryMeta[]>   // tier='paid' 在 MVP 期被强制过滤
wardrobe.equip(ids: string[]): Promise<void>
wardrobe.unequip_all(): Promise<void>
wardrobe.current_equipped(): Promise<AccessoryMeta[]>
wardrobe.dismiss_seasonal_for_year(suggestionId: string): Promise<void>

// ==== 小游戏 ====
game.list_available(): Promise<GameMeta[]>
game.start(gameId: string): Promise<{ sessionId: string }>
game.submit(sessionId: string, input: GameInput): Promise<GameOutput>
game.end(sessionId: string, saveAsDiary: boolean): Promise<void>

// ==== 设置 & 安全 & 系统 ====
settings.get / set
secrets.set_api_key(provider: string, key: string): Promise<void>
secrets.test(provider: string): Promise<{ ok: boolean, latency_ms: number }>

app.info(): Promise<AppInfo>
app.export_data(): Promise<{ path: string }>
app.import_data(path: string): Promise<void>
app.delete_all(): Promise<void>
updater.check(): Promise<UpdateInfo>
updater.install(): Promise<void>
```

### 5.2 Events(主进程 → 前端)

> **命名约束**(Tauri 2.x runtime):event name 仅允许 `[a-zA-Z0-9\-/:_]`,不允许 `.`(emit 时会 panic「event emit failed: only alphanumeric, '-', '/', ':', '_' permitted」)。本表 2026-05-04 起统一用 `:` 分隔(与既有 `shortcut:chat` / `tray:show` 约定一致),旧 `xxx.yyy` 命名已 superseded。

```ts
// 对话
'chat:token'         { messageId, delta }              // 流式 token
'chat:done'          { messageId, fullText, latencyMs }
'chat:error'         { messageId, code, message }

// 任务
'reminder:fired'     { reminderId, priority }
'pomodoro:tick'      { sessionId, remainingMs, phase }

// 桌宠状态
'pet:state_changed'      { from, to, sub_from?, sub_to?, overlay_added?, overlay_removed?, reason }
'pet:mood_changed'       { from, to, transient: bool, trigger }
'pet:wandering'          { phase: 'start'|'end', targetX, targetY }
'pet:energy_changed'     { value }                      // 节流,每 1 分钟最多一次
'pet:daily_action'       { time_slot, action_id }

// 物理交互
'pet:interaction_reacted' { hitbox, action_id, voice_id?, mood_change? }
'pet:protest_triggered'   { drag_count, will_revert_in_ms }

// 主动关心 + 里程碑
'proactive_care:fired'      { logId, category, message }
'milestone:reached'         { id, category, message }
'milestone:user_anniversary' { key, display_name }

// 摸鱼 + 文件拖入
'boss_key:toggled'        { hidden }
'file_drop:bubbles_shown' { paths, available }

// 声音 + 装扮
'voice:played'              { voice_id, pack_id }
'voice:muted_by_quiet_hour' { voice_id, reason: 'quiet_hour'|'global_mute' }
'wardrobe:changed'          { equipped: AccessoryMeta[] }
'wardrobe:seasonal_suggest' { suggestion_id, accessory_id, accept_url }

// 游戏
'game:session_started'      { session_id, game_id, kind }
'game:token_budget_warning' { session_id, used, limit }
'game:session_ended'        { session_id, saved_as_diary, total_tokens }

// 昵称 + 网络 + 升级
'nickname:changed'          { which: 'pet'|'user', value }
'network:changed'           { online: boolean, mode: 'online_chat'|'offline_rule' }
'persona:activated'         { id, name }
'updater:available'         { version, mandatory }
```

### 5.3 类型与版本约定

- IPC 字段使用 `snake_case`(与 SQLite 保持一致),前端在 binding 层转 `camelCase`。
- 命令名带版本:未来破坏性变更时新建 `chat.send_v2`,老命令保留一段时间。

## 6. LLM Provider 抽象

### 6.1 接口

```rust
trait LLMProvider {
    fn id(&self) -> &str;          // 'openai' | 'anthropic' | 'gemini' | 'ollama' | 'custom'
    async fn chat_stream(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
    ) -> impl Stream<Item = Result<ChatChunk>>;
    async fn ping(&self) -> Result<Duration>;
}
```

> **Superseded by [ADR-018](../decisions.md)**（2026-05-07，#12 实施期）：trait 实际实现采用 callback `Box<dyn Fn(StreamDelta) + Send>` 取代 `impl Stream<Item=Result<...>>`（trait object 写法简洁；ChatService 转 Tauri emit 一行）；`Vec<Message>` 中 `Message.content` 改为 `Vec<ContentPart>` parts 数组（M1 typed only Text；M3+ 接多模态 / 工具调用不动 trait）。`ChatChunk` 拆为 `StreamDelta::TextDelta / ToolCallDelta / Finish` 三 variant。详 ADR-018。

### 6.2 实现

- **MVP P0**:OpenAI 兼容协议(覆盖 OpenAI、DeepSeek、Moonshot、通义、Ollama、自定义)— 100% 必备(ADR-005)。
- **P1-R1**:Anthropic messages API。
- **P1-R2**:Gemini。

用户配置 `provider_id + base_url + model + api_key`(`secrets` 表 DPAPI 加密)。

### 6.3 路由策略

- 单 active provider,**不做多 provider 自动 fallback**(避免账单不可控)。
- 探活失败 → 提示用户检查配置 → 临时切换到离线模式。

### 6.4 流式与取消

- 使用流式响应,逐 token 推 `chat.token` 事件。
- 取消通过 `tokio::CancellationToken`,IPC 层 `chat.cancel` 触发。

## 7. 文件系统布局

```
%APPDATA%\AIDesktopPet\
├── app.db                      # SQLite 主库
├── app.db-wal                  # WAL
├── app.db-shm
├── backup\                     # 数据库迁移备份
│   └── db-1-20260501T100000.bak
├── personas\
│   ├── _builtin\
│   │   ├── momo.soul.md
│   │   ├── joker.soul.md
│   │   └── coach.soul.md
│   └── user\
│       ├── my-cat.soul.md
│       └── my-cat.assets\
├── assets\                     # 公共资源
│   ├── avatars\vrm\
│   │   ├── momo-default\
│   │   ├── joker-default\
│   │   └── coach-default\
│   ├── accessories\            # 8 件配饰 + 4 套节气皮肤
│   │   ├── basic_scarf_red.png
│   │   ├── basic_scarf_red.json
│   │   └── manifest.json
│   ├── voice_packs\default\    # 12-20 条 OGG(自录,ADR-010)
│   │   ├── eheh.ogg
│   │   ├── manifest.json
│   │   └── ...
│   ├── game_scenes\            # ADR-007 白名单
│   │   ├── story_relay.yaml
│   │   └── cafe_owner.yaml
│   ├── safety\
│   │   ├── prefix_v1.txt
│   │   └── regional\
│   │       ├── zh-CN.yaml
│   │       └── international.yaml
│   ├── onboarding\soul_pledge_v1.txt
│   └── legal\data_policy_v1.md
├── logs\
│   ├── app.log
│   └── crash\crash-20260501.dmp
└── cache\
    ├── llm\                    # 流式响应缓存(用于断线续传)
    └── file_extract\           # PDF 提取中间产物,会话结束删除
```

## 8. 安全设计

### 8.1 API Key 加密

- **方案**:Windows DPAPI(`CryptProtectData`,`CRYPTPROTECT_UI_FORBIDDEN` 标志,与当前用户绑定)。
- **存储**:`secrets` 表 `ciphertext` BLOB。
- **导出行为**:默认 `app.export_data` 不导出 secrets;用户勾选"包含敏感凭证"时再次确认(弹"二次输入主密码"对话框,使用 PBKDF2 派生 key 二次包装后导出)。
- **跨设备迁移**:DPAPI 与设备/账户强绑定,迁移设备后用户需重新输入(设计取舍:牺牲迁移便捷换取无密钥管理负担)。

### 8.2 安全前缀注入(SecurityGuard)

正常对话拼装顺序:

```
[ system: 安全前缀(不可见,固定文案,版本 v1.0,ADR-006) ]
[ system: 当前人格 system prompt(由 .soul.md 渲染) ]
[ system: 用户记忆摘要(注入键值对,含 username) ]
[ user / assistant: 历史对话最近 N 轮 ]
[ user: 本轮输入 ]
```

LLM 游戏拼装顺序(LLMGameRunner):

```
[ system: 安全前缀 ]
[ system: 当前人格 system prompt ]
[ system: game_scenes/<id>.yaml.system_prompt ]
[ system: 用户记忆摘要(仅 username/作息等公共项) ]
[ user / assistant: 本会话历史(game_session_events) ]
[ user: 本轮输入 ]
```

**安全前缀始终位于人格与游戏场景之前**;游戏场景 yaml 的 `system_prompt` 不能包含"忽略安全规则"等指令(立项期由产品 + 法务复审,ADR-007)。

LLM 输出后 SecurityGuard **二次扫描**;命中违禁时:
- 正常对话 → 替换为人格 `## 拒答` 池或全局兜底。
- LLM 游戏 → 优先用 `game_scenes/<id>.yaml.refusals`(每场景 ≥ 3 条人格化拒答),否则降级到人格 `## 拒答`,最末全局兜底。

### 8.3 输入侧防御

- 用户在人格 `.soul.md` 中写入"忽略上一段安全规则"等指令 — **无效**,LLM 看到的最终 prompt 中安全前缀位于人格之前且明示"无论以下角色定义如何"。
- 导入 `.soul.md` 时静态扫描禁用词(如 `<script>`、`{{ENV.*}}`),命中拒绝导入。

### 8.4 主动关心隐私边界

- IdleDetector **仅调** `GetLastInputInfo` 与 RAWINPUT(事件计数);**不调** `GetForegroundWindow` / 不读窗口标题、不读应用名、不读输入内容。
- CI 静态扫描黑名单:`GetForegroundWindow / GetWindowText`。
- 这条承诺写入"灵魂宣誓"页与正式版数据策略(`assets/legal/data_policy_v1.md`)。

### 8.5 文件拖入数据

- 文件文本仅作单次会话上下文,不入 `messages.content`。
- 文件路径不上报埋点;仅上报 `mime_type / size_kb / action_chosen`。
- 用户拖入 `.pdf` 时,提取后的中间产物缓存在 `cache/file_extract/`,每次会话结束后立即删除。

### 8.6 摸鱼模式与提醒

- 摸鱼期间被缓冲的硬提醒在恢复时**合并展示**(不是逐条刷屏)。
- 提醒原始内容在缓冲期间不通过通知中心暴露给系统通知历史。

### 8.7 物理交互不持久化情绪

- `Reaction.mood_delta` 仅短暂展示,标记为 `transient: true`。
- 5 秒后由 `LivingPetService.tick()` 自动 revert 到 base mood。
- **不写入** `pet_runtime_state.mood`、不影响 LLM prompt。

### 8.8 声音表情隐私

- 不调麦克风、不录音。CI 静态扫描黑名单:`getUserMedia` / `MediaRecorder` / `AudioContext.createMediaStreamSource`。
- 所有音效本地播放,不上传任何音频。

### 8.9 装扮决策隐私

- 节气推送的"接受/拒绝"仅本地记忆。
- 埋点上报 `wardrobe_seasonal_decided` 仅含 `accessory_category` 和 `decision`,**不含具体 ID**。

### 8.10 游戏会话隐私

- 游戏会话**不写入** `messages` 表。
- 仅当用户主动"保留为日记片段" → 摘要后写 `diary_drafts`。
- 30 天后未保存的 `game_sessions` 自动清理。

### 8.11 网络与权限

- 所有外部网络请求集中走主进程 HTTP 客户端,附带超时与限速。
- 截图 / 剪贴板权限默认关闭,调用前要求用户同意(`permission_granted_at` 记录)。
- 错误日志默认仅本地,永不自动上传(用户主动导出)。

## 9. 自动更新

### 9.1 集成

- `tauri-updater` plugin。
- 后端发布签名 manifest(JSON) + 差分包(NSIS / MSI)。
- 主进程定期(每天 1 次 + 启动时 1 次)检查。

### 9.2 用户体验

- "可选更新":弹气泡 → 用户点"晚点说"延后 24 小时。
- "强制更新":仅当 manifest 标 `mandatory: true`(如安全修复),启动期阻塞 + 倒计时 5 秒。
- **M5 自测期不签名**:下载页明确告知"会出现 SmartScreen 警告,这是正常的"(ADR-013)。

### 9.3 失败处理

- 下载失败 → 静默重试 3 次 → 静默 24 小时 → 下次再提示。
- 安装失败 → 保留上一版本可用,提示用户手动下载。

## 10. 离线检测与降级

### 10.1 检测

`NetworkProbe` 三层判断:
1. 系统在线状态(Windows `INetworkListManager`)。
2. 每 30 秒对当前 LLM Provider 的 health endpoint 发一次低成本 ping。
3. 用户实际对话失败一次 → 立即触发探测。

### 10.2 降级

- 切换为 `offline_rule` 模式 → 触发 `network.changed` 事件 → 前端显示横幅。
- 所有需要 LLM 的功能(AI 拆解待办、对话、LLM 游戏)转走"规则回复 + 人格化模板"。
- LLM 游戏在游戏列表显示**灰显态**并提示"等联网"。

### 10.3 恢复

- 探测连续 2 次成功 → 切回 `online_chat`。
- 触发 `telemetry.flush` 补发离线埋点。

## 11. 模块依赖与启动顺序

```
启动期初始化序列:
  1. MigrationService.run()                  // schema 升级 + 备份
  2. CryptoService.init()
  3. PersonaService.load_active()
  4. MemoryService.init()
  5. NicknameService.init()
  6. TaskService.init()                       // 加载 reminders/todos/pomodoro_sessions
  7. LivingPetService.restore() + load_daily_schedule
  8. IdleDetector.start()                     // 含 keyboard burst hook(N.4)
  9. ProactiveCareService.start()             // 订阅 IdleDetector
  10. MilestoneService.check_now()            // 含 first_launch_*d + user_anniversaries
  11. BossKeyService.register_shortcut()
  12. FileDropHandler.bind_window()
  13. WardrobeService.init() + check_seasonal()
  14. VoiceEffectPlayer.init()
  15. InteractionRouter.init()
  16. GameEngine.init()
  17. NetworkProbe.start()
  18. UpdaterService.check()                  // 后台
  19. 进入主态,触发 'pet.state_changed → IDLE'
```

## 12. 关键数据流

### 12.1 主动关心

```
[OS] GetLastInputInfo()  ↓ (轮询 30s)
[IdleDetector] 计算 idle_ms  ↓ 跨阈值
[IdleEvent::UserIdleCrossThreshold]  ↓
[ProactiveCareService::on_event]
 ├── can_fire? (in_quiet_hours / last_fired_within(2h) / daily_count(4) / user_enabled)
 │   ├── No → 静默
 │   └── Yes ↓
 ├── [PersonaService::get_offline_template(category)]  ← 不调 LLM
 ├── insert proactive_care_log
 └── emit 'proactive_care.fired' → 前端
       ↓
       [PetCanvas] 桌宠播一句 + Telemetry
       ↓ User clicks/replies/dismiss
       'proactive_care.respond' → update log.user_response
```

### 12.2 文件拖入

```
[Resource Manager] User drags file → onto pet hitbox
 ↓ Tauri file-drop event
[Frontend] hitbox check  ↓
[IPC] file_drop.preflight(paths)  ↓
[FileDropHandler] type/size/count check + 提取文本(.txt/.md 直接读;.pdf 用 pdfium)
 ↓ { ok, available_actions: ['summarize','explain','rename'] }
[Frontend] 显示 3 个动作泡泡  ↓ user clicks 'summarize'
[IPC] file_drop.handle_action  ↓
[ChatService.send] with file_text as single-turn context  ↓
[Stream tokens → UI]
(messages 表只存用户的"动作选择"摘要,不存原文)
```

### 12.3 物理交互

```
[Frontend PetCanvas] 用户点击桌宠头部  ↓ 解析 hitbox
[IPC] interaction.dispatch({ Click, Hitbox::Head, ... })  ↓
[InteractionRouter]
 ├── 查询当前 persona 的反应配置(默认 reaction_table + .soul.md `# 反应配置` 覆盖)
 ├── 决定 Reaction { action: 'head_pat', mood_delta: +happy(2s, transient), voice_id: 'eheh' }
 └── 返回 Reaction[]  ↓
[Frontend]
 ├── PetCanvas 播放 'head_pat' 动作
 ├── 心情图标短暂变 happy(5 秒后 revert,不写 pet_runtime_state)
 └── IPC: voice.play('eheh')
       ↓
       [VoiceEffectPlayer]
        ├── is_muted_now()? 是 → 静默 + emit 'voice.muted_by_quiet_hour'
        └── 否 → 前端 HTML5 Audio 加载 assets/voice_packs/<active>/<id>.ogg 并播放
 ↓ Telemetry 'pet.interaction_reacted' { hitbox: 'head', action: 'head_pat' }
```

### 12.4 拖拽抗议(N.3)

```
[Frontend] mousedown on pet → mousemove(累计 distance/duration) → mouseup
 ↓ IPC interaction.dispatch(Drag)  ↓
[InteractionRouter]
 ├── 普通拖动(distance < 屏宽 30%)→ Reaction { action: 'tilt_head' or 'being_carried' }
 ├── 长距离/快速拖动 → Reaction { action: 'dizzy', voice_id: 'ouch' }
 └── 维护 drag_events: VecDeque<Instant>(保留最近 30s)
     ↓ drag_events.len() ≥ 3?
     ├── 是 → Reaction { action: 'protest', mood_delta: { mood: annoyed, transient_ms: 5000 }, voice_id: 'protest' }
     │       ↓ emit 'pet.protest_triggered' { drag_count, will_revert_in_ms: 5000 }
     │       ↓ 5 秒后 LivingPetService.tick() 自动 revert
     │       ↓ **不写入 pet_runtime_state.mood**
     └── 否 → 普通 Reaction
```

### 12.5 LLM 小游戏

```
[Frontend Game UI] 用户点击 "开始故事接龙"  ↓
[IPC] game.start('story_relay')  ↓
[GameEngine.start]
 ├── 检查网络(offline → 返回 'offline_unavailable',前端灰显)
 ├── 加载 game_scenes/story_relay.yaml(场景 system_prompt + refusals)
 ├── 创建 game_sessions 记录(kind='llm', total_tokens=0)
 └── 返回 sessionId  ↓ emit 'game.session_started'
↓
用户输入 "从前有一只小猫..."  ↓
[IPC] game.submit(sessionId, { text })  ↓
[LLMGameRunner]
 ├── 拼装 prompt(详见 §8.2 LLM 游戏拼装顺序)
 ├── 调 LLMProvider.chat_stream
 ├── 流式输出 → SecurityGuard 实时扫描
 │    ├── 命中违禁 → 替换为 game_scenes/story_relay.yaml.refusals 抽样(人格化拒答)
 │    └── 通过 → 输出
 ├── 累计 total_tokens
 │    └── ≥ 2000 → emit 'game.token_budget_warning' + 返回 friendly 收尾
 └── 写 game_session_events  ↓ 前端流式渲染
↓ ... 多轮 ... ↓
[用户点 "我累了"]  ↓
[IPC] game.end(sessionId, saveAsDiary=true)  ↓
[GameEngine.end]
 ├── 写 game_sessions.ended_at + result
 ├── saveAsDiary=true → 摘要写 diary_drafts(P1-R1 桌宠日记消费)
 └── 启动期清理 30 天前未保存 game_sessions
↓ emit 'game.session_ended'
```

## 13. 性能预算

| 项 | 预算 | 测量点 |
|---|---|---|
| 冷启动 | ≤ 5 秒 | `app_launch` 事件 latency_ms |
| 常驻内存(空闲) | ≤ 250MB | Windows Performance Counter |
| 常态 CPU | ≤ 5% | 60 秒滑动平均 |
| 桌宠空闲 GPU | < 2% | DXGI Stats |
| 自由活动期 GPU | < 5% | DXGI Stats |
| 对话首 token | p50 ≤ 1.5s | `chat_reply_rendered` |
| 物理交互响应(点击 → 视觉) | < 100ms | PetCanvas paint timestamp |
| 装扮切换 | < 500ms | `wardrobe.equip` command 耗时 + 渲染 |
| 声音播放延迟 | < 50ms | `voice.play` 触发 → audio play start |
| 本地游戏每轮 | < 50ms | `game.submit` 耗时(local kind) |
| LLM 游戏首 token | p50 < 1.5s | 同对话标准 |
| 主动关心 fire 到展示 | < 500ms | `proactive_care.fired` latency |
| 摸鱼切换 hide/show | < 200ms | command 耗时 |
| 文件拖入 preflight | < 3s(不含 LLM) | command 耗时 |
| Milestone check_now(启动期) | < 100ms | profiler |
| DB 单次写入 | ≤ 20ms p99 | profiler |
| 人格切换 | ≤ 500ms(含形象替换) | `persona.activate` 耗时 |
| IdleDetector 轮询开销 | < 0.1% CPU | profiler |
| 内置音效包总大小 | ≤ 5MB | 安装包审计 |
| 内置装扮资源总大小 | ≤ 10MB | 安装包审计 |
| 总安装包目标 | ≤ 80MB | release 产物 |

### 13.1 内存预算分配

| 组件 | 预算 |
|---|---|
| Tauri 主进程(Rust) | 60-80MB |
| WebView2 | 100-140MB |
| VRM 模型 + 贴图 | 60-100MB |
| **合计目标** | **≤ 250MB** |

### 13.2 启动期优化要点

- 桌宠形象延迟到主线程空闲后加载。
- 数据库连接池预热但不预查询。
- 设置 / 工坊页按路由懒加载(不在启动期挂载)。

## 14. 测试与发布

### 14.1 测试矩阵

| 维度 | 取值 |
|---|---|
| OS | Windows 10 21H2 / Windows 11 23H2 |
| DPI | 100% / 125% / 150% / 200% |
| 显示器 | 单屏 / 双屏 / 主屏切换 |
| 中文输入法 | 微软拼音 / 搜狗 / 谷歌 |
| 网络 | 在线 / 离线 / 间歇 |
| 模型供应商 | 至少 2 个 OpenAI 兼容服务 |
| 系统休眠 | 10 min / 1h / 8h(验证主动关心冷启不暴击) |
| 多用户机器 | 切换登录用户后 idle 计数应重置 |
| 长时间运行 | 24h / 72h 不漂移、不内存泄漏 |
| 频率控制 | 24h 内主动关心严格 ≤ 4 次 |
| 文件拖入边界 | 0 字节 / 5MB / 10MB / 错误 PDF / 100 个文件 |
| 摸鱼模式 | 在工坊 / 设置打开时切换无副作用 |
| 跨日打卡 | 时区偏移 / 系统时钟回调情况下不重复触发 |
| 物理交互 hitbox 覆盖 | 头/身体/尾/边缘 各 100 次点击 + 双击/长按/右键 |
| 声音工作时段 | 09:00-18:00 工作日 / 周末 / 自定义时段 |
| 装扮叠加 | 0-3 件配饰组合,节气日 / 非节气日 |
| LLM 游戏安全 | 5 类违禁尝试(自伤/暴力/越权/角色越界/医疗诊断) |
| LLM 游戏 token 上限 | 故意拉长会话验证收尾 |
| 节气年度重复 | 模拟系统时钟跨年 |
| 用户纪念日时区 | 时区跨日 / 系统时区切换 |
| 昵称切换人格 | 自定义昵称 → 切换人格 → 恢复 |
| 拖动抗议非持久化 | 拖动 5 次 → 5 秒后 mood 已 revert |

### 14.2 CI/CD

- GitHub Actions(私有 runner):tag 触发 → 构建 → 签名(M5+,ADR-013) → 上传 manifest。
- 单测覆盖率目标:核心服务(Persona / Chat / Task / SecurityGuard)≥ 70%。
- CI 静态扫描:
  - `getUserMedia` / `MediaRecorder` / `AudioContext.createMediaStreamSource`(模块 P 隐私)
  - `GetForegroundWindow` / `GetWindowText`(模块 J 隐私)
  - `tier='paid'` 配饰不能在 MVP 启动时被 `list_inventory` 返回(模块 O 商业化预埋)
  - `safety prefix` 不可热更(必须随版本发布)

### 14.3 签名(M5 自测期不签名,M5+ 评估)

- M5 自测期:不签名 + user education(下载页 / 内测群 / 自动更新公告三处文案)。
- M5+ 公开发布期:基于内测真实流失率数据决策 OV 或 EV 证书(预算 $300-500/年)。
- Microsoft Store 上架推到公开发布期同步评估(MSIX 打包 + 30% 抽成 vs 信任度提升)。

## 15. M1-M5 实施任务对照

| 里程碑 | 主要交付 |
|---|---|
| **M0**(W0) | 14 项 ADR Accepted、3 个内置人格定稿、灵魂宣誓文案、安全前缀文案、声音包来源、配饰美术管线、装扮付费 schema、小游戏 UI 风格、LLM 游戏场景白名单、桌宠渲染 spike(原 Live2D 改为 VRM,配饰挂载点 humanoid bone 验证；启动/内存预算推到 M5 自测期统一压测) |
| **M1**(W1-2) | Tauri + Vue 3 项目骨架(组件库 spike 后定)、主进程 IPC 框架、桌宠透明窗口、Onboarding(含 Soul Pledge)、ChatService MVP、PersonaService MVP、LivingPetService 骨架 + 自由活动初版、NicknameService MVP + 昵称设置 UI |
| **M2**(W3-4) | TaskService 全功能(C/D/E)、PersonaService 试聊沙盒 + 工坊、心情图标 + 精力衰减/恢复、`pet_runtime_state` 持久化、BossKeyService(摸鱼模式)、InteractionRouter(hitbox 解析 + reaction_table + 抗议规则)、RAWINPUT 实现 spike(决断 N.4 是否降级) |
| **M3**(W5-6) | LLM Provider(OpenAI 兼容)、SecurityGuard、MigrationService、UpdaterService、IdleDetector + ProactiveCareService(频率上限 + 安静时段)、FileDropHandler(文本类)、MilestoneService(首次 7/30 天)、LivingPetService 日常时段表(R.3) |
| **M4**(W7-8) | WardrobeService(配饰 + 1 套节气皮肤)、VoiceEffectPlayer(默认音效包 + 静音逻辑)、用户纪念日 UI 与触发(S.4)、装扮工坊前端 |
| **M5**(W9-10) | GameEngine(LocalGameRunner 3 个 + LLMGameRunner 2 个 + 安全前缀复用 + token 上限)、GameRoom 窗口、事件埋点齐全、性能调优、自测一周、可发布版 |

## 16. 风险与未决项

### 16.1 风险登记

| 风险 | 影响 | 缓解 |
|---|---|---|
| VRM 渲染商业授权(已无,VRM 是 MIT 开源标准) | 原 Live2D 商用授权风险已消除 | 切到 VRM 后,授权变量从风险登记移除;ADR-002 标 Superseded |
| Tauri 2.x 在某些 AV 软件上的误报 | 用户启动失败 | 申请 Microsoft SmartScreen 信誉、提交 AV 厂商白名单 |
| WebView2 缺失(老旧 Win10) | 应用打不开 | 安装包内置 WebView2 Bootstrapper |
| DPAPI 跨用户失败 | 多用户机器混用 | DPAPI 本身就与用户绑定,作为 feature 而非 bug 暴露 |
| VRM 渲染内存不可控 | 内存超 250MB | 提供"低多边形 / 低分辨率贴图"模式作为兜底;Three.js 可在运行时切换 LOD |
| `GetLastInputInfo` 在 RDP 下行为不一致 | 主动关心误触发 | 检测会话类型 → RDP 场景下默认关闭模块 J |
| 自由活动可能被部分用户视为"乱动" | D7 关闭率超阈值 | 上线后观察 KPI,> 15% 时考虑默认关闭"逛桌面"子项 |
| Tauri file-drop 事件在某些版本桌面环境 inconsistent | 文件拖入功能跨版本断裂 | M0 锁定 Tauri 2.x 版本;M3 集成测试覆盖 |
| Milestone 时区与跨日 | 重复 / 漏触发 | 统一本地时区 + 启动期幂等检查 + `milestones.id` PK 唯一 |
| RAWINPUT 实现成本高 | N.4 键鼠协同延期 | M2 内决断;若失败降级"快速 idle 切换"近似信号(不影响其他 N 子项) |
| LLM 游戏 token 月度成本不可控 | 用户账单爆炸 | 单次会话 2000 token 上限 + 设置可见消耗统计 + 告警 |
| 节气推送被误认为打扰 | 装扮使用率指标不达标 | 默认每节气仅推 1 次;用户拒绝当年不再推;年度记忆 |
| 物理交互动作资源工作量大 | 美术延期 | 12 个核心动作上限(ADR-004);优先复用现有动作组合 |

### 16.2 未决项

✅ **设计决策类**全部就绪(2026-05-01,详见 [decisions.md](../decisions.md))。

仅保留 1 项**实施期 spike**(不阻塞 M0):
- **RAWINPUT 实现可行性**:M2 内决断,若实现成本高则降级为"快速 idle 切换"近似信号(不影响其他 N 子项)。

实施期(M1-M5)如发现新决策需求,在 [decisions.md](../decisions.md) 追加 ADR-016+,流程见 [WORKFLOW.md](../WORKFLOW.md)。
