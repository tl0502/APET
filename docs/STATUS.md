---
title: AIPET 项目进度
updated: 2026-05-22
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

- **当前 milestone**：M2 W3 进行中（7/7 落地 + chrome 适配 ✅；待办 + 物理交互待办）
- **当前 session 在做**：[#36](https://github.com/tl0502/APET/issues/36) workspace chrome 层视觉适配 — 8 phase commit `0a7783f→5d60678`（WorkspaceApp 自绘 chrome 三按钮 + brand-bar 占整列 + panel.css/buttons.css 全局抽取 + 8 panel SFC scoped css 重构 + master 节奏对齐业界 + sash 3 状态视觉 + chat 空状态 CTA + brand-bar 微动效）— typecheck / build / cargo check 全绿
- **下一步**：#36 关闭 → [#29](https://github.com/tl0502/APET/issues/29) Todo + #21 KV 实例化 + LivingPet hook + AI 拆解 IPC 占位
- **阻塞**：无
- **展示窗口**：~10 天后产品展示。M2 三件套 + 磁吸全套 + workspace 三栏壳 + 5+3 panel 内嵌 + chat 主床/磁吸双形态全套就位

---

## Milestone 进度

### M1 W1-W2（壳层 + 对话）— 代码层 ✅ 18/18 + 美化补丁 #27 完成

- W1（数据层 + 渲染）✅ #1-#5 完成
- W2（主态可达 + 对话）✅ #6-#16 + #20 完成
- W2 收尾 ✅ [#21](https://github.com/tl0502/APET/issues/21) Onboarding Step 2-6 + LivingPet 自由活动 + VRM 微动作
- 美化补丁 ✅ [#27](https://github.com/tl0502/APET/issues/27) 三窗 design system 收尾（Apple/Bear neutral + Vercel）

### M2 W3-W4（任务三件套 + 物理交互 + 磁吸 + 人格工坊 + workspace 壳）— 进行中（7/7 完成 + chrome 适配 ✅）

- ✅ [#25](https://github.com/tl0502/APET/issues/25) G: 用户头像上传 v2（cropperjs 圆形裁剪）
- ✅ [#26](https://github.com/tl0502/APET/issues/26) A: VRM 头像导出（实时预览 + 表情/镜头 + DPR 安全）
- ✅ [#22](https://github.com/tl0502/APET/issues/22) C: ReminderService MVP（6 IPC + Scheduler 5s polling + OS 通知 + 桌宠气泡 + Tasks 独立窗）
- ✅ [#28](https://github.com/tl0502/APET/issues/28) D: PomodoroService MVP（5 IPC + drift 校准 + Scheduler 1s + FOCUS 期协作: hard 打断/soft 缓冲 + LivingPet wander 跳过）
  - ↳ ✅ [#31](https://github.com/tl0502/APET/issues/31) follow-up `fb78924`：番茄独立窗（Pomotroid 型 360×480 frameless / 全屏 focus / 三入口 / 位置记忆 / phase-driven AOT / OS 首次关闭通知）+ 顺手修 4 bug
- ✅ [#30](https://github.com/tl0502/APET/issues/30) B.3.c: 磁吸窗口系统 constraint-based partial mesh + Forest-walk solver（262 vitest pass）
- ✅ [#35](https://github.com/tl0502/APET/issues/35) ADR-021 P1: Workspace 主壳 + dockview-vue 6.3.0 集成 + Workspace 域层 + 三入口（Ctrl+Alt+W / 托盘菜单 / 托盘双击）+ onboarding 引导气泡
  - ↳ ✅ phase A-E `959408c→9dee869`：后端窗注册 / WorkspaceManager 域层 + 60 单测 / dockview 集成 + 4 实操坑落地 / 命令面板 / 入口三件套
  - ↳ ✅ phase F audit `795769a`：深度工程审查 49 项 (5 P0+22 P1+18 P2+4 P3) 中 P0+P1 全套落地（状态/race 集群 / 事件回灌 / params 透传 / 并发护栏 / kind 字段 / const slice）
- ✅ [#33](https://github.com/tl0502/APET/issues/33) ADR-021 P2: 三栏 Desktop App Shell 重做（认知偏差 mid-task pivot）+ 5+3 panel 迁入 workspace + chat 主床/磁吸双形态共享 ConversationStore + 删 dockview 节约 ~378KB
  - ↳ ✅ phase A `543213d`：ConversationStore + ChatBody 抽离 + 15 case
  - ↳ ✅ phase B/B-redo `62bb732→6c96112`：5 settings panel git mv + VrmAvatarExporter props.isActive；推倒 dockview 改 60+240+余 三栏（BrandBar / MasterColumn / SashHandle / DetailColumn / workspaceLayout store + 9 case）
  - ↳ ✅ phase C/D `c5c54df→f2fd7da`：tasks 3 panel 迁入 + DetailColumn map / chat 主床（拆 ChatThreadPane + ConversationListPane，ChatBody 化为薄 wrapper）
  - ↳ ✅ phase E `b929721`：删 settings/tasks 独立窗 + 托盘菜单精简（5 项）+ pomodoro_start 不弹浮窗（17 文件 +56/-762）
  - ↳ ✅ phase E review / phase F chat `154cc98→b88641e`：lint :is 顺序 + dockview 残注释 + ADR-021 Updated；chat 表面视觉收敛（浮卡 composer + 卡片化 sidebar + 中性化气泡）
- ✅ [#36](https://github.com/tl0502/APET/issues/36) workspace chrome 层视觉适配：WorkspaceApp 自绘 chrome 三按钮（min/max/close）+ brand-bar 占整列 + panel.css/buttons.css 全局抽取 + 8 panel SFC scoped css 重构 + master 节奏对齐业界（14/15/18 typography + 36px item + sticky backdrop blur）+ sash 3 状态视觉 + chat 空状态 + 新对话 CTA + brand-bar 微动效（active 竖条 + persona pulse） — 8 phase commit `0a7783f→5d60678`（业界 research：Discord/Slack/VSCode/Linear + Apple Big Sur vibrancy + NN/G empty state）
- ⏳ [#29](https://github.com/tl0502/APET/issues/29) E + 衔接: Todo + #21 KV 实例化 + LivingPet hook + AI 拆解 IPC 占位（从原 #22 拆出）
- ⏳ [#23](https://github.com/tl0502/APET/issues/23) N+I+K 物理交互 + 心情/精力 + 摸鱼（含 N.4 RAWINPUT spike）

### 立项准备期（2026-04-30 → 2026-05-05）✅

15 项 ADR 敲定 + 6 份基线文档归档 + 文档工程化 + GitHub 仓库接入 + 项目记忆系统。实施期新增 ADR-016/017/018/019/020/021（脚手架 / EP 选型 / LLM 三层抽象 / Onboarding 续接 / 磁吸窗口 hub-spoke / Workspace 多 panel 壳）。

---

> **历史详情**：每个 issue 完整落地报告在 GitHub 关闭的 issue body+comment 里；每天 session 深度回看在 [_archive/sessions/docs/session记录.md](_archive/sessions/session记录.md)。
