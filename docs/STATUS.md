---
title: AIPET 项目进度
updated: 2026-05-07
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
- **下一步**：按优先序推进剩余 10 个新 issue（详见 [#9-#18](https://github.com/tl0502/APET/issues)）。下一步：[#9](https://github.com/tl0502/APET/issues/9) 设置面板骨架（基于 #8 AppShell + StandardDialog）→ [#10](https://github.com/tl0502/APET/issues/10) 拖动 → [#11](https://github.com/tl0502/APET/issues/11) 全局快捷键。
- **阻塞**：无

---

## 已完成（里程碑级摘要）

### M1 启动（2026-05-06 起）

- M1-D1 项目脚手架就位（commit 8952e6e）：Tauri 2 + Vue 3 + TS + Pinia + Vite 7 + pnpm；`pnpm tauri:dev` 跑通 320×320 透明窗口；ADR-016 / ADR-017 入库（commit 3426184）。详见关闭的 [Issue #1](https://github.com/tl0502/APET/issues/1)。
- M1-D2 Element Plus + 主题跟随系统就位（commit 7c387db）：EP 全量 import + zh-CN locale + 三态 Pinia 主题 store（auto/light/dark）+ matchMedia/localStorage；`pnpm tauri:build` 实测 release exe = **4.15 MB**（vs 预估 ~9MB，偏好 53%），ADR-017 已补实测；同 commit 清理 4 处文档过时性能预算（`启动 < 1500ms / 内存 < 150MB` → 推到 M5 自测期统一压测）。详见关闭的 [Issue #2](https://github.com/tl0502/APET/issues/2)。
- M1-D3 PetCanvas + VRM momo 渲染就位（commit 0df9076）：three@0.184 + @pixiv/three-vrm@3.5；vrm.ts（VRMRuntime 透明背景 + 半身相机 + 1.6Hz 呼吸 + A-pose + spring bone）+ useVRMModel composable + 简化版 PetCanvas（193L → 71L，剥离 hitbox/drag/IPC，推到后续 task）；avatar.vrm 用户私有 .gitignore 屏蔽。详见关闭的 [Issue #3](https://github.com/tl0502/APET/issues/3)。
- M1-D4 IPC 框架就位（commit ff32dda）：Rust `commands::system::ping` + `lib.rs` 注册 `invoke_handler![ping]`；前端 `services/ipc.ts` 统一 `invoke<T>` wrapper + `types/ipc.ts` `IpcError`（带命令名上下文）；M1 不引 ts-rs/specta，类型手写。详见关闭的 [Issue #4](https://github.com/tl0502/APET/issues/4)。
- M1-D5 PersonaService MVP + Memory/Nickname 骨架就位（commit e5ca882）：tauri-plugin-sql 2 + sqlx 0.8 + gray_matter 0.2 + ulid 1；001_init.sql 全 27 表一次建（M2-M5 零迁移）+ 002 persona_snapshots unique idx；启动期 spawn `seed_builtin` UPSERT 内置 momo（personas + persona_snapshots）；11 个 IPC 命令（persona_load/activate + nickname 5 个 + memory KV 4 个）。**整文件复用**旧项目 D:\Project\ai桌宠 dogfood 过的 4 个 .rs + 2 个 SQL + momo.soul.md（38 个测试自带）。详见关闭的 [Issue #5](https://github.com/tl0502/APET/issues/5)。
- M1-D6 系统托盘 + 关闭语义就位（commit 558dafb）：tauri features 加 `tray-icon`；新建 services/{tray,window_actions}.rs（**整文件复用 + 裁剪**旧项目 D:\Project\ai桌宠 cut/window 的 dogfood helper：tray.rs 125→105L 删 PASSTHROUGH/TOPMOST 2 个 CheckMenuItem，window_actions.rs 35L 0 改）；lib.rs 接入 tray::setup + on_window_event 拦截 CloseRequested → window.hide()，唯一退出路径 = 托盘"退出"。**关键取舍**：「显示/隐藏」1 项动态文案 + 「单击托盘 toggle」按用户决策删除（仅菜单可操作避免误触）；decorations:false 故无 X 按钮，关闭只走 Alt+F4。详见关闭的 [Issue #6](https://github.com/tl0502/APET/issues/6)。
- M1-D7 视觉 token 体系 + 主题完整化就位：新建 `src/styles/tokens.css`（light/dark 双套 `--aipet-*`，覆盖 color / spacing / font / radius / shadow / motion / z-index 7 组）+ `src/styles/element-overrides.css`（用项目 token 覆写 `--el-*` 关键变量）+ `src/composables/useTokens.ts`（JS 侧 `getToken` / `getAipetToken`）+ `src/views/_dev/TokensPreview.vue`（dev-only `?view=tokens` 视觉对照页）；`src/main.ts` 调整 CSS 加载顺序（EP 默认 → dark css-vars → tokens → EP 覆写 → main.css）+ dev-only 动态 import preview 组件；`src/components/PetCanvas.vue` 提示样式作为首个 token 消费示例。**同 session 顺手修复 plugin-sql migrations 时序**（独立 commit）：`tauri.conf.json` 加 `plugins.sql.preload=['sqlite:aipet.db']`，让 plugin setup 阶段同步 connect + migrate，使 builder.setup 里 `block_on(seed_builtin)` 不再撞 SQLITE_CANTOPEN(14)（D5 完成时是 spawn 异步、前端先调过 plugin load 建库；2026-05-06 code-review 把 spawn 改 block_on 反转了顺序，引入此回归）。详见关闭的 [Issue #7](https://github.com/tl0502/APET/issues/7)。
- M1-D8 通用布局容器就位（commit 82ec827）：新建 `src/components/layouts/AppShell.vue`（两 variant：standalone 含 `[data-tauri-drag-region]` header + body + 可选 footer slot；transparent 纯语义包装无 chrome）+ `src/components/feedback/StandardDialog.vue`（包 EP ElDialog，X / 遮罩 / ESC 三关闭路径透传，loading=true 屏蔽 + spinner overlay + footer 灰禁）+ `src/composables/useToast.ts`（包 EP ElMessage，4 方法 success/error/info/warn=EP 'warning' + ToastAction 闭包绑 instance.close()）+ `src/styles/components.css`（.aipet-shell / .aipet-dialog / .aipet-toast 样式覆写，全部 `--aipet-*` token）；`src/main.ts` CSS 加载序加 components.css（EP → dark → tokens → overrides → components → main）；`src/App.vue` 用 `<AppShell variant=transparent>` 包装 PetCanvas 真实生产消费；`src/views/_dev/TokensPreview.vue` 追加 Components section（4 toast 按钮 + open dialog + toggle loading）+ 修 preview 可滚动（`min-height: 100vh` → `height: 100vh; overflow-y: auto`，因 main.css 给 html/body/#app 设了 overflow:hidden 适配 pet 透明窗）。**关键取舍**：① 拖动用 Tauri 2 推荐的 `[data-tauri-drag-region]` 而非 issue body 字面 `-webkit-app-region: drag`（后者仅 macOS，Windows WebView2 不识别）；② AppShell 不重 init theme（main.ts 全局已调）；③ StandardDialog 不内置 cancel/confirm（footer slot 自由）；④ Toast 按钮 issue 字面 3 个 → 实际 4 个完整覆盖 4 方法验收。详见关闭的 [Issue #8](https://github.com/tl0502/APET/issues/8)。

### 立项准备期（2026-04-30 → 2026-05-05）

- 15 项 ADR 敲定（详见 [decisions.md](decisions.md)）
- 6 份基线文档归档（PRD / 架构 / flows / UAT / 人格 / 路线图）
- 立项档案：[research/competitor-research.md](research/competitor-research.md)
- 文档工程化重构 + 单人化简化（详见 [CHANGELOG.md](CHANGELOG.md) 同期条目）
- 项目记忆系统 + GitHub Issues 工作流设计（CLAUDE.md / STATUS.md / `/resumex` `/new-task` `/sync-status` / `.github/ISSUE_TEMPLATE/` / `.gitignore`）
- 远端仓库接入完成（https://github.com/tl0502/APET，6 milestones M1-M5 + P1，27 labels）

---

## 历史 session 摘要

### 2026-05-07

完成 [Issue #8](https://github.com/tl0502/APET/issues/8) 通用布局容器 AppShell + StandardDialog + useToast（commit 82ec827 + STATUS 同步 commit，将 push 触发自动关闭）：M1 W2 基础设施层。**新增 4 个文件 + 改 3 个文件**：`src/components/layouts/AppShell.vue`（~50L，两 variant：standalone 含 `[data-tauri-drag-region]` header + body + 可选 footer slot；transparent 纯语义包装无 chrome）；`src/components/feedback/StandardDialog.vue`（~65L，包 EP ElDialog，X / 遮罩 / ESC 三关闭路径透传，loading=true 屏蔽全部关闭路径 + body spinner overlay + footer 整体 `pointer-events:none + opacity:0.5`）；`src/composables/useToast.ts`（~60L，包 EP ElMessage，4 方法 success/error/info/warn=EP 'warning' + ToastAction 闭包绑 instance.close()）；`src/styles/components.css`（~140L，.aipet-shell / .aipet-dialog / .aipet-toast 样式覆写，全部 `--aipet-*` token，禁硬编码颜色 / 间距 / 字号）；`src/main.ts` CSS 加载顺序加 components.css（EP → dark → tokens → overrides → components → main）；`src/App.vue` 把 `.pet-shell` div 替换为 `<AppShell variant="transparent">` 真实生产消费，移除局部 style；`src/views/_dev/TokensPreview.vue` 追加 Components section（4 toast 按钮 + open dialog + toggle loading dialog 演示），同时修 preview 可滚动（`min-height: 100vh` → `height: 100vh; overflow-y: auto`，因 main.css 给 html/body/#app 设了 `overflow: hidden` 适配 pet 透明窗）。**关键取舍**：① 拖动用 Tauri 2 推荐的 `[data-tauri-drag-region]` attr 而非 issue body 字面 `-webkit-app-region: drag`（后者仅 macOS 原生支持，Windows WebView2 不识别）；② AppShell 不调 `useThemeStore().init()`（main.ts 全局已调，避免重复 init，`<html class=dark>` 全局生效）；③ StandardDialog 不内置 cancel/confirm（footer slot 自由，PRD §6 弹窗语义多样）；④ Toast 按钮 issue 字面 3 个 → 实际加 4 个覆盖完整 4 方法验收。**实测**：`pnpm typecheck` / `pnpm lint --max-warnings=0` / `pnpm build`（Vite v7.3.2，1622 modules，10.82s，dist 363.59 KB css + 1.71 MB js gzip 509 KB）三件套 0 错 0 警；`pnpm tauri:dev` 后台跑 setup 阶段 `[setup] reached` 后无 panic（plugin-sql preload 修复仍有效），HMR 动态推送 TokensPreview 样式变更生效；用户视觉验证 pet 窗口 320×320 透明 + AppShell variant=transparent 包装无副作用 + `?view=tokens` 4 toast 颜色/3000ms 关闭/Action 按钮 + dialog 三关闭路径 + loading toggle + 三态主题切换（auto/light/dark）全过。

完成 [Issue #7](https://github.com/tl0502/APET/issues/7) 视觉 token 体系 + 主题完整化（feat commit + STATUS 同步 commit + fix commit，将 push 触发自动关闭）：M1 W2 基础设施层。**新增 4 个文件 + 改 2 个文件**：`src/styles/tokens.css`（~40 项 `--aipet-*`，light 默认 + `:root.dark` 覆写，含 color/spacing/font/radius/shadow/motion/z-index 7 组）；`src/styles/element-overrides.css`（用项目 token 覆写 `--el-color-primary` / `--el-bg-color-*` / `--el-text-color-*` / `--el-border-color*` / `--el-border-radius-*` / `--el-font-*` / `--el-box-shadow*` / `--el-transition-duration*`，确保 EP 控件与自写组件视觉一致）；`src/composables/useTokens.ts`（`getToken(name)` 走 `getComputedStyle`，`getAipetToken(name)` 自动补 `--aipet-` 前缀）；`src/views/_dev/TokensPreview.vue`（色块 / spacing ruler / 字号 / 圆角 / 阴影 / motion 视觉对照 + 三态切换按钮）；`src/main.ts` 调整 CSS 加载顺序（EP → dark css-vars → tokens → EP 覆写 → main.css）+ dev-only `await import('@/views/_dev/TokensPreview.vue')` 当 `?view=tokens` 时挂 preview 而非 App（production build 不带 preview chunk）；`src/components/PetCanvas.vue` 把 `.hint/.hint-error/code` 的 6 处硬编码（颜色/字号/间距/圆角/字体/阴影）替换为 token，作为第一处消费示例。**关键取舍**：① 不引 router（项目目前没接，单页角色窗用不上）— preview 走 `?view=tokens` 查询参数 + dev-only 动态 import；② 不动 `useThemeStore` 主题驱动机制（保留 `<html class="dark">` toggle）— token 切换是纯 CSS variable swap 无 JS 重渲染；③ 透明窗口约束依旧由 `main.css` 兜底（`html/body/#app` 强制 `background:transparent`），`--aipet-color-bg` 仅供面板/preview 消费不灌到 body；④ 不做 Figma 同步 / 多品牌主题 / 字体打包 / 高对比度，全部按 issue body out-of-scope 推后。**实测**：`pnpm typecheck` / `pnpm lint --max-warnings=0` / `pnpm tauri:build` 三件套 0 错 0 警；`pnpm tauri:dev` 后台跑确认 setup 阶段 `[setup] reached` 后无 panic；视觉验证（用户跑 `pnpm tauri:dev`）：默认透明角色窗 PetCanvas loading/error 文案样式正常；`?view=tokens` preview 页色块/字号/圆角/阴影渲染正确，三态切换按钮即时切 light/dark。**顺手修 plugin-sql migrations 时序**（独立 commit）：`src-tauri/tauri.conf.json` 加 `plugins.sql.preload=['sqlite:aipet.db']`。**根因**：tauri-plugin-sql 2.4 的 migrations 默认 lazy（前端 `Database.load` 才 connect+migrate），而 lib.rs setup 期 `block_on(seed_builtin)` 用 `services/db.rs::open_app_db`（`create_if_missing(false)`）同步打开 db，时序上 db 文件还不存在 → SQLITE_CANTOPEN(14) → eprintln 失败（不阻塞启动但污染 stderr）。**修法**：plugin builder 注册 preload 列表后，plugin 自身 setup 会调 `DbPool::connect + Migrator::new + pool.migrate`（`tauri-plugin-sql/src/lib.rs:148-163`），且 plugin setup 在 builder.setup 之前执行。**实测**：再跑 `pnpm tauri:dev` 后台，`[setup] reached` 后 stderr 不再出现 `seed_builtin failed: ... unable to open database file`。**回归来源**：D5 完成时是 spawn fire-and-forget，前端先调过 `Database.load` 建库后 spawn 才到 open_app_db；2026-05-06 code-review #4 把 spawn 改 block_on 反转顺序，引入此回归（STATUS lib.rs:49 注释里"冷启 50-200ms 代价低于人感知阈值"换来了正确性，但漏了 plugin lazy migrations 的前提）。

完成 [Issue #6](https://github.com/tl0502/APET/issues/6) 系统托盘菜单（最小集 + 关闭语义，commit 558dafb，将 push 触发自动关闭）：M1 W2 主态可达交付物。**整文件复用 + 裁剪**旧项目 D:\Project\ai桌宠 cut/window/backend/services/ 的 tray.rs（125→105L）+ window_actions.rs（35L 0 改）；裁剪掉旧版 PASSTHROUGH/TOPMOST 2 个 CheckMenuItem（前者依赖未实现 AppState 字段，后者 #6 out-of-scope 推 M2）。**关键决策**：① 「显示/隐藏」按 issue 字面 1 项动态文案：MenuItem clone 多份传 closure，在菜单点击 / hover Enter 时 set_text 刷新；② 「单击托盘 toggle」按用户中途决策删除（仅菜单可操作避免误触）；③ tauri.conf.json `decorations:false` 故实际无 X 按钮，关闭路径只走 Alt+F4。**实测**：cargo check / pnpm typecheck / pnpm lint --max-warnings=0 三件套 0 错 0 警；视觉验证（用户跑 `pnpm tauri:dev`）：托盘图标 + tooltip + 右键 4 项菜单 + Alt+F4 hide + 退出杀进程全部通过。**memory 收纳**：feedback_file_ops（项目根全树读写已永久授权 + 内置 Edit/Write+相对路径 实测稳定，mcp filesystem 在 sandbox 下报 access denied 不用）+ feedback_validate_before_persist（用户提议先实测再写持久化产物，不可行就诚实反馈）。

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
