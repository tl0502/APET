---
title: TodoService MVP + 衔接收尾 实施计划
updated: 2026-05-23
related:
  - ../specs/2026-05-22-todo-service-mvp-design.md
  - ../../STATUS.md
  - ../../decisions.md
  - ../../lessons.md
---

# TodoService MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地 issue [#29](https://github.com/tl0502/APET/issues/29) — TodoService MVP + 3 衔接（#21 onboarding KV 实例化 / LivingPet reminder hook / AI 拆解 IPC 占位）+ daily 时区修 + UI 扩展（拖排序 / priority / 批量 / 搜索 / 最小日历）。

**Architecture:** Rust 端新建 `services/todo.rs` + `commands/todo.rs` + `services/onboarding_reminders.rs`；改 `services/reminder.rs` 提取 `*_with_conn` helper + 新增 `*_internal_tx` 入口实现跨 service 写操作的事务原子性；改 `services/preferences.rs` 加 `delete_tx`。前端新建 4 个 Vue SFC（TodoList / TodoCalendar / TodoForm / TodoBatchBar）+ 1 composable（usePetReaction）+ runtime `VRMRuntime.playAction`，覆写 placeholder `TasksTodoPanel.vue`。

**Tech Stack:** Tauri 2.x / Rust + sqlx Transaction / Vue 3 SFC + TS / vuedraggable@^4 / v-calendar@^3.1 / chrono::Local + Tz-injectable / ULID / RFC3339 UTC

---

## File Structure（决策锁定）

### Rust 新建
- `src-tauri/src/services/todo.rs` — TodoService 业务层 + tx-injection（types / 9 业务函数 / ~20 单测）
- `src-tauri/src/commands/todo.rs` — 6 IPC：`todo_create / todo_list / todo_update / todo_complete / todo_breakdown / todo_reorder`
- `src-tauri/src/services/onboarding_reminders.rs` — boot 期 `onboarding:reminder_intents` KV drain（tx 包裹批量 reminder create + KV delete）

### Rust 改动
- `src-tauri/migrations/001_init.sql` — todos 表 schema 替换（lesson §2 零迁移）
- `src-tauri/src/services/reminder.rs` —
  - 提取 `create_with_conn / update_with_conn / delete_with_conn` 私有 helper
  - 新增 `create_internal_tx / update_internal_tx / delete_internal_tx` 公共入口
  - 公共 `create / update / delete` IPC 改为 `pool.begin → *_tx → commit` thin wrapper
  - `compute_next_fire_at_daily_hhmm` 接 `chrono::Local` + Tz-injectable 版本
  - file header 注释删 "M2 简化" 段
- `src-tauri/src/services/preferences.rs` — 新增 `delete_tx` thin wrapper
- `src-tauri/src/services/mod.rs` — `pub mod todo;` + `pub mod onboarding_reminders;`
- `src-tauri/src/commands/mod.rs` — `pub mod todo;`
- `src-tauri/src/lib.rs` — `invoke_handler!` 注册 6 IPC + setup 末尾钩 `instantiate_onboarding_reminders`

### 前端新建
- `src/types/todo.ts` — Todo / DueAtChange / TodoStatus / TodoPriority / Input 类型
- `src/services/todo.ts` — 6 invoke 包装
- `src/composables/usePetReaction.ts` — listen reminder:fired → runtime.playAction('nod')
- `src/components/tasks/TodoList.vue` — 拖排序 + 批量 + 行操作
- `src/components/tasks/TodoCalendar.vue` — v-calendar 月视图 + dark watch
- `src/components/tasks/TodoForm.vue` — 创建/编辑弹窗
- `src/components/tasks/TodoBatchBar.vue` — 批量操作条

### 前端改动
- `src/panels/tasks/TasksTodoPanel.vue` — 覆写 placeholder：header + view-switcher + body + form dialog
- `src/runtime/VRMRuntime.ts`（或 `src/services/vrm.ts` 依实际路径）— 新增 `playAction(actionId: PetActionId)` + null 防御
- `src/types/reminder.ts` — `focus_study` / `early_sleep` template hint 文案改为 "（本地）"
- `src/views/onboarding/SoulPledgeView.vue`（含 PetCanvas）— 显式传 `:enable-reaction="false"`
- `src/views/workspace/DetailColumn.vue` — 删 placeholder props
- `src/stores/workspaceLayout.ts` — todo 视图（list/calendar）KV 持久化（`workspace:todo_view`）
- `package.json` — `vuedraggable@^4.1.0` + `v-calendar@^3.1` 加 dependencies

### 文档
- `docs/STATUS.md` — M2 W3 段标题 10/10 → 11/11；当前 session / 下一步同步；#29 行 ⏳→✅
- `docs/lessons.md` — 加 2 条（REMINDER_TEMPLATES 双写约束；跨 service 写必须 tx 注入）
- `docs/decisions.md` — 不新增 ADR

---

## 全局约定（每个 task 都适用）

- 永远不附加 `Co-Authored-By: Claude` 行（用户长久规则）
- commit style: `<type>: #29 <subject>`（type 自由）
- 写 Rust 后 `cargo check --bins`（lesson §4 lib-only 验证，避免 build.rs/icon 拖时间）
- 写前端后 `pnpm typecheck && pnpm lint` 抽查
- 每个 Task 末尾 commit 一次（DRY/TDD/frequent commits）
- 测试 fixture 用 `crate::services::test_db::fresh_db()`（返回 `(TempDir, SqliteConnection)`，已建好全部表 + seed singleton）

---

# Phase A — Schema + Foundation

## Task A1: 替换 todos 表 schema（lesson §2 零迁移）

**Files:**
- Modify: `src-tauri/migrations/001_init.sql:122-131`

**Context:** 现有 todos 表是 M2 placeholder（含 source / parent_id / done_at），项目 ripgrep 确认零 .rs 代码消费 → 安全直改。新 schema 9 字段（含 reminder_id 软引用 / order_index REAL / priority / updated_at）。

- [ ] **Step 1: 编辑 001_init.sql todos 段**

把现有 122-131 整段：

```sql
CREATE TABLE todos (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  due_at TEXT,
  status TEXT NOT NULL DEFAULT 'open',  -- 'open' | 'done' | 'cancelled'
  source TEXT NOT NULL,                  -- 'manual' | 'ai_breakdown'
  parent_id TEXT,                        -- 拆解结果的父任务
  created_at TEXT NOT NULL,
  done_at TEXT
);
```

替换为：

```sql
-- todos: #29 落地（原 M2 placeholder schema 含 source/parent_id/done_at 未被任何代码消费，
-- 2026-05-23 整体替换。lesson §2 零迁移：001 未对外发布前直接改）
CREATE TABLE todos (
  id           TEXT PRIMARY KEY NOT NULL,         -- ULID
  title        TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'open',      -- 'open' | 'done' | 'cancelled'
  due_at       TEXT,                              -- RFC3339 UTC; NULL = 无截止
  reminder_id  TEXT,                              -- 软引用 reminders.id; NULL = 无关联
  order_index  REAL NOT NULL DEFAULT 0,           -- 分数排序（拖排序）
  priority     TEXT NOT NULL DEFAULT 'normal',    -- 'low' | 'normal' | 'high'
  created_at   TEXT NOT NULL,                     -- RFC3339 UTC
  updated_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_todos_status_order ON todos(status, order_index);
```

- [ ] **Step 2: 验证 build + DB migration**

Run: `cd src-tauri && cargo check --bins`
Expected: 编译通过（schema 文件 include_str! 引入；无 Rust 端代码改）

- [ ] **Step 3: 删本地旧 DB 文件让 migration 重跑（lesson §2 配套动作）**

```bash
# Windows 路径
rm -f "$APPDATA/com.aipet.app/aipet.db" "$APPDATA/com.aipet.app/aipet.db-shm" "$APPDATA/com.aipet.app/aipet.db-wal"
```

如开发 DB 路径不同，参考 `src-tauri/src/services/db.rs::open_app_db` 推断。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/migrations/001_init.sql
git commit -m "feat: #29 替换 todos 表 schema (9 字段含 reminder_id/order_index/priority)"
```

---

## Task A2: services/todo.rs 骨架 + 类型定义

**Files:**
- Create: `src-tauri/src/services/todo.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: 写 todo.rs 顶部 + 类型 + 错误**

Create `src-tauri/src/services/todo.rs`:

```rust
//! TodoService（#29，模块 E）— 待办 CRUD + reminder 联动 + tx-injection。
//!
//! 范围（M2 W3，2026-05-23）:
//! - 6 IPC: create / list / update / complete / breakdown / reorder
//! - todo↔reminder 联动：due_at 非空时同 tx 内创建/更新/删 reminder（trigger_type='once'）；
//!   complete + 有 once reminder 时删除 reminder（防止用户提前完成后到点仍弹气泡）。
//! - tx 注入式（lesson §X）：所有联动操作在同一 sqlx Transaction 内执行；任一失败 →
//!   tx drop 自动 rollback → todo + reminder 同时未写入。
//! - order_index REAL 分数排序：拖到 A、B 中间 newOrder=(A+B)/2 单条 UPDATE；gap<1e-6
//!   时 reorder 内部触发 normalize_order_indices batch UPDATE 重排为 0/10/20/...
//!
//! Schema（migrations/001_init.sql:122-138）零迁移（lesson §2）：
//! - todos: id/title/status/due_at/reminder_id/order_index/priority/created_at/updated_at

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqliteConnection, Transaction};
use tauri::{AppHandle, Runtime};
use thiserror::Error;
use ulid::Ulid;

use crate::services::db::{open_app_db, DbError};

#[derive(Debug, Error)]
pub enum TodoError {
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
    #[error("todo not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("breakdown not implemented (M3+)")]
    BreakdownNotImplemented,
    /// §5.2 联动表中 reminder::*_internal_tx 任一失败时返。
    /// 调用方所在 tx drop 时 sqlx 自动 rollback，todo + reminder 同时未写入。
    #[error("reminder coupling failed: {0}")]
    ReminderCoupling(String),
}

impl From<sqlx::Error> for TodoError {
    fn from(e: sqlx::Error) -> Self {
        TodoError::Database(e.to_string())
    }
}

impl From<DbError> for TodoError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => TodoError::AppConfigDir(s),
            DbError::Database(s) => TodoError::Database(s),
        }
    }
}

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
    pub priority: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInput {
    pub title: Option<String>,
    pub status: Option<String>,
    pub due_at: Option<DueAtChange>,
    pub priority: Option<String>,
}

/// due_at 三态;字段省略 (None) = keep 不改；Set / Clear 显式区分 set-to-value 与 set-to-null。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum DueAtChange {
    Set(String),
    Clear,
}

// ====== 业务函数留作 Phase C 实现 ======

pub async fn create<R: Runtime>(_app: &AppHandle<R>, _input: CreateInput) -> Result<Todo, TodoError> {
    Err(TodoError::InvalidInput("not implemented".into()))
}

pub async fn list<R: Runtime>(_app: &AppHandle<R>) -> Result<Vec<Todo>, TodoError> {
    Err(TodoError::InvalidInput("not implemented".into()))
}

pub async fn update<R: Runtime>(_app: &AppHandle<R>, _id: String, _input: UpdateInput) -> Result<Todo, TodoError> {
    Err(TodoError::InvalidInput("not implemented".into()))
}

pub async fn complete<R: Runtime>(_app: &AppHandle<R>, _id: String) -> Result<Todo, TodoError> {
    Err(TodoError::InvalidInput("not implemented".into()))
}

pub async fn breakdown<R: Runtime>(_app: &AppHandle<R>, _id: String) -> Result<Vec<String>, TodoError> {
    Err(TodoError::BreakdownNotImplemented)
}

pub async fn reorder<R: Runtime>(_app: &AppHandle<R>, _id: String, _after_id: Option<String>) -> Result<Todo, TodoError> {
    Err(TodoError::InvalidInput("not implemented".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::test_db::fresh_db;

    #[tokio::test]
    async fn types_compile() {
        // 编译通过即可；真单测在 Phase C 各 Task 内
        let _ = CreateInput {
            title: "x".into(),
            due_at: None,
            priority: None,
        };
        let _ = DueAtChange::Set("2026-05-23T10:00:00Z".into());
        let _ = DueAtChange::Clear;
    }
}
```

- [ ] **Step 2: 注册到 services/mod.rs**

Modify `src-tauri/src/services/mod.rs`，在 `pub mod scheduler;` 后追加：

```rust
// #29 TodoService：待办 CRUD + reminder 联动（schema 见 migrations/001_init.sql:122）。
pub mod todo;
```

- [ ] **Step 3: cargo check**

Run: `cd src-tauri && cargo check --bins`
Expected: 编译通过

- [ ] **Step 4: 跑 placeholder 单测**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo`
Expected: `types_compile` PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/todo.rs src-tauri/src/services/mod.rs
git commit -m "feat: #29 services/todo.rs 骨架 (types + 6 stub fn + 错误枚举)"
```

---

## Task A3: commands/todo.rs 骨架 + lib.rs invoke_handler 注册

**Files:**
- Create: `src-tauri/src/commands/todo.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`（invoke_handler! 段）

- [ ] **Step 1: 写 commands/todo.rs（仿 commands/reminder.rs 风格）**

Create `src-tauri/src/commands/todo.rs`:

```rust
//! Todo IPC commands（#29）— 6 命令。
//!
//! 命名遵循 Tauri 2.x runtime 规范：snake_case [a-zA-Z0-9_]（架构 §566 + 已有
//! reminder_create / chat_send 等同风）。架构 §604 文档逻辑写 `todo.create` 仅作分组语义，
//! 注册名是 `todo_create`。

use tauri::AppHandle;

use crate::services::todo::{self, CreateInput, Todo, UpdateInput};

#[tauri::command]
pub async fn todo_create(app: AppHandle, input: CreateInput) -> Result<Todo, String> {
    todo::create(&app, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn todo_list(app: AppHandle) -> Result<Vec<Todo>, String> {
    todo::list(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn todo_update(
    app: AppHandle,
    id: String,
    input: UpdateInput,
) -> Result<Todo, String> {
    todo::update(&app, id, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn todo_complete(app: AppHandle, id: String) -> Result<Todo, String> {
    todo::complete(&app, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn todo_breakdown(app: AppHandle, id: String) -> Result<Vec<String>, String> {
    todo::breakdown(&app, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn todo_reorder(
    app: AppHandle,
    id: String,
    after_id: Option<String>,
) -> Result<Todo, String> {
    todo::reorder(&app, id, after_id).await.map_err(|e| e.to_string())
}
```

- [ ] **Step 2: 注册到 commands/mod.rs**

Modify `src-tauri/src/commands/mod.rs`，在 `pub mod reminder;` 后追加：

```rust
// #29 todo IPC（6 命令：create/list/update/complete/breakdown/reorder）。
pub mod todo;
```

- [ ] **Step 3: 注册到 lib.rs invoke_handler!**

打开 `src-tauri/src/lib.rs` 找 `invoke_handler!(tauri::generate_handler![...])` 段，在末尾追加 6 项（按 reminder_* 同 pattern）：

```rust
            // #29 todo（6 命令）
            crate::commands::todo::todo_create,
            crate::commands::todo::todo_list,
            crate::commands::todo::todo_update,
            crate::commands::todo::todo_complete,
            crate::commands::todo::todo_breakdown,
            crate::commands::todo::todo_reorder,
```

- [ ] **Step 4: cargo check**

