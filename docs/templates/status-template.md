---
title: <项目代号> 项目进度
updated: YYYY-MM-DD
related:
  - ../CLAUDE.md
  - WORKFLOW.md
  - github-workflow.md
  - roadmap/development-roadmap.md
---

# 项目进度（STATUS.md）

> 每个 session 末由 Claude 更新（建议用 `/sync-status` 命令）。新 session 入场先读这个文件。
>
> **任务级清单在 GitHub Issues**。本文件只保留当前状态快照与历史里程碑摘要。

---

## 当前状态

- **当前 milestone**：<M? / 阶段名>
- **当前 session 在做**：<一句话，或 `—`>
- **下一步**：<一句话>
- **阻塞**：<如无写「无」>

---

## 已完成（里程碑级摘要）

### <阶段名>（<日期范围>）

- <要点 1>
- <要点 2>

---

## 历史 session 摘要

### YYYY-MM-DD

<一段简述本日完成的关键事项；不必每个 issue 都写，里程碑级即可>

---

> **使用提示**
> - session 末用 `/sync-status` 命令让 Claude 更新本文件；
> - 任务级清单不写在这里 → 用 `/new-task` 创建 GitHub issue；
> - 关键技术决策不写在这里 → 写到 [decisions.md](../decisions.md)；
> - 文件超 200 行时，把过老的「历史摘要」剪到 [CHANGELOG.md](../CHANGELOG.md)。
