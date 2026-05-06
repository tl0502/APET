---
title: AIPET 项目进度
updated: 2026-05-06
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

- **当前 milestone**：M1 W1（壳层 + 对话）
- **当前 session 在做**：—
- **下一步**：[Issue #3](https://github.com/tl0502/APET/issues/3) 接入 PetCanvas + VRM momo 渲染（A.3 vrm spike，关键路径）
- **阻塞**：无

---

## 已完成（里程碑级摘要）

### M1 启动（2026-05-06 起）

- M1-D1 项目脚手架就位（commit 8952e6e）：Tauri 2 + Vue 3 + TS + Pinia + Vite 7 + pnpm；`pnpm tauri:dev` 跑通 320×320 透明窗口；ADR-016 / ADR-017 入库（commit 3426184）。详见关闭的 [Issue #1](https://github.com/tl0502/APET/issues/1)。
- M1-D2 Element Plus + 主题跟随系统就位（commit 7c387db）：EP 全量 import + zh-CN locale + 三态 Pinia 主题 store（auto/light/dark）+ matchMedia/localStorage；`pnpm tauri:build` 实测 release exe = **4.15 MB**（vs 预估 ~9MB，偏好 53%），ADR-017 已补实测；同 commit 清理 4 处文档过时性能预算（`启动 < 1500ms / 内存 < 150MB` → 推到 M5 自测期统一压测）。详见关闭的 [Issue #2](https://github.com/tl0502/APET/issues/2)。

### 立项准备期（2026-04-30 → 2026-05-05）

- 15 项 ADR 敲定（详见 [decisions.md](decisions.md)）
- 6 份基线文档归档（PRD / 架构 / flows / UAT / 人格 / 路线图）
- 立项档案：[research/competitor-research.md](research/competitor-research.md)
- 文档工程化重构 + 单人化简化（详见 [CHANGELOG.md](CHANGELOG.md) 同期条目）
- 项目记忆系统 + GitHub Issues 工作流设计（CLAUDE.md / STATUS.md / `/resumex` `/new-task` `/sync-status` / `.github/ISSUE_TEMPLATE/` / `.gitignore`）
- 远端仓库接入完成（https://github.com/tl0502/APET，6 milestones M1-M5 + P1，27 labels）

---

## 历史 session 摘要

### 2026-05-06

完成 [Issue #1](https://github.com/tl0502/APET/issues/1) M1-D1 项目脚手架：17 个文件落盘（5 前端配置 + 4 前端入口 + 5 Tauri 后端 + 1 capabilities + 2 icons），`pnpm install / typecheck / lint` + `cargo check` + `pnpm tauri:dev` 全部通过。**实施期发现**：本机 Windows HyperV TCP 排除范围 1423-1522 包含原计划端口 1430，改用 Tauri 2 默认 1420 + HMR 1421。**ADR-016**（脚手架技术栈）+ **ADR-017**（Element Plus 全量 import + 主题跟随系统）入库（commit 3426184）；脚手架代码 commit 8952e6e 已 push。

完成 [Issue #2](https://github.com/tl0502/APET/issues/2) Element Plus + 主题跟随系统（commit 7c387db）：EP 2.13 全量 import + zh-CN locale + dark css-vars；新建 `src/stores/theme.ts` 三态 Pinia store（auto/light/dark）+ matchMedia listener + localStorage 持久化；`useThemeStore().init()` 在 mount 之前调用避免 FOUC。**关键取舍**：不引 VueUse `useDark`（三态 mode 与其布尔语义不匹配）；验证 demo 不放 PetCanvas 主壳（违反 PRD §7.2 角色窗透明约束），改为功能性验证后即清理。**实测**：`pnpm tauri:build` release exe = **4.15 MB**（vs ADR-017 预估 ~9MB，偏好 53%），实测数字已写入 ADR-017。**同时清理 4 处文档过时性能预算**（`启动 < 1500ms / 内存 < 150MB` → 推到 M5 自测期统一压测）：decisions.md ADR-002、prd.md §22、architecture §M0 节点、roadmap §4.1 spike 表。

新建 [Issue #3](https://github.com/tl0502/APET/issues/3) 接入 PetCanvas + VRM momo 渲染（type:spike, module:A-shell, priority:p0），M1 W1 关键路径，下一 session 启动。

### 2026-05-05

1. **文档工程化重构**：文件夹改名（中文 → 英文）、统一 YAML frontmatter（3 字段）、删悬空引用、新增 6 份工程标准件。详见 [CHANGELOG.md](CHANGELOG.md)。
2. **单人化简化**：删 M0 决策周设定、ADR 折叠到 `decisions.md` 单文件、删 KPI 阈值门禁、roadmap 删多层任务粒度。
3. **项目记忆系统**：建 `CLAUDE.md` / `STATUS.md` / `/resumex` 命令 / `templates/status-template.md`；WORKFLOW.md 加 §8。
4. **GitHub Issues 工作流设计**：建 `docs/github-workflow.md`（labels / milestones / 命名约定 / 接入步骤）；建 `.github/ISSUE_TEMPLATE/{feat,spike,fix}.yml`；新增 `/new-task` 与 `/sync-status` 命令；升级 `/resumex` 让它读最近 5 个开放 issue；新增 `.gitignore`。
5. **GitHub 仓库接入**：commit `13ab823` / `4afbae8` 已 push 到 https://github.com/tl0502/APET ；建 6 milestones + 27 labels；接入完成。