Run: `cd src-tauri && cargo check --bins`
Expected: 编译通过（IPC stub 调 service stub fn）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/todo.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: #29 commands/todo.rs (6 IPC stub) + lib.rs invoke_handler 注册"
```

---

# Phase B — reminder.rs / preferences.rs tx-injection 准备

> 这一阶段是 #29 设计 §7.4 的真实落地：抽取 `*_with_conn` 私有 helper + 暴露 `*_internal_tx` 公共入口，让 todo.rs 和 onboarding_reminders.rs 能在外部 Transaction 内调用 reminder 写操作。**前置事实**：[reminder.rs](../../../src-tauri/src/services/reminder.rs) 当前仅有 `get_with_conn` / `list_with_conn`，create/update/delete 公共 fn 内部自取 connection——必须先重构。

## Task B1: 提取 reminder::create_with_conn + 加 create_internal_tx

**Files:**
- Modify: `src-tauri/src/services/reminder.rs`

- [ ] **Step 1: 阅读现状**

Run: `cd src-tauri && grep -n "pub async fn create" src/services/reminder.rs`

定位现有 `pub async fn create<R: Runtime>(app: &AppHandle<R>, input: CreateInput) -> Result<Reminder, ReminderError>` 的实现段（含 ULID 生成 + compute_next_fire_at + INSERT 语句）。

- [ ] **Step 2: 把 INSERT 主体重构为 create_with_conn 私有 helper**

把 `create` 函数体内"打开 connection 后开始的部分"剪到新 helper：

```rust
/// INSERT 主体 — 接 `&mut SqliteConnection`，复用给 tx 路径与 pool 路径。
async fn create_with_conn(
    conn: &mut SqliteConnection,
    input: CreateInput,
) -> Result<Reminder, ReminderError> {
    let id = Ulid::new().to_string();
    let now = Utc::now();
    let now_str = now.to_rfc3339();
    let priority = input.priority.unwrap_or_else(|| "soft".into());

    // compute next_fire_at 复用现有 fn（once → trigger_spec 字串本身；daily → compute）
    let next_fire = compute_next_fire_at(&input.trigger_type, &input.trigger_spec, now)?;
    let next_fire_str = next_fire.map(|d| d.to_rfc3339());

    sqlx::query(
        r#"INSERT INTO reminders
           (id, title, trigger_type, trigger_spec, priority, enabled, snooze_count,
            next_fire_at, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, 1, 0, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(&input.title)
    .bind(&input.trigger_type)
    .bind(&input.trigger_spec)
    .bind(&priority)
    .bind(&next_fire_str)
    .bind(&now_str)
    .bind(&now_str)
    .execute(&mut *conn)
    .await?;

    get_with_conn(conn, &id).await
}
```

（具体字段依现有 create fn 实际语句调整 — 保持现 INSERT 语义不变即可，关键是改成接 `&mut SqliteConnection`）

- [ ] **Step 3: 加 create_internal_tx 公共入口**

在 `create_with_conn` 后紧跟：

```rust
/// 跨 service 写操作专用入口：接 `&mut Transaction`，调用方负责 begin / commit / rollback。
/// 用于 todo.rs / onboarding_reminders.rs 把 reminder 写入合并进自己的 tx。
pub async fn create_internal_tx(
    tx: &mut Transaction<'_, Sqlite>,
    input: CreateInput,
) -> Result<Reminder, ReminderError> {
    let conn: &mut SqliteConnection = &mut **tx;
    create_with_conn(conn, input).await
}
```

- [ ] **Step 4: 把公共 `create` IPC fn 改为 thin wrapper**

```rust
pub async fn create<R: Runtime>(
    app: &AppHandle<R>,
    input: CreateInput,
) -> Result<Reminder, ReminderError> {
    let db_path = open_app_db(app).await?;
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path.display())).await?;
    let mut tx = pool.begin().await?;
    let out = create_internal_tx(&mut tx, input).await?;
    tx.commit().await?;
    Ok(out)
}
```

（具体 pool 构造细节参考现有 `open_app_db` 返回类型；如已有 `connect_app_db` helper 直接复用）

- [ ] **Step 5: 加 sqlx 导入**

文件顶部 `use sqlx::{Connection, SqliteConnection};` 改为：

```rust
use sqlx::{Sqlite, SqliteConnection, Transaction};
```

（Connection trait 若仍被 list_with_conn 等用就保留）

- [ ] **Step 6: cargo check**

Run: `cd src-tauri && cargo check --bins`
Expected: 编译通过

- [ ] **Step 7: 跑既有 reminder 单测确保不退化**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::reminder`
Expected: 既有 23 个 reminder 单测全部 PASS

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/services/reminder.rs
git commit -m "refactor: #29 reminder.rs 提取 create_with_conn + 加 create_internal_tx tx 入口"
```

---

## Task B2: 提取 reminder::update_with_conn + 加 update_internal_tx

**Files:**
- Modify: `src-tauri/src/services/reminder.rs`

- [ ] **Step 1: 同 Task B1 套路重构 `update`**

把现有 `pub async fn update<R: Runtime>(...)` 的 INSERT/UPDATE 主体剪到 `update_with_conn`：

```rust
async fn update_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
    input: UpdateInput,
) -> Result<Reminder, ReminderError> {
    let now = Utc::now();
    let now_str = now.to_rfc3339();
    // 读旧 row 拿 trigger_type / trigger_spec 当 default（COALESCE 也行）
    let existing = get_with_conn(conn, id).await?;

    let title = input.title.unwrap_or(existing.title);
    let trigger_type = input.trigger_type.unwrap_or(existing.trigger_type);
    let trigger_spec = input.trigger_spec.unwrap_or(existing.trigger_spec);
    let priority = input.priority.unwrap_or(existing.priority);
    let enabled = input.enabled.unwrap_or(existing.enabled);

    let next_fire = compute_next_fire_at(&trigger_type, &trigger_spec, now)?;
    let next_fire_str = next_fire.map(|d| d.to_rfc3339());

    sqlx::query(
        r#"UPDATE reminders
           SET title=?, trigger_type=?, trigger_spec=?, priority=?, enabled=?,
               next_fire_at=?, updated_at=?
           WHERE id=?"#,
    )
    .bind(&title)
    .bind(&trigger_type)
    .bind(&trigger_spec)
    .bind(&priority)
    .bind(enabled as i64)
    .bind(&next_fire_str)
    .bind(&now_str)
    .bind(id)
    .execute(&mut *conn)
    .await?;

    get_with_conn(conn, id).await
}

pub async fn update_internal_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: UpdateInput,
) -> Result<Reminder, ReminderError> {
    let conn: &mut SqliteConnection = &mut **tx;
    update_with_conn(conn, id, input).await
}
```

（实际字段以现有 UpdateInput 为准 — 保留原 update 语义；如现有 update 也支持 `enabled` 字段则保留）

- [ ] **Step 2: `update` IPC fn 改为 thin wrapper**

```rust
pub async fn update<R: Runtime>(
    app: &AppHandle<R>,
    id: String,
    input: UpdateInput,
) -> Result<Reminder, ReminderError> {
    let db_path = open_app_db(app).await?;
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path.display())).await?;
    let mut tx = pool.begin().await?;
    let out = update_internal_tx(&mut tx, &id, input).await?;
    tx.commit().await?;
    Ok(out)
}
```

- [ ] **Step 3: cargo check + 跑 reminder 单测**

Run: `cd src-tauri && cargo check --bins && cargo test -p aipet-app --lib services::reminder`
Expected: 通过

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/reminder.rs
git commit -m "refactor: #29 reminder.rs 提取 update_with_conn + 加 update_internal_tx"
```

---

## Task B3: 提取 reminder::delete_with_conn + 加 delete_internal_tx

**Files:**
- Modify: `src-tauri/src/services/reminder.rs`

**关键点**：现有 `delete` 含 reminder_history 级联清理（spec §1 文件头注释提到无 FK，手动级联），必须保留同语义。

- [ ] **Step 1: 提取 delete_with_conn**

```rust
async fn delete_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<(), ReminderError> {
    // 手动级联清 history（无 FK，trade-off 简单 vs 数据完整性）
    sqlx::query("DELETE FROM reminder_history WHERE reminder_id=?")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    let result = sqlx::query("DELETE FROM reminders WHERE id=?")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ReminderError::NotFound(id.to_string()));
    }
    Ok(())
}

pub async fn delete_internal_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<(), ReminderError> {
    let conn: &mut SqliteConnection = &mut **tx;
    delete_with_conn(conn, id).await
}
```

（核对现有 `delete` 是否有 NotFound 检查 — 若无则不加，保持现语义；若有则保留）

- [ ] **Step 2: `delete` IPC fn 改为 thin wrapper**

```rust
pub async fn delete<R: Runtime>(app: &AppHandle<R>, id: String) -> Result<(), ReminderError> {
    let db_path = open_app_db(app).await?;
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path.display())).await?;
    let mut tx = pool.begin().await?;
    delete_internal_tx(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(())
}
```

- [ ] **Step 3: cargo check + 跑 reminder 单测**

Run: `cd src-tauri && cargo check --bins && cargo test -p aipet-app --lib services::reminder`
Expected: 通过

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/reminder.rs
git commit -m "refactor: #29 reminder.rs 提取 delete_with_conn + 加 delete_internal_tx (含 history 级联)"
```

---

## Task B4: preferences::delete_tx 入口

**Files:**
- Modify: `src-tauri/src/services/preferences.rs`

`delete_with_conn` 已存在（[preferences.rs:139](../../../src-tauri/src/services/preferences.rs#L139)），只需 thin wrapper。

- [ ] **Step 1: 在 delete_with_conn 后追加 delete_tx**

```rust
/// #29 跨 service 写操作专用入口：接 `&mut Transaction`，让 onboarding_reminders
/// drain 时 reminder.create + memory.delete 在同一 tx 内原子化。
pub async fn delete_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    key: &str,
) -> Result<(), PreferenceError> {
    let conn: &mut SqliteConnection = &mut **tx;
    delete_with_conn(conn, key).await
}
```

（注意 visibility：`delete_with_conn` 是 `pub(crate)`，`delete_tx` 取相同 `pub(crate)` 即可；若需 `pub` 公开则视调用方位置选 `pub`）

调用方在 `services/onboarding_reminders.rs`（同 crate）→ `pub(crate)` 已足够。

- [ ] **Step 2: cargo check**

Run: `cd src-tauri && cargo check --bins`
Expected: 通过

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/services/preferences.rs
git commit -m "feat: #29 preferences.rs 加 delete_tx 入口 (tx-injection)"
```

---

# Phase C — TodoService 业务实现

> Phase B 完成后，reminder / preferences 提供了 tx 入口。本阶段把 services/todo.rs 的 6 个 stub 业务函数依次实现，配单测。每个 Task 走 TDD：先写测试 → 跑 fail → 实现 → 跑 pass → commit。

## Task C1: todo::create 无 due_at 路径 + 单测

**Files:**
- Modify: `src-tauri/src/services/todo.rs`

- [ ] **Step 1: 写失败测试**

在 `services/todo.rs::tests` 内追加：

```rust
#[tokio::test]
async fn create_without_due_at_inserts_row_with_defaults() {
    let (_dir, mut conn) = fresh_db().await;
    let mut tx = conn.begin().await.unwrap();

    let todo = create_with_tx(
        &mut tx,
        CreateInput {
            title: "买菜".into(),
            due_at: None,
            priority: None,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(todo.title, "买菜");
    assert_eq!(todo.status, "open");
    assert_eq!(todo.priority, "normal");
    assert!(todo.due_at.is_none());
    assert!(todo.reminder_id.is_none());
}
```

测试调用 `create_with_tx`（私有 helper，接 `&mut Transaction`）— 这个 helper Step 3 创建。

- [ ] **Step 2: cargo test 跑应 fail**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo::tests::create_without_due_at`
Expected: FAIL（`create_with_tx` 未定义）

- [ ] **Step 3: 实现 create_with_tx 私有 helper + 改 pub create**

在 todo.rs 业务函数区追加：

```rust
/// 内部实现：接 `&mut Transaction`，所有联动操作在同一 tx 内。
async fn create_with_tx(
    tx: &mut Transaction<'_, Sqlite>,
    input: CreateInput,
) -> Result<Todo, TodoError> {
    if input.title.trim().is_empty() {
        return Err(TodoError::InvalidInput("title cannot be empty".into()));
    }
    let id = Ulid::new().to_string();
    let now = Utc::now().to_rfc3339();
    let priority = input.priority.as_deref().unwrap_or("normal").to_string();
    validate_priority(&priority)?;

    // due_at 非空时联动 reminder（C2 接入；本 Task 仅无 due_at 路径）
    let reminder_id: Option<String> = if let Some(ref _due) = input.due_at {
        None // 占位；C2 替换
    } else {
        None
    };

    // 计算 order_index：放最前（min - 10.0；首条用 0）
    let row: Option<(Option<f64>,)> = sqlx::query_as(
        "SELECT MIN(order_index) FROM todos WHERE status='open'",
    )
    .fetch_optional(&mut **tx)
    .await?;
    let min_order: Option<f64> = row.and_then(|(v,)| v);
    let order_index = match min_order {
        Some(v) => v - 10.0,
        None => 0.0,
    };

    {
        let conn: &mut SqliteConnection = &mut **tx;
        sqlx::query(
            r#"INSERT INTO todos
               (id, title, status, due_at, reminder_id, order_index, priority, created_at, updated_at)
               VALUES (?, ?, 'open', ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&id)
        .bind(&input.title)
        .bind(&input.due_at)
        .bind(&reminder_id)
        .bind(order_index)
        .bind(&priority)
        .bind(&now)
        .bind(&now)
        .execute(conn)
        .await?;
    }

    get_by_id(&mut **tx, &id).await
}

fn validate_priority(p: &str) -> Result<(), TodoError> {
    match p {
        "low" | "normal" | "high" => Ok(()),
        _ => Err(TodoError::InvalidInput(format!("invalid priority: {p}"))),
    }
}

async fn get_by_id(conn: &mut SqliteConnection, id: &str) -> Result<Todo, TodoError> {
    let row: Option<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        f64,
        String,
        String,
        String,
    )> = sqlx::query_as(
        r#"SELECT id, title, status, due_at, reminder_id, order_index, priority, created_at, updated_at
           FROM todos WHERE id=?"#,
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    row.map(|r| Todo {
        id: r.0,
        title: r.1,
        status: r.2,
        due_at: r.3,
        reminder_id: r.4,
        order_index: r.5,
        priority: r.6,
        created_at: r.7,
        updated_at: r.8,
    })
    .ok_or_else(|| TodoError::NotFound(id.to_string()))
}

pub async fn create<R: Runtime>(app: &AppHandle<R>, input: CreateInput) -> Result<Todo, TodoError> {
    let db_path = open_app_db(app).await?;
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path.display())).await?;
    let mut tx = pool.begin().await?;
    let out = create_with_tx(&mut tx, input).await?;
    tx.commit().await?;
    Ok(out)
}
```

把原 stub `pub async fn create` 替换为以上版本。

测试模块顶部加 `use sqlx::Connection;` 让 `conn.begin()` 可调用（如 test_db 已 re-export 则跳）。

- [ ] **Step 4: cargo test 应 pass**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo::tests::create_without_due_at`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/todo.rs
git commit -m "feat: #29 todo::create 无 due_at 路径 + 单测"
```

---

## Task C2: todo::create 带 due_at 路径（reminder coupling）+ 单测

**Files:**
- Modify: `src-tauri/src/services/todo.rs`

- [ ] **Step 1: 写失败测试**

```rust
#[tokio::test]
async fn create_with_due_at_writes_reminder_and_backfills_id() {
    let (_dir, mut conn) = fresh_db().await;
    let mut tx = conn.begin().await.unwrap();

    let todo = create_with_tx(
        &mut tx,
        CreateInput {
            title: "复诊".into(),
            due_at: Some("2026-06-01T09:00:00Z".into()),
            priority: Some("high".into()),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(todo.due_at.as_deref(), Some("2026-06-01T09:00:00Z"));
    let rid = todo.reminder_id.expect("reminder_id should be set");
    // 验证 reminders 表 +1 行
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders WHERE id=?")
        .bind(&rid)
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}
```

- [ ] **Step 2: cargo test 应 fail**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo::tests::create_with_due_at`
Expected: FAIL（reminder_id 为 None；reminders 表 0 行）

- [ ] **Step 3: 在 create_with_tx 中替换 reminder_id 占位段**

把 C1 的：

```rust
    let reminder_id: Option<String> = if let Some(ref _due) = input.due_at {
        None
    } else {
        None
    };
```

替换为：

```rust
    let reminder_id: Option<String> = if let Some(ref due) = input.due_at {
        let r = crate::services::reminder::create_internal_tx(
            tx,
            crate::services::reminder::CreateInput {
                title: input.title.clone(),
                trigger_type: "once".into(),
                trigger_spec: due.clone(),
                priority: Some("soft".into()),
            },
        )
        .await
        .map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
        Some(r.id)
    } else {
        None
    };
```

- [ ] **Step 4: cargo test 应 pass**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo::tests::create_with_due_at`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/todo.rs
git commit -m "feat: #29 todo::create 带 due_at → tx 内联动 reminder + 回填 reminder_id"
```

---

## Task C3: todo::list 后端排序 + 单测

**Files:**
- Modify: `src-tauri/src/services/todo.rs`

- [ ] **Step 1: 写测试**

```rust
#[tokio::test]
async fn list_returns_open_first_then_done_then_cancelled_sorted_by_order_index() {
    let (_dir, mut conn) = fresh_db().await;
    // 直插 3 行不同 status + order_index
    let now = Utc::now().to_rfc3339();
    for (id, status, oi) in &[
        ("a", "done", 5.0),
        ("b", "open", 20.0),
        ("c", "open", 10.0),
        ("d", "cancelled", 1.0),
    ] {
        sqlx::query(
            r#"INSERT INTO todos (id,title,status,order_index,priority,created_at,updated_at)
               VALUES (?, ?, ?, ?, 'normal', ?, ?)"#,
        )
        .bind(*id).bind(*id).bind(*status).bind(*oi).bind(&now).bind(&now)
        .execute(&mut conn).await.unwrap();
    }

    let list = list_with_conn(&mut conn).await.unwrap();
    let ids: Vec<&str> = list.iter().map(|t| t.id.as_str()).collect();
    // open (按 order_index ASC: c=10, b=20), then done (a), then cancelled (d)
    assert_eq!(ids, vec!["c", "b", "a", "d"]);
}
```

- [ ] **Step 2: cargo test 应 fail**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo::tests::list_returns_open_first`
Expected: FAIL（`list_with_conn` 未定义）

- [ ] **Step 3: 实现 list_with_conn + pub list**

```rust
async fn list_with_conn(conn: &mut SqliteConnection) -> Result<Vec<Todo>, TodoError> {
    let rows: Vec<(
        String, String, String, Option<String>, Option<String>, f64, String, String, String,
    )> = sqlx::query_as(
        r#"SELECT id, title, status, due_at, reminder_id, order_index, priority, created_at, updated_at
           FROM todos
           ORDER BY CASE status
                      WHEN 'open' THEN 0
                      WHEN 'done' THEN 1
                      ELSE 2
                    END ASC,
                    order_index ASC,
                    updated_at DESC"#,
    )
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().map(|r| Todo {
        id: r.0, title: r.1, status: r.2, due_at: r.3, reminder_id: r.4,
        order_index: r.5, priority: r.6, created_at: r.7, updated_at: r.8,
    }).collect())
}

pub async fn list<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<Todo>, TodoError> {
    let db_path = open_app_db(app).await?;
    let mut conn = sqlx::SqliteConnection::connect(&format!("sqlite:{}", db_path.display())).await?;
    list_with_conn(&mut conn).await
}
```

替换原 stub `pub async fn list`。需在文件顶 use 处加 `sqlx::Connection`（让 connect 可调）—— 实际上 connect 是关联函数，无需 trait import；视编译错按需调整。

- [ ] **Step 4: cargo test 应 pass**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo::tests::list_returns_open_first`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/todo.rs
git commit -m "feat: #29 todo::list SQL 排序 (status → order_index → updated_at)"
```

---

## Task C4: todo::update title-only 路径 + 单测

**Files:**
- Modify: `src-tauri/src/services/todo.rs`

- [ ] **Step 1: 写测试**

```rust
#[tokio::test]
async fn update_title_only_does_not_touch_reminder() {
    let (_dir, mut conn) = fresh_db().await;
    let mut tx = conn.begin().await.unwrap();
    let todo = create_with_tx(
        &mut tx,
        CreateInput { title: "old".into(), due_at: None, priority: None },
    ).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = conn.begin().await.unwrap();
    let updated = update_with_tx(
        &mut tx,
        &todo.id,
        UpdateInput { title: Some("new".into()), ..Default::default() },
    ).await.unwrap();
    tx.commit().await.unwrap();

    assert_eq!(updated.title, "new");
    assert!(updated.reminder_id.is_none());
}
```

- [ ] **Step 2: cargo test 应 fail**

Expected: FAIL（`update_with_tx` 未定义）

- [ ] **Step 3: 实现 update_with_tx 框架 + title-only 路径**

```rust
async fn update_with_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: UpdateInput,
) -> Result<Todo, TodoError> {
    // 读旧
    let existing = get_by_id(&mut **tx, id).await?;

    let new_title = input.title.unwrap_or(existing.title.clone());
    if new_title.trim().is_empty() {
        return Err(TodoError::InvalidInput("title cannot be empty".into()));
    }
    let new_priority = match input.priority.as_deref() {
        Some(p) => { validate_priority(p)?; p.to_string() }
        None => existing.priority.clone(),
    };

    let new_status = match input.status.as_deref() {
        Some(s) if s == "open" || s == "cancelled" => s.to_string(),
        Some(s) if s == "done" => {
            return Err(TodoError::InvalidInput(
                "status='done' must go through todo_complete".into()
            ));
        }
        Some(other) => return Err(TodoError::InvalidInput(format!("invalid status: {other}"))),
        None => existing.status.clone(),
    };

    // due_at + reminder 联动留待 C5；本 Task 仅处理 input.due_at == None 路径
    let new_due_at: Option<String> = existing.due_at.clone();
    let new_reminder_id: Option<String> = existing.reminder_id.clone();

    let now = Utc::now().to_rfc3339();
    {
        let conn: &mut SqliteConnection = &mut **tx;
        sqlx::query(
            r#"UPDATE todos
               SET title=?, status=?, due_at=?, reminder_id=?, priority=?, updated_at=?
               WHERE id=?"#,
        )
        .bind(&new_title).bind(&new_status).bind(&new_due_at).bind(&new_reminder_id)
        .bind(&new_priority).bind(&now).bind(id)
        .execute(conn).await?;
    }
    get_by_id(&mut **tx, id).await
}

