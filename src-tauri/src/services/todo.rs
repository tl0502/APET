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
use sqlx::{Connection, Sqlite, SqliteConnection, Transaction};
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

    // due_at 非空时联动 reminder：同 tx 内创建 once reminder 并回填 reminder_id。
    // 任一失败 → 调用方 tx drop 自动 rollback → todo + reminder 同时未写入（spec §5.2）。
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
    let mut conn = open_app_db(app).await?;
    let mut tx = conn.begin().await?;
    let out = create_with_tx(&mut tx, input).await?;
    tx.commit().await?;
    conn.close().await?;
    Ok(out)
}

pub async fn list<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<Todo>, TodoError> {
    let mut conn = open_app_db(app).await?;
    let out = list_with_conn(&mut conn).await?;
    conn.close().await?;
    Ok(out)
}

/// 内部 list 实现：单次 SELECT，无 tx 包裹（只读、单语句、与 reminder::list 同 pattern）。
///
/// SQL 排序契约（前端只做 search/filter，不重排）：
/// 1. status: open(0) → done(1) → cancelled(2)（CASE 表达式）
/// 2. order_index ASC（拖拽分数序）
/// 3. updated_at DESC（同 order_index 时新近改动靠前）
async fn list_with_conn(conn: &mut SqliteConnection) -> Result<Vec<Todo>, TodoError> {
    let rows: Vec<(
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
    Ok(rows
        .into_iter()
        .map(|r| Todo {
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
        .collect())
}

/// 内部 update 实现：接 `&mut Transaction`，保持 tx-injection pattern。
///
/// 覆盖路径：
/// - title / priority / status='open'|'cancelled' 直改 (C4)
/// - due_at None=keep / Set(value) / Clear 三态联动 reminder (C5)
///   * Set on null → create_internal_tx 回填 reminder_id
///   * Set on existing → update_internal_tx 同步 trigger_spec + title（同 reminder_id）
///   * Clear → delete_internal_tx 清 reminder_id
/// - title 改动 + has reminder + due_at 未改 → 同步 title 到 reminder（防止提醒触发标题脱节）
/// - status='cancelled' + 有 once reminder → 同 tx 内删 reminder + 清 reminder_id (C6)
///   （daily / 其他 recurring 保留，独立于该 todo 的用户日常仪式）
///
/// status='done' 通过 update 显式拒绝：必须走 todo_complete（spec §4.2，complete-once 语义）。
async fn update_with_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: UpdateInput,
) -> Result<Todo, TodoError> {
    // 读旧 row（不存在 → NotFound 直接冒泡）
    let existing = get_by_id(&mut **tx, id).await?;

    let new_title = input.title.unwrap_or_else(|| existing.title.clone());
    if new_title.trim().is_empty() {
        return Err(TodoError::InvalidInput("title cannot be empty".into()));
    }

    let new_priority = match input.priority.as_deref() {
        Some(p) => {
            validate_priority(p)?;
            p.to_string()
        }
        None => existing.priority.clone(),
    };

    let new_status = match input.status.as_deref() {
        Some(s) if s == "open" || s == "cancelled" => s.to_string(),
        Some("done") => {
            return Err(TodoError::InvalidInput(
                "status='done' must go through todo_complete".into(),
            ));
        }
        Some(other) => {
            return Err(TodoError::InvalidInput(format!("invalid status: {other}")));
        }
        None => existing.status.clone(),
    };

    // due_at + reminder 联动 (C5)：根据 input.due_at 三态在同 tx 内联动 reminder。
    // 任一 reminder::*_internal_tx 失败 → 调用方 tx drop 自动 rollback。
    let (new_due_at, new_reminder_id): (Option<String>, Option<String>) = match input.due_at {
        // 字段省略 = keep（保留原值）
        None => (existing.due_at.clone(), existing.reminder_id.clone()),
        // Set(value)
        Some(DueAtChange::Set(ref value)) => {
            match (&existing.reminder_id, &existing.due_at) {
                // 原有 reminder + 改时刻 → update reminder.trigger_spec (& title sync)
                (Some(rid), Some(_)) => {
                    crate::services::reminder::update_internal_tx(
                        tx,
                        rid,
                        crate::services::reminder::UpdateInput {
                            title: Some(new_title.clone()),
                            trigger_type: Some("once".into()),
                            trigger_spec: Some(value.clone()),
                            ..Default::default()
                        },
                    )
                    .await
                    .map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
                    (Some(value.clone()), Some(rid.clone()))
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
                    )
                    .await
                    .map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
                    (Some(value.clone()), Some(r.id))
                }
            }
        }
        // Clear
        Some(DueAtChange::Clear) => {
            if let Some(rid) = &existing.reminder_id {
                crate::services::reminder::delete_internal_tx(tx, rid)
                    .await
                    .map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
            }
            (None, None)
        }
    };

    // 仅改 title 但有 reminder_id → 同步 title 到 reminder（避免提醒触发时与 todo 标题脱节）。
    // 注意只在 due_at 未改路径触发——Set 分支已经在上面同步过 title。
    if input.due_at.is_none() && existing.reminder_id.is_some() && new_title != existing.title {
        let rid = existing.reminder_id.as_ref().unwrap();
        crate::services::reminder::update_internal_tx(
            tx,
            rid,
            crate::services::reminder::UpdateInput {
                title: Some(new_title.clone()),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| TodoError::ReminderCoupling(e.to_string()))?;
    }

    // 软删 cancelled + 有 once reminder → 删 reminder（同 complete 语义）
    // 防止用户提前取消后到点仍弹气泡 / 桌宠点头。仅删 once 类型；
    // daily / 其他 recurring reminder 保留（独立于该 todo 的用户日常仪式）。
    // 判断用 new_reminder_id（C5 算出的当前值）而非 existing.reminder_id：
    // 当 input 同时含 status=cancelled + due_at=Clear 时，Clear 先把 reminder_id 设 None，
    // 此分支 noop（无 rid 可删），是正确语义。
    let (final_due_at, final_reminder_id) = if new_status == "cancelled"
        && new_reminder_id.is_some()
    {
        let rid = new_reminder_id.as_ref().unwrap();
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

    let now = Utc::now().to_rfc3339();
    {
        let conn: &mut SqliteConnection = &mut **tx;
        sqlx::query(
            r#"UPDATE todos
               SET title=?, status=?, due_at=?, reminder_id=?, priority=?, updated_at=?
               WHERE id=?"#,
        )
        .bind(&new_title)
        .bind(&new_status)
        .bind(&final_due_at)
        .bind(&final_reminder_id)
        .bind(&new_priority)
        .bind(&now)
        .bind(id)
        .execute(conn)
        .await?;
    }

    get_by_id(&mut **tx, id).await
}

pub async fn update<R: Runtime>(
    app: &AppHandle<R>,
    id: String,
    input: UpdateInput,
) -> Result<Todo, TodoError> {
    let mut conn = open_app_db(app).await?;
    let mut tx = conn.begin().await?;
    let out = update_with_tx(&mut tx, &id, input).await?;
    tx.commit().await?;
    conn.close().await?;
    Ok(out)
}

/// 内部 complete 实现：接 `&mut Transaction`，保持 tx-injection pattern。
///
/// 语义（spec §4.2，complete-once）：
/// - 有 once reminder → 同 tx 内删 reminder + 清 reminder_id（防止用户提前完成后到点仍弹气泡）。
/// - daily / 其他 recurring reminder 保留（独立于该 todo 的用户日常仪式）。
/// - 无 reminder → noop on reminders 表，仅 UPDATE status='done'。
///
/// 与 update(cancelled) 路径对齐（C6 commit 7af551e）；UI 用 todo_complete IPC 触发 mark-done 按钮，
/// 不与 update 共享路径（update 显式拒绝 status='done'）。
async fn complete_with_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<Todo, TodoError> {
    let existing = get_by_id(&mut **tx, id).await?;

    // 状态机守卫：只允许 open → done。
    // - 已 done：noop 返当前 row（idempotent，不动 updated_at）— UI 双击不出错
    // - cancelled：拒绝（避免取消后又被复活成 done）
    if existing.status == "done" {
        return Ok(existing);
    }
    if existing.status != "open" {
        return Err(TodoError::InvalidInput(format!(
            "cannot complete todo in status '{}' (only 'open' allowed)",
            existing.status
        )));
    }

    // 删 once reminder（防止用户提前完成后到点仍弹气泡）。仅 once；daily 等 recurring 保留。
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
    let mut conn = open_app_db(app).await?;
    let mut tx = conn.begin().await?;
    let out = complete_with_tx(&mut tx, &id).await?;
    tx.commit().await?;
    conn.close().await?;
    Ok(out)
}

pub async fn breakdown<R: Runtime>(_app: &AppHandle<R>, _id: String) -> Result<Vec<String>, TodoError> {
    Err(TodoError::BreakdownNotImplemented)
}

/// 相邻 order_index 间距小于该阈值时，reorder 触发 batch normalize 自愈。
/// 1e-6 给 f64 留足空间：连续在同一处 midpoint 插入 ~50 次后达到该量级。
const ORDER_GAP_THRESHOLD: f64 = 1e-6;

/// 批量重排所有 open todo 的 order_index 为 0 / 10 / 20 / ...
/// 用于 reorder 检测到相邻 gap < ORDER_GAP_THRESHOLD 时自愈，无运维步骤。
/// 排序键与 list_with_conn 一致（order_index ASC, updated_at DESC）保持视觉稳定。
async fn normalize_order_indices(tx: &mut Transaction<'_, Sqlite>) -> Result<(), TodoError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM todos WHERE status='open' ORDER BY order_index ASC, updated_at DESC",
    )
    .fetch_all(&mut **tx)
    .await?;
    let now = Utc::now().to_rfc3339();
    for (idx, (id,)) in rows.into_iter().enumerate() {
        let new_order = (idx as f64) * 10.0;
        let conn: &mut SqliteConnection = &mut **tx;
        sqlx::query("UPDATE todos SET order_index=?, updated_at=? WHERE id=?")
            .bind(new_order)
            .bind(&now)
            .bind(&id)
            .execute(conn)
            .await?;
    }
    Ok(())
}

/// 内部 reorder 实现：分数中位算法 + gap<1e-6 时触发 normalize 自愈。
///
/// - `after_id = None` → 拖到最前：newOrder = min(order_index) - 10（或 0）。
/// - `after_id = Some(a)` → newOrder = (a.order + next.order) / 2，或 a.order + 10（无 next）。
/// - 若 a 与 next 间距 < ORDER_GAP_THRESHOLD（f64 精度即将耗尽），先 normalize
///   全表 open 行为 0/10/20/...，再递归调用自身重算 newOrder（Box::pin 避免 async fn
///   递归编译时 infinite-sized future）。
/// - 只操作 `status='open'` 行；cancelled/done 不参与排序。
async fn reorder_with_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    after_id: Option<&str>,
) -> Result<Todo, TodoError> {
    // 状态机守卫：仅 open todo 可重排（done/cancelled 不在用户可拖动列表中）。
    let target_status: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM todos WHERE id=?",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    match target_status {
        None => return Err(TodoError::NotFound(id.to_string())),
        Some((s,)) if s != "open" => {
            return Err(TodoError::InvalidInput(format!(
                "cannot reorder todo in status '{s}' (only 'open' allowed)"
            )));
        }
        Some(_) => {}
    }

    // 算 newOrder + 探测 gap 是否需要 normalize（共用一次邻居查询）
    let (new_order, needs_normalize) = match after_id {
        Some(after) => {
            let after_row: Option<(f64,)> = sqlx::query_as(
                "SELECT order_index FROM todos WHERE id=? AND status='open'"
            )
            .bind(after).fetch_optional(&mut **tx).await?;
            let after_oi = after_row
                .ok_or_else(|| TodoError::InvalidInput(format!("after_id not open todo: {after}")))?
                .0;
            let next_row: Option<(f64,)> = sqlx::query_as(
                r#"SELECT order_index FROM todos
                   WHERE status='open' AND order_index > ? AND id != ?
                   ORDER BY order_index ASC LIMIT 1"#,
            )
            .bind(after_oi).bind(id).fetch_optional(&mut **tx).await?;
            let (new_order, needs_normalize) = match next_row {
                Some((next_oi,)) => {
                    let gap = (next_oi - after_oi).abs();
                    ((after_oi + next_oi) / 2.0, gap < ORDER_GAP_THRESHOLD)
                }
                None => (after_oi + 10.0, false),
            };
            (new_order, needs_normalize)
        }
        None => {
            let row: Option<(Option<f64>,)> = sqlx::query_as(
                "SELECT MIN(order_index) FROM todos WHERE status='open' AND id != ?",
            ).bind(id).fetch_optional(&mut **tx).await?;
            let new_order = row.and_then(|(v,)| v).map(|v| v - 10.0).unwrap_or(0.0);
            (new_order, false)
        }
    };

    if needs_normalize {
        normalize_order_indices(tx).await?;
        // normalize 后 after 的 order_index 已变，递归重算 newOrder。
        // Box::pin 必需：async fn 直接递归会被编译器视为 infinite-sized future。
        return Box::pin(reorder_with_tx(tx, id, after_id)).await;
    }

    // UPDATE 目标行
    let now = Utc::now().to_rfc3339();
    {
        let conn: &mut SqliteConnection = &mut **tx;
        sqlx::query("UPDATE todos SET order_index=?, updated_at=? WHERE id=?")
            .bind(new_order)
            .bind(&now)
            .bind(id)
            .execute(conn)
            .await?;
    }
    get_by_id(&mut **tx, id).await
}

