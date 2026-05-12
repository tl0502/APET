---
title: AI 桌宠 决策记录
updated: 2026-05-12
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
- **Updated 2026-05-12（修「重来」启动期跳过 SoulPledge bug）**：原文写"「重来」= 清 KV"。实测发现：用户「重来」后停在 SoulPledge 关窗 → 启动期 `consent.granted=true` + KV 不存在 → 错认为"已完成 onboarding" → 跳过 SoulPledge 直接进 pet 主态。修正：「重来」改为 **写 KV='soul-pledge'**（不 clear）。`'soul-pledge'` 不在前端 RESUMABLE_STEPS 中,onMounted 看到该值不弹模态,正常显示 Step 1;但 KV 存在足以让启动期路由保持"未完成 onboarding"判定,开 onboarding 窗。`onboarding_reset` IPC / `resetOnboarding` 前端函数保留(M3 G "重置应用数据" 入口未来可用),当前 onboarding 流程不再调用。

---

## 命名约定

新决策：`D-<NNN>-<kebab-case-title>`，编号单调递增。当前空闲：**ADR-020**。

被覆盖的决策不删除，在原条目末尾加 `**Supersedes**：ADR-XXX (理由)`。