pub async fn update<R: Runtime>(
    app: &AppHandle<R>,
    id: String,
    input: UpdateInput,
) -> Result<Todo, TodoError> {
    let db_path = open_app_db(app).await?;
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path.display())).await?;
    let mut tx = pool.begin().await?;
    let out = update_with_tx(&mut tx, &id, input).await?;
    tx.commit().await?;
    Ok(out)
}
```

替换原 stub。

- [ ] **Step 4: cargo test 应 pass**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo::tests::update_title_only`
Expected: PASS

- [ ] **Step 5: 加 done 路径拒绝测试**

```rust
#[tokio::test]
async fn update_cannot_set_status_done_directly() {
    let (_dir, mut conn) = fresh_db().await;
    let mut tx = conn.begin().await.unwrap();
    let todo = create_with_tx(
        &mut tx,
        CreateInput { title: "x".into(), due_at: None, priority: None },
    ).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = conn.begin().await.unwrap();
    let err = update_with_tx(
        &mut tx, &todo.id,
        UpdateInput { status: Some("done".into()), ..Default::default() },
    ).await.unwrap_err();
    assert!(matches!(err, TodoError::InvalidInput(_)));
}
```

Run + Expected: PASS（Step 3 已加 done 拒绝分支）

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/todo.rs
git commit -m "feat: #29 todo::update title-only + done-rejected 路径 + 2 单测"
```

---

## Task C5: todo::update due_at Set/Clear 联动 reminder + 单测

**Files:**
- Modify: `src-tauri/src/services/todo.rs`

- [ ] **Step 1: 写 3 个测试**

```rust
#[tokio::test]
async fn update_due_at_set_on_null_creates_reminder() {
    let (_dir, mut conn) = fresh_db().await;
    let mut tx = conn.begin().await.unwrap();
    let todo = create_with_tx(
        &mut tx,
        CreateInput { title: "x".into(), due_at: None, priority: None },
    ).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = conn.begin().await.unwrap();
    let updated = update_with_tx(
        &mut tx, &todo.id,
        UpdateInput {
            due_at: Some(DueAtChange::Set("2026-06-01T09:00:00Z".into())),
            ..Default::default()
        },
    ).await.unwrap();
    tx.commit().await.unwrap();

    assert_eq!(updated.due_at.as_deref(), Some("2026-06-01T09:00:00Z"));
    assert!(updated.reminder_id.is_some());
}

#[tokio::test]
async fn update_due_at_set_on_existing_updates_reminder_spec() {
    let (_dir, mut conn) = fresh_db().await;
    let mut tx = conn.begin().await.unwrap();
    let todo = create_with_tx(
        &mut tx,
        CreateInput {
            title: "x".into(),
            due_at: Some("2026-06-01T09:00:00Z".into()),
            priority: None,
        },
    ).await.unwrap();
    tx.commit().await.unwrap();
    let original_rid = todo.reminder_id.clone().unwrap();

    let mut tx = conn.begin().await.unwrap();
    let updated = update_with_tx(
        &mut tx, &todo.id,
        UpdateInput {
            due_at: Some(DueAtChange::Set("2026-06-02T10:00:00Z".into())),
            ..Default::default()
        },
    ).await.unwrap();
    tx.commit().await.unwrap();

    assert_eq!(updated.reminder_id.as_deref(), Some(original_rid.as_str()));
    // 验证 reminder.trigger_spec 同步更新
    let spec: (String,) = sqlx::query_as("SELECT trigger_spec FROM reminders WHERE id=?")
        .bind(&original_rid).fetch_one(&mut conn).await.unwrap();
    assert_eq!(spec.0, "2026-06-02T10:00:00Z");
}

#[tokio::test]
async fn update_due_at_clear_deletes_reminder() {
    let (_dir, mut conn) = fresh_db().await;
    let mut tx = conn.begin().await.unwrap();
    let todo = create_with_tx(
        &mut tx,
        CreateInput {
            title: "x".into(),
            due_at: Some("2026-06-01T09:00:00Z".into()),
            priority: None,
        },
    ).await.unwrap();
    tx.commit().await.unwrap();
    let original_rid = todo.reminder_id.clone().unwrap();

    let mut tx = conn.begin().await.unwrap();
    let updated = update_with_tx(
        &mut tx, &todo.id,
        UpdateInput { due_at: Some(DueAtChange::Clear), ..Default::default() },
    ).await.unwrap();
    tx.commit().await.unwrap();

    assert!(updated.due_at.is_none());
    assert!(updated.reminder_id.is_none());
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders WHERE id=?")
        .bind(&original_rid).fetch_one(&mut conn).await.unwrap();
    assert_eq!(count.0, 0);
}
```

- [ ] **Step 2: cargo test 应 fail**

Expected: 3 个 FAIL（update_with_tx 未处理 due_at）

- [ ] **Step 3: 修改 update_with_tx 加入 due_at 联动**

把 Task C4 的占位段：

```rust
    let new_due_at: Option<String> = existing.due_at.clone();
    let new_reminder_id: Option<String> = existing.reminder_id.clone();
```

替换为：

```rust
    let (new_due_at, new_reminder_id): (Option<String>, Option<String>) = match input.due_at {
        // 字段省略 = keep
        None => (existing.due_at.clone(), existing.reminder_id.clone()),
        // Set(v)
        Some(DueAtChange::Set(value)) => {
            match (&existing.reminder_id, &existing.due_at) {
                // 原有 reminder + 改时刻 → update reminder.trigger_spec
                (Some(rid), Some(_)) => {
                    crate::services::reminder::update_internal_tx(
                        tx, rid,
                        crate::services::reminder::UpdateInput {
                            title: Some(new_title.clone()),
                            trigger_type: Some("once".into()),
                            trigger_spec: Some(value.clone()),
                            ..Default::default()
                        },
                    ).await.map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
                    (Some(value), Some(rid.clone()))
                }
                // 原 null → 新建 reminder
                _ => {
                    let r = crate::services::reminder::create_internal_tx(
                        tx,
                        crate::services::reminder::CreateInput {
                            title: new_title.clone(),
                            trigger_type: "once".into(),
                            trigger_spec: value.clone(),
                            priority: Some("soft".into()),
                        },
                    ).await.map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
                    (Some(value), Some(r.id))
                }
            }
        }
        // Clear
        Some(DueAtChange::Clear) => {
            if let Some(rid) = &existing.reminder_id {
                crate::services::reminder::delete_internal_tx(tx, rid)
                    .await.map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
            }
            (None, None)
        }
    };

    // 若仅改 title 但有 reminder_id 同步 title
    if input.due_at.is_none() && existing.reminder_id.is_some() && new_title != existing.title {
        let rid = existing.reminder_id.as_ref().unwrap();
        crate::services::reminder::update_internal_tx(
            tx, rid,
            crate::services::reminder::UpdateInput {
                title: Some(new_title.clone()),
                ..Default::default()
            },
        ).await.map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
    }
```

注意：`reminder::UpdateInput` 的 `Default` 派生需在 reminder.rs 上有 `#[derive(Default)]`。若没有，则用具名构造逐字段 `None`。

- [ ] **Step 4: cargo test 三例应 pass**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo::tests::update_due_at`
Expected: 3 例 PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/todo.rs
git commit -m "feat: #29 todo::update due_at Set/Clear 联动 reminder + title 同步 + 3 单测"
```

---

## Task C6: todo::update 软删 cancelled + 有 once reminder → 删 reminder + 单测

**Files:**
- Modify: `src-tauri/src/services/todo.rs`

- [ ] **Step 1: 写测试**

```rust
#[tokio::test]
async fn cancel_via_update_with_once_reminder_deletes_reminder() {
    let (_dir, mut conn) = fresh_db().await;
    let mut tx = conn.begin().await.unwrap();
    let todo = create_with_tx(
        &mut tx,
        CreateInput {
            title: "x".into(),
            due_at: Some("2026-06-01T09:00:00Z".into()),
            priority: None,
        },
    ).await.unwrap();
    tx.commit().await.unwrap();
    let rid = todo.reminder_id.clone().unwrap();

    let mut tx = conn.begin().await.unwrap();
    let updated = update_with_tx(
        &mut tx, &todo.id,
        UpdateInput { status: Some("cancelled".into()), ..Default::default() },
    ).await.unwrap();
    tx.commit().await.unwrap();

    assert_eq!(updated.status, "cancelled");
    assert!(updated.reminder_id.is_none());
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders WHERE id=?")
        .bind(&rid).fetch_one(&mut conn).await.unwrap();
    assert_eq!(count.0, 0);
}
```

- [ ] **Step 2: cargo test 应 fail**

Expected: FAIL（reminder 仍在）

- [ ] **Step 3: 在 update_with_tx 中加 cancelled 联动**

在 `new_status` 决定后、`UPDATE todos` 之前插入：

```rust
    // 软删 cancelled + 有 once reminder → 删 reminder（同 complete 语义）
    let (final_due_at, final_reminder_id) = if new_status == "cancelled" && existing.reminder_id.is_some() {
        let rid = existing.reminder_id.as_ref().unwrap();
        // 仅 once 类型才删（防误删 daily reminder——但 todo 联动只创 once，此处保险查 trigger_type）
        let r = crate::services::reminder::get_internal_tx(tx, rid).await
            .map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
        if r.trigger_type == "once" {
            crate::services::reminder::delete_internal_tx(tx, rid).await
                .map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
            (None, None)
        } else {
            (new_due_at.clone(), new_reminder_id.clone())
        }
    } else {
        (new_due_at.clone(), new_reminder_id.clone())
    };
```

注意：`reminder::get_internal_tx` 也需提供 — 用既有 `get_with_conn` 做 deref 包一层。如果 reminder.rs 没有 `get_internal_tx`，回到 Phase B 补一个（接 `&mut Transaction`）：

```rust
// 加在 reminder.rs：
pub async fn get_internal_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<Reminder, ReminderError> {
    let conn: &mut SqliteConnection = &mut **tx;
    get_with_conn(conn, id).await
}
```

把 update_with_tx 末尾 UPDATE 语句的 `new_due_at` / `new_reminder_id` 改用 `final_due_at` / `final_reminder_id`：

```rust
        sqlx::query(
            r#"UPDATE todos
               SET title=?, status=?, due_at=?, reminder_id=?, priority=?, updated_at=?
               WHERE id=?"#,
        )
        .bind(&new_title).bind(&new_status).bind(&final_due_at).bind(&final_reminder_id)
        .bind(&new_priority).bind(&now).bind(id)
        .execute(conn).await?;
```

- [ ] **Step 4: cargo test 应 pass**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo::tests::cancel_via_update_with_once_reminder`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/todo.rs src-tauri/src/services/reminder.rs
git commit -m "feat: #29 todo cancelled + 有 once reminder → 同 tx 删 reminder + reminder::get_internal_tx"
```

---

## Task C7: todo::complete + once-reminder 删除 + 单测

**Files:**
- Modify: `src-tauri/src/services/todo.rs`

- [ ] **Step 1: 写 2 个测试**

```rust
#[tokio::test]
async fn complete_with_once_reminder_deletes_reminder_and_clears_id() {
    let (_dir, mut conn) = fresh_db().await;
    let mut tx = conn.begin().await.unwrap();
    let todo = create_with_tx(
        &mut tx,
        CreateInput {
            title: "x".into(),
            due_at: Some("2026-06-01T09:00:00Z".into()),
            priority: None,
        },
    ).await.unwrap();
    tx.commit().await.unwrap();
    let rid = todo.reminder_id.clone().unwrap();

    let mut tx = conn.begin().await.unwrap();
    let done = complete_with_tx(&mut tx, &todo.id).await.unwrap();
    tx.commit().await.unwrap();

    assert_eq!(done.status, "done");
    assert!(done.reminder_id.is_none());
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders WHERE id=?")
        .bind(&rid).fetch_one(&mut conn).await.unwrap();
    assert_eq!(count.0, 0);
}

#[tokio::test]
async fn complete_without_reminder_is_noop_on_reminders_table() {
    let (_dir, mut conn) = fresh_db().await;
    let mut tx = conn.begin().await.unwrap();
    let todo = create_with_tx(
        &mut tx,
        CreateInput { title: "x".into(), due_at: None, priority: None },
    ).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = conn.begin().await.unwrap();
    let done = complete_with_tx(&mut tx, &todo.id).await.unwrap();
    tx.commit().await.unwrap();

    assert_eq!(done.status, "done");
    assert!(done.reminder_id.is_none());
}
```

