---
title: AI 桌宠 决策记录
updated: 2026-05-20
related:
  - README.md
  - WORKFLOW.md
---

# 决策记录（decisions.md）

记录关键技术与产品决策，每个决策三句话：**为什么需要决策、选了什么、代价**。

> 不走 Proposed/Accepted/Superseded 状态机；写下来本身就是决定。被推翻就在原条目下追加 *Superseded by 新决策* 一行，不删除。

---

## 立项期决策（W0 准备阶段）

### ADR-001 前端框架

- **为什么**：Tauri 默认让 webview 跑前端，需要选一个团队顺手的栈。
- **选了什么**：Vue 3 + TypeScript + Pinia + Vite。组件库 Element Plus
- **代价**：放弃 React/Solid 生态；Vue 的 SFC 需要 IDE 插件支持。

### ADR-002 桌宠渲染管线

- **为什么**：原 Live2D Cubism 4 路线在立项期发现 Cubism Core 6 ABI 破坏 + `pixi-live2d-display` 上游停更，无法用。
- **选了什么**：**VRM 3D**（Three.js + `@pixiv/three-vrm`），MIT 开源无授权风险。
- **代价**：3D 模型比 2D Live2D 贵；启动 / 内存预算（PRD §10）作为整体性能目标的一部分，**M5 自测期统一压测**，不在 M1 单独 spike 验证。
- **Supersedes**：原 Live2D 路线（ADR-002a）已废止。

### ADR-003 配饰美术管线

- **为什么**：VRM 切换后原 Live2D 插槽方案失效，需要新的配饰挂载方式。
- **选了什么**：VRM humanoid bone attach + VRMC_node_constraint。每个 VRM 模型预留 head/neck/leftEye 等标准 bone 作为挂载点；配饰为独立 .glb 节点（含 transform/scale/父骨骼）。切换 < 500ms。
- **代价**：每个配饰需要按 VRM 标准建模；artist 学习成本。

### ADR-004 物理交互动作清单

- **为什么**：物理动作太多会让美术工作量爆炸；太少不好玩。
- **选了什么**：12 个核心动作 ID — `head_pat / tilt_head / tail_wiggle / lean_in / surprised / fall_asleep / stretch / yawn / dizzy / protest / cheer / rub_eyes`。默认 reaction_table 可被 `.soul.md` 覆盖。
- **代价**：上限 12 个限制了表达力；扩展需要每次评估必要性。

### ADR-005 默认 LLM Provider

- **为什么**：硬绑某个云模型容易锁死用户；强制用户首次输 API Key 又会高流失。
- **选了什么**：零默认 + 6 个 preset（OpenAI / DeepSeek / Moonshot / 通义千问 / 本地 Ollama / 自定义）。Onboarding Step 6 不强制 API Key，首启 5 个本地能力即可使用；首次唤起对话失败再引导。
- **代价**：初次启动用户拿不到对话功能；需要清楚的引导文案。

### ADR-006 安全前缀

- **为什么**：用户人格可以很顽皮，但安全规则（自伤/违法/未成年）必须不可绕过。
- **选了什么**：通用核心（全球 5 条：自伤/暴力/违法不指导、不冒充医疗/法律/金融、未成年保守、不泄露隐私、角色扮演不诱导混淆现实） + 地区补充（zh-CN 010-82951332/12320-5；international US 988 / UK 116123 / EU 116123）。版本号 v1.0 写入 `consent.version`，变更时强制重确认。
- **代价**：拒答场景需要人格化文案，否则突兀；地区补充需要法务签字（或自查）。

### ADR-007 LLM 游戏场景

- **为什么**：LLM 游戏 token 成本高 + 法务风险高，需要先小范围验证。
- **选了什么**：1+1 双场景 — Q.3 故事接龙（通用） + Q.4 角色扮演"咖啡店老板"（法务低风险）。每场景 yaml 含 ≥ 3 条人格化拒答模板。其他场景推到 P1-R2。
- **代价**：游戏多样性受限；想加场景必须先评估法务。

### ADR-008 灵魂宣誓文案

- **为什么**：传统隐私同意页打扰且不被读，不如用桌宠人格转译。
- **选了什么**：温暖叙述版 v1.0，由"默默 momo"以第一人称叙述（5 句承诺 + "完整数据策略"链接）。"我懂了"按钮等价于同意，写入 `consent.granted=true + consent.method='soul_pledge' + consent.version=1`。
- **代价**：变体文案不易做（变了等于变了同意条款）；需要法务/自查认可"灵魂宣誓 = 隐私同意"等价。

### ADR-009 三个内置人格

- **为什么**：让用户上来就能选，覆盖三种典型偏好。
- **选了什么**：**默默（温暖陪伴） / 阿吉（活泼幽默） / 教官（严厉督学）**。tone_profile / 离线模板 / 调侃 / 庆祝 / 反应配置全部就位，写入 `personas/_builtin/{momo,joker,coach}.soul.md`。
- **代价**：三个人格需要全套美术 + 文案；后续不能轻易撤一个。

### ADR-010 音效包来源

- **为什么**：调 TTS 烧 token 且延迟高；外购音效授权风险高。
- **选了什么**：自录（产品配音）。12-20 条短音效（每条 < 500ms，OGG Vorbis 44.1kHz），预算 ¥3000-6000，100% 自有版权。
- **代价**：录制 + 编辑 + 调音工作量；无法即时迭代。

