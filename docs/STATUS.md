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

- **当前 milestone**：M1 W1-W2（壳层 + 对话）
- **当前 session 在做**：—
- **下一步**：按优先序推进 13 个新 issue（详见 [#6-#18](https://github.com/tl0502/APET/issues)）。第一批可个人测试基线：[#6](https://github.com/tl0502/APET/issues/6) 托盘 → [#7](https://github.com/tl0502/APET/issues/7) 视觉 token → [#8](https://github.com/tl0502/APET/issues/8) 通用容器 → [#9](https://github.com/tl0502/APET/issues/9) 设置面板骨架 → [#10](https://github.com/tl0502/APET/issues/10) 拖动 → [#11](https://github.com/tl0502/APET/issues/11) 全局快捷键。
- **阻塞**：无

---

## 已完成（里程碑级摘要）

### M1 启动（2026-05-06 起）

- M1-D1 项目脚手架就位（commit 8952e6e）：Tauri 2 + Vue 3 + TS + Pinia + Vite 7 + pnpm；`pnpm tauri:dev` 跑通 320×320 透明窗口；ADR-016 / ADR-017 入库（commit 3426184）。详见关闭的 [Issue #1](https://github.com/tl0502/APET/issues/1)。
- M1-D2 Element Plus + 主题跟随系统就位（commit 7c387db）：EP 全量 import + zh-CN locale + 三态 Pinia 主题 store（auto/light/dark）+ matchMedia/localStorage；`pnpm tauri:build` 实测 release exe = **4.15 MB**（vs 预估 ~9MB，偏好 53%），ADR-017 已补实测；同 commit 清理 4 处文档过时性能预算（`启动 < 1500ms / 内存 < 150MB` → 推到 M5 自测期统一压测）。详见关闭的 [Issue #2](https://github.com/tl0502/APET/issues/2)。
- M1-D3 PetCanvas + VRM momo 渲染就位（commit 0df9076）：three@0.184 + @pixiv/three-vrm@3.5；vrm.ts（VRMRuntime 透明背景 + 半身相机 + 1.6Hz 呼吸 + A-pose + spring bone）+ useVRMModel composable + 简化版 PetCanvas（193L → 71L，剥离 hitbox/drag/IPC，推到后续 task）；avatar.vrm 用户私有 .gitignore 屏蔽。详见关闭的 [Issue #3](https://github.com/tl0502/APET/issues/3)。
- M1-D4 IPC 框架就位（commit ff32dda）：Rust `commands::system::ping` + `lib.rs` 注册 `invoke_handler![ping]`；前端 `services/ipc.ts` 统一 `invoke<T>` wrapper + `types/ipc.ts` `IpcError`（带命令名上下文）；M1 不引 ts-rs/specta，类型手写。详见关闭的 [Issue #4](https://github.com/tl0502/APET/issues/4)。
- M1-D5 PersonaService MVP + Memory/Nickname 骨架就位（commit e5ca882）：tauri-plugin-sql 2 + sqlx 0.8 + gray_matter 0.2 + ulid 1；001_init.sql 全 27 表一次建（M2-M5 零迁移）+ 002 persona_snapshots unique idx；启动期 spawn `seed_builtin` UPSERT 内置 momo（personas + persona_snapshots）；11 个 IPC 命令（persona_load/activate + nickname 5 个 + memory KV 4 个）。**整文件复用**旧项目 D:\Project\ai桌宠 dogfood 过的 4 个 .rs + 2 个 SQL + momo.soul.md（38 个测试自带）。详见关闭的 [Issue #5](https://github.com/tl0502/APET/issues/5)。

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

完成 [Issue #3](https://github.com/tl0502/APET/issues/3) PetCanvas + VRM momo 渲染（commit 0df9076，已 push 触发自动关闭）：装 three@0.184 + @pixiv/three-vrm@3.5 + @types/three(dev)；从旧项目 D:\Project\ai桌宠 整文件复制 src/services/vrm.ts（207L）+ src/composables/useVRMModel.ts（45L）；新写简化版 src/components/PetCanvas.vue（71L，剥离 hitbox/drag/IPC/petStore，仅保留 canvas + Loading/Error 兜底文案）；App.vue 引用 PetCanvas。**关键取舍**：vrm.ts 整文件复制（含未消费的 getBounds() ~50L），下个 hitbox task 直接复用避免重写；PetCanvas 严守 spike 边界（hitbox 推到 A.6，drag 推到 N 模块）；WebView 同源访问 public/avatar/avatar.vrm 无需额外 capabilities。**验收**：tauri:dev 视觉确认 momo 渲染正常 + 背景透明（半身像 / 双臂自然下垂 / 轻微呼吸）；typecheck / lint / cargo check 全过。

完成 [Issue #4](https://github.com/tl0502/APET/issues/4) IPC 框架（commit ff32dda，已 push 触发自动关闭）：与 #3 完全并行的后端壳；新建 `src-tauri/src/commands/{mod,system}.rs`（`#[tauri::command] async fn ping() -> "pong"`）+ `src/services/ipc.ts`（统一 `invoke<T>` wrapper，转发 `@tauri-apps/api/core::invoke` 并把异常包成 `IpcError` 带命令名上下文）+ `src/types/ipc.ts`（`IpcError` class）；`lib.rs` 注册 `invoke_handler![commands::system::ping]`。**关键取舍**：M1 不引 ts-rs/specta（手写类型；M2-M3 视规模决定）；alert 验证 demo 挂在 main.ts dev-only 守卫块（`if (import.meta.env.DEV)`），避开 PetCanvas 透明窗（PRD §7.2），验证通过后清理回原状态；`alert` 而非 `console.log` 是为了让单人 vibecoding 阶段的功能性验证最直观。**实测**：`pnpm tauri:dev` 弹出 `[ipc-verify] ping → pong`；`pnpm typecheck` / `pnpm lint --max-warnings=0` / `cargo check` 三件套 0 错 0 警。

完成 [Issue #5](https://github.com/tl0502/APET/issues/5) PersonaService MVP + Memory/Nickname 骨架（commit e5ca882，已 push 触发自动关闭）：M1 W1 数据层一次性入库。**整文件复用**旧项目 D:\Project\ai桌宠 dogfood 过的 4 个 .rs（persona/memory/nickname/test_db）+ 2 个 SQL migration（001_init 27 张表 + 002 persona_snapshots unique idx）+ momo.soul.md（单文件 + frontmatter）；后端 commands 层新写 3 个 wrapper（persona/nickname/memory）+ 前端 6 个 IPC binding（types + services）。**关键取舍**：① schema 走 architecture §4 完整版（27 表一次建，M2-M5 零迁移成本）而非 issue body §2 极简版（避免后续每个 milestone 写迁移）；② persona 资源用单文件 .soul.md 而非 issue body §1 的目录+meta.json（与 persona-design.md §4.1 / 旧项目 dogfood 一致）；③ persona.activate 写 DB（personas.is_active=1 + persona_snapshots UPSERT），跨重启保留，不走 issue body 字面"暂存内存"；④ services/memory.rs 含 messages 表 CRUD 超集，#5 内未消费，加 mod 级 `#[allow(dead_code)]` 屏蔽 warning，M1 W2 ChatService MVP 接入时去掉。**依赖**：tauri-plugin-sql 2.x（建库 + WAL）+ sqlx 0.8（service 层短期连接，plugin DbPool 在 2.x 不公开）+ gray_matter 0.2 yaml-only + ulid 1。**实测**：`cargo test` 38 测试全过（DB 集成 + 单元）；`cargo check` / `pnpm typecheck` / `pnpm lint --max-warnings=0` 三件套 0 错 0 警；`pnpm tauri:dev` setup 阶段 `[setup] reached` 后无 panic、`seed_builtin` 异步 UPSERT 内置 momo 无 eprintln 失败。

### 2026-05-05

1. **文档工程化重构**：文件夹改名（中文 → 英文）、统一 YAML frontmatter（3 字段）、删悬空引用、新增 6 份工程标准件。详见 [CHANGELOG.md](CHANGELOG.md)。
2. **单人化简化**：删 M0 决策周设定、ADR 折叠到 `decisions.md` 单文件、删 KPI 阈值门禁、roadmap 删多层任务粒度。
3. **项目记忆系统**：建 `CLAUDE.md` / `STATUS.md` / `/resumex` 命令 / `templates/status-template.md`；WORKFLOW.md 加 §8。
4. **GitHub Issues 工作流设计**：建 `docs/github-workflow.md`（labels / milestones / 命名约定 / 接入步骤）；建 `.github/ISSUE_TEMPLATE/{feat,spike,fix}.yml`；新增 `/new-task` 与 `/sync-status` 命令；升级 `/resumex` 让它读最近 5 个开放 issue；新增 `.gitignore`。
5. **GitHub 仓库接入**：commit `13ab823` / `4afbae8` 已 push 到 https://github.com/tl0502/APET ；建 6 milestones + 27 labels；接入完成。