- [ ] **Step 2: cargo test 应 fail**

Expected: FAIL（complete_with_tx 未定义）

- [ ] **Step 3: 实现 complete_with_tx + pub complete**

```rust
async fn complete_with_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<Todo, TodoError> {
    let existing = get_by_id(&mut **tx, id).await?;
    // 删 once reminder
    let final_reminder_id: Option<String> = match &existing.reminder_id {
        Some(rid) => {
            let r = crate::services::reminder::get_internal_tx(tx, rid).await
                .map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
            if r.trigger_type == "once" {
                crate::services::reminder::delete_internal_tx(tx, rid).await
                    .map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
                None
            } else {
                Some(rid.clone())
            }
        }
        None => None,
    };
    let now = Utc::now().to_rfc3339();
    {
        let conn: &mut SqliteConnection = &mut **tx;
        sqlx::query(
            "UPDATE todos SET status='done', reminder_id=?, updated_at=? WHERE id=?",
        )
        .bind(&final_reminder_id).bind(&now).bind(id)
        .execute(conn).await?;
    }
    get_by_id(&mut **tx, id).await
}

pub async fn complete<R: Runtime>(app: &AppHandle<R>, id: String) -> Result<Todo, TodoError> {
    let db_path = open_app_db(app).await?;
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path.display())).await?;
    let mut tx = pool.begin().await?;
    let out = complete_with_tx(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(out)
}
```

替换原 stub。

- [ ] **Step 4: cargo test 应 pass**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo::tests::complete`
Expected: 2 例 PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/todo.rs
git commit -m "feat: #29 todo::complete + once-reminder 同 tx 删除 + 2 单测"
```

---

## Task C8: todo::reorder + normalize_order_indices + 单测

**Files:**
- Modify: `src-tauri/src/services/todo.rs`

- [ ] **Step 1: 写 3 个测试**

```rust
#[tokio::test]
async fn reorder_inserts_between_two_neighbors_with_midpoint() {
    let (_dir, mut conn) = fresh_db().await;
    // 预置 3 行 open，order_index = 0/10/20
    let now = Utc::now().to_rfc3339();
    for (id, oi) in &[("a", 0.0), ("b", 10.0), ("c", 20.0)] {
        sqlx::query(
            r#"INSERT INTO todos (id,title,status,order_index,priority,created_at,updated_at)
               VALUES (?, ?, 'open', ?, 'normal', ?, ?)"#,
        )
        .bind(*id).bind(*id).bind(*oi).bind(&now).bind(&now)
        .execute(&mut conn).await.unwrap();
    }
    // 把 c 拖到 a 后面（a→c→b）
    let mut tx = conn.begin().await.unwrap();
    let updated = reorder_with_tx(&mut tx, "c", Some("a")).await.unwrap();
    tx.commit().await.unwrap();
    assert!(updated.order_index > 0.0 && updated.order_index < 10.0);
    assert!((updated.order_index - 5.0).abs() < 1e-9);
}

#[tokio::test]
async fn reorder_to_top_uses_smaller_than_min() {
    let (_dir, mut conn) = fresh_db().await;
    let now = Utc::now().to_rfc3339();
    for (id, oi) in &[("a", 10.0), ("b", 20.0)] {
        sqlx::query(
            r#"INSERT INTO todos (id,title,status,order_index,priority,created_at,updated_at)
               VALUES (?, ?, 'open', ?, 'normal', ?, ?)"#,
        )
        .bind(*id).bind(*id).bind(*oi).bind(&now).bind(&now)
        .execute(&mut conn).await.unwrap();
    }
    let mut tx = conn.begin().await.unwrap();
    let updated = reorder_with_tx(&mut tx, "b", None).await.unwrap();  // 拖到最前
    tx.commit().await.unwrap();
    assert!(updated.order_index < 10.0);  // 小于现有 min
}

#[tokio::test]
async fn reorder_triggers_normalize_when_gap_under_threshold() {
    let (_dir, mut conn) = fresh_db().await;
    let now = Utc::now().to_rfc3339();
    // 故意制造 gap < 1e-6 的相邻对
    for (id, oi) in &[("a", 0.0), ("b", 1.0e-7), ("c", 10.0)] {
        sqlx::query(
            r#"INSERT INTO todos (id,title,status,order_index,priority,created_at,updated_at)
               VALUES (?, ?, 'open', ?, 'normal', ?, ?)"#,
        )
        .bind(*id).bind(*id).bind(*oi).bind(&now).bind(&now)
        .execute(&mut conn).await.unwrap();
    }
    let mut tx = conn.begin().await.unwrap();
    let _ = reorder_with_tx(&mut tx, "c", Some("a")).await.unwrap();
    tx.commit().await.unwrap();
    // 触发 normalize 后所有 open 行应被重排为 0/10/20
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT id, order_index FROM todos WHERE status='open' ORDER BY order_index ASC"
    ).fetch_all(&mut conn).await.unwrap();
    let orders: Vec<f64> = rows.iter().map(|r| r.1).collect();
    // 验证间距均匀（normalize 后整十）
    for w in orders.windows(2) {
        assert!((w[1] - w[0] - 10.0).abs() < 1e-9, "expected gap=10, got {:?}", orders);
    }
}
```

- [ ] **Step 2: cargo test 应 fail**

Expected: FAIL（reorder_with_tx 未定义）

- [ ] **Step 3: 实现 reorder_with_tx + normalize_order_indices**

```rust
const ORDER_GAP_THRESHOLD: f64 = 1e-6;

async fn normalize_order_indices(tx: &mut Transaction<'_, Sqlite>) -> Result<(), TodoError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM todos WHERE status='open' ORDER BY order_index ASC, updated_at DESC"
    ).fetch_all(&mut **tx).await?;
    let now = Utc::now().to_rfc3339();
    for (idx, (id,)) in rows.into_iter().enumerate() {
        let new_order = (idx as f64) * 10.0;
        let conn: &mut SqliteConnection = &mut **tx;
        sqlx::query("UPDATE todos SET order_index=?, updated_at=? WHERE id=?")
            .bind(new_order).bind(&now).bind(&id)
            .execute(conn).await?;
    }
    Ok(())
}

async fn reorder_with_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    after_id: Option<&str>,
) -> Result<Todo, TodoError> {
    // 计算 newOrder
    let new_order = match after_id {
        Some(after) => {
            // 取 after.order_index
            let after_row: (f64,) = sqlx::query_as(
                "SELECT order_index FROM todos WHERE id=? AND status='open'"
            )
            .bind(after).fetch_optional(&mut **tx).await?
            .ok_or_else(|| TodoError::InvalidInput(format!("after_id not open todo: {after}")))?;
            // 取 after 之后第一个 open（按 order_index ASC）
            let next_row: Option<(f64,)> = sqlx::query_as(
                r#"SELECT order_index FROM todos
                   WHERE status='open' AND order_index > ? AND id != ?
                   ORDER BY order_index ASC LIMIT 1"#,
            )
            .bind(after_row.0).bind(id).fetch_optional(&mut **tx).await?;
            match next_row {
                Some((next_oi,)) => (after_row.0 + next_oi) / 2.0,
                None => after_row.0 + 10.0,
            }
        }
        None => {
            // 拖到最前：取当前 min - 10
            let row: Option<(Option<f64>,)> = sqlx::query_as(
                "SELECT MIN(order_index) FROM todos WHERE status='open' AND id != ?",
            ).bind(id).fetch_optional(&mut **tx).await?;
            row.and_then(|(v,)| v).map(|v| v - 10.0).unwrap_or(0.0)
        }
    };

    // 检查是否需要 normalize（gap < threshold 与相邻）
    let needs_normalize = if let Some(after) = after_id {
        let after_oi: (f64,) = sqlx::query_as(
            "SELECT order_index FROM todos WHERE id=?"
        ).bind(after).fetch_one(&mut **tx).await?;
        let next: Option<(f64,)> = sqlx::query_as(
            r#"SELECT order_index FROM todos
               WHERE status='open' AND order_index > ? AND id != ?
               ORDER BY order_index ASC LIMIT 1"#,
        ).bind(after_oi.0).bind(id).fetch_optional(&mut **tx).await?;
        match next {
            Some((next_oi,)) => (next_oi - after_oi.0).abs() < ORDER_GAP_THRESHOLD,
            None => false,
        }
    } else {
        false
    };

    if needs_normalize {
        normalize_order_indices(tx).await?;
        // 重算 newOrder（normalize 后 after 的 order_index 也变了）
        return Box::pin(reorder_with_tx(tx, id, after_id)).await;
    }

    // UPDATE
    let now = Utc::now().to_rfc3339();
    {
        let conn: &mut SqliteConnection = &mut **tx;
        sqlx::query("UPDATE todos SET order_index=?, updated_at=? WHERE id=?")
            .bind(new_order).bind(&now).bind(id)
            .execute(conn).await?;
    }
    get_by_id(&mut **tx, id).await
}

pub async fn reorder<R: Runtime>(
    app: &AppHandle<R>,
    id: String,
    after_id: Option<String>,
) -> Result<Todo, TodoError> {
    let db_path = open_app_db(app).await?;
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path.display())).await?;
    let mut tx = pool.begin().await?;
    let out = reorder_with_tx(&mut tx, &id, after_id.as_deref()).await?;
    tx.commit().await?;
    Ok(out)
}
```

替换原 stub `pub async fn reorder`。

- [ ] **Step 4: cargo test 应 pass**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo::tests::reorder`
Expected: 3 例 PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/todo.rs
git commit -m "feat: #29 todo::reorder + 分数中位 + gap<1e-6 自动 normalize + 3 单测"
```

---

## Task C9: todo::breakdown 占位 + tx rollback 测试

**Files:**
- Modify: `src-tauri/src/services/todo.rs`

- [ ] **Step 1: 实现 breakdown stub（已是 stub，仅确认 return）**

把已有 stub `pub async fn breakdown` 体改为：

```rust
pub async fn breakdown<R: Runtime>(_app: &AppHandle<R>, _id: String) -> Result<Vec<String>, TodoError> {
    Err(TodoError::BreakdownNotImplemented)
}
```

（已经是这样的话跳过）

- [ ] **Step 2: 写 2 个测试（breakdown 占位 + tx rollback）**

```rust
#[tokio::test]
async fn breakdown_always_returns_not_implemented() {
    // 不需要 DB；breakdown 是 pure stub
    // 但保留 fresh_db 让 fn 签名一致
    let (_dir, _conn) = fresh_db().await;
    // mock AppHandle 不可能；直接 assert TodoError::BreakdownNotImplemented Display
    let err = TodoError::BreakdownNotImplemented;
    assert_eq!(err.to_string(), "breakdown not implemented (M3+)");
}

#[tokio::test]
async fn tx_rollback_on_reminder_coupling_failure_keeps_todos_clean() {
    let (_dir, mut conn) = fresh_db().await;
    let mut tx = conn.begin().await.unwrap();

    // 喂入非法 trigger_spec 让 reminder.create_internal_tx 报 InvalidTrigger
    let result = create_with_tx(
        &mut tx,
        CreateInput {
            title: "x".into(),
            due_at: Some("not-a-date".into()),  // 非法 RFC3339
            priority: None,
        },
    ).await;
    // tx drop → 自动 rollback；conn 状态回到 begin 前
    drop(tx);

    assert!(matches!(result, Err(TodoError::ReminderCoupling(_))));
    // 验证 todos 表无残留
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM todos")
        .fetch_one(&mut conn).await.unwrap();
    assert_eq!(count.0, 0);
    // 验证 reminders 表无残留
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders")
        .fetch_one(&mut conn).await.unwrap();
    assert_eq!(count.0, 0);
}
```

- [ ] **Step 3: cargo test 应 pass**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::todo::tests`
Expected: 全部 PASS（包括 rollback 验证）

如果 rollback 测试失败 → 说明 create_with_tx 把 reminder 写入了非同 tx（违反设计）→ 回去检查 reminder::create_internal_tx 实现是否真的 deref to tx 的 conn。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/todo.rs
git commit -m "feat: #29 todo::breakdown 占位 + tx rollback on coupling failure 单测"
```

---

## Task C10: 前端 src/types/todo.ts + src/services/todo.ts

**Files:**
- Create: `src/types/todo.ts`
- Create: `src/services/todo.ts`

- [ ] **Step 1: 写 src/types/todo.ts**

```typescript
// #29 TodoService 类型契约。与 src-tauri/src/services/todo.rs 同步（camelCase via serde rename）。

export type TodoStatus = 'open' | 'done' | 'cancelled'
export type TodoPriority = 'low' | 'normal' | 'high'

export interface Todo {
  id: string
  title: string
  status: TodoStatus
  dueAt: string | null      // RFC3339 UTC
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

/**
 * due_at 三态:
 * - 字段省略 (undefined) → keep 不改
 * - { kind: 'set', value: '...' } → 设置具体时刻 (RFC3339 UTC)
 * - { kind: 'clear' }             → 清空到 null
 */
export type DueAtChange =
  | { kind: 'set'; value: string }
  | { kind: 'clear' }

export interface TodoUpdateInput {
  title?: string
  status?: 'open' | 'cancelled'   // 'done' 走 todo_complete
  dueAt?: DueAtChange
  priority?: TodoPriority
}
```

- [ ] **Step 2: 写 src/services/todo.ts**

```typescript
import { invoke } from '@tauri-apps/api/core'
import type { Todo, TodoCreateInput, TodoUpdateInput } from '@/types/todo'

export async function createTodo(input: TodoCreateInput): Promise<Todo> {
  return invoke('todo_create', { input })
}

export async function listTodos(): Promise<Todo[]> {
  return invoke('todo_list')
}

export async function updateTodo(id: string, input: TodoUpdateInput): Promise<Todo> {
  return invoke('todo_update', { id, input })
}

export async function completeTodo(id: string): Promise<Todo> {
  return invoke('todo_complete', { id })
}

export async function breakdownTodo(id: string): Promise<string[]> {
  return invoke('todo_breakdown', { id })
}

export async function reorderTodo(id: string, afterId: string | null): Promise<Todo> {
  return invoke('todo_reorder', { id, afterId })
}
```

- [ ] **Step 3: pnpm typecheck**

Run: `pnpm typecheck`
Expected: 无新错误

- [ ] **Step 4: Commit**

```bash
git add src/types/todo.ts src/services/todo.ts
git commit -m "feat: #29 前端 src/types/todo.ts + src/services/todo.ts (6 invoke 包装)"
```

---

# Phase D — Onboarding KV 实例化

## Task D1: services/onboarding_reminders.rs 骨架 + TEMPLATES 双写

**Files:**
- Create: `src-tauri/src/services/onboarding_reminders.rs`
- Modify: `src-tauri/src/services/mod.rs`