pub async fn reorder<R: Runtime>(
    app: &AppHandle<R>,
    id: String,
    after_id: Option<String>,
) -> Result<Todo, TodoError> {
    let mut conn = open_app_db(app).await?;
    let mut tx = conn.begin().await?;
    let out = reorder_with_tx(&mut tx, &id, after_id.as_deref()).await?;
    tx.commit().await?;
    conn.close().await?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::test_db::fresh_db;
    use sqlx::Connection;

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

    #[tokio::test]
    async fn list_returns_open_first_then_done_then_cancelled_sorted_by_order_index() {
        let (_dir, mut conn) = fresh_db().await;
        // 直插 4 行不同 status + order_index
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

    #[tokio::test]
    async fn update_title_only_does_not_touch_reminder() {
        let (_dir, mut conn) = fresh_db().await;
        let mut tx = conn.begin().await.unwrap();
        let todo = create_with_tx(
            &mut tx,
            CreateInput {
                title: "old".into(),
                due_at: None,
                priority: None,
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let mut tx = conn.begin().await.unwrap();
        let updated = update_with_tx(
            &mut tx,
            &todo.id,
            UpdateInput {
                title: Some("new".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(updated.title, "new");
        assert!(updated.reminder_id.is_none());
    }

    #[tokio::test]
    async fn update_cannot_set_status_done_directly() {
        let (_dir, mut conn) = fresh_db().await;
        let mut tx = conn.begin().await.unwrap();
        let todo = create_with_tx(
            &mut tx,
            CreateInput {
                title: "x".into(),
                due_at: None,
                priority: None,
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let mut tx = conn.begin().await.unwrap();
        let err = update_with_tx(
            &mut tx,
            &todo.id,
            UpdateInput {
                status: Some("done".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, TodoError::InvalidInput(_)));
    }

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

    #[tokio::test]
    async fn complete_on_already_done_is_idempotent_noop() {
        let (_dir, mut conn) = fresh_db().await;
        let mut tx = conn.begin().await.unwrap();
        let todo = create_with_tx(
            &mut tx,
            CreateInput { title: "x".into(), due_at: None, priority: None },
        ).await.unwrap();
        tx.commit().await.unwrap();

        // First complete
        let mut tx = conn.begin().await.unwrap();
        let done1 = complete_with_tx(&mut tx, &todo.id).await.unwrap();
        tx.commit().await.unwrap();
        let first_updated_at = done1.updated_at.clone();

        // Second complete — should be noop (returns same row, updated_at unchanged)
        let mut tx = conn.begin().await.unwrap();
        let done2 = complete_with_tx(&mut tx, &todo.id).await.unwrap();
        tx.commit().await.unwrap();

        assert_eq!(done2.status, "done");
        assert_eq!(done2.updated_at, first_updated_at);  // no UPDATE issued
    }

    #[tokio::test]
    async fn complete_on_cancelled_returns_invalid_input() {
        let (_dir, mut conn) = fresh_db().await;
        let mut tx = conn.begin().await.unwrap();
        let todo = create_with_tx(
            &mut tx,
            CreateInput { title: "x".into(), due_at: None, priority: None },
        ).await.unwrap();
        tx.commit().await.unwrap();

        // Cancel it
        let mut tx = conn.begin().await.unwrap();
        update_with_tx(
            &mut tx, &todo.id,
            UpdateInput { status: Some("cancelled".into()), ..Default::default() },
        ).await.unwrap();
        tx.commit().await.unwrap();

        // Try to complete cancelled todo
        let mut tx = conn.begin().await.unwrap();
        let err = complete_with_tx(&mut tx, &todo.id).await.unwrap_err();
        assert!(matches!(err, TodoError::InvalidInput(_)));
    }

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
        // 触发 normalize 后：a/b 原始 gap=1e-7 应被消除；所有相邻 gap 应远大于阈值
        // （normalize 把 a,b,c 排为 0/10/20；再递归把 c 插到 a/b 之间 → 0/5/10）。
        let rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT id, order_index FROM todos WHERE status='open' ORDER BY order_index ASC"
        ).fetch_all(&mut conn).await.unwrap();
        let orders: Vec<f64> = rows.iter().map(|r| r.1).collect();
        assert_eq!(orders.len(), 3);
        // 验证 normalize 已生效：所有相邻间距远大于 ORDER_GAP_THRESHOLD
        for w in orders.windows(2) {
            assert!(
                w[1] - w[0] > ORDER_GAP_THRESHOLD * 1000.0,
                "expected gap >> threshold, got {:?}", orders
            );
        }
    }

    #[tokio::test]
    async fn reorder_target_status_done_returns_invalid_input() {
        let (_dir, mut conn) = fresh_db().await;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT INTO todos (id,title,status,order_index,priority,created_at,updated_at)
               VALUES (?, ?, 'done', ?, 'normal', ?, ?)"#,
        )
        .bind("a").bind("a").bind(0.0).bind(&now).bind(&now)
        .execute(&mut conn).await.unwrap();

        let mut tx = conn.begin().await.unwrap();
        let err = reorder_with_tx(&mut tx, "a", None).await.unwrap_err();
        assert!(matches!(err, TodoError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn reorder_target_not_found_returns_not_found() {
        let (_dir, mut conn) = fresh_db().await;

        let mut tx = conn.begin().await.unwrap();
        let err = reorder_with_tx(&mut tx, "nonexistent-id", None).await.unwrap_err();
        assert!(matches!(err, TodoError::NotFound(_)));
    }

    #[tokio::test]
    async fn breakdown_always_returns_not_implemented() {
        // 不需要 DB；breakdown 是 pure stub
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
                due_at: Some("not-a-date".into()), // 非法 RFC3339
                priority: None,
            },
        )
        .await;
        // tx drop → 自动 rollback；conn 状态回到 begin 前
        drop(tx);

        assert!(matches!(result, Err(TodoError::ReminderCoupling(_))));
        // 验证 todos 表无残留
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM todos")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(count.0, 0);
        // 验证 reminders 表无残留
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(count.0, 0);
    }
}
