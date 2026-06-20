---
title: AIPET 项目进度
updated: 2026-06-20
related:
  - ../AGENTS.md
  - ../CLAUDE.md
  - WORKFLOW.md
  - github-workflow.md
  - lessons.md
  - roadmap/development-roadmap.md
---

# 项目进度（STATUS.md）

> 每个 session 末由 Claude / Codex 更新。Claude 用 `/sync-status`；Codex 按 [AGENTS.md](../AGENTS.md) 的 `Codex sync-status` 等价流程执行。新 session 入场先读这个文件，再扫一眼 [lessons.md](lessons.md) 避免重复掉同一个坑。
>
> **任务级清单在 GitHub Issues**（[https://github.com/tl0502/APET/issues](https://github.com/tl0502/APET/issues)）。本文件只保留**当前状态快照**与**里程碑进度索引**。**历史 session 详情**已归档至 [_archive/sessions/](_archive/sessions/)。

---

## 当前状态

- **当前 milestone**：Companion Agent Runtime v3 Phase A0（Safety & Secrets）✅ 完成；M2 W3 [#23] 物理交互仍待开始
- **当前 session 在做**：A2-C0 人格塑形收口已完成：工坊滑杆/tagline/relationshipStyle/dislikes/initiative 编译为自然语言 `SoulRuntimeProfile.style_prompt`；异常滑杆 clamp + warning；前端 name/capabilities 校验与 Rust blocking 口径对齐；momo/joker/coach 内置 `# 例对话` 补齐。
- **下一步**：进入 A2-C 示例预览 / LLM 辅助生成评估，或回到 M2 [#23] 物理交互 + 心情/精力 + 摸鱼。
- **阻塞**：无
- **展示窗口**：~10 天后产品展示。M2 三件套 + 磁吸全套 + workspace 三栏壳 + L 型 chrome 框 + 5+3 panel 内嵌 + chat 主床/磁吸双形态 + Profile popup 全套就位

---

## Milestone 进度

### M1 W1-W2（壳层 + 对话）— 代码层 ✅ 18/18 + 美化补丁 #27 完成

- W1（数据层 + 渲染）✅ #1-#5 完成
- W2（主态可达 + 对话）✅ #6-#16 + #20 完成
- W2 收尾 ✅ [#21](https://github.com/tl0502/APET/issues/21) Onboarding Step 2-6 + LivingPet 自由活动 + VRM 微动作
- 美化补丁 ✅ [#27](https://github.com/tl0502/APET/issues/27) 三窗 design system 收尾（Apple/Bear neutral + Vercel）

### M2 W3-W4（任务三件套 + 物理交互 + 磁吸 + 人格工坊 + workspace 壳）— 进行中（11/11 完成 ✅）

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
- ✅ [#36](https://github.com/tl0502/APET/issues/36) workspace chrome 层视觉适配：WorkspaceApp 自绘 chrome 三按钮（min/max/close）+ brand-bar 占整列 + panel.css/buttons.css 全局抽取 + 8 panel SFC scoped css 重构 + master 节奏对齐业界 + sash 3 状态视觉 + chat 空状态 + brand-bar 微动效 — 8 phase commit `0a7783f→5d60678`（被 #37 chrome L 型框重做覆盖）
- ✅ [#37](https://github.com/tl0502/APET/issues/37) ADR-021 P3: workspace 重设计 — 实色 48px L 型 chrome 框 + Profile in-workspace popup（880×580 overlay + sidebar nav profile/account/privacy/notifications/help/about 6 项）+ panel.css 容器公约（panel--form 720 / panel--chat 880 / panel--list fluid）+ SettingsNickname/About 搬入 popup + workspace settings 简化为 外观+LLM Provider — 24 commit `dbf03e0→76dedd7`（spec + plan + impl + 4 P0/P1 structural fix + 7 视觉错位 fix）
  - ↳ ✅ structural fix `339d1e6→f52e8be`：workspace.css reset / capabilities min/max / grid 列宽 CSS 变量 / 清 grid 子组件 height:100%
  - ↳ ✅ 错位 fix `832a981→76dedd7`：popup 4 panel 删冗余 panel__title / chat master conv-sidebar 适配 / topbar 整层 drag-region / popup-main min-height:0 / panel__title 实色 bg / **panel__title 删 margin negative**（治 sticky 与 layout 算术冲突 — 关键根因）/ NicknameForm 内嵌 panel__title 改 subtitle
- ✅ [#38](https://github.com/tl0502/APET/issues/38) [设计系统] dark mode token 阶梯改造：tokens.css 单文件 patch（light 背峰式 3+1 + dark 保守型 4 色阶 总跨 28 + dark border #333→#3d 衍生 fix + border-faint 6%→8%/10% + dark bubble-assistant 跟 L2）— 1 commit `d4dff7d` + spec/plan/ADR-024，293/293 vitest pass，4 大窗 × 2 主题手动 e2e 全绿
- ✅ [#34](https://github.com/tl0502/APET/issues/34) ADR-021 P3 收尾：workspace 主窗 rect 跨重启持久化 — window_state.rs 新增 LastRect + WorkspaceSaveDebouncer + apply_initial_workspace_rect 复用 pet/pomodoro pattern + lib.rs Moved/Resized 钩子；min 800×520 自愈 / 拔屏 fallback 主屏 center；1 commit `408a555`，cargo test 230 pass，手动 e2e 5 例全绿
- ✅ [#29](https://github.com/tl0502/APET/issues/29) E + 衔接: TodoService MVP + onboarding KV 实例化 + LivingPet reminder hook + daily 时区修 + UI 扩展（拖排序/priority/批量/搜索/最小日历）— ~45 commits `1a03cf8..4c5b9f5`，cargo test 264 pass / vitest 293 pass / 手动 e2e 16 例全绿（spec §12.3）；新增 lessons §15（tx 注入式）+ §16（REMINDER_TEMPLATES 双写）
- ⏳ [#23](https://github.com/tl0502/APET/issues/23) N+I+K 物理交互 + 心情/精力 + 摸鱼（含 N.4 RAWINPUT spike）

### Companion Agent Runtime v3 — Phase A0（Safety & Secrets，spec 驱动 pre-stability）✅ 8/8 + Task 5b 收尾

> Spec: `docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md`（v3, ~2800 行，已过 third-party review-2）
> Plan: `docs/superpowers/plans/2026-05-24-phase-a0-safety-secrets.md`（8 task，`b6732a9`）
> DoD：CI 黑名单 OS API ✅ / 7-state FSM 单测覆盖 ✅ / DPAPI 真落地 ✅ / 分发 gate 关闭 ✅

- ✅ Task 1 `7fcb879→8f7d01a`：SafetyGuard 7-state FSM + ADR-006 prefix 真注入（include_str! 编译时嵌入 `assets/safety/prefix_v1.txt`）+ review fix
- ✅ Task 2 `10fab2f→3cdd41d`：StateStore Repository pattern + migration 002（messages.token_count/safety_scan_status + secrets.created_at ALTER + context_access_log 新表）+ ALTER 适配 fix
- ✅ Task 3 `f0b1421→146f133`：DenyOnlyPermissionService + context_access_log 审计写入；review fix DenyOnly 不变量（`?` 不能透传 Db/Repo error）
- ✅ Task 4 `688ffee`：GrantBroker trait + DenyAllGrantBroker + MockGrantBroker
- ✅ Task 5a `a70be94→f9a3144`：DPAPI CryptoService + SecretRepo（windows-sys 0.59 + ZeroizeOnDrop derive）+ review fix（SAFETY 注释 / 现代 derive / null 防御）
- ✅ Task 6 `2742c08`：LifecycleManager 5-state FSM + Kernel::boot 1-7 序列 + lib.rs::setup 集成（Tauri 2.x dev mode resource_dir 问题用 include_str! 解）
- ✅ Task 7 `525087c→58e3a2b`：ChatService SafetyGuard 集成 + StreamEvent::ReplaceMessage（4 ReplaceReason 变体）+ history mode filter（safety_redacted/safety_blocked/safety_scan_failed 排除避免 A6-pattern 复现）+ ChatError::SafetyScanFailed
- ✅ Task 8 `a6aae34`：CI 黑名单 OS context API 脚本（8 forbidden symbols：getUserMedia / MediaRecorder / GetForegroundWindow / GetWindowText / BitBlt / ReadClipboardText / GetCursorPos）+ DoD checklist
- ✅ Task 5b `1962101→c16ed83`：LLM Provider API Key DPAPI 加密迁移 SecretRepo（distribution-gate 关闭）+ 收尾 fix（5b 实施时误把 SecretRepo::set 改回 3-列、改回测试 schema、test_db.rs 漂移；c16ed83 还原 4-列 + test_db 加 apply 002 + 设计意图注释回写）
- 测试覆盖：cargo test --lib 358/358 pass，含 Task 5a 5 个 secret_repo + Task 5b 6 个 secret_migration 真 DPAPI round-trip
- Follow-up issues（P1，非分发 gate 阻塞）：
  - [#48](https://github.com/tl0502/APET/issues/48) ChatService test connectivity probe 绕过 SafetyGuard（设计完整性）
  - [#49](https://github.com/tl0502/APET/issues/49) SafetyGuard::scan_token trailing-window 优化（Phase A1 mid-stream scan 落地前 O(n²)→O(window)）

### 立项准备期（2026-04-30 → 2026-05-05）✅

15 项 ADR 敲定 + 6 份基线文档归档 + 文档工程化 + GitHub 仓库接入 + 项目记忆系统。实施期新增 ADR-016/017/018/019/020/021（脚手架 / EP 选型 / LLM 三层抽象 / Onboarding 续接 / 磁吸窗口 hub-spoke / Workspace 多 panel 壳）。

---

> **历史详情**：每个 issue 完整落地报告在 GitHub 关闭的 issue body+comment 里；每天 session 深度回看在 [_archive/sessions/docs/session记录.md](_archive/sessions/session记录.md)。