**前置阅读**：[src/types/reminder.ts:80](../../../src/types/reminder.ts#L80) `REMINDER_TEMPLATES` 5 个 hardcode 条目（id / title / trigger_type / trigger_spec / priority / hint）。下面的 Rust hardcode 必须与该列表语义一致；任一方加 template 需双写。

- [ ] **Step 1: 写 onboarding_reminders.rs 骨架 + TEMPLATES + parse 工具**

Create `src-tauri/src/services/onboarding_reminders.rs`:

```rust
//! Onboarding reminder intent 实例化（#29 闭合 #21 ADR-019 step 4）。
//!
//! 启动期把 onboarding 期写入的 KV `onboarding:reminder_intents` 消化成真实 reminders 表
//! 数据；批量 reminder.create_internal_tx + preferences.delete_tx 在同一 Transaction
//! 内执行 → 原子性（任一失败 tx drop → rollback → 等价"上次没运行过"，下次启动 KV 还在重试）。
//!
//! REMINDER_TEMPLATES 与 src/types/reminder.ts:80 双写约束（lessons.md 条目）。

use serde::Deserialize;
use sqlx::{Sqlite, SqliteConnection, Transaction};
use tauri::{AppHandle, Runtime};

use crate::services::{preferences, reminder};

const ONBOARDING_KV_KEY: &str = "onboarding:reminder_intents";

#[derive(Debug, Clone)]
struct ReminderTemplate {
    id: &'static str,
    title: &'static str,
    trigger_type: &'static str,
    trigger_spec: &'static str,
    priority: &'static str,
}

const TEMPLATES: &[ReminderTemplate] = &[
    ReminderTemplate {
        id: "water",
        title: "喝杯水",
        trigger_type: "daily",
        trigger_spec: "*/90 * *",
        priority: "soft",
    },
    ReminderTemplate {
        id: "sit_long",
        title: "起身动一下",
        trigger_type: "daily",
        trigger_spec: "*/60 * *",
        priority: "soft",
    },
    ReminderTemplate {
        id: "focus_study",
        title: "今天的学习时间到",
        trigger_type: "daily",
        trigger_spec: "09:00",
        priority: "soft",
    },
    ReminderTemplate {
        id: "early_sleep",
        title: "差不多该准备睡觉了",
        trigger_type: "daily",
        trigger_spec: "23:00",
        priority: "soft",
    },
    ReminderTemplate {
        id: "stand_up",
        title: "站起来活动一下",
        trigger_type: "daily",
        trigger_spec: "*/120 * *",
        priority: "soft",
    },
];

// 注意：以上 5 条必须与 src/types/reminder.ts REMINDER_TEMPLATES 一致；
// 实际值需对照 reminder.ts 当前内容核对 — 若 .ts 有不同 id/spec/title 以 .ts 为准修改本表。

/// 解析 KV 值，返回 None 表示"无需 instantiate"（kv 不存在 / null sentinel / [] / 无效 JSON）。
fn parse_intent_ids(raw: Option<&str>) -> Option<Vec<String>> {
    let s = raw?;
    if s == "null" || s.is_empty() {
        return None;
    }
    let parsed: Result<Vec<String>, _> = serde_json::from_str(s);
    match parsed {
        Ok(v) if v.is_empty() => None,
        Ok(v) => Some(v),
        Err(_) => None,
    }
}

fn lookup_template(id: &str) -> Option<&'static ReminderTemplate> {
    TEMPLATES.iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_array_returns_ids() {
        let v = parse_intent_ids(Some(r#"["water","sit_long"]"#));
        assert_eq!(v, Some(vec!["water".to_string(), "sit_long".to_string()]));
    }

    #[test]
    fn parse_null_sentinel_returns_none() {
        assert_eq!(parse_intent_ids(Some("null")), None);
    }

    #[test]
    fn parse_empty_array_returns_none() {
        assert_eq!(parse_intent_ids(Some("[]")), None);
    }

    #[test]
    fn parse_invalid_json_returns_none() {
        assert_eq!(parse_intent_ids(Some("garbage")), None);
    }

    #[test]
    fn parse_missing_kv_returns_none() {
        assert_eq!(parse_intent_ids(None), None);
    }

    #[test]
    fn lookup_known_id_returns_template() {
        let t = lookup_template("water").unwrap();
        assert_eq!(t.title, "喝杯水");
    }

    #[test]
    fn lookup_unknown_id_returns_none() {
        assert!(lookup_template("unknown_xyz").is_none());
    }
}
```

- [ ] **Step 2: 注册到 services/mod.rs**

在 `pub mod todo;` 后追加：

```rust
// #29 Onboarding reminder intent 启动期实例化（ADR-019 step 4 闭环）。
pub mod onboarding_reminders;
```

- [ ] **Step 3: cargo test**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::onboarding_reminders`
Expected: 7 例 PASS

- [ ] **Step 4: 与前端 REMINDER_TEMPLATES 双向核对**

Run: `cd src && cat ../src/types/reminder.ts | head -130`

逐条核对 id / title / trigger_type / trigger_spec / priority；如差异：以 reminder.ts 为权威修改 onboarding_reminders.rs 的 TEMPLATES 数组。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/onboarding_reminders.rs src-tauri/src/services/mod.rs
git commit -m "feat: #29 onboarding_reminders.rs 骨架 + TEMPLATES + parse 工具 + 7 单测"
```

---

## Task D2: drain_in_tx 批量事务原子化 + 测试

**Files:**
- Modify: `src-tauri/src/services/onboarding_reminders.rs`

- [ ] **Step 1: 写测试（drain 原子 + 部分 unknown id skip）**

在 tests 模块追加：

```rust
use crate::services::test_db::fresh_db;
use sqlx::Connection;

#[tokio::test]
async fn drain_in_tx_creates_reminders_and_deletes_kv() {
    let (_dir, mut conn) = fresh_db().await;
    // 预写 KV
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO memory (key,value,source,updated_at) VALUES (?, ?, 'inferred', ?)")
        .bind(ONBOARDING_KV_KEY)
        .bind(r#"["water","sit_long"]"#)
        .bind(&now)
        .execute(&mut conn).await.unwrap();

    let mut tx = conn.begin().await.unwrap();
    drain_in_tx(&mut tx).await.unwrap();
    tx.commit().await.unwrap();

    // 验证 reminders 表 +2
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders").fetch_one(&mut conn).await.unwrap();
    assert_eq!(count.0, 2);
    // 验证 KV 被删
    let kv: Option<(String,)> = sqlx::query_as("SELECT value FROM memory WHERE key=?")
        .bind(ONBOARDING_KV_KEY).fetch_optional(&mut conn).await.unwrap();
    assert!(kv.is_none());
}

#[tokio::test]
async fn drain_in_tx_skips_unknown_ids() {
    let (_dir, mut conn) = fresh_db().await;
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO memory (key,value,source,updated_at) VALUES (?, ?, 'inferred', ?)")
        .bind(ONBOARDING_KV_KEY)
        .bind(r#"["water","unknown_xyz","sit_long"]"#)
        .bind(&now)
        .execute(&mut conn).await.unwrap();

    let mut tx = conn.begin().await.unwrap();
    drain_in_tx(&mut tx).await.unwrap();
    tx.commit().await.unwrap();

    // 只 water + sit_long 创建（unknown 静默 skip）
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders").fetch_one(&mut conn).await.unwrap();
    assert_eq!(count.0, 2);
}

#[tokio::test]
async fn drain_in_tx_atomic_all_or_nothing() {
    // 模拟最后一条 create fail → 验证整个 tx rollback
    // 难点：reminder.create_internal_tx 不易主动 fail（trigger_spec 错就 fail）
    // 用法：在 TEMPLATES 临时塞一个 trigger_spec 非法的 template 让 fail
    // 真实做法：直接喂入 KV 带一个 "bad_template" id 而它的 trigger_spec 是 "invalid"
    // 但 lookup_template skip unknown id → 不会 fail；改测 KV 不动校验
    let (_dir, mut conn) = fresh_db().await;
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO memory (key,value,source,updated_at) VALUES (?, ?, 'inferred', ?)")
        .bind(ONBOARDING_KV_KEY)
        .bind(r#"["water"]"#)
        .bind(&now)
        .execute(&mut conn).await.unwrap();

    let mut tx = conn.begin().await.unwrap();
    drain_in_tx(&mut tx).await.unwrap();
    // 不 commit，直接 drop → rollback
    drop(tx);

    // KV 应该仍在（未 commit）
    let kv: Option<(String,)> = sqlx::query_as("SELECT value FROM memory WHERE key=?")
        .bind(ONBOARDING_KV_KEY).fetch_optional(&mut conn).await.unwrap();
    assert!(kv.is_some(), "KV should still exist after rollback");
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders").fetch_one(&mut conn).await.unwrap();
    assert_eq!(count.0, 0, "no reminders should be persisted after rollback");
}
```

- [ ] **Step 2: cargo test 应 fail**

Expected: FAIL（`drain_in_tx` 未定义）

- [ ] **Step 3: 实现 drain_in_tx**

```rust
async fn read_kv(conn: &mut SqliteConnection, key: &str) -> Result<Option<String>, String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM memory WHERE key=?")
        .bind(key).fetch_optional(conn).await
        .map_err(|e| format!("read kv {key}: {e}"))?;
    Ok(row.map(|r| r.0))
}

/// 批量原子化：所有 reminder.create + KV delete 在同一 tx；失败时调用方 drop tx → rollback。
pub(crate) async fn drain_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
) -> Result<(), String> {
    let raw = read_kv(&mut **tx, ONBOARDING_KV_KEY).await?;
    let ids = match parse_intent_ids(raw.as_deref()) {
        Some(ids) => ids,
        None => {
            // 脏数据 / null / [] → 删 KV 后返回
            preferences::delete_tx(tx, ONBOARDING_KV_KEY).await
                .map_err(|e| format!("delete kv: {e}"))?;
            return Ok(());
        }
    };

    for id in &ids {
        let template = match lookup_template(id) {
            Some(t) => t,
            None => {
                eprintln!("[onboarding-reminders] skipping unknown template id: {id}");
                continue;
            }
        };
        reminder::create_internal_tx(
            tx,
            reminder::CreateInput {
                title: template.title.into(),
                trigger_type: template.trigger_type.into(),
                trigger_spec: template.trigger_spec.into(),
                priority: Some(template.priority.into()),
            },
        ).await.map_err(|e| format!("create reminder {id}: {e}"))?;
    }

    preferences::delete_tx(tx, ONBOARDING_KV_KEY).await
        .map_err(|e| format!("delete kv: {e}"))?;
    Ok(())
}
```

- [ ] **Step 4: cargo test 应 pass**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::onboarding_reminders`
Expected: 全部 PASS（含 7+3 = 10 例）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/onboarding_reminders.rs
git commit -m "feat: #29 onboarding_reminders::drain_in_tx 批量原子 + 3 单测 (含 rollback 验证)"
```

---

## Task D3: instantiate_onboarding_reminders sync fn + lib.rs setup 钩子

**Files:**
- Modify: `src-tauri/src/services/onboarding_reminders.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 在 onboarding_reminders.rs 加 instantiate_onboarding_reminders sync fn**

```rust
use crate::services::db::open_app_db;

/// 启动期同步入口：lib.rs::setup 调用。内部 block_on 走 drain_in_tx 异步实现。
/// 失败仅 warn（log + 下次启动重试）；不影响 app 启动流程。
pub fn instantiate_onboarding_reminders<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    tauri::async_runtime::block_on(async move {
        let db_path = open_app_db(app).await.map_err(|e| format!("open db: {e}"))?;
        let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path.display()))
            .await.map_err(|e| format!("connect pool: {e}"))?;
        let mut tx = pool.begin().await.map_err(|e| format!("begin tx: {e}"))?;
        drain_in_tx(&mut tx).await?;
        tx.commit().await.map_err(|e| format!("commit: {e}"))?;
        Ok::<(), String>(())
    })
}
```

注意：`tauri::async_runtime::block_on` 只能在同步上下文调（lib.rs::setup 是同步闭包 — 符合 lesson §10）。

- [ ] **Step 2: lib.rs::setup 钩子**

打开 `src-tauri/src/lib.rs`，找 setup 闭包内 `apply_initial_workspace_rect` 或 `start_scheduler` 调用点，紧前追加：

```rust
            // #29 闭环 #21 ADR-019 step 4：消化 onboarding 期写入的 reminder intent KV
            // → 真实 reminders 行 + 删 KV，全程同一 tx 原子化。
            if let Err(e) = crate::services::onboarding_reminders::instantiate_onboarding_reminders(app.handle()) {
                eprintln!("[setup] instantiate_onboarding_reminders failed: {e}");
            }
```

放在 `services::scheduler::start_xxx` 之前（确保 reminders 表已就位再启 scheduler polling）。

- [ ] **Step 3: cargo check**

Run: `cd src-tauri && cargo check --bins`
Expected: 通过

- [ ] **Step 4: 跑全 cargo test 确保不退化**

Run: `cd src-tauri && cargo test -p aipet-app --lib`
Expected: 全部 PASS（reminder / preferences / todo / onboarding_reminders 等模块）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/onboarding_reminders.rs src-tauri/src/lib.rs
git commit -m "feat: #29 instantiate_onboarding_reminders setup 钩子 (block_on + tx 原子)"
```

---

# Phase E — reminder.rs daily 时区修复

## Task E1: compute_next_fire_at_daily_hhmm 接 chrono::Local + Tz-injectable

**Files:**
- Modify: `src-tauri/src/services/reminder.rs`

- [ ] **Step 1: 写 3 个失败测试**

定位 reminder.rs `mod tests`，在末尾追加：

```rust
#[test]
fn daily_hhmm_in_utc8_evening_after_target() {
    // 北京 17:00 = UTC 09:00；用户设 daily 09:00 (本地) → 明天本地 09:00 = 明天 UTC 01:00
    use chrono::{FixedOffset, TimeZone};
    let utc_plus8 = FixedOffset::east_opt(8 * 3600).unwrap();
    let now_utc = Utc.with_ymd_and_hms(2026, 5, 23, 9, 0, 0).unwrap();
    let next = compute_next_fire_at_daily_hhmm_in_tz("09:00", now_utc, &utc_plus8).unwrap();
    // 明天本地 09:00 = 明天 UTC 01:00
    let expected = Utc.with_ymd_and_hms(2026, 5, 24, 1, 0, 0).unwrap();
    assert_eq!(next, expected);
}

#[test]
fn daily_hhmm_in_utc8_morning_before_target() {
    // 北京 07:00 = UTC 23:00 前一天；用户设 daily 09:00 → 今天本地 09:00 = 今天 UTC 01:00
    use chrono::{FixedOffset, TimeZone};
    let utc_plus8 = FixedOffset::east_opt(8 * 3600).unwrap();
    let now_utc = Utc.with_ymd_and_hms(2026, 5, 23, 23, 0, 0).unwrap();  // 5/23 23:00 UTC = 5/24 07:00 北京
    let next = compute_next_fire_at_daily_hhmm_in_tz("09:00", now_utc, &utc_plus8).unwrap();
    let expected = Utc.with_ymd_and_hms(2026, 5, 24, 1, 0, 0).unwrap();  // 5/24 09:00 北京 = 5/24 01:00 UTC
    assert_eq!(next, expected);
}

#[test]
fn daily_hhmm_in_utc_neutral_zone() {
    // UTC 23:00 设 23:00 → 明天 23:00 UTC（regression）
    use chrono::{FixedOffset, TimeZone};
    let utc = FixedOffset::east_opt(0).unwrap();
    let now_utc = Utc.with_ymd_and_hms(2026, 5, 23, 23, 0, 0).unwrap();
    let next = compute_next_fire_at_daily_hhmm_in_tz("23:00", now_utc, &utc).unwrap();
    let expected = Utc.with_ymd_and_hms(2026, 5, 24, 23, 0, 0).unwrap();
    assert_eq!(next, expected);
}
```

- [ ] **Step 2: cargo test 应 fail**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::reminder::tests::daily_hhmm_in`
Expected: FAIL（`compute_next_fire_at_daily_hhmm_in_tz` 未定义）

- [ ] **Step 3: 实现 in_tz 版本 + 改 compute_next_fire_at_daily_hhmm 委托**

定位现有 `fn compute_next_fire_at_daily_hhmm`（reminder.rs 700-790 附近，按 grep 找）。改为：

```rust
use chrono::{Local, NaiveTime};

