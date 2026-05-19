---
title: AIPET 项目进度
updated: 2026-05-18
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

- **当前 milestone**：M2 W3 进行中（提醒 + 番茄 + 磁吸已落地，待办/物理交互/人格工坊待办）
- **当前 session 在做**：[#30](https://github.com/tl0502/APET/issues/30) 磁吸窗口系统 constraint-based partial mesh + Forest-walk solver（uncommitted；114 vitest 通过）→ [#31](https://github.com/tl0502/APET/issues/31) follow-up A（T1-T5：view_preset 联动 + preview indicator + commit tween + chat header 36×36 + pet drag hook + AOT 托盘开关）→ follow-up B（T6-T10：group-drag anchor 角色 + 矩形 outline 沿对接边 + TRIGGER_ZONE 24→60 + chat header grip 纹 + AOT 前端兜底）。共 132 vitest pass / typecheck / lint / cargo check 通过
- **下一步**：dev 实测 8 项（plan §6.2 / zippy-baking-dusk.md §T11.5）→ 通过则 commit 拆 3 + close #30 #31 → 否则补 fix
- **阻塞**：无
- **展示窗口**：~10 天后产品展示。M2 三件套 #22 + #28 已就位（+ 番茄独立窗 + 磁吸 follow-up 加分），10 天 demo 链路完整：「设置提醒 → 启动番茄 → FOCUS 期硬提醒打断 / 软提醒缓冲 → REST 时合并展示」+ chat 吸 pet 联动拖动

---

## Milestone 进度

### M1 W1-W2（壳层 + 对话）— 代码层 ✅ 18/18 + 美化补丁 #27 完成

- W1（数据层 + 渲染）✅ #1-#5 完成
- W2（主态可达 + 对话）✅ #6-#16 + #20 完成
- W2 收尾 ✅ [#21](https://github.com/tl0502/APET/issues/21) Onboarding Step 2-6 + LivingPet 自由活动 + VRM 微动作
- 美化补丁 ✅ [#27](https://github.com/tl0502/APET/issues/27) 三窗 design system 收尾（Apple/Bear neutral + Vercel）

### M2 W3-W4（任务三件套 + 物理交互 + 磁吸 + 人格工坊）— 进行中（5/7 完成）

- ✅ [#25](https://github.com/tl0502/APET/issues/25) G: 用户头像上传 v2（cropperjs 圆形裁剪）
- ✅ [#26](https://github.com/tl0502/APET/issues/26) A: VRM 头像导出（实时预览 + 表情/镜头 + DPR 安全）
- ✅ [#22](https://github.com/tl0502/APET/issues/22) C: ReminderService MVP（6 IPC + Scheduler 5s polling + OS 通知 + 桌宠气泡 + Tasks 独立窗）
- ✅ [#28](https://github.com/tl0502/APET/issues/28) D: PomodoroService MVP（5 IPC + drift 校准 + Scheduler 1s + FOCUS 期协作: hard 打断/soft 缓冲 + LivingPet wander 跳过）
  - ↳ ✅ [#31](https://github.com/tl0502/APET/issues/31) follow-up `fb78924`：番茄独立窗（Pomotroid 型 360×480 frameless / 全屏 focus / 三入口 / 位置记忆 / phase-driven AOT / OS 首次关闭通知）+ 顺手修 4 bug（PomodoroPanel 倒计时 / listener race / 全屏 hide 保留 / hide_hint toast 不可见）
- 🟡 [#30](https://github.com/tl0502/APET/issues/30) B.3.c: 磁吸窗口系统 constraint-based partial mesh + Forest-walk solver（uncommitted；114 vitest pass；待 dev 实测 → commit + close）
  - ↳ 🟡 [#31](https://github.com/tl0502/APET/issues/31) follow-up A T1-T5（uncommitted；plan `pure-beaming-salamander.md`）：view_preset onResized + solver / preview indicator dragSession reactive ref / commit tween 150ms cubic / chat header 36×36 + hover scale / pet drag hook 扩 selector / AOT 托盘开关 + window_state.rs backend 持久化
  - ↳ 🟡 [#31](https://github.com/tl0502/APET/issues/31) follow-up B T6-T10（uncommitted；plan `zippy-baking-dusk.md`）：anchor 角色 group-drag 模式（修拖 pet 脱钩 bug；Winamp/VS docking）/ 矩形 outline 沿对接边 + 渐进 intensity / TRIGGER_ZONE 24→60 / chat header grip 纹 / AOT 前端 listen 兜底
- ⏳ [#29](https://github.com/tl0502/APET/issues/29) E + 衔接: Todo + #21 KV 实例化 + LivingPet hook + AI 拆解 IPC 占位（从原 #22 拆出）
- ⏳ [#23](https://github.com/tl0502/APET/issues/23) N+I+K 物理交互 + 心情/精力 + 摸鱼（含 N.4 RAWINPUT spike）

### 立项准备期（2026-04-30 → 2026-05-05）✅

15 项 ADR 敲定 + 6 份基线文档归档 + 文档工程化 + GitHub 仓库接入 + 项目记忆系统。实施期新增 ADR-016/017/018/019/020（脚手架 / EP 选型 / LLM 三层抽象 / Onboarding 续接 / 磁吸窗口 hub-spoke）。

---

> **历史详情**：每个 issue 完整落地报告在 GitHub 关闭的 issue body+comment 里；每天 session 深度回看在 [_archive/sessions/docs/session记录.md](_archive/sessions/session记录.md)。
