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
- `order_index REAL` 而非 INTEGER：拖到 A、B 中间时 newOrder = (A+B)/2，单条 UPDATE 不动其他行；当 gap < 1e-6 时 todo_reorder 内部触发 `normalize_order_indices()` 一次 batch UPDATE 重排为 0/10/20/...（自动自愈，无运维步骤）
- `reminder_id` 软引用（无 FK）：与 reminder_history 同 pattern；删 reminder 时手动 NULL 化 todos.reminder_id（懒清理：list 时检测 + 静默置 NULL）
- 不加 `breakdown_parent_id`（schema A 极简；M3+ 需要再加列）

### 5.2 todo↔reminder 联动语义

| Todo 操作 | due_at 变化 | reminder 联动 |
|---|---|---|
| `create({title})` 无 due_at | null | 不调 reminder |
| `create({title, due_at})` | T | `reminder::create_internal_tx(&mut tx, {title, trigger_type:'once', trigger_spec:T, priority:'soft'})` → 回填 todos.reminder_id |
| `update({due_at: Set(T)})` 原 null | null→T | `reminder::create_internal_tx(&mut tx, ...)` → 回填 reminder_id |
| `update({due_at: Set(T2)})` 原 T1 | T1→T2 | `reminder::update_internal_tx(&mut tx, reminder_id, {trigger_spec:T2})` |
| `update({due_at: Clear})` 原 T1 | T1→null | `reminder::delete_internal_tx(&mut tx, reminder_id)` → 清 reminder_id |
| `update({title: T2})` has due_at | 不变 | `reminder::update_internal_tx(&mut tx, reminder_id, {title:T2})` 标题同步 |
| `complete()` has reminder + once | 不变 | **删除 reminder** + clear reminder_id（防止用户提前完成后到点仍弹气泡 / 桌宠点头）|
| `complete()` 无 reminder 或非 once | 不变 | 不动 reminder |
| 软删 `update({status:'cancelled'})` has reminder + once | 不变 | **删除 reminder** + clear reminder_id（同 complete 语义） |

**tx 注入式事务保证**：所有联动操作在同一 sqlx `Transaction<'_, Sqlite>` 内执行；reminder.rs 提供 `create_internal_tx / update_internal_tx / delete_internal_tx` 三个内部入口接 `&mut Transaction` 参数。todo.rs 业务函数 `pool.begin()` → 调 todo + reminder 各 *_tx → `tx.commit()`。任一步失败 → `tx` drop 时自动 rollback（sqlx 默认行为）→ todo + reminder 同时未写入。

**trigger_spec 类型选择**：todo `due_at` 是用户点 datetime picker 选的具体时刻 → 永远用 `trigger_type='once'` + RFC3339 UTC trigger_spec；不踩 §10 daily 时区路径。

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

/// due_at 三态;字段省略 (None) = keep 不改;Set / Clear 显式建模区分 set-to-value 与 set-to-null
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum DueAtChange {
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
    /// §5.2 联动表中 reminder::create_internal_tx / update_internal_tx / delete_internal_tx 任一失败时返。
    /// 调用方所在 tx 被 drop 时 sqlx 自动 rollback,todo + reminder 同时未写入。
    #[error("reminder coupling failed: {0}")]
    ReminderCoupling(String),
}
```

### 6.3 6 IPC 行为详表

| IPC | 入参 | 出参 | 副作用 |
|---|---|---|---|
| `todo_create` | `{title, due_at?, priority?}` | `Todo` | tx 内：INSERT todos + 若有 due_at → reminder::create_internal_tx + 回填 reminder_id；tx.commit |
| `todo_list` | — | `Vec<Todo>` | 单条 SELECT 出参已排序（后端 SQL）：`ORDER BY CASE status WHEN 'open' THEN 0 WHEN 'done' THEN 1 ELSE 2 END, order_index ASC, updated_at DESC`；前端只做 search + filter |
| `todo_update` | `id, {title?, status?, due_at?, priority?}` | `Todo` | tx 内：读旧 → UPDATE → 按 due_at change 同步 reminder（§5.2 表；cancelled + has once → 删 reminder）；tx.commit |
| `todo_complete` | `id` | `Todo` | tx 内：UPDATE status='done' + updated_at；若 reminder_id 非空且 reminder.trigger_type='once' → reminder::delete_internal_tx + UPDATE todos.reminder_id=NULL；tx.commit |
| `todo_breakdown` | `id` | `Vec<String>` | M2 永返 `Err(BreakdownNotImplemented)`；M3 接 LLM 返子任务 title 数组 |
| `todo_reorder` | `id, after_id?` | `Todo` | 算 newOrder = (after.order + nextSibling.order)/2 或 after.order + 10.0（无 next）或 (min existing.order) - 10.0（after_id=None）；若 newOrder 与相邻 gap < 1e-6 → 触发 `normalize_order_indices(&mut tx)` batch UPDATE 重排 0/10/20/... 再算 newOrder；UPDATE 单条（或 normalize 时全表）；tx 包裹 |

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
  | { kind: 'set'; value: string }
  | { kind: 'clear' }

export interface TodoUpdateInput {
  title?: string
  status?: 'open' | 'cancelled'
  dueAt?: DueAtChange             // 字段省略 (undefined) = keep 不改
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
| `'["water","sit_long"]'`（正常） | tx 内批量 reminder::create_internal_tx + tx 内 delete KV → commit | **原子**：全成功才提交，任一失败 tx drop 自动 rollback → reminders + KV 都未变 → 下次启动从头重试，**无重复 create 风险** |
| 无效 JSON / 字符串 / 数字 | warn + no-op | 删 KV（脏数据清理） |
| 数组里有未知 id | skip + warn | 不影响其他（仍在同 tx 内 commit） |

**幂等保证**：批量 reminder.create + delete KV 在同一 `Transaction<'_, Sqlite>` 内执行。任何一步失败 → tx drop → rollback → 等价于"上次没运行过"；下次启动 KV 还在 → 重新尝试整个 batch。绝不可能"一半成功"。

