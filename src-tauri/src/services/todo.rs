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
