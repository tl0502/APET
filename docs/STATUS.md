---
title: AIPET 项目进度
updated: 2026-05-05
related:
  - ../CLAUDE.md
  - WORKFLOW.md
  - github-workflow.md
  - roadmap/development-roadmap.md
---

# 项目进度（STATUS.md）

> 每个 session 末由 Claude 更新（建议用 `/sync-status` 命令）。新 session 入场先读这个文件。
>
> **任务级清单在 GitHub Issues**（[https://github.com/tl0502/APET/issues](https://github.com/tl0502/APET/issues)）。本文件只保留当前状态快照与历史里程碑摘要。

---

## 当前状态

- **当前 milestone**：立项准备 → M1 过渡（15 项 ADR 敲定，M1 待启动）
- **当前 session 在做**：—
- **下一步**：M1 第 1 天 —— Tauri 2.x + Vue 3 项目脚手架 + 组件库 spike（Naive UI vs Element Plus）
- **阻塞**：无

---

## 已完成（里程碑级摘要）

### 立项准备期（2026-04-30 → 2026-05-05）

- 15 项 ADR 敲定（详见 [decisions.md](decisions.md)）
- 6 份基线文档归档（PRD / 架构 / flows / UAT / 人格 / 路线图）
- 立项档案：[research/competitor-research.md](research/competitor-research.md)
- 文档工程化重构 + 单人化简化（详见 [CHANGELOG.md](CHANGELOG.md) 同期条目）
- 项目记忆系统 + GitHub Issues 工作流设计（CLAUDE.md / STATUS.md / `/resumex` `/new-task` `/sync-status` / `.github/ISSUE_TEMPLATE/` / `.gitignore`）
- 远端仓库接入完成（https://github.com/tl0502/APET，6 milestones M1-M5 + P1，27 labels）

---

## 历史 session 摘要

### 2026-05-05

1. **文档工程化重构**：文件夹改名（中文 → 英文）、统一 YAML frontmatter（3 字段）、删悬空引用、新增 6 份工程标准件。详见 [CHANGELOG.md](CHANGELOG.md)。
2. **单人化简化**：删 M0 决策周设定、ADR 折叠到 `decisions.md` 单文件、删 KPI 阈值门禁、roadmap 删多层任务粒度。
3. **项目记忆系统**：建 `CLAUDE.md` / `STATUS.md` / `/resumex` 命令 / `templates/status-template.md`；WORKFLOW.md 加 §8。
4. **GitHub Issues 工作流设计**：建 `docs/github-workflow.md`（labels / milestones / 命名约定 / 接入步骤）；建 `.github/ISSUE_TEMPLATE/{feat,spike,fix}.yml`；新增 `/new-task` 与 `/sync-status` 命令；升级 `/resumex` 让它读最近 5 个开放 issue；新增 `.gitignore`。
5. **GitHub 仓库接入**：commit `13ab823` / `4afbae8` 已 push 到 https://github.com/tl0502/APET ；建 6 milestones + 27 labels；接入完成。
