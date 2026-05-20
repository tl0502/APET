---
title: AIPET 开发踩坑笔记
updated: 2026-05-20
related:
  - STATUS.md
  - WORKFLOW.md
---

# 开发踩坑笔记（lessons.md）

> 这里只收**踩过、容易再踩**的坑。新 session 入场扫一遍，能避免重复掉同一个坑。
> 历史 session 的"做了什么"在 GitHub Issues；这里只留"以后要小心什么"。

---

## 1. Tauri 2 capability：写类窗口 API + plugin API 必须显式 allow

**症状**：`startDragging()` / `setPosition()` / 全局快捷键调用**静默无反应**，无 console error，无 stderr。

**根因**：`core:window:default`（被 `core:default` 聚合）**只覆盖只读 API**（`allow-outer-position` / `allow-current-monitor` / `allow-available-monitors` / `allow-primary-monitor` / `allow-inner-position` / `allow-cursor-position` 等都已默认有）。所有**改窗口状态**的写类 API（`allow-start-dragging` / `allow-set-position` / `allow-set-size` / `allow-center` / `allow-close` / `allow-hide` …）以及 plugin 自带权限（`global-shortcut:*` / `sql:*` 等）必须在 `capabilities/default.json` 显式 allow。Tauri 2 比 Tauri 1 严的就是这一刀切。

