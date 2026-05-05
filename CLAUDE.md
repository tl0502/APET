# AI 桌宠（AIPET）— Claude Code 项目导引

> 单人 vibecoding 项目，10 周 MVP。本文件给新 session 提供入场上下文，详细文档在 `docs/`。

## 你正在帮我做什么（WHY）

一个由用户亲手塑造、会主动关心、能一起玩的 AI 桌宠。三引擎差异化：

1. 用户自主人格（`.soul.md` 完全归用户）
2. 主动陪伴（本地空闲信号，不读屏幕）
3. 共同活动（物理交互、装扮、声音、本地 + LLM 小游戏）

## 项目地图（WHAT）

- **项目根**：`D:/Project/temp/4/`
- **文档库**：`docs/`（已工程化，单一权威源）
- **代码**：尚未启动；M1 第一天会建 Tauri 2.x + Vue 3 + TS + Pinia + Vite 脚手架
- **当前阶段**：立项已完成（15 项 ADR 敲定），M1 即将启动

## 入场标准动作（HOW，重要）

**新 session 第一件事**：读 `docs/STATUS.md` —— 它告诉你上次到哪了、下一步做什么、有没有阻塞。

之后按需读：

- `docs/README.md` —— 文档地图
- `docs/roadmap/development-roadmap.md` —— 当前 milestone 详细
- `docs/decisions.md` —— 15 项决策记录（不要重新讨论已决事项）
- `docs/WORKFLOW.md` —— 我的工作约定（commit 风格、文档头规范等）
- `docs/github-workflow.md` —— GitHub Issues 工作流（labels / milestones / 命名约定）

不需要每次都全读；按当前任务需要按需展开。

## 技术栈（必须遵守）

- **桌面框架**：Tauri 2.x（Rust 主进程 + WebView2 前端）—— 不是 Electron
- **前端**：Vue 3 + TypeScript + Pinia + Vite —— 不是 React
- **3D 渲染**：Three.js + `@pixiv/three-vrm`
- **存储**：SQLite + WAL；secrets 用 DPAPI 加密
- **LLM**：OpenAI 兼容协议 + 6 个 preset（OpenAI / DeepSeek / Moonshot / Qwen / Ollama / 自定义）

## 行为约定（重要）

- 这是单人项目；不要建议加 reviewer / CI / 灰度发布 / KPI 阈值门禁等团队流程。
- 决策记在 `docs/decisions.md`（单文件 ADR-NNN，三句话）；不要建独立 `adr/` 目录。
- 文档头是 3 字段 YAML frontmatter（title / updated / related）；不要扩成 9 字段。
- commit 风格 `<type>: <subject>`，type 自由不强制。
- 每个 session 结束前，**更新 `docs/STATUS.md`**（用 `/sync-status` 命令；详见 `docs/WORKFLOW.md` §8）。
- **任务级清单在 GitHub Issues**，不在 STATUS.md（详见 `docs/github-workflow.md`）。
- 远端仓库**未接入**时，gh 命令调用应回退为只更新本地文件；不要强行报错。

## 常用命令

- `/resume` —— 召回项目上下文（读 STATUS + README + 当前 milestone + 最近 5 个开放 issue）
- `/new-task <描述>` —— 从对话创建 GitHub issue（自动推断 type/module label + 反向链接）
- `/sync-status` —— session 末同步：关闭已完成 issue + 更新 STATUS.md
- 文档术语查询 → `docs/GLOSSARY.md`
- 历史变更 → `docs/CHANGELOG.md`

## GitHub 仓库

- 远端 URL：**https://github.com/tl0502/APET**
- 接入步骤见 `docs/github-workflow.md §8`
