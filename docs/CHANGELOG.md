---
title: 文档变更日志
updated: 2026-05-05
related:
  - README.md
  - WORKFLOW.md
---

# 文档变更日志

记录基线文档的演化。新条目加在顶部。日期格式 YYYY-MM-DD。

> 单人项目，常规 commit 信息以 git log 为准；本文件只记里程碑级变化。

---

## 2026-05-05

### 项目记忆系统 + GitHub Issues 工作流

- 新增 `CLAUDE.md`（项目根）—— Claude Code 入口；WHY/WHAT/HOW 三段 + 行为约定 + 常用命令。
- 新增 `docs/STATUS.md` —— 当前进度快照（任务级清单交给 GitHub Issues）。
- 新增 `docs/github-workflow.md` —— Labels 体系（type/module/priority/status）+ Milestones（M1-M5/P1）+ 命名约定 + 接入步骤。
- 新增 `docs/templates/status-template.md`、`.github/ISSUE_TEMPLATE/{feat,spike,fix}.yml`、`.gitignore`。
- 新增 3 个自定义 slash 命令：`/resumex`（加读最近 5 个开放 issue）、`/new-task`（创建 issue）、`/sync-status`（关闭完成 issue + 更新 STATUS）。
- 修改 `WORKFLOW.md`（加 §8 Session 进度管理 + §9 GitHub Issues 工作流）、`README.md`（新人路径加任务跟踪指引）。
- **远端仓库未接入**（先做设计，接入推迟到用户决定时机）。

### 单人化简化

- 删除 M0 决策周设定（W0 改称"立项准备"，不再算独立 milestone）。
- 折叠 ADR 形式化：`adr/` 目录改为 `decisions.md` 单文件，每决策三句话；删除 Proposed/Accepted/Superseded 状态机叙事。
- `CONTRIBUTING.md` 改名 `WORKFLOW.md`，重写为单人工作约定（删 reviewer / CI / 多层分支策略）。
- frontmatter 从 9 字段瘦身到 3 字段（title / updated / related）。
- 删除 KPI 阈值与杀死指标（telemetry-uat 改为"建议观察口径"）；删除 M5 W1/W2 双门灰度，改为"自测一周即可发"。
- 简化 roadmap：删 §7 状态门 / §6 风险登记表 / §8 分支策略 / §9 任务粒度多层结构。

### 工程化重构

- 文件夹改名：`架构设计/` → `architecture/`，`角色与人格/` → `persona/`，`需求设计/` → `requirements/`；研究/路线图分别独立为 `research/`、`roadmap/`。
- 文件改名：去掉日期前缀与版本后缀，例如 `2026-05-01-ai-desktop-pet-prd-v1.0.md` → `requirements/prd.md`。
- 入口替换：`BASELINE.md` 重写为 `README.md`。
- 文档头：统一为 YAML frontmatter。
- 新增工程标准件：`WORKFLOW.md`、`GLOSSARY.md`、`CHANGELOG.md`、`templates/doc-template.md`、`decisions.md`、`_archive/README.md`。
- 清理悬空引用：所有指向不存在路径（`M0-ADRs/...md`、`progress/m1.md`、`D:\Project\ai桌宠\`）的链接已替换为决策编号纯文本或指向 `decisions.md`。

## 2026-05-02

### 新增决策 ADR-015

ADR-015《对话面板三形态架构》写入 decisions.md（hub 总面板 + 磁吸浮窗 + 漫画气泡 + ConversationStore 共享）。

### 受影响文档

- `requirements/prd.md`：§7.1 加控制按钮区 + §7.2 重写为 3 形态共存 + §7.12 接收源扩展
- `architecture/system-architecture.md`：§2.2 加 hub 行 + §3.1 拆 ChatPanel view + §4 conversations 加 title/archived 字段 + §5.1 IPC 新增 conversation.* 6 命令
- `requirements/flows.md`：§2 加形态选择分支 + §2.2 形态切换流 + §2.3 磁吸状态机
- `roadmap/development-roadmap.md`：§3.2 模块矩阵新增 ConversationStore / 控制按钮区 / hub 总面板行；ChatService + LLMProvider 拆 B.3.a-f 跨 M1-M5

## 2026-05-01

### 实施基线压平

把 v0.1 → v0.7 系列迭代草稿压平为实施基线，结合立项期 14 项决策结果：

- `requirements/prd.md`：替代 v0.1/v0.3/v0.4/v0.5/v0.6/v0.7
- `architecture/system-architecture.md`：替代 v0.1/v0.2/v0.3/v0.4
- `requirements/flows.md`：替代 v0.3/v0.4/v0.5/v0.6
- `requirements/telemetry-uat.md`：替代 v0.3/v0.5/v0.6
- `persona/persona-design.md`：替代 v0.1/v0.2

### 立项期 14 项 ADR

ADR-001 ~ ADR-014 全部敲定，详见 [decisions.md](decisions.md)。

### 关键决策

- **桌宠渲染管线切换**：原 Live2D Cubism 4 路线在立项期废止（Cubism Core 6 ABI 不兼容 + `pixi-live2d-display` 上游停更），切换到 **VRM 3D**（Three.js + `@pixiv/three-vrm`）。详见 ADR-002 + ADR-003。

## 2026-04-30

### 立项

- `research/competitor-research.md` 初版：Replika / Character.AI / Nomi / Clawster / PetClaw / Desktop Mate 等竞品调研。