### 7.3 REMINDER_TEMPLATES 双向同步约束

[src/types/reminder.ts:80](../../../../src/types/reminder.ts#L80) 前端 5 条 hardcode + `services/onboarding_reminders.rs` 内 Rust hardcode `TEMPLATES: &[ReminderTemplate]` 一份等价数据。

扩 template 时需双写 → 加入 `docs/lessons.md` 一条："REMINDER_TEMPLATES 前后端双写约束"。

### 7.4 reminder.rs 提供 *_tx 内部入口

当前 `commands/reminder.rs::reminder_create / update / delete` 是 `#[tauri::command]` 包装。新增 3 个 sqlx tx 注入式内部入口（不挂 #[tauri::command]）：

```rust
// services/reminder.rs
pub async fn create_internal_tx(
    tx: &mut Transaction<'_, Sqlite>,
    input: CreateInput,
) -> Result<Reminder, ReminderError>;

pub async fn update_internal_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: UpdateInput,
) -> Result<Reminder, ReminderError>;

pub async fn delete_internal_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<(), ReminderError>;
```

原 IPC command（`reminder_create / update / delete`）改为 thin wrapper：内部 `let mut tx = pool.begin().await?` → 调对应 `*_tx` → `tx.commit().await?`。这样 #22 已有的 6 IPC 行为不变，新增入口供 todo.rs + onboarding_reminders.rs 共用。

**实现策略**：reminder.rs 已有 `*_with_conn(conn: &mut SqliteConnection, ...)` 内部 helper（[reminder.rs:798](../../../../src-tauri/src/services/reminder.rs#L798)）。`*_tx` 实现可直接 `let conn = &mut **tx; helper(conn, ...).await` deref 复用（sqlx Transaction deref 到 &mut Connection）。复杂度 ≈ 3 个 thin wrapper 函数。

**preferences.rs 同步加 `delete_tx`**：onboarding_reminders.rs drain KV 需要在同一 tx 内 `reminder.create_internal_tx × N + preferences.delete_tx`。现有 [preferences.rs:139](../../../../src-tauri/src/services/preferences.rs#L139) `delete_with_conn` 同样可 deref 复用。

**snooze / complete / list 不需要 *_tx**：当前不被 todo / onboarding 调用，保持原 pool.execute pattern。如未来 #23 物理交互需要在 tx 内 snooze（可能性低），届时再加。

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

**vrm-未 ready 防御**：`playAction` 首行 `if (!this.vrm) return Promise.resolve()` 静默 return — usePetReaction listener 可能在 VRM 加载完成前就接到 reminder:fired event（onboarding 期 / VRM 加载失败时），这种情况点头动作 no-op 是正确行为，不报错不弹 toast。

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
├── header
│   ├── row1: panel__title "待办" + actions (view-switcher list/calendar + refresh + 新建)
│   └── row2: search input (占满宽度)
├── batch-bar (v-show selectedIds.size > 0, position:sticky top:0 z-index:5)
├── body
│   ├── TodoList (v-show view === 'list')
│   │   row = drag-handle | checkbox | priority 色条 | title | due_at | 🔔 link | complete / edit / cancel
│   └── TodoCalendar (v-show view === 'calendar')
│       v-calendar 月视图 + 当天有 todo 显小圆点 + 点格 popover 列当天 todo
└── ElDialog (TodoForm)
```

header 拆两行：detail column 在 240px master 旁实际可用宽度约 600-800px，单行塞 title + 5 控件 + search input 显挤；title+actions 一行 / search 一行视觉更清晰。

batch-bar sticky：长列表底部勾选时滚到顶仍可见，避免"滚回去找按钮"。

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

| priority | 行左侧 4px 色条 (border-radius: 2px) | Form chip |
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

### 11.10 前端 list / filter（排序在后端）

后端 SQL 已完成主排序（§6.3 todo_list）：status (open→done→cancelled) + order_index ASC + updated_at DESC。前端 computed 只做 search + showAll 过滤：

```typescript
const filtered = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  let list = todos.value  // 已经是后端排序后的
  if (!showAll.value) list = list.filter(t => t.status === 'open')
  if (q) list = list.filter(t => t.title.toLowerCase().includes(q))
  return list  // 不再 .sort()
})
```

**拖排序后必须 re-list**：todo_reorder IPC 返回单个更新后的 Todo，前端无法本地预测全表新排序（normalize 触发时全表 order_index 都变）→ onDragEnd 后调 listTodos 全表刷新。

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
- `complete_once_reminder_deletes_reminder_and_clears_id` *(新)* / `complete_no_reminder_is_noop` *(新)*
- `cancel_via_update_with_once_reminder_deletes_reminder` *(新)*
- `delete_via_cancel_status_keeps_row`
- `breakdown_always_returns_not_implemented_in_m2`
- `update_status_cannot_set_done_directly`
- `reorder_inserts_between_two_neighbors` / `reorder_to_top_uses_smaller_than_min`
- `reorder_triggers_normalize_when_gap_under_threshold` *(新)*
- `priority_default_normal`
- `tx_rollback_on_reminder_coupling_failure` *(新；模拟 reminder fail → todo + reminder 都未写入)*

**`services/reminder.rs::tests`**（时区）：
- `daily_hhmm_in_utc8_evening_after_target`（北京 17:00 设 09:00 → 明天 01:00 UTC）
- `daily_hhmm_in_utc8_morning_before_target`（北京 07:00 设 09:00 → 今天 01:00 UTC）
- `daily_hhmm_in_utc_neutral_zone`（UTC 23:00 设 23:00 → 明天 23:00 UTC，regression）

**`services/onboarding_reminders.rs::tests`**：
- `parse_array_returns_ids` / `parse_null_sentinel_returns_none` / `parse_empty_array_returns_empty`
- `parse_invalid_json_returns_none` / `parse_array_filters_unknown_ids`
- `lookup_template_known_id` / `lookup_template_unknown_id`
- `drain_in_tx_atomic_all_or_nothing` *(新；模拟最后一条 create fail → tx rollback → 第一条也不写入 → KV 仍在)*

合计 ~24 新单测；当前 230 → ~254。

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
| 6 | complete 有 due_at + 未触发 todo（has once reminder） | status='done'；**reminder 被删除**；reminder_id NULL；列表 🔔 图标消失 |
| 6b | complete 有 due_at + **已触发**过的 todo（snooze 后再 complete） | status='done'；reminder 仍删除（history 保留）；reminder_id NULL |
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
- `cargo test` ~254 pass
- `cargo check --bins`（lesson §4）
- `pnpm typecheck` / `pnpm lint`

不做：Playwright / 视觉回归 / 自动化 e2e。

## 13. 工时

| 阶段 | 工时 |
|---|---|
| 后端 6 IPC + todo↔reminder 联动 + tx 注入式 + 单测 | ~4h |
| reminder.rs / preferences.rs 加 *_tx 入口 | ~0.5h |
| KV 实例化（tx 包裹 batch + 删 KV 原子）+ lib.rs setup 钩子 + 单测 | ~0.7h |
| reminder.rs daily 时区修 + 单测 | ~1h |
| LivingPet hook (VRMRuntime.playAction + usePetReaction + null 防御) | ~1h |
| Tasks 待办 panel 基础（list + form） | ~1.5h |
| 拖排序（含 normalize_order_indices）+ 批量 + 搜索 | ~3h |
| v-calendar 月视图集成 | ~1.5h |
| AI 拆解占位 + 错误 | ~0.3h |
| 手动 e2e 16 例 + 工具链 | ~1.5h |
| commit + STATUS + lessons + close issue | ~0.5h |
| **合计** | **~15.5h** |

体量分 2-3 个 session 切。

## 14. 风险

| # | 风险 | 缓解 |
|---|---|---|
| R1 | KV 实例化期 reminder.create 部分成功 → 重启重试重复 create | **已通过 tx 注入式批量事务消解**：batch reminder.create_internal_tx + preferences.delete_tx 在同一 `Transaction<'_, Sqlite>` 内执行，全成功才 commit；任一失败 tx drop → rollback → 等价 "上次没运行过"；下次启动 KV 还在 → 重新尝试整个 batch。零重复 create 风险 |
| R2 | daily 时区改本地后老数据 next_fire_at 一次性早/晚 ~8h | scheduler 第一次 fire 后调 compute_next 推 next_fire_at，新值本地版；自愈 |
| R3 | v-calendar dark mode 跟 :root.dark token 不一致 | spec §12.3 e2e #11/#15 双主题验证；偏差时改 wrap css override |
| R4 | vuedraggable + Vue 3 + TS strict 兼容（社区已有 issue） | 用 `^4.1.0` + 必要时 `// @ts-expect-error` 包导入；项目已用其他第三方 Vue 组件同 pattern |
| R5 | order_index 浮点精度耗尽（连续插中位 ~50 次） | **自动自愈**：todo_reorder 内部检测 gap < 1e-6 时触发 `normalize_order_indices(&mut tx)` batch UPDATE 全表重排为 0/10/20/...；用户无感；无运维步骤 |
| R6 | todo_reorder 引用已删 / cancelled todo | 后端 SELECT after_id 取不到 → InvalidInput；前端 UI 不会暴露（仅 open 子集可拖） |
| R7 | usePetReaction 在 onboarding 窗误装 listener | spec §8.5 prop `enableReaction` 默认 true；SoulPledgeView 传 false |
| R8 | VRM 未 ready 时收到 reminder:fired | VRMRuntime.playAction 首行 `if (!this.vrm) return` 静默 no-op（spec §8.2） |

## 15. 文档同步

| 文档 | 处理 |
|---|---|
| `docs/STATUS.md` | M2 W3 段标题 10/10 → 11/11；session 行同步；M2 W3-W4 ⏳ 行 #29 改 ✅ |
| `docs/decisions.md` | 不新增 ADR（属规划落地，无新设计决策）；可选在 ADR-018 末追"todo_breakdown M2 占位"备注 |
| `docs/lessons.md` | 加 2 条：(1) "REMINDER_TEMPLATES 前后端双写约束（扩 template 需同步 .ts + .rs 两份）"；(2) "跨 service 写操作必须 tx 注入式：service A 调 service B 的 *_internal 入口，若 B 内部自取 connection 则 rollback 失效；标准做法是 B 提供 *_tx 接 &mut Transaction 入口，A `pool.begin()` → 调 B::*_tx → `tx.commit()`" |
| `docs/architecture/system-architecture.md` | 不动（§604 IPC 命名表已有 todo 6 项） |
| `docs/requirements/prd.md` | 不动 |
| `docs/roadmap/development-roadmap.md` | 不动 |

## 16. 关联

- 父 issue [#29](https://github.com/tl0502/APET/issues/29)
- 依赖：[#21](https://github.com/tl0502/APET/issues/21) LivingPet（已完成；hook 接入点）/ [#22](https://github.com/tl0502/APET/issues/22) ReminderService（已完成；新增 `create_internal_tx / update_internal_tx / delete_internal_tx` 内部入口 + daily 时区路径修复）
- 下游：[#23](https://github.com/tl0502/APET/issues/23) 物理交互 + reaction_table（usePetReaction 内部改一行接入）
- follow-up：`#X 日程化扩展`（E.2 schema event 区间 + 时间轴 + 拖拽改 due_at）
- 既有代码：[reminder.rs](../../../../src-tauri/src/services/reminder.rs) 加 3 个 *_tx 入口（deref 复用 *_with_conn）；[preferences.rs](../../../../src-tauri/src/services/preferences.rs) 加 `delete_tx`；[living_pet.rs](../../../../src-tauri/src/services/living_pet.rs) 不动；[PetCanvas.vue](../../../../src/components/PetCanvas.vue) +1 prop +1 调用；[TasksTodoPanel.vue](../../../../src/panels/tasks/TasksTodoPanel.vue) 整体改写