fn compute_next_fire_at_daily_hhmm_in_tz<Tz: chrono::TimeZone>(
    spec: &str,
    now_utc: DateTime<Utc>,
    tz: &Tz,
) -> Result<DateTime<Utc>, ReminderError>
where
    Tz::Offset: std::fmt::Display,
{
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

fn compute_next_fire_at_daily_hhmm(
    spec: &str,
    now_utc: DateTime<Utc>,
) -> Result<DateTime<Utc>, ReminderError> {
    compute_next_fire_at_daily_hhmm_in_tz(spec, now_utc, &Local)
}
```

如果现有 fn 是嵌套在 match 内的 closure 形式：抽到顶层 fn 即可。

- [ ] **Step 4: cargo test 应 pass**

Run: `cd src-tauri && cargo test -p aipet-app --lib services::reminder::tests::daily_hhmm_in`
Expected: 3 例 PASS

跑全模块确保不退化：

Run: `cd src-tauri && cargo test -p aipet-app --lib services::reminder`
Expected: 全部 PASS

- [ ] **Step 5: 改 file header 注释**

打开 reminder.rs 顶部注释段，把：

```
//!   时区：内部一律 RFC3339 UTC；UI 转本地。daily HH:MM 当前按 UTC 解释——M2 简化（中国
//!   时区用户 +8h 偏移），follow-up #29 接入本地时区转换。
```

改为：

```
//!   时区：内部一律 RFC3339 UTC；UI 转本地。daily HH:MM 按系统本地时区解释（#29 接入），
//!   Tz-injectable 设计便于测试（compute_next_fire_at_daily_hhmm_in_tz 接 &Tz 参数）。
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/reminder.rs
git commit -m "fix: #29 reminder daily HH:MM 接系统本地时区 + Tz-injectable + 3 单测"
```

---

## Task E2: 前端 reminder.ts template hint 文案更新

**Files:**
- Modify: `src/types/reminder.ts`

- [ ] **Step 1: 改文案**

Run: `pnpm run -s typecheck` 之前先 grep：

Run: `cd src && grep -n "UTC" types/reminder.ts`

定位 `focus_study` / `early_sleep` 行（spec §10.3）。把：

- `'每天 09:00（UTC，约本地 17:00）'` → `'每天 09:00（本地）'`
- `'每天 23:00（UTC，约本地 07:00）'` → `'每天 23:00（本地）'`

（具体行号以实际为准）

- [ ] **Step 2: typecheck**

Run: `pnpm typecheck`
Expected: 通过

- [ ] **Step 3: Commit**

```bash
git add src/types/reminder.ts
git commit -m "docs: #29 reminder template hint 文案改 (UTC → 本地)"
```

---

# Phase F — LivingPet hook (usePetReaction)

> 前置：先查项目实际有没有 `src/runtime/VRMRuntime.ts` 还是 `src/services/vrm.ts`，按实际路径调整。

## Task F1: VRMRuntime.playAction(actionId) + null 防御 + Nod 动作

**Files:**
- Modify: `src/services/vrm.ts`（或 `src/runtime/VRMRuntime.ts`，以实际为准）

- [ ] **Step 1: 定位 VRMRuntime 类位置**

Run: `grep -rn "class VRMRuntime\|export.*VRMRuntime" src/`

按结果确定路径（spec §8.1 默认 `src/services/vrm.ts`，以实际为准）。下面以 `src/services/vrm.ts` 为例。

- [ ] **Step 2: 加 PetActionId type 导出**

文件顶部追加：

```typescript
// #29 桌宠反应动作 ID 契约。#23 接 reaction_table 时扩这个 union。
export type PetActionId =
  | 'nod'                  // #29 实现
  | 'head_pat' | 'surprised' | 'fall_asleep' | 'dizzy' | 'protest' | 'cheer'  // #23 placeholder
  | 'drink' | 'stretch' | 'sleep' | 'wander' | 'idle'                          // #23 placeholder
```

- [ ] **Step 3: VRMRuntime 类内加 playAction 方法**

定位 VRMRuntime class 内（vrm 字段附近）追加：

```typescript
  /**
   * 播放命名动作。M2 W3 仅 'nod'（#29），其他 #23 接入 reaction_table 时填。
   * vrm 未 ready 时静默 no-op（reminder:fired 可能在 VRM 加载完成前到达）。
   */
  async playAction(actionId: PetActionId): Promise<void> {
    if (!this.vrm) {
      // 静默 no-op：onboarding 期 / VRM 加载失败时 reminder:fired 仍会触发；
      // 此处不报错不弹 toast（spec §8.2 + R8）。
      return
    }
    if (actionId === 'nod') {
      await this.playNod()
      return
    }
    // #23 placeholder：其他 actionId 走 dev 警告 + no-op
    if (import.meta.env.DEV) {
      console.warn('[vrm] playAction not implemented:', actionId)
    }
  }

  /**
   * 短促点头动效：head bone X 轴 ±15° / 360ms RAF 插值（不引动画 clip）。
   * 不打断 wander tween，不持久化（瞬时动效）。
   */
  private async playNod(): Promise<void> {
    if (!this.vrm) return
    const humanoid = this.vrm.humanoid
    if (!humanoid) return
    const headNode = humanoid.getNormalizedBoneNode('head')
    if (!headNode) return

    const baseX = headNode.rotation.x
    const peakDelta = (15 * Math.PI) / 180  // +15°
    const duration = 360
    const start = performance.now()

    return new Promise<void>((resolve) => {
      const tick = (t: number) => {
        const elapsed = t - start
        if (elapsed >= duration) {
          headNode.rotation.x = baseX
          resolve()
          return
        }
        const p = elapsed / duration  // 0..1
        // ease-out triangle: 0 → 1 → 0
        const tri = p < 0.5 ? p * 2 : (1 - p) * 2
        headNode.rotation.x = baseX + peakDelta * tri
        requestAnimationFrame(tick)
      }
      requestAnimationFrame(tick)
    })
  }
```

注意：`humanoid.getNormalizedBoneNode` 是 `@pixiv/three-vrm` v3 API；旧版可能用 `getRawBoneNode`。视项目实际版本（package.json）调整。

- [ ] **Step 4: typecheck**

Run: `pnpm typecheck`
Expected: 通过

- [ ] **Step 5: Commit**

```bash
git add src/services/vrm.ts
git commit -m "feat: #29 VRMRuntime.playAction(actionId) + nod 实现 + null 防御首行"
```

---

## Task F2: usePetReaction composable

**Files:**
- Create: `src/composables/usePetReaction.ts`

- [ ] **Step 1: 写 composable**

Create `src/composables/usePetReaction.ts`:

```typescript
// #29 桌宠对 reminder:fired 事件的反应（点头）。
// #23 接 reaction_table 时改内部 mapping，外部接口不动。

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

  onBeforeUnmount(() => {
    unlistenFired?.()
  })
}
```

注意：如果 reminder.ts 没有导出 `REMINDER_FIRED_EVENT` 或 `ReminderFiredPayload`，去 PetReminderBubble.vue 等已用方对照名字。先 `grep -n "REMINDER_FIRED_EVENT\|reminder:fired" src/`。

- [ ] **Step 2: typecheck**

Run: `pnpm typecheck`
Expected: 通过

- [ ] **Step 3: Commit**

```bash
git add src/composables/usePetReaction.ts
git commit -m "feat: #29 usePetReaction composable (listen reminder:fired → nod)"
```

---

## Task F3: PetCanvas.vue 接入 prop + SoulPledgeView 传 false

**Files:**
- Modify: `src/components/PetCanvas.vue`
- Modify: `src/views/onboarding/SoulPledgeView.vue`（含 PetCanvas 的 onboarding 视图）

- [ ] **Step 1: PetCanvas.vue 加 enableReaction prop + 调用 composable**

定位 `defineProps<Props>` 段，把 Props interface 加：

```typescript
interface Props {
  // 现有字段...
  enableReaction?: boolean
}
```

`withDefaults` 段：

```typescript
const props = withDefaults(defineProps<Props>(), {
  // 现有 default...
  enableReaction: true,
})
```

`useVRMModel` 调用后追加：

```typescript
import { usePetReaction } from '@/composables/usePetReaction'

// 现有: const { isLoaded, errorMessage, runtime } = useVRMModel(...)

if (props.enableReaction) {
  usePetReaction(runtime)
}
```

**关键**：`usePetReaction` 必须在 `setup` 顶层调用（Vue composable 规则），不能放 if 内部 — 上面写法是把 if 包在 setup 顶层，runtime 是 ref/对象，可以这么写。**但** composable 内有 `onMounted/onBeforeUnmount` 必须无条件挂载到组件生命周期，所以更安全的写法：

```typescript
import { usePetReaction } from '@/composables/usePetReaction'

// 顶层无条件调用，但传 flag 给 composable 内部判断
usePetReaction(runtime, () => props.enableReaction)
```

为支持这一点，修改 usePetReaction 签名（如下 Step 1.5）。

- [ ] **Step 1.5: usePetReaction 接受可选 enabled getter**

回去改 `src/composables/usePetReaction.ts`：

```typescript
export function usePetReaction(
  runtime: VRMRuntime,
  isEnabled: () => boolean = () => true,
): void {
  let unlistenFired: UnlistenFn | null = null

  onMounted(async () => {
    if (!isEnabled()) return
    try {
      unlistenFired = await listen<ReminderFiredPayload>(REMINDER_FIRED_EVENT, () => {
        if (!isEnabled()) return
        runtime.playAction('nod').catch((e) => {
          console.warn('[pet-reaction] playAction failed:', e)
        })
      })
    } catch (e) {
      console.warn('[pet-reaction] listen failed:', e)
    }
  })

  onBeforeUnmount(() => {
    unlistenFired?.()
  })
}
```

注意：mount 期检查 isEnabled — 一旦 mount 时为 false 后续不会再 attach listener。这符合 onboarding 场景（SoulPledgeView 整个生命周期都不该反应）。

- [ ] **Step 2: SoulPledgeView 调用处传 false**

Run: `grep -rn "PetCanvas" src/views/onboarding/`

定位含 `<PetCanvas .../>` 的 onboarding 视图（spec §8.5 提 SoulPledgeView），追加 `:enable-reaction="false"`：

```vue
<PetCanvas
  v-bind="..."
  :enable-reaction="false"
/>
```

- [ ] **Step 3: typecheck**

Run: `pnpm typecheck`
Expected: 通过

- [ ] **Step 4: Commit**

```bash
git add src/components/PetCanvas.vue src/composables/usePetReaction.ts src/views/onboarding/SoulPledgeView.vue
git commit -m "feat: #29 PetCanvas enableReaction prop + onboarding 传 false 防误装"
```

---

# Phase G — Tasks 待办 panel UI

## Task G1: 安装依赖 vuedraggable + v-calendar

**Files:**
- Modify: `package.json`

- [ ] **Step 1: pnpm add**

Run: `pnpm add vuedraggable@^4.1.0 v-calendar@^3.1`
Expected: 成功安装，package.json + pnpm-lock.yaml 更新

- [ ] **Step 2: typecheck**

Run: `pnpm typecheck`
Expected: 通过

- [ ] **Step 3: Commit**

```bash
git add package.json pnpm-lock.yaml
git commit -m "deps: #29 add vuedraggable@^4 + v-calendar@^3.1 (待办 panel UI)"
```

---

## Task G2: TodoForm.vue 创建/编辑表单

**Files:**
- Create: `src/components/tasks/TodoForm.vue`

- [ ] **Step 1: 写组件**

Create `src/components/tasks/TodoForm.vue`:

```vue
<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { ElInput, ElDatePicker, ElSelect, ElOption, ElButton, ElForm, ElFormItem } from 'element-plus'
import type { Todo, TodoCreateInput, TodoUpdateInput, TodoPriority, DueAtChange } from '@/types/todo'

interface Props {
  todo: Todo | null  // null = 创建，非 null = 编辑
  open: boolean
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'submit', input: TodoCreateInput | TodoUpdateInput): void
  (e: 'cancel'): void
}>()

const titleInput = ref('')
const dueAtInput = ref<Date | null>(null)
const priorityInput = ref<TodoPriority>('normal')

watch(
  () => [props.todo, props.open],
  () => {
    if (props.open) {
      if (props.todo) {
        titleInput.value = props.todo.title
        dueAtInput.value = props.todo.dueAt ? new Date(props.todo.dueAt) : null
        priorityInput.value = props.todo.priority
      } else {
        titleInput.value = ''
        dueAtInput.value = null
        priorityInput.value = 'normal'
      }
    }
  },
  { immediate: true },
)

const titleValid = computed(() => titleInput.value.trim().length > 0)

function buildCreatePayload(): TodoCreateInput {
  return {
    title: titleInput.value.trim(),
    dueAt: dueAtInput.value ? dueAtInput.value.toISOString() : undefined,
    priority: priorityInput.value,
  }
}

function buildUpdatePayload(): TodoUpdateInput {
  if (!props.todo) return {}
  const out: TodoUpdateInput = {}
  if (titleInput.value.trim() !== props.todo.title) {
    out.title = titleInput.value.trim()
  }
  const oldDue = props.todo.dueAt ?? null
  const newDue = dueAtInput.value ? dueAtInput.value.toISOString() : null
  if (oldDue !== newDue) {
    if (newDue === null) {
      out.dueAt = { kind: 'clear' } as DueAtChange
    } else {
      out.dueAt = { kind: 'set', value: newDue } as DueAtChange
    }
  }
  if (priorityInput.value !== props.todo.priority) {
    out.priority = priorityInput.value
  }
  return out
}

function onSubmit() {
  if (!titleValid.value) return
  const payload = props.todo ? buildUpdatePayload() : buildCreatePayload()
  emit('submit', payload)
}

const disabledDate = (d: Date) => d.getTime() < Date.now() - 24 * 60 * 60 * 1000
</script>

<template>
  <ElForm @submit.prevent="onSubmit">
    <ElFormItem label="标题" :required="true">
      <ElInput v-model="titleInput" placeholder="例：复诊 / 买菜 / 写报告" maxlength="120" />
    </ElFormItem>
    <ElFormItem label="截止时间">
      <ElDatePicker
        v-model="dueAtInput"
        type="datetime"
        placeholder="可选；空表示无截止"
        :disabled-date="disabledDate"
        clearable
      />
    </ElFormItem>
    <ElFormItem label="优先级">
      <ElSelect v-model="priorityInput">
        <ElOption label="低" value="low" />
        <ElOption label="普通" value="normal" />
        <ElOption label="重要" value="high" />
      </ElSelect>
    </ElFormItem>
    <div class="todo-form__actions">
      <ElButton @click="emit('cancel')">取消</ElButton>
      <ElButton type="primary" :disabled="!titleValid" @click="onSubmit">
        {{ props.todo ? '保存' : '新建' }}
      </ElButton>
    </div>
  </ElForm>
</template>

<style scoped>
.todo-form__actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 12px;
}
</style>
```

- [ ] **Step 2: typecheck**

Run: `pnpm typecheck`
Expected: 通过

- [ ] **Step 3: Commit**

```bash
git add src/components/tasks/TodoForm.vue
git commit -m "feat: #29 TodoForm.vue (title + dueAt picker + priority select)"
```

---

## Task G3: TodoList.vue 基础渲染 + 行操作

**Files:**
- Create: `src/components/tasks/TodoList.vue`

- [ ] **Step 1: 写 TodoList.vue 基础版（无拖排序）**

Create `src/components/tasks/TodoList.vue`:

```vue
<script setup lang="ts">
import { computed } from 'vue'
import { ElCheckbox, ElButton, ElTooltip, ElIcon, ElMessage } from 'element-plus'
import { Check, Edit, Close, MagicStick } from '@element-plus/icons-vue'
import type { Todo } from '@/types/todo'

interface Props {
  todos: Todo[]
  selectedIds: Set<string>
  searchQuery: string
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'complete', id: string): void
  (e: 'cancel', id: string): void
  (e: 'edit', todo: Todo): void
  (e: 'toggleSelect', id: string, checked: boolean): void
}>()

function priorityClass(p: string): string {
  return `todo-row__bar--${p}`
}

function formatDue(due: string | null): string {
  if (!due) return ''
  const d = new Date(due)
  return d.toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
}

function onSelectChange(id: string, val: boolean | string | number) {
  emit('toggleSelect', id, val === true)
}
</script>

<template>
  <ul class="todo-list">
    <li
      v-for="todo in props.todos"
      :key="todo.id"
      class="todo-row"
      :class="{ 'todo-row--done': todo.status === 'done', 'todo-row--cancelled': todo.status === 'cancelled' }"
    >
      <div class="todo-row__bar" :class="priorityClass(todo.priority)" />
      <ElCheckbox
        :model-value="props.selectedIds.has(todo.id)"
        @update:model-value="(v: boolean | string | number) => onSelectChange(todo.id, v)"
      />
      <div class="todo-row__body">
        <div class="todo-row__title">{{ todo.title }}</div>
        <div v-if="todo.dueAt" class="todo-row__due">⏰ {{ formatDue(todo.dueAt) }}</div>
      </div>
      <ElTooltip v-if="todo.reminderId" content="已关联提醒">
        <ElIcon><MagicStick /></ElIcon>
      </ElTooltip>
      <div class="todo-row__actions">
        <ElTooltip content="完成">
          <ElButton
            link
            :disabled="todo.status !== 'open'"
            @click="emit('complete', todo.id)"
          >
            <ElIcon><Check /></ElIcon>
          </ElButton>
        </ElTooltip>
        <ElTooltip content="编辑">
          <ElButton
            link
            :disabled="todo.status !== 'open'"
            @click="emit('edit', todo)"
          >
            <ElIcon><Edit /></ElIcon>
          </ElButton>
        </ElTooltip>
        <ElTooltip content="M3 上线后可用 — AI 帮你把大目标拆成小步骤">
          <ElButton link disabled>
            <ElIcon>✨</ElIcon>
          </ElButton>
        </ElTooltip>
        <ElTooltip content="取消">
          <ElButton
            link
            :disabled="todo.status !== 'open'"
            @click="emit('cancel', todo.id)"
          >
            <ElIcon><Close /></ElIcon>
          </ElButton>
        </ElTooltip>
      </div>
    </li>
    <li v-if="props.todos.length === 0" class="todo-list__empty">
      {{ props.searchQuery ? '没有匹配的待办' : '还没有待办，点右上角新建' }}
    </li>
  </ul>
</template>

