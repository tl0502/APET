# AI 桌宠 文档基线（单一权威入口）

- 项目代号：AIPET（个人项目，自用为主）
- 适用阶段：**MVP 实施期**（M1-M5，10 周）

> **新人路径**：先读本页 §一句话定位 → §五份对齐文档 → 路线图，5 分钟可入场。
> **实施期入场**：直接读 [STATUS.md](STATUS.md)（当前进度 / 下一步 / 阻塞）；Claude 用 `/resumex`，Codex 按根目录 [AGENTS.md](../AGENTS.md) 的 `Codex resumex` 召回上下文。
> **任务跟踪**：GitHub Issues（详见 [github-workflow.md](github-workflow.md)）；Claude 用 `/new-task` 与 `/sync-status`，Codex 按 `Codex new-task` 与 `Codex sync-status` 等价流程维护。
> **工作约定**：见 [WORKFLOW.md](WORKFLOW.md)；长期协作记忆见 [agent-memory.md](agent-memory.md)；术语解释见 [GLOSSARY.md](GLOSSARY.md)；决策记录见 [decisions.md](decisions.md)。

---

## 一句话定位

**`一个由你亲手塑造、会主动关心你、能和你一起玩的 AI 桌宠`**

三引擎差异化：

1. **用户自主人格** —— `.soul.md` 完全归属用户（参考 OpenClaw）。
2. **主动陪伴** —— 桌宠"在那里"被感知（基于本地空闲信号，不读屏幕内容）。
3. **共同活动** —— 物理交互、装扮、声音表情、本地 + LLM 小游戏。

---

## 五份对齐文档（实施期权威源）

按推荐阅读顺序：

| # | 文档 | 路径 | 用途 |
|---|---|---|---|
| 1 | **PRD** | [requirements/prd.md](requirements/prd.md) | 业务需求、模块清单、版本计划 |
| 2 | **架构** | [architecture/system-architecture.md](architecture/system-architecture.md) | 技术栈、服务边界、SQLite schema、IPC、文件布局 |
| 3 | **人格设计** | [persona/persona-design.md](persona/persona-design.md) | `.soul.md` schema、3 个内置人格、安全前缀拼装 |
| 4 | **flows** | [requirements/flows.md](requirements/flows.md) | Onboarding、状态机、关键流程图 |
| 5 | **埋点 UAT** | [requirements/telemetry-uat.md](requirements/telemetry-uat.md) | 事件字典、观察口径、自测场景 |

实施路线图：

| 文档 | 路径 | 用途 |
|---|---|---|
| **开发路线图** | [roadmap/development-roadmap.md](roadmap/development-roadmap.md) | M1-M5 甘特图、模块依赖 DAG、关键路径 |

---

## 决策记录

立项期已写下 **15 项关键决策**（ADR-001 到 ADR-015），全部归在 [decisions.md](decisions.md)。

新决策从 ADR-016 起，每条三句话（为什么/选什么/代价）。

---

## 关键约束（贯穿所有决策）

1. **Local-first**：不引入用户数据强制上传。
2. **用户自主权**：不削弱用户对 `.soul.md` / 装扮 / 设置的控制。
3. **非养成原则**：不引入流失 / 死亡 / 必须签到机制。
4. **隐私边界**：不读应用名 / 窗口标题 / 输入内容 / 麦克风。
5. **安全护栏不可绕过**：任何人格 / 游戏场景不能覆盖系统安全前缀。

---

## MVP 实施路线（10 周）

> W0 是立项准备（决策已敲定），M1-M5 是真正的开发期。

| 里程碑 | 周次 | 主要交付 |
|---|---|---|
| **M1** | W1-W2 | Tauri + Vue 3 项目骨架、桌宠壳层、对话、Onboarding、灵魂宣誓、自由活动初版、U.1/U.2 昵称 |
| **M2** | W3-W4 | 任务三件套（C/D/E）、人格系统、心情/精力、摸鱼、N 物理交互（含 RAWINPUT spike）|
| **M3** | W5-W6 | 记忆、隐私治理、自动更新、情境关心（J）、文件拖入（L）、R.3 桌宠日常 |
| **M4** | W7-W8 | O 装扮（配饰 + 节气）、P 声音表情（音效 + 静音）、S.4 用户纪念日 |
| **M5** | W9-W10 | Q 小游戏（本地 3 + LLM 2）、优化、自测、可发布版 |

P1 路线：M6-M7 R1 情感深化 / M8-M9 R2 效率与娱乐深化 / M10+ R3 生态扩展（详见 [requirements/prd.md](requirements/prd.md) §5.2）。

---

## 性能预算速查

| 项 | 预算 |
|---|---|
| 总常驻内存 | ≤ 250MB |
| 总安装包 | ≤ 80MB |
| 冷启动 | ≤ 5 秒 |
| 对话首 token | p50 ≤ 1.5s |
| 物理交互响应 | < 100ms |
| 装扮切换 | < 500ms |
| 声音播放延迟 | < 50ms |
| 本地游戏每轮 | < 50ms |
| 摸鱼切换 | < 200ms |

详见 [requirements/prd.md](requirements/prd.md) §10 与 [architecture/system-architecture.md](architecture/system-architecture.md) §13。

---

## 立项档案

- [research/competitor-research.md](research/competitor-research.md) — Replika / Character.AI / Nomi / Clawster / PetClaw / Desktop Mate / OpenClaw / OpenAI Codex Pets / Microsoft Copilot Vision / Amazon Quick 等。

---

## 目录结构

```
docs/
├── README.md              ← 本文件（单一权威入口）
├── STATUS.md              ← 项目进度（实施期入场先读）
├── WORKFLOW.md            ← 我的工作约定
├── agent-memory.md        ← Agent 长期协作记忆（Claude memory 迁移）
├── GLOSSARY.md            ← 项目术语表
├── github-workflow.md     ← GitHub Issues 工作流
├── decisions.md           ← 决策记录（ADR-001 ... ADR-015）
│
├── architecture/
│   └── system-architecture.md
├── requirements/
│   ├── prd.md
│   ├── flows.md
│   └── telemetry-uat.md
├── persona/
│   └── persona-design.md
├── research/
│   └── competitor-research.md
├── roadmap/
│   └── development-roadmap.md
├── design/
│   └── desktop-ui-principles.md  ← 前端是桌面应用不是网页（UI 范式约束）
│
├── _archive/              ← 历史版本归档（仅供回溯）
│   └── README.md
└── templates/
    └── doc-template.md
```

---

## 历史归档说明

`_archive/` 用于保留 v0.x 系列历史版本（实施前的迭代草案）。日常以 git 提交记录为准；归档目录是离线快照。
