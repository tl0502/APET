---
title: AI 桌宠 决策记录
updated: 2026-05-06
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
- **选了什么**：Vue 3 + TypeScript + Pinia + Vite。组件库 Naive UI 或 Element Plus，M1 W1 试一下再定。
- **代价**：放弃 React/Solid 生态；Vue 的 SFC 需要 IDE 插件支持。

### ADR-002 桌宠渲染管线

- **为什么**：原 Live2D Cubism 4 路线在立项期发现 Cubism Core 6 ABI 破坏 + `pixi-live2d-display` 上游停更，无法用。
- **选了什么**：**VRM 3D**（Three.js + `@pixiv/three-vrm`），MIT 开源无授权风险。
- **代价**：3D 模型比 2D Live2D 贵；启动 < 1500ms / 内存 < 150MB 需要验证（M1 spike）。
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

---

## 命名约定

新决策：`D-<NNN>-<kebab-case-title>`，编号单调递增。当前空闲：**ADR-018**。

被覆盖的决策不删除，在原条目末尾加 `**Supersedes**：ADR-XXX (理由)`。