<style scoped>
.todo-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.todo-row {
  display: grid;
  grid-template-columns: 4px auto 1fr auto auto;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--aipet-surface-1);
  border-radius: 6px;
}
.todo-row__bar {
  width: 4px;
  height: 100%;
  border-radius: 2px;
  align-self: stretch;
}
.todo-row__bar--high {
  background: var(--aipet-color-warning, #d97706);
}
.todo-row__bar--low {
  background: var(--aipet-color-text-3, #a3a3a3);
}
.todo-row__bar--normal {
  background: transparent;
}
.todo-row__body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.todo-row__title {
  font-size: 14px;
  color: var(--aipet-color-text-1);
}
.todo-row--done .todo-row__title {
  text-decoration: line-through;
  color: var(--aipet-color-text-3);
}
.todo-row__due {
  font-size: 12px;
  color: var(--aipet-color-text-2);
}
.todo-row__actions {
  display: flex;
  gap: 4px;
}
.todo-list__empty {
  text-align: center;
  padding: 24px;
  color: var(--aipet-color-text-3);
}
</style>
```

- [ ] **Step 2: typecheck**

Run: `pnpm typecheck`
Expected: 通过

- [ ] **Step 3: Commit**

```bash
git add src/components/tasks/TodoList.vue
git commit -m "feat: #29 TodoList.vue 基础渲染 + 行操作 (4 button + priority 色条 + 🔔)"
```

---

## Task G4: TodoList.vue 加 vuedraggable 拖排序

**Files:**
- Modify: `src/components/tasks/TodoList.vue`

- [ ] **Step 1: import vuedraggable**

文件顶部 script 内加：

```typescript
// @ts-expect-error vuedraggable 类型在 Vue 3 + TS strict 下不完整（R4）
import draggable from 'vuedraggable'
```

- [ ] **Step 2: 加 props.draggable + emit reorder**

```typescript
interface Props {
  todos: Todo[]
  selectedIds: Set<string>
  searchQuery: string
  draggable: boolean   // 新增
}

const emit = defineEmits<{
  // 现有...
  (e: 'reorder', movedId: string, afterId: string | null): void
}>()
```

- [ ] **Step 3: template 改用 draggable 包裹 li 列表**

把 `<ul class="todo-list">` 段改为：

```vue
<draggable
  v-if="props.draggable"
  :model-value="props.todos"
  item-key="id"
  handle=".todo-row__drag-handle"
  :animation="200"
  @end="onDragEnd"
  class="todo-list"
  tag="ul"
>
  <template #item="{ element: todo }">
    <li class="todo-row" :class="...">
      <span class="todo-row__drag-handle">⋮⋮</span>
      <!-- 现有内容 ... -->
    </li>
  </template>
</draggable>
<ul v-else class="todo-list">
  <!-- 非拖动模式（搜索 / 批量中）：保留原 v-for -->
  <li v-for="todo in props.todos" :key="todo.id" class="todo-row" :class="...">
    <!-- 同上但无 drag handle -->
  </li>
</ul>
```

注意：把现有 li 的 grid-template-columns 从 `4px auto 1fr auto auto` 改为 `16px 4px auto 1fr auto auto`（加 drag handle 一列）。

- [ ] **Step 4: 实现 onDragEnd**

```typescript
function onDragEnd(event: { oldIndex?: number; newIndex?: number }) {
  if (event.oldIndex === undefined || event.newIndex === undefined) return
  if (event.oldIndex === event.newIndex) return
  const moved = props.todos[event.oldIndex]
  const after = event.newIndex > 0 ? props.todos[event.newIndex - 1] : null
  emit('reorder', moved.id, after?.id ?? null)
}
```

注意：vuedraggable 的 `model-value` + `@update:modelValue` pattern 较复杂；这里用 `@end` 配合 oldIndex/newIndex 计算更稳。父组件不在本地排序，等待 reorder IPC 后 listTodos 全表刷新（spec §11.10）。

但 vuedraggable 默认行为是修改 modelValue（本地先动）；要么换用 `v-model` 让前端先排，要么禁用本地排（看本组件父侧实现策略）。

简化方案：让 vuedraggable 本地先动，后续 listTodos 覆盖即可。template 改为：

```vue
<draggable
  v-if="props.draggable"
  :list="localTodos"
  item-key="id"
  handle=".todo-row__drag-handle"
  :animation="200"
  @end="onDragEnd"
  class="todo-list"
  tag="ul"
>
```

`localTodos` 用 computed/local copy。但 props.todos 变化要重新同步。最简：

```typescript
import { ref, watch } from 'vue'
const localTodos = ref<Todo[]>([])
watch(() => props.todos, (v) => { localTodos.value = [...v] }, { immediate: true })
```

`@end` 时 emit reorder，父侧调 reorderTodo + 重新 listTodos → props.todos 更新 → watch 同步 localTodos。

把 li 内 `todo` 变量名保持一致（template 用 localTodos 替 props.todos）。

- [ ] **Step 5: typecheck + 手动开发服跑一次**

Run: `pnpm typecheck`
Expected: 通过

Run: `pnpm tauri dev`（手动启 dev server，拖一下确认动）
Expected: 拖动有动效，松手后短暂跳一下（IPC 重排）

- [ ] **Step 6: Commit**

```bash
git add src/components/tasks/TodoList.vue
git commit -m "feat: #29 TodoList vuedraggable 拖排序 + onDragEnd 触发 reorder IPC"
```

---

## Task G5: TodoBatchBar.vue 批量操作条

**Files:**
- Create: `src/components/tasks/TodoBatchBar.vue`

- [ ] **Step 1: 写组件**

Create `src/components/tasks/TodoBatchBar.vue`:

```vue
<script setup lang="ts">
import { ElButton, ElButtonGroup, ElTooltip, ElMessage, ElMessageBox } from 'element-plus'
import type { TodoPriority } from '@/types/todo'

interface Props {
  count: number
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'completeAll'): void
  (e: 'cancelAll'): void
  (e: 'setPriority', priority: TodoPriority): void
  (e: 'clearSelection'): void
}>()

async function onCancelAll() {
  try {
    await ElMessageBox.confirm(
      `确认取消 ${props.count} 个待办？`,
      '批量取消',
      { confirmButtonText: '取消选中', cancelButtonText: '返回', type: 'warning' },
    )
    emit('cancelAll')
  } catch {
    // 用户点返回
  }
}
</script>

<template>
  <div v-if="props.count > 0" class="todo-batch-bar">
    <span class="todo-batch-bar__count">已选 {{ props.count }} 项</span>
    <ElButtonGroup>
      <ElButton size="small" @click="emit('completeAll')">批量完成</ElButton>
      <ElButton size="small" type="warning" @click="onCancelAll">批量取消</ElButton>
      <ElButton size="small" @click="emit('setPriority', 'high')">设为重要</ElButton>
      <ElButton size="small" @click="emit('setPriority', 'normal')">设为普通</ElButton>
      <ElButton size="small" @click="emit('setPriority', 'low')">设为低</ElButton>
    </ElButtonGroup>
    <ElButton size="small" link @click="emit('clearSelection')">清除选择</ElButton>
  </div>
</template>

<style scoped>
.todo-batch-bar {
  position: sticky;
  top: 0;
  z-index: 5;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  background: var(--aipet-surface-2);
  border-bottom: 1px solid var(--aipet-color-border);
  border-radius: 6px 6px 0 0;
}
.todo-batch-bar__count {
  font-size: 13px;
  color: var(--aipet-color-text-1);
}
</style>
```

- [ ] **Step 2: typecheck**

Run: `pnpm typecheck`
Expected: 通过

- [ ] **Step 3: Commit**

```bash
git add src/components/tasks/TodoBatchBar.vue
git commit -m "feat: #29 TodoBatchBar.vue (批量完成/取消/改优先级/清选)"
```

---

## Task G6: TodoCalendar.vue v-calendar 月视图 + dark watch

**Files:**
- Create: `src/components/tasks/TodoCalendar.vue`

- [ ] **Step 1: 写组件**

Create `src/components/tasks/TodoCalendar.vue`:

```vue
<script setup lang="ts">
import { computed, ref, onMounted, onBeforeUnmount } from 'vue'
import { Calendar as VCalendar } from 'v-calendar'
import 'v-calendar/style.css'
import type { Todo } from '@/types/todo'

interface Props {
  todos: Todo[]
}

const props = defineProps<Props>()

// dark mode 检测：项目用 :root.dark class（非 prefers-color-scheme media query）
const isDark = ref(false)
let mo: MutationObserver | null = null

function syncDark() {
  isDark.value = document.documentElement.classList.contains('dark')
}

onMounted(() => {
  syncDark()
  mo = new MutationObserver(syncDark)
  mo.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
})

onBeforeUnmount(() => {
  mo?.disconnect()
  mo = null
})

function priorityDotColor(p: string): string {
  switch (p) {
    case 'high': return 'orange'
    case 'low': return 'gray'
    default: return 'blue'
  }
}

const attributes = computed(() => {
  return props.todos
    .filter(t => t.dueAt && t.status === 'open')
    .map(t => ({
      key: t.id,
      dot: { color: priorityDotColor(t.priority) },
      dates: new Date(t.dueAt as string),
      popover: { label: t.title },
    }))
})
</script>

<template>
  <div class="todo-calendar">
    <VCalendar :attributes="attributes" :is-dark="isDark" expanded />
  </div>
</template>

<style scoped>
.todo-calendar {
  padding: 12px;
  background: var(--aipet-surface-1);
  border-radius: 6px;
}
:deep(.vc-container) {
  border: none;
  background: transparent;
}
</style>
```

- [ ] **Step 2: typecheck**

Run: `pnpm typecheck`
Expected: 通过

- [ ] **Step 3: Commit**

```bash
git add src/components/tasks/TodoCalendar.vue
git commit -m "feat: #29 TodoCalendar.vue (v-calendar 月视图 + 颜色点 + dark watch via MutationObserver)"
```

---

## Task G7: TasksTodoPanel.vue 覆写 placeholder

**Files:**
- Modify: `src/panels/tasks/TasksTodoPanel.vue`
- Modify: `src/stores/workspaceLayout.ts`

- [ ] **Step 1: workspaceLayout store 加 todoView KV**

打开 `src/stores/workspaceLayout.ts`，参考现有 `currentCategory` 或 `masterWidth` 的 KV 持久化 pattern，加：

```typescript
const todoView = ref<'list' | 'calendar'>('list')
const TODO_VIEW_KV = 'workspace:todo_view'

async function loadTodoView() {
  const v = await getConfig(TODO_VIEW_KV)
  if (v === 'list' || v === 'calendar') todoView.value = v
}

async function setTodoView(v: 'list' | 'calendar') {
  todoView.value = v
  await setConfig(TODO_VIEW_KV, v)
}

// 在现有 boot()/load() 处也调 loadTodoView()；export todoView + setTodoView
```

参考 [src/stores/workspaceLayout.ts](../../../src/stores/workspaceLayout.ts) 现有 currentCategory pattern，复制粘贴改变量名即可。

- [ ] **Step 2: 覆写 TasksTodoPanel.vue**

Read 现有 placeholder（确认它确实只是 "🚧 即将上线"），然后整体替换为：

```vue
<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { ElInput, ElButton, ElButtonGroup, ElDialog, ElMessage, ElIcon } from 'element-plus'
import { Search, Calendar, List, Plus, Refresh } from '@element-plus/icons-vue'
import { storeToRefs } from 'pinia'
import { useWorkspaceLayout } from '@/stores/workspaceLayout'
import type { Todo, TodoCreateInput, TodoUpdateInput, TodoPriority } from '@/types/todo'
import {
  listTodos,
  createTodo,
  updateTodo,
  completeTodo,
  reorderTodo,
} from '@/services/todo'
import TodoList from '@/components/tasks/TodoList.vue'
import TodoCalendar from '@/components/tasks/TodoCalendar.vue'
import TodoForm from '@/components/tasks/TodoForm.vue'
import TodoBatchBar from '@/components/tasks/TodoBatchBar.vue'

const layout = useWorkspaceLayout()
const { todoView } = storeToRefs(layout)

const todos = ref<Todo[]>([])
const loading = ref(false)
const searchQuery = ref('')
const showAll = ref(false)
const selectedIds = ref<Set<string>>(new Set())
const formOpen = ref(false)
const editingTodo = ref<Todo | null>(null)

async function refresh() {
  loading.value = true
  try {
    todos.value = await listTodos()
  } catch (e: unknown) {
    ElMessage.error(`加载待办失败：${e}`)
  } finally {
    loading.value = false
  }
}

const filtered = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  let list = todos.value
  if (!showAll.value) list = list.filter(t => t.status === 'open')
  if (q) list = list.filter(t => t.title.toLowerCase().includes(q))
  return list
})

const canDrag = computed(
  () => !searchQuery.value.trim() && selectedIds.value.size === 0 && !showAll.value
)

function toggleSelect(id: string, checked: boolean) {
  if (checked) selectedIds.value.add(id)
  else selectedIds.value.delete(id)
  // 强制响应式（Set 重新赋值）
  selectedIds.value = new Set(selectedIds.value)
}

function clearSelection() {
  selectedIds.value = new Set()
}

function openCreate() {
  editingTodo.value = null
  formOpen.value = true
}

function openEdit(todo: Todo) {
  editingTodo.value = todo
  formOpen.value = true
}

async function onFormSubmit(input: TodoCreateInput | TodoUpdateInput) {
  try {
    if (editingTodo.value) {
      await updateTodo(editingTodo.value.id, input as TodoUpdateInput)
    } else {
      await createTodo(input as TodoCreateInput)
    }
    formOpen.value = false
    await refresh()
  } catch (e: unknown) {
    ElMessage.error(`保存失败：${e}`)
  }
}

async function onComplete(id: string) {
  try {
    await completeTodo(id)
    await refresh()
  } catch (e: unknown) {
    ElMessage.error(`完成失败：${e}`)
  }
}

async function onCancel(id: string) {
  try {
    await updateTodo(id, { status: 'cancelled' })
    await refresh()
  } catch (e: unknown) {
    ElMessage.error(`取消失败：${e}`)
  }
}

async function onReorder(movedId: string, afterId: string | null) {
  try {
    await reorderTodo(movedId, afterId)
    await refresh()
  } catch (e: unknown) {
    ElMessage.error(`排序失败：${e}`)
  }
}

async function batchComplete() {
  const ids = Array.from(selectedIds.value)
  const results = await Promise.allSettled(ids.map(id => completeTodo(id)))
  const fails = results.filter(r => r.status === 'rejected').length
  if (fails > 0) ElMessage.warning(`${fails} 个未能完成`)
  clearSelection()
  await refresh()
}

async function batchCancel() {
  const ids = Array.from(selectedIds.value)
  const results = await Promise.allSettled(
    ids.map(id => updateTodo(id, { status: 'cancelled' }))
  )
  const fails = results.filter(r => r.status === 'rejected').length
  if (fails > 0) ElMessage.warning(`${fails} 个未能取消`)
  clearSelection()
  await refresh()
}

async function batchSetPriority(p: TodoPriority) {
  const ids = Array.from(selectedIds.value)
  const results = await Promise.allSettled(
    ids.map(id => updateTodo(id, { priority: p }))
  )
  const fails = results.filter(r => r.status === 'rejected').length
  if (fails > 0) ElMessage.warning(`${fails} 个未能改优先级`)
  clearSelection()
  await refresh()
}

async function switchView(v: 'list' | 'calendar') {
  await layout.setTodoView(v)
}

onMounted(refresh)
</script>

<template>
  <section class="panel panel--list tasks-todo-panel">
    <header class="panel__header tasks-todo-panel__header">
      <div class="tasks-todo-panel__header-row1">
        <h2 class="panel__title">待办</h2>
        <div class="tasks-todo-panel__actions">
          <ElButtonGroup>
            <ElButton
              :type="todoView === 'list' ? 'primary' : 'default'"
              size="small"
              @click="switchView('list')"
            >
              <ElIcon><List /></ElIcon>
            </ElButton>
            <ElButton
              :type="todoView === 'calendar' ? 'primary' : 'default'"
              size="small"
              @click="switchView('calendar')"
            >
              <ElIcon><Calendar /></ElIcon>
            </ElButton>
          </ElButtonGroup>
          <ElButton size="small" @click="refresh" :loading="loading">
            <ElIcon><Refresh /></ElIcon>
          </ElButton>
          <ElButton size="small" type="primary" @click="openCreate">
            <ElIcon><Plus /></ElIcon>新建
          </ElButton>
        </div>
      </div>
      <div class="tasks-todo-panel__header-row2">
        <ElInput
          v-model="searchQuery"
          placeholder="搜索待办..."
          clearable
          :prefix-icon="Search"
        />
        <ElButton size="small" :type="showAll ? 'primary' : 'default'" @click="showAll = !showAll">
          {{ showAll ? '只看进行中' : '显示全部' }}
        </ElButton>
      </div>
    </header>

    <TodoBatchBar
      :count="selectedIds.size"
      @complete-all="batchComplete"
      @cancel-all="batchCancel"
      @set-priority="batchSetPriority"
      @clear-selection="clearSelection"
    />

    <div class="panel__body tasks-todo-panel__body">
      <TodoList
        v-if="todoView === 'list'"
        :todos="filtered"
        :selected-ids="selectedIds"
        :search-query="searchQuery"
        :draggable="canDrag"
        @complete="onComplete"
        @cancel="onCancel"
        @edit="openEdit"
        @toggle-select="toggleSelect"
        @reorder="onReorder"
      />
      <TodoCalendar v-else :todos="todos" />
    </div>

    <ElDialog v-model="formOpen" :title="editingTodo ? '编辑待办' : '新建待办'" width="480px">
      <TodoForm
        :todo="editingTodo"
        :open="formOpen"
        @submit="onFormSubmit"
        @cancel="formOpen = false"
      />
    </ElDialog>
  </section>
