---
title: TodoService MVP + 3 衔接（#21 KV / LivingPet hook / AI 拆解占位）+ daily 时区修 + UI 扩展 设计
updated: 2026-05-23
related:
  - ../../../decisions.md
  - ../../../STATUS.md
  - ../../../../src-tauri/src/services/reminder.rs
  - ../../../../src-tauri/src/services/living_pet.rs
  - ../../../../src/types/reminder.ts
  - ../../../../src/panels/tasks/TasksTodoPanel.vue
---

# TodoService MVP + 衔接收尾 设计文档

> 对应 issue [#29](https://github.com/tl0502/APET/issues/29)。M2 W3 第三刀 — 把 issue body 5 件事落地，并按 brainstorm 期扩张加入 6 项 UI 增强（拖排序 / priority / 批量 / 搜索 / 最小日历）+ 时区修复，跟 #22 后续 follow-up 段对齐。

## 1. 背景

issue #29 创建于 2026-05-17（依赖 #22 完成后），原 body 范围 5 件事：
1. TodoService MVP（5 IPC：create / list / update / complete / breakdown_with_ai）
2. onboarding KV `onboarding:reminder_intents` 启动期实例化（#21 闭环）
3. LivingPet 联动 hook（reminder:fired → 点头 stub）
4. Tasks 窗待办 tab 接入
5. AI 拆解 IPC 灰显占位（M3 接 LLM 时 UI/IPC 不动）

brainstorm 期核查代码现实：
- ❌ `services/todo.rs` / `commands/todo.rs` 零代码
- ❌ `lib.rs` 启动期无 KV 消化逻辑 → onboarding step 4 闭环现实断
- ❌ `living_pet.rs` 514 行无 `reminder:fired` listen
- ✅ `TasksTodoPanel.vue` 已存在但是 placeholder（"🚧 即将上线"）
- ✅ `DetailColumn` 已 map TasksTodo + workspaceLayout master items 已含
- ✅ `PetReminderBubble.vue` 已 listen reminder:fired 推气泡（关键：listener 一对多，桌宠 hook 是并列新增）
- ✅ `REMINDER_TEMPLATES` ([reminder.ts:80](../../../../src/types/reminder.ts#L80)) 5 个 hardcode 模板已就位（按 id 反查 → CreateInput）

brainstorm 期同时拍板：
- **daily HH:MM 时区修**合并进 #29（[reminder.rs:14](../../../../src-tauri/src/services/reminder.rs#L14) 注释明确 "follow-up #29 接入本地时区"）
- **UI 扩展 4 项**进 #29：拖排序 / priority tag / 批量操作 / 搜索框
- **E.1 最小日历**进 #29（v-calendar 月视图），E.2 完整日程化（schema event 区间）拆 follow-up issue

## 2. 设计目标

- TodoService 6 IPC（5+1 reorder）跟 ReminderService 风格对齐
- todo↔reminder 联动：引用 + 级联（schema 加 reminder_id 字段）
- onboarding step 4 闭环：boot 期 KV → reminders 表 → 删 KV
- 桌宠点头反应通过 composable 抽出，#23 接 reaction_table 时只改 composable 内部
- reminder daily HH:MM 时区从 UTC 改本地（chrono Local），老用户自愈
- 待办 panel 视觉媲美 ReminderPanel（拖排序 + priority 色条 + 批量 + 搜索 + list/calendar 双视图）

## 3. 不在范围内

- AI 拆解 confirm dialog UI / M3 prompt 模板 / BreakdownError 子类扩展 → M3 接 LLM
- onboarding "重做" 流程 → M3+ 装扮工坊扩展
- 时间轴 / 周视图 / 日视图 / 拖拽改 due_at / schema event 区间（start_at/end_at/all_day）→ follow-up issue `日程化扩展`
- 物理硬删 todo IPC（cancelled 软删足够）
- todo 父子关系字段（breakdown_parent_id 不预留）
- 自动化 e2e（手动验证；单人项目 YAGNI）

## 4. 架构

#29 一共 6 件事落到代码：

| # | 改动 | 文件 |
|---|---|---|
| 1 | TodoService 后端 | `src-tauri/src/services/todo.rs` 新建 + `commands/todo.rs` 新建 + `migrations/001_init.sql` 加 `todos` 表（lesson #2 零迁移） |
| 2 | TodoService 前端契约 | `src/types/todo.ts` 新建 + `src/services/todo.ts` 新建 |
| 3 | Onboarding KV 实例化 | `src-tauri/src/services/onboarding_reminders.rs` 新建 + `lib.rs::setup` 末尾加 `instantiate_onboarding_reminders` block_on（复用 #34 sync-fn-block_on-async pattern） |
| 4 | LivingPet reminder hook | `src/composables/usePetReaction.ts` 新建 + `services/vrm.ts` `VRMRuntime` 加 `playAction(actionId)` + `PetCanvas.vue` 接入一行 |
| 5 | Tasks 待办 panel 真实化 | `src/panels/tasks/TasksTodoPanel.vue` 改写 + 新建 `components/tasks/TodoList.vue` / `TodoForm.vue` / `TodoCalendar.vue` / `TodoBatchBar.vue` + `DetailColumn.vue` 删 placeholder props + `package.json` 加 vuedraggable + v-calendar |
| 6 | daily HH:MM 时区修 | `src-tauri/src/services/reminder.rs` `compute_next_fire_at_daily_hhmm` 改本地时区 + 单测改 |

**架构原则**：
- TodoService 是 ReminderService 联动方，不并存独立"提醒"机制；todo `due_at` 实际依赖 reminder 表 + scheduler
- `reminder:fired` listener 一对多：PetReminderBubble（气泡）+ TasksReminderPanel（refresh）+ usePetReaction（新增点头）三方订阅互不影响
- AI 拆解 M3 接入返 `Vec<String>` → 前端 confirm dialog batch `todo_create`；子任务跟父 todo 在 DB 层无 FK 关联

## 5. 数据模型

### 5.1 todos 表 schema（9 字段）

```sql
CREATE TABLE IF NOT EXISTS todos (
  id           TEXT PRIMARY KEY NOT NULL,         -- ULID
  title        TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'open',      -- 'open' | 'done' | 'cancelled'
  due_at       TEXT,                              -- RFC3339 UTC; NULL = 无截止
  reminder_id  TEXT,                              -- 关联 reminders.id 软引用; NULL = 无关联
  order_index  REAL NOT NULL DEFAULT 0,           -- 分数排序（拖排序用，避免大批量 reindex）
  priority     TEXT NOT NULL DEFAULT 'normal',    -- 'low' | 'normal' | 'high'
  created_at   TEXT NOT NULL,                     -- RFC3339 UTC
  updated_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_todos_status_order ON todos(status, order_index);
```

**字段决策**：
- `order_index REAL` 而非 INTEGER：拖到 A、B 中间时 newOrder = (A+B)/2，单条 UPDATE 不动其他行；连续插中位 ~50 次后浮点精度耗尽才需 reindex（单人项目 ≈ 不触发）
- `reminder_id` 软引用（无 FK）：与 reminder_history 同 pattern；删 reminder 时手动 NULL 化 todos.reminder_id（懒清理：list 时检测 + 静默置 NULL）
- 不加 `breakdown_parent_id`（schema A 极简；M3+ 需要再加列）

### 5.2 todo↔reminder 联动语义

| Todo 操作 | due_at 变化 | reminder 联动 |
|---|---|---|
| `create({title})` 无 due_at | null | 不调 reminder |
| `create({title, due_at})` | T | `reminder.create_internal({title, trigger_type:'once', trigger_spec:T, priority:'soft'})` → 回填 todos.reminder_id |
| `update({due_at: Set(T)})` 原 null | null→T | `reminder.create_internal(...)` → 回填 reminder_id |
| `update({due_at: Set(T2)})` 原 T1 | T1→T2 | `reminder.update_internal(reminder_id, {trigger_spec:T2})` |
| `update({due_at: Clear})` 原 T1 | T1→null | `reminder.delete_internal(reminder_id)` → 清 reminder_id |
| `update({title: T2})` has due_at | 不变 | `reminder.update_internal(reminder_id, {title:T2})` 标题同步 |
| `complete()` | 不变 | **不动 reminder**（已触发的 history 保留；未来的 once reminder 由 scheduler 消化或用户手动清） |
| 软删 `update({status:'cancelled'})` | 不变 | **不动 reminder**（同 complete 语义）|

**事务保证**：联动操作（todo write + reminder write）在同一 sqlx 事务内；联动失败 → 整体回滚到 due_at 设置前。

**trigger_spec 类型选择**：todo `due_at` 是用户点 datetime picker 选的具体时刻 → 永远用 `trigger_type='once'` + RFC3339 UTC trigger_spec；不踩 §7 daily 时区路径。

## 6. 6 IPC 契约

### 6.1 后端注册名（snake_case）

```rust
#[tauri::command] pub async fn todo_create(app: AppHandle, input: CreateInput) -> Result<Todo, String>;
#[tauri::command] pub async fn todo_list(app: AppHandle) -> Result<Vec<Todo>, String>;
#[tauri::command] pub async fn todo_update(app: AppHandle, id: String, input: UpdateInput) -> Result<Todo, String>;
#[tauri::command] pub async fn todo_complete(app: AppHandle, id: String) -> Result<Todo, String>;
#[tauri::command] pub async fn todo_breakdown(app: AppHandle, id: String) -> Result<Vec<String>, String>;
#[tauri::command] pub async fn todo_reorder(app: AppHandle, id: String, after_id: Option<String>) -> Result<Todo, String>;
```

issue body 写 5 IPC，本扩展加 1 `todo_reorder`（拖排序）= 6 IPC，closing comment 说明偏离原因。

架构 §604 文档逻辑名 `todo.create / list / update / complete / breakdown_with_ai / reorder` 保持作分组语义；实际注册名上述 6 个。

### 6.2 类型定义（src-tauri/src/services/todo.rs）

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    pub id: String,
    pub title: String,
    pub status: String,
    pub due_at: Option<String>,
    pub reminder_id: Option<String>,
    pub order_index: f64,
    pub priority: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInput {
    pub title: String,
    pub due_at: Option<String>,
    pub priority: Option<String>,    // 默认 'normal'
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInput {
    pub title: Option<String>,
    pub status: Option<String>,       // 仅接受 'open' | 'cancelled'（'done' 走 todo_complete）
    pub due_at: Option<DueAtChange>,
    pub priority: Option<String>,
}

/// due_at 三态显式建模(serde Option<Option<T>> 二义)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum DueAtChange {
    Keep,              // 缺省/不改
    Set(String),       // RFC3339 UTC
    Clear,             // 清空到 null
}

#[derive(Debug, Error)]
pub enum TodoError {
    #[error("database error: {0}")]
    Database(String),
    #[error("todo not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("breakdown not implemented (M3+)")]
    BreakdownNotImplemented,
    /// §5.2 联动表中 reminder.create_internal / update_internal / delete_internal 任一失败时返。
    /// 调用方应整体回滚 todo write（事务）。
    #[error("reminder coupling failed: {0}")]
    ReminderCoupling(String),
}
```

### 6.3 6 IPC 行为详表

| IPC | 入参 | 出参 | 副作用 |
|---|---|---|---|
| `todo_create` | `{title, due_at?, priority?}` | `Todo` | INSERT todos + 若有 due_at → reminder.create_internal + 回填 reminder_id；事务 |
| `todo_list` | — | `Vec<Todo>` | 全表 SELECT；前端排序 / 过滤 |
| `todo_update` | `id, {title?, status?, due_at?, priority?}` | `Todo` | 读旧 → UPDATE → 按 due_at change 同步 reminder（§5.2 表）；事务 |
| `todo_complete` | `id` | `Todo` | UPDATE status='done' + updated_at；不动 reminder |
| `todo_breakdown` | `id` | `Vec<String>` | M2 永返 `Err(BreakdownNotImplemented)`；M3 接 LLM 返子任务 title 数组 |
| `todo_reorder` | `id, after_id?` | `Todo` | after_id=None → newOrder = (min existing.order) - 1.0；有 after_id → newOrder = (after.order + nextSibling.order)/2 或 after.order + 1.0（无 next）；UPDATE 单条 |

### 6.4 前端类型 + IPC binding

`src/types/todo.ts` 与后端 camelCase 同步（类型形状映射 5.x 字段；DueAtChange tagged union 跟 Rust enum 对齐）：

```typescript
export type TodoStatus = 'open' | 'done' | 'cancelled'
export type TodoPriority = 'low' | 'normal' | 'high'

export interface Todo {
  id: string
  title: string
  status: TodoStatus
  dueAt: string | null
  reminderId: string | null
  orderIndex: number
  priority: TodoPriority
  createdAt: string
  updatedAt: string
}

export interface TodoCreateInput {
  title: string
  dueAt?: string
  priority?: TodoPriority
}

export type DueAtChange =
  | { kind: 'keep' }
  | { kind: 'set'; value: string }
  | { kind: 'clear' }

export interface TodoUpdateInput {
  title?: string
  status?: 'open' | 'cancelled'
  dueAt?: DueAtChange
  priority?: TodoPriority
}
```

`src/services/todo.ts` 6 函数 invoke 包装：`createTodo / listTodos / updateTodo / completeTodo / breakdownTodo / reorderTodo`。

## 7. Onboarding KV 实例化

### 7.1 入口位置

新文件 `src-tauri/src/services/onboarding_reminders.rs` 提供 sync fn `instantiate_onboarding_reminders<R: Runtime>(app: &AppHandle<R>) -> Result<(), String>`，内部 `block_on(async)`。

`lib.rs::setup` 末尾追加（在 `start_scheduler` 之前）：

```rust
let _ = window_state::apply_initial_workspace_rect(app.handle());
// #29 新增
if let Err(e) = onboarding_reminders::instantiate_onboarding_reminders(app.handle()) {
    eprintln!("[setup] instantiate_onboarding_reminders failed: {e}");
}
```

### 7.2 KV 三态行为

| KV 值 | 动作 | KV 处理 |
|---|---|---|
| null（key 不存在） | no-op | 不动 |
| `"null"`（用户不要） | no-op | 删 KV |
| `"[]"`（中间态） | no-op | 删 KV |
| `'["water","sit_long"]'`（正常） | 5 reminder.create_internal | 删 KV（全成功后） |
| 任意 reminder.create_internal 失败 | 已 create 的不回滚 | **保留 KV**，下次重试 |
| 无效 JSON / 字符串 / 数字 | warn + no-op | 删 KV（脏数据清理） |
| 数组里有未知 id | skip + warn | 不影响其他 |

### 7.3 REMINDER_TEMPLATES 双向同步约束

[src/types/reminder.ts:80](../../../../src/types/reminder.ts#L80) 前端 5 条 hardcode + `services/onboarding_reminders.rs` 内 Rust hardcode `TEMPLATES: &[ReminderTemplate]` 一份等价数据。

扩 template 时需双写 → 加入 `docs/lessons.md` 一条："REMINDER_TEMPLATES 前后端双写约束"。

### 7.4 reminder::create_internal 内部入口

当前 `commands/reminder.rs::reminder_create` 是 `#[tauri::command]` 包装。新增 `services/reminder.rs::create_internal(app, input) -> Result<Reminder, ReminderError>` 抽出业务逻辑；command 改 thin wrapper 调 internal + `.map_err(|e| e.to_string())`。

同 pattern 提供 `update_internal` / `delete_internal`（todo↔reminder 联动用）。

## 8. LivingPet hook（usePetReaction composable）

### 8.1 文件分工

| 文件 | 改动 | 内容 |
|---|---|---|
| `src/composables/usePetReaction.ts` | 新建 | listen `reminder:fired` → `runtime.playAction('nod')`；#23 接 reaction_table 时只改本文件 |
| `src/services/vrm.ts` | 加方法 | `VRMRuntime.playAction(actionId: PetActionId)` 公共 API |
| `src/components/PetCanvas.vue` | +2 行 | import composable + 条件调用（按 `enableReaction` prop） |

### 8.2 PetActionId 契约

```typescript
// src/services/vrm.ts 顶部新增 export
export type PetActionId =
  | 'nod'                  // #29 实现
  | 'head_pat' | 'surprised' | 'fall_asleep' | 'dizzy' | 'protest' | 'cheer'  // #23
  | 'drink' | 'stretch' | 'sleep' | 'wander' | 'idle'                          // #23
```

`VRMRuntime.playAction(actionId)` M2 W3 只实现 `'nod'`；其他 id 走 placeholder（dev console.warn + no-op，#23 时填）。

### 8.3 Nod 实现

humanoid head bone X 轴 ±15° 短促摆动 360ms（RAF ease-out 插值；不引动画 clip）：
- t=0：headRotation.x = 0
- t=180ms：headRotation.x = +15°（低头）
- t=360ms：headRotation.x = 0

**不打断 wander tween / 不持久化**（瞬时动效）。

### 8.4 usePetReaction.ts 内容

```typescript
import { onBeforeUnmount, onMounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { REMINDER_FIRED_EVENT, type ReminderFiredPayload } from '@/types/reminder'
import type { VRMRuntime } from '@/services/vrm'

export function usePetReaction(runtime: VRMRuntime): void {
  let unlistenFired: UnlistenFn | null = null
  onMounted(async () => {
    try {
      unlistenFired = await listen<ReminderFiredPayload>(REMINDER_FIRED_EVENT, () => {
        runtime.playAction('nod').catch((e) => {
          console.warn('[pet-reaction] playAction failed:', e)
        })
      })
    } catch (e) {
      console.warn('[pet-reaction] listen failed:', e)
    }
  })
  onBeforeUnmount(() => { unlistenFired?.() })
}
```

### 8.5 PetCanvas.vue 接入

新增 prop `enableReaction?: boolean`（默认 true），SoulPledgeView（onboarding 窗）显式传 false 避免误装 listener：

```typescript
const props = withDefaults(defineProps<Props>(), {
  draggable: true, view: 'half', size: () => ({ width: 320, height: 320 }),
  enableReaction: true,  // 新增
})

const { isLoaded, errorMessage, runtime } = useVRMModel(canvasRef, MODEL_URL, props.view)
if (props.enableReaction) {
  usePetReaction(runtime)
}
```

## 9. AI 拆解占位（todo_breakdown）

### 9.1 后端行为

`todo_breakdown(id)` 永返 `Err(TodoError::BreakdownNotImplemented)`，错误字符串 `"breakdown not implemented (M3+)"`（thiserror display）。

不检查 id 是否存在 → 简洁；M3 接 LLM 后改成"先查 todo title 拼 prompt"时再加 NotFound 分支。

### 9.2 前端 UI 状态

待办列表每行右侧"AI 拆解"按钮（图标 ✨ + tooltip "M3 上线后可用 — AI 帮你把大目标拆成小步骤"），**永远 disabled**（不点不弹错），M3 接入时删 disabled + 接 onClick。

`breakdownTodo()` IPC 函数 M2 不被调用 — 仅 import 占位，M3 接入时前端代码已就位。

### 9.3 测试

无单测（实现 = 单行 `Err(BreakdownNotImplemented)`）。

手动 e2e：dev console `await invoke('todo_breakdown', { id: 'x' })` → `Error: breakdown not implemented (M3+)`。1 例。

## 10. daily HH:MM 时区修复（reminder.rs）

### 10.1 现状（M2 简化）

[reminder.rs:13-15](../../../../src-tauri/src/services/reminder.rs#L13) daily HH:MM 按 UTC 解释 → 中国时区用户 +8h 偏移（如 23:00 → 7AM 本地）。

### 10.2 修法

把 HH:MM 解释为系统本地时区，算出本地 next_fire 后转 UTC 存库：

```rust
use chrono::{Local, TimeZone, NaiveTime};

fn compute_next_fire_at_daily_hhmm_in_tz<Tz: TimeZone>(
    spec: &str,
    now_utc: DateTime<Utc>,
    tz: &Tz,
) -> Result<DateTime<Utc>, ReminderError> {
    let hhmm = NaiveTime::parse_from_str(spec, "%H:%M")
        .map_err(|e| ReminderError::InvalidTrigger(format!("daily HH:MM: {e}")))?;
    let now_local = now_utc.with_timezone(tz);
    let today_local_naive = now_local.date_naive().and_time(hhmm);
    let next_local_naive = if now_local.naive_local() < today_local_naive {
        today_local_naive
    } else {
        today_local_naive + chrono::Duration::days(1)
    };
    let next_local = tz
        .from_local_datetime(&next_local_naive)
        .latest()
        .ok_or_else(|| ReminderError::InvalidTrigger(format!("DST gap: {next_local_naive}")))?;
    Ok(next_local.with_timezone(&Utc))
}

fn compute_next_fire_at_daily_hhmm(spec: &str, now_utc: DateTime<Utc>) -> Result<DateTime<Utc>, ReminderError> {
    compute_next_fire_at_daily_hhmm_in_tz(spec, now_utc, &Local)
}
```

Tz 可注入解决测试无法控制系统时区问题（单测传 `&FixedOffset::east_opt(8*3600)`）。

### 10.3 影响范围

| 项 | 改动 |
|---|---|
| `compute_next_fire_at_daily_hhmm` | 重写（上述） |
| `compute_next_fire_at_daily_every_n_minutes`（`*/N * *`） | 不动（每 N 分钟与时区无关） |
| 老用户数据 next_fire_at | 静默自愈：scheduler 第一次 fire 后调 compute_next 推 next_fire_at，新值为本地时区版 |
| 文档同步 | `reminder.rs` file header 删 "M2 简化 follow-up #29" 段，改 "已接入本地时区" |
| [src/types/reminder.ts:103](../../../../src/types/reminder.ts#L103) `focus_study` template hint | `'每天 09:00（UTC，约本地 17:00）'` → `'每天 09:00（本地）'`；`early_sleep` 同改 |

### 10.4 不动

- once 类型 trigger_spec（已带时区）
- `*/N * *` 每 N 分钟（与时区无关）
- weekly / cron 时区（M2 不实现）
- 时区 picker UI（复用系统时区；M2 无应用级 timezone override）

## 11. Tasks 待办 panel UI

### 11.1 文件结构

| 文件 | 状态 | 内容 |
|---|---|---|
| `src/panels/tasks/TasksTodoPanel.vue` | 改写（覆盖 placeholder） | header + search + view-switcher + body + form dialog |
| `src/components/tasks/TodoList.vue` | 新建 | 列表 + 拖排序 + 批量选择 + 行操作 |
| `src/components/tasks/TodoCalendar.vue` | 新建 | v-calendar 月视图 + 点格 popover |
| `src/components/tasks/TodoForm.vue` | 新建 | 表单：title + due_at + priority |
| `src/components/tasks/TodoBatchBar.vue` | 新建 | 顶部批量操作条 |
| `src/views/workspace/DetailColumn.vue` | 改 | 删 placeholder props |
| `package.json` | 改 | 加 `vuedraggable@^4` + `v-calendar@^3.1` |

### 11.2 库选型

- **vuedraggable** ~30KB：Vue 3 拖排库事实标准（包装 Sortable.js）
- **v-calendar** ~80KB：Vue 3 原生月历组件，dark mode 支持，视觉接近 Apple Calendar（符合 Apple/Bear neutral 路线）
- 不选 FullCalendar（~180KB，视觉偏 Google Calendar 风，需重 css 改造融入 token）

### 11.3 panel 布局

```
panel
├── header (panel__title "待办" + search input + view-switcher list/calendar + refresh / 新建)
├── batch-bar (v-show selectedIds.size > 0)
├── body
│   ├── TodoList (v-show view === 'list')
│   │   row = drag-handle | checkbox | priority 色条 | title | due_at | 🔔 link | complete / edit / cancel
│   └── TodoCalendar (v-show view === 'calendar')
│       v-calendar 月视图 + 当天有 todo 显小圆点 + 点格 popover 列当天 todo
└── ElDialog (TodoForm)
```

### 11.4 视图持久化

```typescript
const TODO_VIEW_KV = 'workspace:todo_view'  // 'list' | 'calendar'
const view = ref<'list' | 'calendar'>('list')
// onMounted 读 KV → ref；setView 时 setConfig
```

两视图共享同一份 todos + 同一份 batch selection。

### 11.5 拖排序细节

- 仅 status='open' 子集可拖（done/cancelled 不允许）
- searchQuery 非空 / batchSelecting 时禁用拖动（避免视觉错乱）
- `onDragEnd` 算 newOrder = `(prev.order + next.order) / 2` → `todo_reorder(id, prevId)`

### 11.6 priority 视觉

| priority | 行左侧 3px 色条 | Form chip |
|---|---|---|
| low | `--aipet-color-text-3`（neutral-400） | "低" |
| normal | 无 | 无 |
| high | `--aipet-color-warning`（amber） | "重要" |

### 11.7 批量操作

- TodoList 每行加 ElCheckbox（hover 显，选中常亮）
- selectedIds Set；TodoBatchBar v-show selectedIds.size > 0
- 批量完成：`Promise.all(ids.map(completeTodo))`
- 批量取消：`Promise.all(ids.map(id => updateTodo(id, {status:'cancelled'})))`
- 单条失败 ElMessage 警告，其他成功保留

### 11.8 搜索

```typescript
const searchQuery = ref('')
const filtered = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  let list = showAll.value ? todos.value : todos.value.filter(t => t.status === 'open')
  if (q) list = list.filter(t => t.title.toLowerCase().includes(q))
  return list.sort(/* §11.10 排序 */)
})
```

### 11.9 TodoCalendar — v-calendar 月视图

```vue
<VCalendar :attributes="calendarAttrs" :is-dark="isDark" />
```

- `calendarAttrs` = todo 列表 → `[{dates: dueAtDate, dot: {color: priorityColor}, popover: {label: title}}]`
- 点格 → 自带 popover 列该日 todos
- dark mode 通过 `:is-dark` prop 传；**检测方式：watch `document.documentElement.classList.contains('dark')` + MutationObserver 监听 class 变化**（项目用 `:root.dark` class 切换，非 `prefers-color-scheme` media query）
- 日历视图不允许拖排序 / 不显示无 due_at 的 todo

### 11.10 排序规则

```typescript
const sorted = [...filtered].sort((a, b) => {
  // 1. open 优先（仅 showAll 时）
  if (a.status !== b.status) {
    const order = { open: 0, done: 1, cancelled: 2 }
    return order[a.status] - order[b.status]
  }
  // 2. open 内部按 order_index 升序（user 自定义拖排）
  if (a.status === 'open') return a.orderIndex - b.orderIndex
  // 3. done/cancelled 按 updated_at 倒序（最近的在上）
  return b.updatedAt.localeCompare(a.updatedAt)
})
```

### 11.11 TodoForm（弹窗）

| 字段 | 控件 | 校验 |
|---|---|---|
| title | ElInput | required，trim 后 ≥ 1 |
| due_at | ElDatePicker type='datetime' | optional；disabledDate 不允许过去；提交转 RFC3339 UTC |
| priority | ElSelect (low/normal/high) | 默认 normal |

编辑模式：传 `props.todo`，form 预填；提交调 `todo_update`。

### 11.12 UX 细节：reminder 联动可视化

todo 行右侧小图标 `🔔` 提示"关联了一个提醒"（仅 reminder_id 非 null 显示）。

## 12. 测试

### 12.1 cargo 单测（新增 ~20）

**`services/todo.rs::tests`**：
- `create_then_list_returns_one_row`
- `create_with_due_at_writes_reminder` / `update_due_at_clear_drops_reminder` / `update_due_at_change_syncs_reminder_spec`
- `complete_keeps_reminder_intact` / `delete_via_cancel_status_keeps_row`
- `breakdown_always_returns_not_implemented_in_m2`
- `update_status_cannot_set_done_directly`
- `reorder_inserts_between_two_neighbors` / `reorder_to_top_uses_smaller_than_min`
- `priority_default_normal`

**`services/reminder.rs::tests`**（时区）：
- `daily_hhmm_in_utc8_evening_after_target`（北京 17:00 设 09:00 → 明天 01:00 UTC）
- `daily_hhmm_in_utc8_morning_before_target`（北京 07:00 设 09:00 → 今天 01:00 UTC）
- `daily_hhmm_in_utc_neutral_zone`（UTC 23:00 设 23:00 → 明天 23:00 UTC，regression）

**`services/onboarding_reminders.rs::tests`**：
- `parse_array_returns_ids` / `parse_null_sentinel_returns_none` / `parse_empty_array_returns_empty`
- `parse_invalid_json_returns_none` / `parse_array_filters_unknown_ids`
- `lookup_template_known_id` / `lookup_template_unknown_id`

合计 ~20 新单测；当前 230 → ~250。

### 12.2 vitest（新增 4）

- `todo store computed.sorted` 排序规则
- `todo batch.completeMany` 部分失败保留其他
- `DueAtChange enum serde shape`（保 IPC 契约）
- `usePetReaction listen unmount` 生命周期

当前 293 → 297。

### 12.3 手动 e2e（15 例）

| # | 操作 | 期望 |
|---|---|---|
| 1 | onboarding step 4 勾 water+sit_long → finalize → 重启 | KV 消失；reminders 表+2 行；scheduler 正常 |
| 2 | onboarding 选 "我不需要" → 重启 | KV 消失；reminders 表无新增 |
| 3 | tasks 待办 tab 新建 todo（无 due_at） | 列表显示；reminder 表无新增 |
| 4 | 编辑 todo 加 due_at（5min 后） | reminders 表+1 once + reminder_id 回填；5min 后桌宠点头 + 气泡 + reminder tab 出现 |
| 5 | 清 due_at | reminders 行被 delete；reminder_id NULL |
| 6 | complete 有 due_at + 未触发 todo | status='done'；reminder 保留 |
| 7 | reminder tab 手动建 reminder | tasks tab 不出现新 todo（联动单向）|
| 8 | 拖排序 todo | order_index 更新；重启后顺序保留 |
| 9 | 批量完成 3 个 | 全部 status='done'；batch bar 消失 |
| 10 | 搜索 "喝" | 过滤生效；清空恢复 |
| 11 | 切日历视图 → 看小圆点 → 点格 | popover 列当天 todo |
| 12 | 切日历后关闭 → 重启 | 自动停日历视图（KV 持久化） |
| 13 | dev console invoke todo_breakdown | `Error: breakdown not implemented (M3+)` |
| 14 | 本地 17:00 操作建 daily 09:00 reminder | next_fire_at = 明天本地 09:00（约 UTC 01:00） |
| 15 | reminder fired → 桌宠点头 | VRM head bone 360ms ±15° 摆动；不打断 wander |

### 12.4 工具链全过

- `pnpm vitest run` 297 pass
- `cargo test` ~250 pass
- `cargo check --bins`（lesson §4）
- `pnpm typecheck` / `pnpm lint`

不做：Playwright / 视觉回归 / 自动化 e2e。

## 13. 工时

| 阶段 | 工时 |
|---|---|
| 后端 6 IPC + todo↔reminder 联动 + 单测 | ~3.5h |
| KV 实例化 + lib.rs setup 钩子 + 单测 | ~0.7h |
| reminder.rs daily 时区修 + 单测 | ~1h |
| LivingPet hook (VRMRuntime.playAction + usePetReaction) | ~1h |
| Tasks 待办 panel 基础（list + form） | ~1.5h |
| 拖排序 + 批量 + 搜索 | ~3h |
| v-calendar 月视图集成 | ~1.5h |
| AI 拆解占位 + 错误 | ~0.3h |
| 手动 e2e 15 例 + 工具链 | ~1.5h |
| commit + STATUS + lessons + close issue | ~0.5h |
| **合计** | **~14.5h** |

体量分 2-3 个 session 切。

## 14. 风险

| # | 风险 | 缓解 |
|---|---|---|
| R1 | KV 实例化期 reminder.create 部分成功 → 重启重试重复 create | 单条失败立即 return Ok 保留 KV；下次重启重试已成功的 id 重复 create（无去重）→ 接受（用户可手动删；M3+ idempotency token） |
| R2 | daily 时区改本地后老数据 next_fire_at 一次性早/晚 ~8h | scheduler 第一次 fire 后调 compute_next 推 next_fire_at，新值本地版；自愈 |
| R3 | v-calendar dark mode 跟 :root.dark token 不一致 | spec §12.3 e2e #11/#15 双主题验证；偏差时改 wrap css override |
| R4 | vuedraggable + Vue 3 + TS strict 兼容（社区已有 issue） | 用 `^4.1.0` + 必要时 `// @ts-expect-error` 包导入；项目已用其他第三方 Vue 组件同 pattern |
| R5 | order_index 浮点精度耗尽（连续插中位 ~50 次） | 单人项目此风险≈0；触发时手动 `UPDATE todos SET order_index = ROWID` reindex（运维步骤） |
| R6 | todo_reorder 引用已删 / cancelled todo | 后端 SELECT after_id 取不到 → InvalidInput；前端 UI 不会暴露（仅 open 子集可拖） |
| R7 | usePetReaction 在 onboarding 窗误装 listener | spec §8.5 prop `enableReaction` 默认 true；SoulPledgeView 传 false |

## 15. 文档同步

| 文档 | 处理 |
|---|---|
| `docs/STATUS.md` | M2 W3 段标题 10/10 → 11/11；session 行同步；M2 W3-W4 ⏳ 行 #29 改 ✅ |
| `docs/decisions.md` | 不新增 ADR（属规划落地，无新设计决策）；可选在 ADR-018 末追"todo_breakdown M2 占位"备注 |
| `docs/lessons.md` | 加 1 条："REMINDER_TEMPLATES 前后端双写约束（扩 template 需同步 .ts + .rs 两份）" |
| `docs/architecture/system-architecture.md` | 不动（§604 IPC 命名表已有 todo 6 项） |
| `docs/requirements/prd.md` | 不动 |
| `docs/roadmap/development-roadmap.md` | 不动 |

## 16. 关联

- 父 issue [#29](https://github.com/tl0502/APET/issues/29)
- 依赖：[#21](https://github.com/tl0502/APET/issues/21) LivingPet（已完成；hook 接入点）/ [#22](https://github.com/tl0502/APET/issues/22) ReminderService（已完成；create_internal 内部入口 + daily 时区路径）
- 下游：[#23](https://github.com/tl0502/APET/issues/23) 物理交互 + reaction_table（usePetReaction 内部改一行接入）
- follow-up：`#X 日程化扩展`（E.2 schema event 区间 + 时间轴 + 拖拽改 due_at）
- 既有代码：[reminder.rs](../../../../src-tauri/src/services/reminder.rs) `create_internal` 新增入口；[living_pet.rs](../../../../src-tauri/src/services/living_pet.rs) 不动；[PetCanvas.vue](../../../../src/components/PetCanvas.vue) +1 prop +1 调用；[TasksTodoPanel.vue](../../../../src/panels/tasks/TasksTodoPanel.vue) 整体改写
