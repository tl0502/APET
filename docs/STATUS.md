---
title: AIPET 项目进度
updated: 2026-05-08
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

- **当前 milestone**：M1 W1-W2（壳层 + 对话）
- **当前 session 在做**：#13 修正落地（IPC 流式契约从全局 emit 迁移到 `tauri::ipc::Channel<StreamEvent>`，cancel/切换会话死锁修复）；#14/#15/#16 余下 bug 待下次 session 处理
- **下一步**：处理 #14（ChatPanel 流式 + ProviderDrawer create 模式 test 误关 drawer bug）/ #15（昵称切换污染对话，需注入 system 转场消息 + persona 切换清旧 nickname）/ #16（视图层 SoulPledgeView）相关 bug 与增强（详 plan：模型探测 / 昵称污染解决方案）
- **阻塞**：无

---

## Milestone 进度

### M1 W1-W2（壳层 + 对话）— 进行中（13/18 issue 完成）

- W1（数据层 + 渲染）✅ #1-#5 完成
- W2（主态可达 + 对话）✅ #6-#13 完成
- 剩余：#14 ChatPanel / #15 昵称 UI / #16 灵魂宣誓 / #17 Onboarding 骨架 / #18 LivingPet

### 立项准备期（2026-04-30 → 2026-05-05）✅

15 项 ADR 敲定 + 6 份基线文档归档 + 文档工程化 + GitHub 仓库接入 + 项目记忆系统。

---

> **历史详情**：每个 issue 完整落地报告在 GitHub 关闭的 issue body+comment 里；每天 session 深度回看在 [_archive/sessions/2026-05.md](_archive/sessions/2026-05.md)。
