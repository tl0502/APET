---
title: AIPET 项目进度
updated: 2026-05-11
related:
  - ../CLAUDE.md
  - WORKFLOW.md
  - github-workflow.md
  - lessons.md
  - roadmap/development-roadmap.md
---

# 项目进度（STATUS.md）

> 每个 session 末由 Claude 更新（建议用 `/sync-status` 命令）。新 session 入场先读这个文件，再扫一眼 [lessons.md](lessons.md) 避免重复掉同一个坑。
>
> **任务级清单在 GitHub Issues**（[https://github.com/tl0502/APET/issues](https://github.com/tl0502/APET/issues)）。本文件只保留**当前状态快照**与**里程碑进度索引**。**历史 session 详情**已归档至 [_archive/sessions/](_archive/sessions/)。

---

## 当前状态

- **当前 milestone**：M1 W1-W2 收尾期（M1 出口 1 issue 未完）
- **当前 session 在做**：规划 M1 收尾 + M2 双子刀（#21/#22/#23 已创建入库；本 session 未动代码）
- **下一步**：进入 [#21](https://github.com/tl0502/APET/issues/21) 实施（Onboarding Step 2-6 + LivingPet 自由活动 + VRM 微动作），完成后跨入 M2
- **阻塞**：无

---

## Milestone 进度

### M1 W1-W2（壳层 + 对话）— 收尾中（17/18 issue 完成）

- W1（数据层 + 渲染）✅ #1-#5 完成
- W2（主态可达 + 对话）✅ #6-#16 + #20 完成
- 剩余：[#21](https://github.com/tl0502/APET/issues/21) M1 收尾（Onboarding Step 2-6 + LivingPet + VRM 微动作）

### M2 W3-W4（任务三件套 + 物理交互）— 待启动（0/2 计划 issue）

- [#22](https://github.com/tl0502/APET/issues/22) C+D+E TaskService MVP（提醒 + 番茄 + 待办 + Scheduler + OS 通知）
- [#23](https://github.com/tl0502/APET/issues/23) N+I+K 物理交互 + 心情/精力 + 摸鱼（含 N.4 RAWINPUT spike）

### 立项准备期（2026-04-30 → 2026-05-05）✅

15 项 ADR 敲定 + 6 份基线文档归档 + 文档工程化 + GitHub 仓库接入 + 项目记忆系统。实施期新增 ADR-016/017/018（脚手架 / EP 选型 / LLM 三层抽象）。

---

> **历史详情**：每个 issue 完整落地报告在 GitHub 关闭的 issue body+comment 里；每天 session 深度回看在 [_archive/sessions/docs/session记录.md](_archive/sessions/session记录.md)。