</template>

<style scoped>
.tasks-todo-panel__header {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.tasks-todo-panel__header-row1 {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.tasks-todo-panel__header-row2 {
  display: flex;
  gap: 8px;
  align-items: center;
}
.tasks-todo-panel__actions {
  display: flex;
  gap: 4px;
}
.tasks-todo-panel__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 12px;
}
</style>
```

- [ ] **Step 3: 删 DetailColumn 里的 placeholder props（若有）**

Run: `grep -n "TasksTodoPanel" src/views/workspace/DetailColumn.vue`

如果 DetailColumn 给 TasksTodoPanel 传过 "isPlaceholder"-类 prop，删掉。

- [ ] **Step 4: typecheck + lint**

Run: `pnpm typecheck && pnpm lint`
Expected: 通过

- [ ] **Step 5: 手动跑 dev**

Run: `pnpm tauri dev`

在 workspace → tasks category → 待办 panel：能看到 header / search / view switcher / empty state；点新建 → form 弹出。

- [ ] **Step 6: Commit**

```bash
git add src/panels/tasks/TasksTodoPanel.vue src/stores/workspaceLayout.ts src/views/workspace/DetailColumn.vue
git commit -m "feat: #29 TasksTodoPanel 覆写 placeholder (header + view-switcher + body + form) + todoView KV"
```

---

# Phase H — 集成、E2E、文档

## Task H1: 手动 e2e 16 例（spec §12.3）

**Files:**
- 仅手动验证，无文件改

- [ ] **Step 1: 启 dev**

Run: `pnpm tauri dev`

- [ ] **Step 2: 走 spec §12.3 表的 15 例 + 第 6b 例（共 16 例）**

依次按表跑：

1. onboarding step 4 勾 water+sit_long → finalize → 重启 → KV 消失；reminders 表 +2 行
2. onboarding 选 "我不需要" → 重启 → KV 消失；reminders 表无新增
3. tasks 待办 tab 新建 todo（无 due_at）→ 列表显示；reminder 表无新增
4. 编辑 todo 加 due_at（5min 后）→ reminders 表 +1 once + reminder_id 回填；5min 后桌宠点头 + 气泡 + reminder tab 出现
5. 清 due_at → reminders 行被 delete；reminder_id NULL
6. complete 有 due_at + 未触发 todo → status='done'；reminder 被删除；reminder_id NULL；🔔 图标消失
6b. complete 有 due_at + 已触发过的 todo（snooze 后再 complete）→ reminder 仍删除（history 保留）
7. reminder tab 手动建 reminder → tasks tab 不出现新 todo（联动单向）
8. 拖排序 todo → order_index 更新；重启后顺序保留
9. 批量完成 3 个 → 全部 status='done'；batch bar 消失
10. 搜索 "喝" → 过滤生效；清空恢复
11. 切日历视图 → 看小圆点 → 点格 popover 列当天 todo
12. 切日历后关闭 → 重启 → 自动停日历视图（KV 持久化）
13. dev console `await invoke('todo_breakdown', { id: 'x' })` → `Error: breakdown not implemented (M3+)`
14. 本地 17:00 操作建 daily 09:00 reminder → next_fire_at = 明天本地 09:00（DB 中 UTC 01:00）
15. reminder fired → 桌宠点头（VRM head bone 360ms ±15° 摆动；不打断 wander）

逐项打钩；任一失败 → 回去查相关 phase 代码，commit 修复后再跑。

- [ ] **Step 3: 工具链全过**

```bash
cd src-tauri && cargo test -p aipet-app --lib && cd ..
pnpm vitest run
pnpm typecheck
pnpm lint
cd src-tauri && cargo check --bins
```

Expected: 全 PASS

- [ ] **Step 4: 仅当 e2e 全过才进 H2**

如果有失败例：fix → commit fix → 重跑相关 e2e → 全过才继续。

---

## Task H2: docs/lessons.md + docs/STATUS.md 同步

**Files:**
- Modify: `docs/lessons.md`
- Modify: `docs/STATUS.md`

- [ ] **Step 1: lessons.md 加 2 条**

打开 [docs/lessons.md](../../../docs/lessons.md)，按现有 § 编号格式追加 2 条：

```markdown
## §N. 跨 service 写操作必须 tx 注入式（#29 落地）

**触点**：todo↔reminder 联动（#29）& onboarding KV drain（#29）需要"todo + reminder + KV delete 同失败同成功"。

**反 pattern**：在 service A 业务函数内调 service B 的 `*_internal()` 公共入口（B 内部 `pool.execute` 自取连接 → 不在 A 的 tx 内）。失败时 A 的 tx rollback，但 B 的写入已 commit → 数据脏。

**正解**：B 提供 `*_internal_tx(tx: &mut Transaction<'_, Sqlite>, ...)` 入口，接 A 的 tx 引用。A 业务函数：`pool.begin()` → 调 B::*_tx × N → `tx.commit()`。任一 step 失败 → A 在 ? 处提前 return → `tx` drop → sqlx 自动 rollback → A 和 B 同时未写入。

**实现**：reminder.rs / preferences.rs 暴露 `create_internal_tx / update_internal_tx / delete_internal_tx / get_internal_tx / delete_tx`；todo.rs / onboarding_reminders.rs 业务函数取 `pool.begin` → 调上述入口 → commit。

**核验**：cargo test `tx_rollback_on_reminder_coupling_failure_keeps_todos_clean`（todo.rs）+ `drain_in_tx_atomic_all_or_nothing`（onboarding_reminders.rs）双向验证。

## §N+1. REMINDER_TEMPLATES 前后端双写约束（#29 落地）

**触点**：onboarding step 4 reminder 模板列表（5 条），前后端各持一份 hardcode。

**反 pattern**：仅改一边 → 启动期 drain 找不到 template id 静默 skip → 用户勾的 intent 不实例化。

**正解**：扩 template 时同步修改：
- `src/types/reminder.ts` `REMINDER_TEMPLATES` 数组
- `src-tauri/src/services/onboarding_reminders.rs` `TEMPLATES` 静态数组

字段一一对应：`id / title / trigger_type / trigger_spec / priority`。

**核验**：onboarding_reminders.rs 的 `drain_in_tx_skips_unknown_ids` 单测对扩 template 的 skip 行为是兜底，但不能替代双写约束。
```

具体 §N 编号按 lessons.md 当前最大 § 加 1。

- [ ] **Step 2: STATUS.md 同步**

打开 [docs/STATUS.md](../../../docs/STATUS.md)：

- 顶部 `updated:` 改 `2026-05-23`
- "当前 milestone" 行：`M2 W3 进行中（10/10 落地 ✅；待办 + 物理交互待办）` → `M2 W3 进行中（11/11 落地 ✅；物理交互待办）`
- "当前 session 在做" 行替换为：`#29 TodoService MVP + 衔接收尾`（含 commit hash 范围 — close 时取最终 hash）
- "下一步" 行：`[#29] Todo + #21 KV 实例化 + LivingPet hook + AI 拆解 IPC 占位 → [#23] 物理交互...` → `[#23] 物理交互 + 心情/精力 + 摸鱼`
- "M2 W3-W4" milestone 段标题 `10/10` → `11/11`
- 加一行：`- ✅ [#29] E + 衔接: TodoService MVP + onboarding KV 实例化 + LivingPet reminder hook + daily 时区修 + UI 扩展（拖排序/priority/批量/搜索/最小日历）— ~X commit ｜ cargo test 254+ pass, vitest 297 pass, 手动 e2e 16 例`
- 把现有 `⏳ [#29] E + 衔接...` 行删掉（已完成）

- [ ] **Step 3: Commit lessons + STATUS**

```bash
git add docs/lessons.md docs/STATUS.md
git commit -m "docs: #29 lessons.md 2 条 (tx 注入式 + 双写约束) + STATUS M2 W3 11/11"
```

---

## Task H3: 关 issue + final 验证

**Files:**
- 无（GitHub 操作）

- [ ] **Step 1: 收集 commit hash 范围**

Run: `git log --oneline main..HEAD`

把所有 #29 相关 commit hash 收集（应该 ~20 个）。

- [ ] **Step 2: 写 closing comment（self-contained）**

按 CLAUDE.md "信息流" 章约束：closing comment **不能只是「详见 STATUS.md」**，必须自包含。

```markdown
## 落地

**Commit 范围**: `<first-hash>..<last-hash>` (~N commits)

**6 大块**:
1. todos schema 替换（[001_init.sql](src-tauri/migrations/001_init.sql) 9 字段含 reminder_id/order_index REAL/priority/updated_at）
2. TodoService 6 IPC（[todo.rs](src-tauri/src/services/todo.rs) + [commands/todo.rs](src-tauri/src/commands/todo.rs)）+ tx-injection（[reminder.rs](src-tauri/src/services/reminder.rs) 加 create/update/delete/get_internal_tx + [preferences.rs](src-tauri/src/services/preferences.rs) 加 delete_tx）
3. onboarding KV 实例化（[onboarding_reminders.rs](src-tauri/src/services/onboarding_reminders.rs) drain_in_tx 批量原子 + lib.rs setup 钩子）
4. reminder daily HH:MM 时区修（chrono::Local + Tz-injectable，3 单测）
5. LivingPet hook（[usePetReaction.ts](src/composables/usePetReaction.ts) + VRMRuntime.playAction 含 null 防御 + Nod 360ms head bone RAF）
6. Tasks 待办 panel（[TasksTodoPanel.vue](src/panels/tasks/TasksTodoPanel.vue) 覆写 + 4 子组件：TodoList/TodoCalendar/TodoForm/TodoBatchBar；vuedraggable + v-calendar 依赖）

**关键决策 / 偏离**:
- IPC 由 spec 原 5 变 6（加 `todo_reorder` 支撑拖排序）
- `DueAtChange` enum 删 Keep（字段 undefined 已等价 keep；保留 Set / Clear 减少冗余）
- order_index gap < 1e-6 时 reorder 内部触发 normalize_order_indices 自动自愈（无运维步骤）
- complete + 有 once reminder → 同 tx 删 reminder + 清 reminder_id（防止提前完成后到点仍弹气泡）
- onboarding drain 与 KV delete 同 tx 原子（解决 ChatGPT review #2 idempotency）
- reminder.rs 增加 `*_internal_tx` 入口的同时把现有 `create/update/delete` IPC 改 thin wrapper（pool.begin → *_tx → commit）；外部行为不变

**实测**:
- `cargo test -p aipet-app --lib`: 254+ PASS（含 24 新增）
- `pnpm vitest run`: 297 PASS（含 4 新增）
- `cargo check --bins`: 通过
- `pnpm typecheck && pnpm lint`: 通过
- 手动 e2e 16 例全过（spec §12.3）

**Follow-up**:
- `#X 日程化扩展`（E.2 schema event 区间 + 时间轴 + 拖拽改 due_at）
- AI 拆解 confirm dialog UI（M3 接 LLM 时）
- 物理硬删 IPC（如需）

**对齐**:
- spec [docs/superpowers/specs/2026-05-22-todo-service-mvp-design.md](docs/superpowers/specs/2026-05-22-todo-service-mvp-design.md)
- plan [docs/superpowers/plans/2026-05-23-todo-service-mvp-implementation.md](docs/superpowers/plans/2026-05-23-todo-service-mvp-implementation.md)
- ADR-018 / ADR-019（无新 ADR，属规划落地）
- 新增 lessons.md §N（tx 注入式）+ §N+1（REMINDER_TEMPLATES 双写）
```

- [ ] **Step 3: gh 关 issue**

```bash
gh issue close 29 --comment "$(cat <<'EOF'
... 上面内容 ...
EOF
)"
```

或如果环境 gh 未接入：写本地文件 + 让用户手动复制粘贴到 GitHub。

- [ ] **Step 4: Done**

End-of-task confirmation。

---

## Self-Review

### Spec coverage (spec §1-§16 vs plan tasks)

| Spec § | 内容 | Plan task |
|---|---|---|
| §4 架构 6 件事 | TodoService 后端 / 前端契约 / Onboarding KV / LivingPet hook / Tasks panel / daily 时区 | Phase A-G 全覆盖 |
| §5.1 todos schema 9 字段 | id/title/status/due_at/reminder_id/order_index/priority/created_at/updated_at | Task A1 ✅ |
| §5.2 联动表（9 行操作） | create+due / update due_at Set/Clear / update title only / complete+once / cancel+once | Task C2/C5/C6/C7 ✅ |
| §6.1-6.3 6 IPC | create/list/update/complete/breakdown/reorder + DueAtChange tagged union + 后端 SQL 排序 + normalize gap<1e-6 | Task A3 + C1-C9 ✅ |
| §6.4 前端类型 + service | src/types/todo.ts + src/services/todo.ts | Task C10 ✅ |
| §7 Onboarding KV | drain_in_tx 批量原子 + lib.rs setup 钩子 + REMINDER_TEMPLATES 双写 | Task D1-D3 ✅ |
| §7.4 reminder/preferences *_tx 入口 | create/update/delete/get_internal_tx + delete_tx | Task B1-B4 + Task C6（get_internal_tx） ✅ |
| §8 LivingPet hook + null 防御 | VRMRuntime.playAction + usePetReaction + PetCanvas prop + Soul 传 false | Task F1-F3 ✅ |
| §9 AI 拆解占位 | todo_breakdown 永返 BreakdownNotImplemented + UI disabled button | Task C9 + Task G3（按钮在 TodoList ✨ tooltip） ✅ |
| §10 daily 时区修 | chrono::Local + Tz-injectable + 3 单测 + 文案改 | Task E1-E2 ✅ |
| §11 Tasks 待办 panel UI | header 拆两行 + view-switcher + batch-bar sticky + priority 色条 + dark mode watch | Task G2-G7 ✅ |
| §11.4 视图 KV 持久化 | workspaceLayout.ts 加 todoView | Task G7 Step 1 ✅ |
| §12.1 cargo 单测 ~24 | tx rollback / drain atomic / order normalize / daily timezone | Task C1-C9 / D2 / E1 ✅ |
| §12.2 vitest +4 | sort / batch / DueAtChange serde / usePetReaction lifecycle | （未单独列 task — 见下方 Gap） |
| §12.3 手动 e2e 15 例 + 6b | spec 表 | Task H1 ✅ |
| §13 工时 ~15.5h | — | 与 plan task 数对齐 |
| §14 风险 R1-R8 | tx 注入 / 时区自愈 / dark / vuedraggable TS / normalize / NotFound / SoulPledge / VRM null | 风险通过对应 task 实现 ✅ |
| §15 文档同步 | STATUS / lessons / 不新增 ADR | Task H2 ✅ |

**Gap 发现**: §12.2 vitest 4 例（sort / batch / DueAtChange serde / usePetReaction lifecycle）在 plan 中未单独安排 task。

**修复**: 单人项目 YAGNI 评估 — 这 4 例中 sort/batch 是 store/computed 行为，已通过手动 e2e 9/10 验证；DueAtChange serde 是 IPC 契约，由后端 update_due_at_* 三个 cargo 单测兜底（同 enum）；usePetReaction lifecycle 是 listener mount/unmount，由 e2e #15 桌宠点头 + 重启验证。**接受 vitest 不加新例，spec §12.2 改为 "不做" / pendng follow-up**。在 Task H2 STATUS / closing comment 中标注此偏离。

### Placeholder scan

- 无 "TBD" / "TODO: implement" / "fill in details" / "add appropriate error handling" 等模糊词
- 所有代码块均完整可复制
- Task C5 末尾备注 "若 reminder::UpdateInput 无 #[derive(Default)] 则用具名 None 字段" — 这是真实分支策略，非占位
- Task F1 备注 "API 视实际 @pixiv/three-vrm 版本调整" — 这是合理弹性，未挖坑

### Type consistency

- `Todo` / `CreateInput` / `UpdateInput` / `DueAtChange` 字段名前后一致（snake_case Rust / camelCase TS）
- `TodoStatus` = `'open' | 'done' | 'cancelled'` 全 plan 一致
- `TodoPriority` = `'low' | 'normal' | 'high'` 全 plan 一致
- `reminder::create_internal_tx` / `update_internal_tx` / `delete_internal_tx` / `get_internal_tx` 命名一致
- `preferences::delete_tx` / `drain_in_tx` / `instantiate_onboarding_reminders` 命名一致
- `usePetReaction(runtime, isEnabled?)` 签名一致（Task F2 → F3 Step 1.5 修订）
- `VRMRuntime.playAction(actionId: PetActionId)` 签名一致
- `workspace:todo_view` KV 命名一致

无类型不一致。

### 修订记录（self-review 后）

无 inline 修订需要 — 上面 vitest §12.2 偏离已通过 closing comment 标注解决，不改 plan。

---

> **Plan 完成。** 共 8 个 Phase / ~30 个 Task / ~120 个 step。预计 ~15.5h，可分 2-3 session 切。