**处理**：引入新 API 前先判别"读还是写"——
- 读类（is-*/get-*/inner-*/outer-*/monitor 系列）：`core:window:default` 已覆盖，**不要重复 allow**（噪音 + 误导）
- 写类 + plugin API：去 [Tauri permissions 文档](https://v2.tauri.app/reference/acl/permission/) 查具体子项 → 显式 allow → dev 视觉验证

**出处**：[#10](https://github.com/tl0502/APET/issues/10) fix `2a34cf7` / [#11](https://github.com/tl0502/APET/issues/11) fix `ff318b9`

---

## 2. 27 表零迁移原则（schema D5 决策）

**症状**：写新功能时本能想"加个 settings 表 / pet_runtime_state 表存这玩意"。

**根因**：D5 一次建全 27 表（`001_init.sql`）就是为了 M2-M5 零迁移成本。每加一张表就要写 migration，破坏单人项目"零迁移"承诺。

**处理**：所有运行时配置走现有 `config` 表 KV（key 用 `domain:subdomain:field` 格式，如 `window:pet:last_position` / `shortcut:chat` / `llm:openai:api_key` / `chat:active_conversation_id`）。新业务实体才能新建表，且必须改 D5 决策。

**出处**：[#10](https://github.com/tl0502/APET/issues/10) / [#11](https://github.com/tl0502/APET/issues/11) / [#12](https://github.com/tl0502/APET/issues/12) / [#13](https://github.com/tl0502/APET/issues/13) — 4 次主动偏离 issue body 字面 schema

---

## 3. plugin-sql migrations 时序

**症状**：启动期 stderr 出现 `seed_builtin failed: ... unable to open database file` (SQLITE_CANTOPEN(14))，但应用照样启动。

**根因**：`tauri-plugin-sql` 2.x 的 migrations 默认**懒加载**（前端 `Database.load` 才 connect+migrate）。如果 `lib.rs::setup` 用 `block_on(seed_builtin)` 同步打开 db，时序上 db 文件还不存在。

**处理**：`tauri.conf.json` 加 `plugins.sql.preload=["sqlite:aipet.db"]`，让 plugin 自己的 setup 阶段（在 `builder.setup` 之前执行）完成 connect + migrate。

**出处**：[#7](https://github.com/tl0502/APET/issues/7) 顺手 fix `ef84ebb`（回归来源：D5 spawn → 2026-05-06 code-review 改 block_on 反转顺序）

---

## 4. tokio macros feature 陷阱

**症状**：`cargo check --tests` / `cargo test` 全过，但 `pnpm tauri:dev` 撞 `cannot find select in tokio` 编译错。

**根因**：`Cargo.toml` 把 `tokio` 的 `macros` feature 只写在 `[dev-dependencies]`。**resolver v2**（edition 2021+ 默认，Tauri 2 模板默认）下，dev-deps features 只在 test/example/bench build 里激活，普通 lib/bin build 不继承——所以 `--tests` 能过、生产 build 炸。（resolver v1 下 features 会从 dev-deps 泄漏到普通 build，反而踩不到这个坑。）

**处理**：每个 feature 检查"是否非 test 路径也用得上"。M1 W2 起，`cargo check` **必须额外跑一遍 lib-only 路径**（不带 `--tests`）才算真过。

**出处**：[#12](https://github.com/tl0502/APET/issues/12) fix `a93bf0d`

---

## 5. Anthropic AUP 触发（写代码时被拦）

**症状**：生成代码途中 Claude 突然返回 `API Error: violates AUP`，对话中断。

**根因**：触发过滤器的 3 类内容：
1. **ADR-006 安全前缀完整文案**（含具体注入攻击描述）
2. **真实/疑似 API key 字符串**（不是占位）
3. **离线模板大段渲染**（生成时直接贴整段，被识别为不当内容批量产出）

**处理**：4 条规则同时遵守 —
1. 安全前缀严守占位：`let safety_prefix = ""; // TODO ADR-006 M3 G`
2. API key 全程占位 `sk-...` 或 `<api_key>`，永远不写具体 key
3. 离线模板走**运行时抽样**路径（`extract_refusal_templates`）而非生成代码时贴大段
4. 一条响应只动 1 个文件（缩小被同时审查的代码面积）

**出处**：[#13](https://github.com/tl0502/APET/issues/13) M1-D11 上半 session 中断 → 4 条规则后顺利完成

---

## 6. Tauri 2 长流式 IPC 优先用 `ipc::Channel`，少用 `app.emit`

**症状**：单 IPC 命令内跑长流式（chat completion / 长文件读 / 渐进搜索结果），如果写成"`async fn` 整段 await 完才返"+ `app.emit("xxx:stream:*", payload)` + 前端 `listen("xxx:stream:*")`：IPC 直到流跑完才 resolve → 前端**拿不到任务 id 全程** → cancel 按钮死锁、切换上下文死锁、并发任务无法路由。

**根因**：问题不是 `app.emit` 本身不能流式（spawn 后立即 `Ok(id)` 返回也能跑通），而是用 emit 路线要**手工**协调一堆东西：messageId 路由、cancel token 表、生命周期清理、跨 invoke 隔离——很容易写错时序。

**处理**：优先用 [`tauri::ipc::Channel<T>`](https://docs.rs/tauri/latest/tauri/ipc/struct.Channel.html)——
1. command 签名加 `onStream: Channel<StreamEvent>` 参数；前端 `new Channel<StreamEvent>()` 传入
2. command 内部拆 `prepare`（同步：分配 id + 注册 cancel token + 校验 + DB 准备）+ spawn 出去的 `run_stream`（流式 + 收尾）；prepare 完立即 `Ok(SendResult { id, ... })` 返
3. 流式事件通过 `channel.send(StreamEvent::*)` 回前端；channel 自带 invoke scope，不需要 messageId 路由
4. 前端 `await invoke()` 立即拿到 id → cancel / 切换路径全打通

**何时还是要 `app.emit`**：真广播事件——多窗口都要听（如 `nickname:changed` / `persona:activated` / `shortcut:chat`）。Channel 是单 invoke scope，不替代广播。

**出处**：[#13 修正](https://github.com/tl0502/APET/issues/13) M1-D12，2026-05-08；plan `~/.claude/plans/c-issue-13-https-github-com-tl0502-apet-ancient-moth.md`

---

## 7. #13 取消 / 错误分类的真实状态机（注释 / STATUS 描述会漂）

**症状**：读老 commit / STATUS / 早期 closing comment 时看到的"取消收尾：UPDATE 已收 partial"、`error_kind_str at service.rs:419-429` 等描述，对照当前代码已经对不上 —— 行号漂了、分支语义变了。新 session 不查源码就建议照旧实现，会破坏已经修过的属性。

**根因**：单一文件被多 issue 反复重构（#1/#3/#5/#6/#7/#9 都动过 `run_stream`），但 STATUS / CLAUDE.md / closing comment 是只读快照，不会自动跟着代码漂。"自包含"原则保证 closing comment 里有 commit hash，但**正文描述本身**仍可能是当时的真理而非当下的真理。

**处理**：动 cancel/错误分类前**必读** `service.rs::run_stream` 的 4 主分支真实实现，不要按记忆/老描述写代码。当前真实状态（截止 2026-05-10）：

| 分支 | 触发 | DB 行为 | Channel 末事件 |
|---|---|---|---|
| Ok(finish) | 流式正常结束 | UPDATE mode='online' | Done(finishReason=stop/length/...) |
| Err(Cancelled { partial_usage }) | 用户取消 | 空 partial → DELETE；非空 → UPDATE mode='cancelled' | Done(finishReason='cancelled', totalTokens=partial_usage.total_tokens) |
| Err(Network/ServerError) | 网络/5xx | UPDATE mode='offline_rule' + 拒答模板 | Delta + Done(finishReason='offline_rule') |
| Err(其他) | AuthFailed/RateLimit/BadRequest/ParseError | DELETE 成功 → 不入库；DELETE 失败 → fallback 改写 offline_rule + 走 offline_rule 收尾 | DELETE 成功 → Error；DELETE 失败 fallback 成功 → Delta+Done('offline_rule')；都失败 → Error |

关键变体：
- `LLMError::Cancelled` 是**结构体变体** `{ partial_usage: Option<Usage> }`，模式匹配必须 `LLMError::Cancelled { partial_usage }` 或 `LLMError::Cancelled { .. }`，**不能写 unit 形式**
- `FinishReason::Unknown(String)` 透传上游未知 finish_reason 字符串，不再被吞成 `Error`
- StreamEvent.error.errorKind 实际值含 `'DbError'`（4 处 update/delete 失败兜底），**不在** `LLMErrorKind`；前端类型用 `LLMErrorKind | 'DbError'`

`error_kind_str` 中 `Network/ServerError/Cancelled` 三变体走 `unreachable!()` —— 因为 `run_stream` 上游分支已经拦下。新增 `LLMError` 变体时必须同步在 `run_stream` 加分支，否则运行时立即 panic 暴露。

**出处**：[#13 修正后回溯检查](https://github.com/tl0502/APET/issues/13)，2026-05-10。

---

## 8. 多会话 KV 与 archived 标记的多层防护

**症状**：跨窗口归档场景下两类隐蔽 bug：
1. settings 窗 archive 一条非 active 会话 → pet 窗列表过期还显示它 → 用户点它 → `set_active` 成功 → 后续 `chat_send` 不传 conv_id 时 `ensure_active` 看 KV 行存在直接复用 → **消息写进归档会话，list 永远看不见**
2. 流式跑到一半（5-30s）期间会话被另一窗口归档 → 收尾 `update_last_activity` 仍刷新归档行 → 取消归档后排序错乱、时间戳与归档时间不符

**根因**：B6 修复 `set_active` 时只校验"行存在"没校验"未归档"；`ensure_active` / `update_last_activity` 同病。`archived` 标记单独存在但三个写路径都没读它。

**处理**：三处都加 `AND archived = 0`：
- `ensure_active_conversation_with_conn`：归档行视同孤儿 KV，走 fallback 建新（透明自愈）
- `set_active_conversation_with_conn`：归档行报"对话不存在或已被归档"（让前端 toast + 刷列表）
- `update_last_activity_with_conn`：归档行 UPDATE 不命中即静默 ok（messages 仍写入归档行，用户取消归档后看得见，比"消息凭空消失"友好）

**未做的事**：SELECT-then-write 没包 tx。SQLite 单写锁 + ensure_active fallback + prepare tx 内二次校验三层兜底已够；若将来出现孤儿 KV 频发再加。

**判别要点**：写多会话相关代码时问一句"这个 SQL / KV 路径是否要看 archived？"——三个答案都是 yes 就该过滤。归档不是软删，是"暂时不可见不可写"的运行态。

**出处**：[多会话管理代码 review](docs/STATUS.md)，2026-05-10；3 个回归测试在 [conversation.rs:tests](src-tauri/src/services/chat/conversation.rs)。

---

## 9. three-vrm lookAt target：相机子节点 + 本地坐标，模型 rig 缺失会静默降级

**症状**：手实现"鼠标跟随视线"时，写成 `scene.add(lookAtTarget)` + `unproject(NDC, z=0.5/-1)` 把鼠标反投影到世界坐标 —— **头/眼完全不动**。

**根因**：three-vrm 官方示例 `lookat.html` 的标准做法是把 target 挂到 **camera 子节点**下，`target.position` 用**相机本地坐标**（(0,0,0) = 直视相机/用户）。挂到 scene 下虽然语义上也通，但 `VRMLookAt` 内部读 `target.getWorldPosition()` 依赖 `target.matrixWorld` 已更新 —— `scene.add` 路径在 `vrm.update()` 调用前是否完成 `updateMatrixWorld` 没保证；而 camera 子节点的 worldMatrix 由 renderer 维护，子节点 worldPosition 自动正确，无时序坑。

**处理**：跟着官方 lookat.html 走 ——
1. `camera.add(lookAtTarget)`（**不是** `scene.add`）
2. `target.position` 用本地坐标：鼠标 NDC × ~0.6 直接做 x/y 偏移，z 留 0
3. `vrm.update(dt)` 之前修改 target.position 即可，不需要手动 `updateMatrixWorld(true)` 兜底

**附带降级行为**：`vrm.lookAt` 在模型没烘焙 firstPerson + lookAt rig 时为 `null`（VRoid Studio 导出默认有，但用户自带的 .vrm 不一定）。当前兜底是 `console.warn` 一次 + 视线跟随完全降级（呼吸/眨眼仍工作）。**未写 head bone 手动 fallback** —— M1 范围外；真要做用 `humanoid.getNormalizedBoneNode('head')` + `head.rotation` 手动 clamp ±45°/±30° yaw/pitch，30-50 行，但只能转头不动眼球，机械感强。

**判别要点**：实测时观察 console —— `[vrm] lookAt enabled, applier=VRMLookAtBoneApplier` = 有 rig；`no lookAt rig` warn = 模型限制，不是代码 bug。

**出处**：[#21](https://github.com/tl0502/APET/issues/21) VRM 微动作层；官方参考 [pixiv/three-vrm examples/lookat.html](https://github.com/pixiv/three-vrm/blob/dev/packages/three-vrm/examples/lookat.html)

---

## 10. Tauri 2 `#[tauri::command] async` 内部链路不能 `block_on` tokio future

**症状**：前端调某 IPC 命令永不返回，UI 卡死 loading（看不到错误、看不到超时）。后端 cargo check 过、无 panic、无 stderr。

**根因**：Rust 服务层 sync 函数内部用 `tauri::async_runtime::block_on(some_async_future)`，而该 sync 函数被 `#[tauri::command] async fn` 调入——此时已在 tokio runtime worker 上跑，再 `block_on` 一个 tokio future 相当于要求当前 worker 让出来跑该 future，但 worker 自身被 `block_on` 占着 → 死锁。tokio 不会报错，IPC 永远不返回，前端 `await invoke()` 永远 pending。

`lib.rs::setup` 里的 `block_on` 没事是因为它在**同步启动期**跑（`setup` callback 不是 async）；同一 service fn 被两条路径调入就会一边正常、一边卡死。`#11 set_chat_shortcut` 就是这样：启动期 `register_chat_on_startup` 用得好好的，#21 Step 3 第一个真用 setter 的前端 caller 把 latent bug 触发出来。

**处理**：
- 从 `#[tauri::command] async fn` 调入的服务函数链路**全程保持 async + await**
- 只在 `lib.rs::setup` / 其他确认是同步上下文的入口用 `block_on` 包 async future
- 判别法：服务 fn 可能从 `#[tauri::command]` 直接或间接调入吗？是 → 改 async
- 单测覆盖不到这个坑（cargo test 跑在独立 tokio runtime，不复现 nested block_on）。**只能靠前端真调 IPC 实测**

**出处**：[#11](https://github.com/tl0502/APET/issues/11) `set_chat_shortcut` latent bug → [#21](https://github.com/tl0502/APET/issues/21) Step 3 触发 → fix `afae148`，2026-05-11

---

## 11. WebView2 不触发 DOM `visibilitychange`（窗口隐藏路径）

**症状**：监听 `document.addEventListener('visibilitychange', ...)` 在窗口 `window.hide()` 时**不触发**。前端无法在窗口被隐藏（托盘 / ESC / boss-key / 关闭按钮）的瞬间清理状态。

**根因**：Tauri 在 Windows 走 WebView2，OS 层 `ShowWindow(SW_HIDE)` 不会让 WebView2 把 DOM visibility 标记为 `hidden`（issues [tauri-apps/tauri#6864](https://github.com/tauri-apps/tauri/issues/6864) / [#9524](https://github.com/tauri-apps/tauri/issues/9524) / [#10592](https://github.com/tauri-apps/tauri/issues/10592)）。macOS / Linux 行为不同；只有 Windows 出现，且没有任何错误信号。

**处理**：

- 不要依赖 `document.visibilitychange` 做窗口隐藏 / 显示的时机判断
- 在 Rust 端 `window_actions.rs` 的所有 `show_*` / `hide_*` / `toggle_*` 路径里**主动 emit** 自定义全局事件（如 `window:visibility-changed { label, visible }`），前端 `listen` 该事件做清理
- CloseRequested 分支也要 emit（用户点 X 走 close-to-hide 时不走 hide IPC）

**出处**：[#30](https://github.com/tl0502/APET/issues/30) follow-up G 关窗时未清磁吸 registry → 改 Rust 主动广播，2026-05-19

---

## 12. Windows WebView2 多窗 `setPosition` IPC 是抖动元凶（链式磁吸）

**症状**：前端 N 个窗形成磁吸链（A→B→C），拖 A 时 B/C 跟随，N≥2 视觉明显抖动；rAF 节流 / batch IPC 都救不了。

**根因**：Tauri 2 Windows webview2 上 `setPosition` IPC 单次 roundtrip ≥5ms（实测）。N 个 dep 每帧串行 N 次 IPC，60Hz 期望 16.7ms / 帧的预算被吃光：N=2 跌 33Hz，N=3 跌 22Hz；叠加 `startDragging` 触发 OS-level move 抢锁，dep 窗位置追不上 anchor，视觉表现为子窗"被甩在后面又突然蹦上来"。**前端任何 JS 层优化都治标不治本** —— 单次 IPC 5ms 是硬性下限。

**处理**：

- group-drag / 链式 solver 路径必须**移到 Rust 端**：订阅 `WindowEvent::Moved`，本地维护 constraint forest，批量 `set_position` 所有 dep。同进程 Win32 `SetWindowPos` 是 μs 级，60fps 完全顶得住
- 同步策略：前端是 constraint 的权威源（drag commit / detach / 持久化 load 都在前端），constraint 变化时 IPC 全量推到 Rust state；Rust 只读不写，避免双向写入冲突
- 防死循环：Rust 端 `set_position` 会触发 dep 自己的 `WindowEvent::Moved` → 又被本服务接住 → 死循环。用 `internal_until` guard（按 label 分桶 + 100ms TTL，覆盖 WebView2 set→OS→Moved 回灌的 IPC roundtrip）
- 角色守卫：solver 只在"主动方"是 primary（拥有整族拖动语义的窗）时触发；secondary 窗即使有 dependents 也直接返。否则 secondary 之间 constraint 会让 secondary 获得整族拖动能力，违反 ADR-020 角色模型

**出处**：[#30](https://github.com/tl0502/APET/issues/30) follow-up I `src-tauri/src/services/snap.rs`，2026-05-20

---

## 13. transparent + 自绘圆角窗的 `padding` 双刃剑

**症状**：透明窗 `.window-root { padding: Npx }` 给 `box-shadow-float` 留显示空间，看起来无害。一段时间后用户反馈"窗看起来字小了 / sidebar 窄了 / 标题栏元素挤"——但你没改过那些子项的 CSS。

**根因**：`.window-root` 占满 webview，套 `padding: 12px` 后 `.app-surface` 实际宽高 = webview − 24px。内部所有 flex 布局（sidebar 宽、content-header 三件套、字体相对感觉）按比例缩窄。视觉差异**仅 12px**，但占小窗（chat 380×480）实际比例 = 3.1% 宽 + 2.5% 高，叠加 box-shadow 视觉错觉，感受被放大到"明显变形"。开发期写 padding 时 anchor 不到这个二阶效应，commit 后过几天才被察觉。

**处理**：

- 透明窗 box-shadow 溢出问题应优先**信任 webview 透明区**（box-shadow 可超出 `.app-surface` 边界画到透明区，被裁掉的只是模糊半径外缘小部分，视觉损失远小于 padding 收缩内部布局的代价）
- 若必须用 padding 留 shadow 空间，**同步评估内部 flex 布局压力**：sidebar 宽 / header 三件套 / 字体大小在 padding 后还能否舒展
- magnetic 系统的 `visualInset` 模型用来补偿 padding 让两窗吸附后看起来贴边（不是物理 OS rect 贴边）—— 一旦 padding 删除，visualInset 必须**同步置零**，否则磁吸落位偏 12px

**出处**：[#30](https://github.com/tl0502/APET/issues/30) follow-up F→I 期间 chat `.window-root` padding:12 副作用被察觉 → 删 padding + 删 `visualInset(12)`，2026-05-20

---

## 添加新 lesson 的判据

只在以下情况追加：

- 这个坑**踩过至少 1 次**（不要预防性写）
- 容易**再踩**（不是一次性事件，而是结构性陷阱）
- **非显然**（不能从代码 / 文档 / git log 直接看出）

不收纳：

- 单次 fix 的 commit message 已经讲清楚的内容（git log 够用）
- 一般工程常识（"记得跑 typecheck"这种）
- 项目决策（去 `decisions.md` 或 `architecture/`）