### ADR-011 装扮付费 schema

- **为什么**：MVP 不开商店，但要预埋付费结构方便后续上线。
- **选了什么**：结构化对象 schema + JSON 列存储。`UnlockSpec` 支持 6 种 kind：`always / milestone / date_range / purchase / gift / user_upload`。MVP 期 `tier='paid'` 全部不返回前端。
- **代价**：schema 复杂；商店上线时需要解除过滤 + 实现支付。

### ADR-012 小游戏 UI 形态

- **为什么**：在桌宠主窗里玩游戏沉浸感差；独立窗口又怕用户找不到。
- **选了什么**：独立游戏舱 GameRoom 窗口（480 × 600，固定中央或桌宠旁）。全部 5 个游戏（本地 3 + LLM 2）统一在游戏舱内承载；桌宠在屏幕保持可见（IN_GAME 叠加态）。
- **代价**：多一个 Tauri 窗口（内存 + 开发成本）；hub 形态 1 仅做 launcher 不做承载。

### ADR-013 代码签名

- **为什么**：EV 证书 6000+/年，灰度期投入产出不划算。
- **选了什么**：自测期不签名 + user education。下载页明确告知"会出现 SmartScreen 警告，这是正常的"。EV/OV 证书与 Microsoft Store 上架推到公开发布期评估。
- **代价**：SmartScreen 警告会劝退一部分用户；需要好的 onboarding 文案。

### ADR-014 本地小模型路径

- **为什么**：完全本地推理是隐私铁粉的需求，但 MVP 内嵌模型会爆体积。
- **选了什么**：调用本地 Ollama。MVP 不内嵌任何模型；用户在设置中可配置 `base_url=http://localhost:11434/v1` 走 OpenAI 兼容协议。推荐 Qwen2.5-3B-Instruct-Q4。
- **代价**：用户需要自己装 Ollama + 下模型；P1-R3 才正式推出。

---

## 实施期决策

### ADR-015 对话面板三形态架构

- **为什么**：实施期 D3 发现"单一对话面板"在不同场景下都不合适：聊天专注时要大窗，问一句话时要小气泡，跨任务时要 hub。
- **选了什么**：**3 形态共存**（hub 总面板 + 磁吸浮窗 + 漫画气泡） + ConversationStore 共享数据层。三形态共享同一 conversation 数据，仅视图不同。M1 极简（B.3.a）→ M2 完整（B.3.c）→ M4 hub（B.3.e）→ M5 气泡（B.3.f）。
- **代价**：原 PRD §7.2 整段重写；ChatService 拆 B.3.a-f 跨 M1-M5；增加控制按钮区（模块 A 延伸）。

### ADR-016 项目脚手架决策

- **为什么**：M1 D1 起步需要确定 Tauri + Vue 工具链与工程化基线；旧项目 `D:\Project\ai桌宠` 已有完整可参考实现，但需按 15 项 ADR 重新审计后手写到新仓库（不复制粘贴），避免把旧项目的偶然选择带入。
- **选了什么**：**Tauri 2 + Vue 3.5 + TS 5.6 + Pinia 2.2 + Vite 7.x + pnpm**；ESLint 9 flat config + `eslint-config-prettier` 关冲突；`vite.config.ts` 用 `fileURLToPath` 绝对路径写法防中文目录踩雷；`src-tauri/Cargo.toml` 先只装 tauri 2 + serde + thiserror + chrono + windows(DPAPI)，其它依赖按 STATUS 节奏后置；release profile `lto=true / panic=abort / strip=true`。**与旧项目偏离**：① 不装 commitlint + husky（CLAUDE.md 写"commit 风格自由不强制"，单人 vibecoding 不需要门禁）；② 不装独立 prettier 配置（用 `eslint-config-prettier` 关冲突即可）；③ Vite 5.4 → 7.x 跟最新版。
- **代价**：Vite 7 vs 5 有少量 API 变动（`server.fs.allow` 默认更严格），首次启动需验；不装 husky 意味着提交期没格式 / lint 自动门禁，依赖 CI 或开发自觉。

### ADR-017 组件库选型 — 全量 Element Plus

- **为什么**：ADR-001 当时把组件库选型推到 M1 W1 试用再定。M1 D1 复盘：①桌宠 M1-M5 涉及 ChatPanel / Onboarding / 设置面板 / 装扮选择器 / 小游戏 UI 等 ≥ 15 个组件场景，自封工作量 > 学习一个成熟库；② Element Plus 全量 import min+gzip ~314 KB（CSS ~80 KB），估算 Tauri release 二进制 ~7-9 MB，PRD §10 总安装包预算 80MB 占比 < 12%，体积非决定性约束；③ 全量 import 比 `unplugin-vue-components` 按需配置维护成本低，单人项目省心。
- **选了什么**：**Element Plus 2.13+ 全量 import** + `@element-plus/icons-vue` + 暗色 `theme-chalk/dark/css-vars.css` + 中文 locale `zhCn`。**主题策略**：默认跟随系统（`prefers-color-scheme` 监听）+ 系统托盘菜单可手动切换 `[跟随系统 / 亮色 / 暗色]`，状态走 Pinia `useThemeStore`，先 localStorage 持久化、M3 G 模块时挪到 SQLite `settings` 表。不做按需 import；不引 Naive UI；不走旧项目"原生 + 自封"路线。
- **代价**：产物多 ~250 KB（接受）；所有 EP 组件 CSS 启动即载（透明窗口下视觉无影响）；锁定 EP 设计语言（若 M5 末实测冷启动 > 5s 预算超支，回滚到 `unplugin-vue-components` 按需 import，评估 0.5 天）。**Supersedes**：ADR-001 中"Naive UI 或 Element Plus，M1 W1 试一下再定"悬而未决项。
- **实测**（M1-D2，2026-05-06）：`pnpm tauri:build` 产物 `src-tauri/target/release/aipet.exe` = **4.15 MB / 4,350,976 bytes**（debug 对比 = 11.94 MB；release 编译 4m 17s）。预估 7-9 MB，**实测优于预估约 50%**（release profile `lto=true / codegen-units=1 / strip=true` 三件套效果显著）。`bundle.active=false` 故无 .msi/.exe 安装包；M5 末再测全量 bundle 体积（含 WebView2 Bootstrapper 与图标资源）。

