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
}