### ADR-018 LLM 三层抽象 + AgentService 工具调用框架

- **为什么**：M1 #12 LLMProvider 设计若按 issue body 字面 `content: String` 实现，M3 接多模态（图/音/文件输入）+ Claude Code 级本地文件能力（Read/Edit/Write/Glob/Grep/Bash 等 agent 工具调用）会被迫重写 trait，所有上游 caller（ChatService / LLMGameRunner 等）跟着改；2026 年实测调研：OpenAI Chat Completions / Anthropic messages / DeepSeek / Moonshot / Qwen / Ollama 已全部走 parts 数组 + tool_calls 协议，按 issue body 字面写 = 已知未来负债。
- **选了什么**：三层分离 — **Layer 1 LLMProvider**（消息进 token 出 + tool_call 透传，#12 M1 落地；types 用 `Vec<ContentPart>` parts 数组 + `ToolCall` / `ToolDefinition` typed 但 M1 只走 `Text` variant + `tools=vec![]`；M3 接多模态 / 工具调用 = 添 impl 路径不动 trait）；**Layer 2 ChatService**（编排 Persona + Memory + SecurityGuard，#13 起逐步完整；M1 不实现）；**Layer 3 AgentService + ToolRegistry**（Claude Code 风格 agent loop + 内置 tool — Read / Edit / Write / Glob / Grep / Bash 等 6 个起步，M3+ 新增；具体路径白名单 / 命令沙盒细则待 ADR-019 决议）。stream 接口选 callback `Box<dyn Fn(StreamDelta) + Send>` 而非架构 §6.1 字面的 `impl Stream<Item=Result<...>>`（callback 转 Tauri emit 一行；Stream 在 trait object 上需 `Pin<Box<dyn Stream...>>` 写法繁琐；调用方无 take_while / buffer 等组合需求）。OpenAI 协议序列化策略：单 `ContentPart::Text` 序列化成旧 string `{role,content:"s"}`（兼容老 model 与 Ollama / Qwen / Moonshot 各家 fallback 路径），多 part 或非 Text → parts 数组；`messages.content` SQLite 列保留 TEXT，多模态消息 JSON-encode `Vec<ContentPart>` 内嵌（`[{` 前缀探嗅区分），守"27 表零迁移"D5 原则。M1 API key 走 `config` 表 KV 明文（key=`llm:openai:api_key`，与 #10 #11 同款偏离 issue body 的"settings 表"），M3 G CryptoService 上线后迁移到 `secrets` 表 DPAPI 加密（ADR-005 已说明可推迟）。
- **代价**：M1 多写 ~50 行类型定义（ContentPart 5 variant + ToolCall + ToolDefinition + StreamDelta enum + FinishReason），M1 不消费；M3 才有真消费方。架构 §6.1 stream 形状字面被本 ADR 覆盖（已在原章节追加 *Superseded by ADR-018* 跳转）。tool 沙盒细则未定，M3 G 模块前必须**独立 ADR**（编号届时分配）拍板，否则 AgentService 不能开工。
- **Updated 2026-05-08**：Layer 2 ChatService → 前端的流式 IPC 契约从全局 `app.emit("chat:stream:*")` 改为 `tauri::ipc::Channel<StreamEvent>`（`#13` 修正）。原契约下 `chat_send` 直到流式跑完才 resolve，前端拿不到 messageId 全程导致 cancel 死锁；新契约 IPC 立即返 IDs + 流式走专属 channel，类型安全 + 并发隔离 + 零 messageId 路由。`llm:test:delta` 暂保留（独立 issue 处理）；`nickname:changed` / `persona:activated` / `shortcut:chat` 保留 emit（真广播事件多窗口都要听）。详 [docs/lessons.md #6](lessons.md)。

### ADR-019 Onboarding 进度持久化与续接

- **为什么**：flows §1.5 原写"任意 Step 关窗 → 配置不写入 → 下次启动重头"。但 Step 1 完成时 `consent.granted=true` 已入库，后续任意 step 关窗 → 重启后 `consent::check_version` 返 `Match` → onboarding 窗**根本不开** → 用户永远走不完 Step 2-6（#21 实施期发现的 latent bug）。同时"走到 Step 5 才关窗 → 全部重做"对用户也是 UX 灾难（重选人格、重设快捷键等）。
- **选了什么**：**续接 + 用户选**。KV `onboarding:current_step`（config 表，与 `window:pet:last_position` / `shortcut:chat` 同段）记录当前停留 step；每次 advanceStep 前写入，`onboarding_complete` 时 `delete` KV（= "已完成" 信号）。启动期路由（lib.rs::setup）扩展为"`consent::check_version=Match` 但 KV 存在 → 仍开 onboarding"。onboarding 窗 onMounted 检测到 KV 存在 → 弹「继续 X / 重来 / 退出」三选模态：「继续」= 跳到 saved step；「重来」= 清 KV + 跳回 `soul-pledge`，**不动 `consent.granted`**（合规标记不被 UX 流程 reset，避免假"我撤回同意"路径）；「退出」= `app.exit(0)`，下次启动仍是续接状态。
- **代价**：每个 step 切换多一次轻量 KV 写 IPC（毫秒级，可忽略）；前端启动期多一次 IPC 等待 + 可能的模态。「重来」不能表达"我撤回数据同意" — 这类需求由 M3 G 设置面板的"重置数据"入口承担（清 consent + 删库 + 重启）。**Supersedes**：requirements/flows.md §1.5 「中途退出重头」（已在 flows.md §1.5 加 Superseded 跳转）。
- **Updated 2026-05-12（修「重来」启动期跳过 SoulPledge bug）**：原文写"「重来」= 清 KV"。实测发现：用户「重来」后停在 SoulPledge 关窗 → 启动期 `consent.granted=true` + KV 不存在 → 错认为"已完成 onboarding" → 跳过 SoulPledge 直接进 pet 主态。修正：「重来」改为 **写 KV='soul-pledge'**（不 clear）。`'soul-pledge'` 不在前端 RESUMABLE_STEPS 中,onMounted 看到该值不弹模态,正常显示 Step 1;但 KV 存在足以让启动期路由保持"未完成 onboarding"判定,开 onboarding 窗。原 `onboarding_reset` IPC + 前端 `resetOnboarding` 函数随后删除（dead code,M3 G "重置应用数据" 入口若需要可重建）。`services::onboarding::clear_current_step` Rust 函数保留,`onboarding_complete` / `setup` 路径仍消费。

### ADR-020 磁吸窗口系统拓扑 + 全局参数

- **为什么**：B.3.c 原计划只做 chat 磁吸（PRD §7.2.3），实施期决定扩展到 tasks 等"陪伴类"独立窗，需选拓扑——mesh 在 5 窗项目下连接数 O(n²) + 状态机爆炸，单人项目 ROI 不匹配；物理阈值（PRD §7.2.3 Q4 TBD）必须本 ADR 拍板，否则 B.3.c 不能动工。
- **选了什么**：**Hub-spoke 拓扑**——pet 为 hub，chat / tasks 为初始 spoke，spoke 之间不互吸；封装为通用 composable `useSnapWindow({ target, config })`，未来 M4 hub 总面板 / 装扮工坊可直接复用。**全局参数**（所有 spoke 共用）：① 核心语义"强吸附"（spoke 窗 `decorations: false / transparent: true / alwaysOnTop: true / skipTaskbar: true` + 12px 圆角 + drop-shadow），② 物理阈值断开 20px / 吸引区 30px / 拖动起始 5px / 跟随 throttle 16ms (60fps)，③ 默认吸附边右优先动态 fallback（右 → 下 → 左 → 上），④ 吸附时尺寸仅锁位置（用户记忆值，默认 chat 380×480 / tasks 420×600），⑤ 失焦收缩条件"所有 spoke + pet 都不在焦"才触发，收缩到 pet 边的控制按钮区（合并 B.3.b 骨架），⑥ 位置持久化走 config KV（`{label}:detached_x/y/w/h` + `{label}:state`），跨屏允许吸附，越界 fallback 默认位，⑦ 动画吸附 150ms `cubic-bezier(0.4,0,0.2,1)` ease-out + 断开 4px 弹跳。
- **代价**：放弃 chat ↔ tasks 互吸（用户期望管理，M3 视实际使用频率再考虑升级 partial mesh）；tasks 默认窗体 800×600 → 420×600 + 透明无装饰，需回归测试 M2 #22 任务面板的现有交互；通用 composable + SnapManager 单例比硬编码多 ~0.5d；物理阈值是经验取值，M2 W4 自测可能微调（直接改本 ADR 不走 Supersedes 流程，标 *Updated YYYY-MM-DD*）。
- **Updated 2026-05-18（架构改写为 constraint-based partial mesh）**：用户提出"别的窗之间也能磁吸"+"pet 与 spoke 拓扑平等" → 原 hub-spoke 推翻。新架构 = **Constraint-based Partial Mesh + Forest-Walk Solver**。核心数据 `SnapConstraint = { sourceId, targetId, sourceEdge, targetEdge, offset, enabled, createdAt }`；5 条不变量：**I1** 每窗至多 1 constraint、**I2** commit 前实时环检测（沿 attachedTo 链向上追到 self 即 reject）、**I3** drag 期间 constraint 临时挂起仅显示 ghost、**I4** 任一窗 onMoved → 走 `solve(roots)` 而非递归 setPosition、**I5** pet 与其他窗在拓扑上平等（pet 仅靠位置稳定 + memoryBias 实际成为常被吸的 anchor）。**Solver = BFS over forest**（I1+I2 保证图必为森林，无需 Kahn topo sort）。Drag session 状态机：`Idle → Dragging → PreviewSnap → Commit / Cancel(ESC)`；ESC 回滚需快照整个森林 Rect 而非仅 source 窗 constraint。**几何参数全面更新**：trigger zone 24px、corner dead zone 24×24（防角落抖动）、projection overlap 阈值 `max(72, edge × 0.25)`、candidate score = `distance × 0.6 + overlapPenalty × 0.2 + (1 − memoryBias) × 0.2`（memoryBias ∈ [-0.5, 0.5]，30s 内 detach 反向惩罚 -0.5，已存在 attachment +0.5）。**Tauri 集成**：`isInternalMove` 按窗口分桶 + rAF 释放 guard（防 setPosition → onMoved → setPosition 死循环）；wander 走专属 `pet:wander:tween_frame` 事件不走 onMoved（living_pet.rs +4 行 emit）。**持久化**：单 KV key `snap:constraints` 存 JSON 数组；启动 load + solve；anchor 缺失自动 downgrade free。**BossKey**：仅改 `visible=false` 不清 constraint，恢复时 solve(roots) 重定位。**文件结构** `src/lib/snap/`：types / registry / constraintStore / solver / candidates / dragSession / internalMove / persistence + `src/composables/useSnapWindow.ts`。**估时** S1-S9 共 ~3.6d（原 hub-spoke 2.5d +1.1d，多在 solver + 几何工具单测 + ESC 森林快照 + isInternalMove guard）。**Supersedes（本 ADR 内部）**：原"选了什么"中"Hub-spoke 拓扑 / `useSnapWindow(target='pet')` / 右优先动态 fallback / 断开 20px+吸引 30px / 跟随 16ms throttle / 链式跟随用 emit-listen"被替代；**保留**："强吸附"视觉语义、圆角阴影、chat 380×480 / tasks 420×600 默认尺寸、tauri.conf decorations/transparent/alwaysOnTop/skipTaskbar 改造、失焦收缩合并 B.3.b、跨屏允许、150ms ease-out 吸附动画、BossKey 全 hide。

- **Updated 2026-05-20（follow-up A-I 全部落地：角色模型 + 反向吸引 + 占用 + 视觉边距 + 关窗清理 + 焦点 AOT + Rust solver）**：B.3.c 实施期完整迭代。**角色模型**（follow-up D）：pet 硬编码 primary、其余 secondary；primary 拖动且有 dependents 走 group-drag，primary 无 dependents 走 primary-attract（反向吸引附近 secondary 写 `secondary→primary`），secondary 拖动走 source + 首帧 detachAll；新增不变量 **I3'：constraint.sourceId 永远不是 primary**（commit 路径 + cleanupDirtyPrimaryOutbound 保证）。**参数收紧**：ATTACH 10px / DETACH 18px / FIELD_RADIUS 20（与 PowerToys FancyZones / Photoshop grid 同档），Shift 或 Ctrl 拖动 escape hatch 跳过本次吸附。**occupancy + offset snap**（follow-up F）：edge 上多窗共占检测 + 自动滑入 free segment，避免多窗叠在同位置。**visualInset 模型**（follow-up F）：window 可声明视觉边距，candidates / solver 全程用 visual rect 做贴边几何（消除 padding 带来的视觉缝隙；初版给 chat 加 12px 后实测 padding 与子项挤压副作用大，回退 padding=0 / inset=0）。**关窗清理**（follow-up G）：WebView2 不触发 DOM visibilitychange（Tauri #6864/#9524/#10592），改 Rust 端 window_actions 主动 emit `window:visibility-changed`，前端 listen 后清 registry + 退出 dragSession。**焦点 AOT**（follow-up H）：`useFocusAOT` composable，工具窗（chat / pomodoro）平时 AOT=false，被 focus 时升 topmost，失焦降回；pet 始终 topmost；解决工具窗互相遮掩。**pomodoro 入磁吸**（follow-up E）：作 secondary 参与；全屏开/关自动 detach + 复位。**Rust solver**（follow-up I）：前端 group-drag 路径 Windows webview2 setPosition IPC ≥5ms，N=2 链跌 33Hz、N=3 跌 22Hz、严重视觉抖动；改 Rust 端订阅 `WindowEvent::Moved` 本地维护 constraint forest + visualInset，批量 SetWindowPos（同进程 μs 级），完全替代前端 group-drag。同步策略：前端是 constraint 权威源，commit/detach/load 后调 `snap_sync_constraints` 全量推到 Rust state；Rust 只读不写避免双向冲突。防死循环：`internal_until` guard（label 分桶 + 100ms TTL）跳过自递归。**角色守卫**：Rust `on_window_moved` 入口 `is_primary` 检查（与前端 PRIMARY_LABELS 镜像，硬编码 `pet`）— 只有 primary 拖动触发 BFS solver，避免 secondary 之间 constraint 让 secondary 获得整族拖动能力。**新文件**：`src-tauri/src/services/snap.rs` / `src/lib/snap/edgeSegments.ts` / `src/lib/snap/roles.ts` / `src/composables/useFocusAOT.ts`。**测试**：262 vitest pass（+33：edgeSegments 24 + visualInset 9 + candidates 占用 6）+ cargo check 全绿。

---

### ADR-021 Single-window workspace + dockable panel 架构

- **为什么**：M2 W3 复盘发现 settings / tasks / 未来 hub-chat / wardrobe-studio / persona-workshop / debug-tools 等"工具型窗"独立 BrowserWindow 化产生三处问题：① [SettingsApp.vue](../src/views/settings/SettingsApp.vue) 与 [TasksApp.vue](../src/views/tasks/TasksApp.vue) 在 AppShell + ElTabs 左排自绘 ✕ 等 ~70 行模板+CSS 已逐字符复制（PomodoroPanel 之后是第三次复制），未来 wardrobe / personas / debug-tools 还要复制 3-5 次；② 每加一模块多一个独立 Tauri window 带来启动开销 / 任务栏污染 / 主题分散；③ 用户感知散乱浮窗群 ≠ 桌面专业伙伴的产品定位。需在 ADR-015 chat 三形态 + ADR-020 partial mesh 不变前提下给工具型窗一个统一容器。

- **选了什么**：**VSCode 简版 workspace shell**（左 Activity Bar + 中 Main dock 区，Bottom Panel M3 接入；不引 Obsidian 双 sidebar / Figma 三栏固定）+ **[dockview-vue 6.x](https://github.com/mathuo/dockview)**（实测 6.3.0；3.1k★ 活跃维护、零依赖、原生 dock/split/floating/serialize；**P0 spike #32 实测体积 ~60KB gzip，远低于原估 150KB**）+ **WorkspaceManager / DockviewAdapter 双层分层**（见下"分层契约"）+ **PanelRegistry 静态注册 13 字段 schema**（见下）+ **dockview 内置 renderer keep-alive**（`always` 保 DOM / `onlyWhenVisible` 默认省内存；映射 PanelDescriptor `mountStrategy`，**不用 Vue `<keep-alive>` 包裹**）+ **panel 通过 Vue component registry 注册**（`app.component(id, comp)` 全局；6.x API，不是 5.x 的 named slot）+ **Pinia + Tauri 事件复用**（不引 workspace eventBus）+ **MVP 同期 Ctrl+Shift+P 命令面板**（panel 数 21）。**唤起**：托盘左键双击（推翻 [tray.rs](../src-tauri/src/services/tray.rs#L5) 原"双击无操作"决策，误触风险评估为零）+ 托盘右键"打开工作区" + 全局 Ctrl+Alt+W。**主窗**：1100×720 default / 800×520 min，decorations:false + 自绘 36px titlebar，hidden 启动 + KV `workspace:{last_visible,rect,layout,last_active_panel}` 持久化，关闭走 hide。**首启焦点** chat.hub。**保留独立窗**：pet 角色窗 / chat 磁吸浮窗（ADR-020 节点）/ pomodoro 浮窗（[#31](https://github.com/tl0502/APET/issues/31) 飘窗）/ onboarding（ADR-019 续接不变）。**双入口共享 service**：pomodoro 浮窗 + `pomodoro.console` panel 共享 PomodoroService（console 嵌迷你倒计时，Start 不弹浮窗）；chat 磁吸窗 + `chat.hub` panel 共享 ConversationStore（P2 抽 `ChatBody.vue` 业务壳层 + 引入 **ChatSession**（业务对象 = 一会话）+ **ConversationTab**（panel 实例 = 当前选中哪个 session），panel 内用 `localActiveSessionId` 避免 M4 多实例时重构数据层）。**chat.hub MVP `singleton:true`**（M4 视呼声开放多实例，数据层已就位无需重构）；**personas.workshop multi-instance**（`instanceKey=personaId`，`beforeClose` 拦截未保存）。**Onboarding 完成引导**：pet 旁一次性 tooltip + KV `onboarding:workspace_intro_seen`。**Activity Bar ≤ 7 项硬约束**（顶 Chat/Tasks/Personas/Wardrobe + 底 Settings/Help = 6，M5+ 加 GameRoom = 7 上限；后续 Memory/Plugins/Debug 等入口禁止再加 Activity Bar 项，全部走命令面板，阻止 [VSCode 自己踩过的"settings 成垃圾桶"反模式](https://code.visualstudio.com/api/ux-guidelines/activity-bar)）。**ADR-021 不影响**：ADR-015 / ADR-017 / ADR-019 / ADR-020；BossKey 加 hide workspace 一行调用。**P0 spike #32 验证 8✅ + 1❌**（⑧ Tauri popout 结构性不可行，与下文代价 ⑦ 一致；详见 [spikes/workspace-spike/REPORT.md](spikes/workspace-spike/REPORT.md)）。

- **分层契约（强制）**：
  - **WorkspaceManager**（`src/lib/workspace/manager.ts`，纯 TS service，无 Vue / dockview 依赖）：管 panel registration / state / lifecycle / commands / when DSL 求值 / layout serialize / contextKey 反应式订阅。API：`registerPanel(d) / openPanel(id, options?) / revealPanel(id) / closePanel(id, force?) / serializeLayout() / loadLayout(json) / setContextKey(k,v) / getContextKey(k)`。100% 单测覆盖。
  - **DockviewAdapter**（`src/lib/workspace/dockview-adapter.ts`，renderer 桥接层）：监听 WorkspaceManager state 变更并调 dockview API；**是唯一被允许 `import` 任何 dockview API 的模块**。
  - **业务代码强制约束**（panel 组件 / pinia store / IPC handler / composable）：**禁止** `import` 任何 dockview API；所有 panel 操作走 WorkspaceManager 实例（`provide/inject` 或 `useWorkspaceManager()` composable 注入）。代码 review 须把关。
  - 设计原型：[VSCode `ILayoutService` / `IViewsService` / `IEditorService`](https://code.visualstudio.com/api/references/contribution-points)；保证未来若 dockview 出问题可换 GoldenLayoutAdapter / 自封 adapter 而不动业务。

- **PanelDescriptor schema（13 字段）**：

  ```ts
  interface PanelDescriptor {
    // 核心
    id: string                                      // 'settings.theme' | ...
    title: string | ((instance?) => string)         // 多实例下函数化
    component: () => Promise<Component>             // 懒加载 .vue
    category: 'chat' | 'task' | 'creation' | 'config' | 'debug' | 'play'

    // 行为
    singleton?: boolean                             // 默认 true
    instanceKey?: (props) => string                 // 多实例必需
    closable?: boolean                              // 默认 true
    beforeClose?: (instance) => Promise<boolean>    // 未保存拦截
    mountStrategy?: 'lazy' | 'always' | 'on-demand' // 映射 dockview renderer：lazy→onlyWhenVisible（默认省内存）/ always→always（保 DOM 给含表单的 panel）/ on-demand→首次后切 always

    // 入口
    defaultLocation?: 'main' | 'main.right' | 'bottom'
    icon?: Component
    when?: string                                   // VSCode-style when clause DSL

    // 命令面板（MVP 同期）
    commands?: CommandDescriptor[]                  // 该 panel 暴露给 Ctrl+Shift+P
  }
  ```

  `when` 例：`"persona.active && consent.granted"` / `"!debug.banned"` / `"persona.active && (debug.enabled || dev.mode)"`。MVP 实现 ~30 行 mini-parser 支持 `&&` `||` `!` 三操作符 + 简单 key 求值（[VSCode when clause](https://code.visualstudio.com/api/references/when-clause-contexts) 同款 DSL）；M3+ 视需求扩展 `==` `!=` `in` 等不破坏 schema。**数据订阅生命周期由 service 自主管理**（如 AgentService 持续 buffer 所有 tool calls；panel mount 时从 buffer 读 + listen update，unmount 时仅取消 listen 不影响 service 运行）—— 不在 descriptor 层放 `alwaysSubscribe` 字段（违反"service 单一真相源"原则）。

  21 panel 跨 M1-M5 注册：chat.hub (M4) / tasks.{reminder,pomodoro_console,todo} (M2-M3) / personas.{list,workshop,sandbox} (M2) / wardrobe.{studio,gallery,anniversary} (M4) / memory.browser (M3) / debug.{agent_tools,llm_console} (M3) / settings.{theme,provider,persona,nickname,voice,shortcuts} (M1-M3) / help.{about,changelog} (M1)。

- **迁移阶段**：
  - **P0 spike + 决策**（已完成，issue [#32](https://github.com/tl0502/APET/issues/32) `a370204` on `spike/dockview-poc` 分支）：dockview-vue 6.3.0 isolated POC 验 9 项 — ①SFC 集成 ✅ / ②EP token ✅ / ③popper z-index ✅ / ④中文 IME ✅ / ⑤ResizeObserver ✅ / ⑥bundle ~60KB gzip ✅ / ⑦keep-alive 无泄漏 ✅ / ⑧popout Tauri 结构性 ❌（[Tauri #14263](https://github.com/tauri-apps/tauri/issues/14263)，与代价 ⑦ MVP 不做一致）/ ⑨shadow DOM ✅。**4 实操坑（P1 必须遵循）**：DockviewAdapter 必须自带 ResizeObserver 喂 `api.layout` / panel 用 component registry 非 slot / panel SFC props 嵌套 `{params:{params,api,containerApi,tabLocation}}` 需 `PanelContext<T>` 工具类型 / popout Tauri 不可用。详见 [spikes/workspace-spike/REPORT.md](spikes/workspace-spike/REPORT.md)
  - **P1 workspace shell**（3-4d）：tauri.conf.json 加 label='workspace'；WorkspaceManager 实现 + 100% 单测（panel 注册去重 / openPanel 幂等 / revealPanel 已开未开两路径 / when DSL 求值 / layout serialize 往返 / contextKey 变化触发 panel 显隐）+ DockviewAdapter + when mini-parser；WorkspaceShell.vue + ActivityBar.vue + PanelRegistry 数据；3 个空 panel 占位；3 路唤起；命令面板基础设施 + 命令注册器
  - **P2 迁移现有 panel + Chat 抽取**（3-4d）：settings 5 + tasks 3 panel 全迁；chat 拆 `ChatBody.vue` 业务壳层 + `ChatSession` 业务对象 + `ConversationTab` panel 局部状态；删 settings.html / tasks.html 及对应 Tauri 窗；hideSettings/hideTasks IPC 替换为 `workspaceManager.openPanel(id)`；托盘菜单精简
  - **P3 layout 持久化 + 抛光**（1-1.5d）：workspace:layout KV + 防御性 load（已删 panel id 自动 fallback）+ 重置布局菜单 + [desktop-ui-principles §7 反例自检](design/desktop-ui-principles.md)
  - **P4 hub chat 入驻**（M4 B.3.e 自然完成，原计划独立 hub 窗 → workspace 一个 panel）
  - **总计 8-11.5d**（不含 hub chat M4）；插入时机为 [#30](https://github.com/tl0502/APET/issues/30) / [#31](https://github.com/tl0502/APET/issues/31) follow-up commit close 之后

- **代价**：① dockview-vue 6.3.0 **~60KB gzip 实测**（P0 spike #32 验证，远低于原估 150KB；合 EP ~370KB 仍在 ADR-017 < 12% 体积预算内；周下载 920 = "not popular" 社区小，踩坑要靠源码 + spike 报告 4 实操坑）；② 主题需写 dockview CSS variable → aipet token 桥接 ~50 行；③ [tray.rs](../src-tauri/src/services/tray.rs#L5) "左键单击/双击托盘图标 → 无操作"决策被推翻（误触风险评估为零，commit 注释加说明）；④ chat.hub MVP `singleton:true` 限制（M4 视呼声开放；数据层已就位无需重构）；⑤ [#29 todo](https://github.com/tl0502/APET/issues/29) 等 P0 spike + ADR 出后才开工（拖延 1-2d，省一次后续迁移）；⑥ ChatApp.vue 460+ 行抽 `ChatBody.vue` + `ChatSession` + `ConversationTab` +1.1-1.5d（回报：M4 多实例零重构 + 两形态改一处即可，符合 ADR-015 数据层共享设计）；⑦ MVP 不支持 panel undock 飘出（M4+ 视需求加；天然规避 dockview popout window 在 Tauri 跨平台不一致问题）；⑧ workspace 体验失败可 rollback ≤ 0.5d（panel 组件本身零改动可回独立窗，WorkspaceManager 单测和 Adapter 也可丢弃）；⑨ Tauri 多窗 → 单窗使 [capabilities](https://v2.tauri.app/security/capabilities/) 集中，frontend XSS blast radius 由"该窗权限"扩大到"workspace 全部权限"；桌宠场景真实威胁低（无远程内容 / 无第三方插件 / Markdown 输出已 sanitize），但 P1+ 接插件系统时需回头评估"插件沙盒窗"或启用 [Isolation Pattern](https://v2.tauri.app/concept/inter-process-communication/isolation/)；⑩ WorkspaceManager 100% 单测是 P1 出口必需（放弃单测就放弃 R1 分层全部价值），+0.5d 已计入 P1 工时。

- **Supersedes**：[desktop-ui-principles.md §1](design/desktop-ui-principles.md) "多窗 ≠ 单页路由"隐含的"工具型窗都独立化"默认（改写见该文档 *Updated 2026-05-20*）；[tray.rs:5](../src-tauri/src/services/tray.rs#L5) "左键单击/双击托盘图标 → 无操作"决策（双击改为 workspace toggle）；原 SettingsApp.vue / TasksApp.vue 中 ~70 行 ElTabs 左排自绘 ✕ 复制粘贴模板（P2 后两文件删除）。
- **Updated 2026-05-21（#33 phase B-redo 砍 dockview）**：P2 实施中决定砍掉 dockview-vue + DockviewAdapter + WorkspaceManager + PanelRegistry + when DSL + fuzzyMatch + persistence 7 src 文件 + 6 单测（~6800 LOC 净删）。改为手写三栏 Desktop App Shell（[BrandBar.vue](../src/views/workspace/BrandBar.vue) + [MasterColumn.vue](../src/views/workspace/MasterColumn.vue) + [DetailColumn.vue](../src/views/workspace/DetailColumn.vue) + [SashHandle.vue](../src/views/workspace/SashHandle.vue)）+ [workspaceLayout.ts](../src/stores/workspaceLayout.ts) pinia store（4 category × N item KV）+ DetailColumn `v-show` 全挂载 switch（保 VRM RAF / Tauri listeners / scroll / 表单状态）。**为什么砍**：本 ADR 验收是"8 panel 行为等价旧独立窗"——旧独立窗本来就没 panel drag-drop / tab tear-off / persistent layout，dockview 提供的是超规格能力；panel 数 8，DetailColumn switch 比 13 字段 schema 诚实。**M4 重引信号**：若 personas.workshop multi-instance / wardrobe.studio / hub-chat 真需要 panel tear-off 或 drag-drop，再评估 dockview 或自封 layout 引擎（panel SFC 本身零改动可回独立窗或 dock 容器，workspaceLayout state 可丢弃）。**保留生效**：Activity Bar ≤ 7 项硬约束（现 BrandBar 4 category + 底栏 2）、双入口共享 service（pomodoro / chat）、chat.hub singleton:true、Onboarding 完成引导、tray 左键双击唤起 workspace。**P0 spike #32 9✅+1❌结论仍有效**（spike 确认了 dockview 能力上限，是判断"本 milestone 不需要它"的依据；[REPORT.md](spikes/workspace-spike/REPORT.md) 归档保留）。

---

## 命名约定

新决策：`D-<NNN>-<kebab-case-title>`，编号单调递增。当前空闲：**ADR-022**。

被覆盖的决策不删除，在原条目末尾加 `**Supersedes**：ADR-XXX (理由)`。
