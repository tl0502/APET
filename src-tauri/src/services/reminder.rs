//! ReminderService（#22，模块 C）— 提醒 CRUD + 触发 + 启动 catch-up。
//!
//! 范围（M2，2026-05-17）:
//! - 6 IPC: create / list / update / delete / snooze / complete
//! - trigger_type: 'once'（RFC3339）+ 'daily'（"HH:MM" 或 "*/N * *" 每 N 分钟）。
//!   'weekly' / 'cron' 占位，M2 不实现（避免范围蔓延，issue body 拍板）。
//! - 触发链：scheduler::find_due（NOT EXISTS 防重入）→ reminder::fire（写 history +
//!   推进 next_fire_at + emit + OS notification）。
//! - 防重入：reminder_history.fired_at >= reminders.next_fire_at 表示"该 anchor 已触发"，
//!   find_due 用 NOT EXISTS 过滤，fire 必须立即推进 next_fire_at（once → NULL+enabled=0；
//!   daily → 加一天/重算）。
//! - snooze 5/15/30 × 最多 3 次（ADR 决策 14 + UAT-Reminder-3）；前端 UI 在
//!   snooze_count==3 时隐藏稍后按钮，后端 MAX_SNOOZE_COUNT 防御一次。
//! - 时区：内部一律 RFC3339 UTC；UI 转本地。daily HH:MM 当前按 UTC 解释——M2 简化（中国
//!   时区用户 +8h 偏移），follow-up #29 接入本地时区转换。
//!
//! Schema（migrations/001_init.sql:99-120）零迁移（lesson #2）:
//! - reminders: id/title/trigger_type/trigger_spec/priority/enabled/snooze_count/
//!   next_fire_at/created_at/updated_at
//! - reminder_history: id/reminder_id/fired_at/action/snooze_count
//! - 无 FK，delete 时手动级联清 history（trade-off：简单 vs 数据完整性，M2 接受）。
//!
//! Event:
//! - 'reminder:fired' { reminderId, priority, title, snoozeCount }（architecture §683 契约）
//! - 'reminder:catch_up' [{reminderId, title, priority}]（启动期合并补提）

use chrono::{DateTime, Datelike, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Connection, SqliteConnection};
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_notification::NotificationExt;
use thiserror::Error;
use ulid::Ulid;

use crate::services::db::{open_app_db, DbError};

const REMINDER_FIRED_EVENT: &str = "reminder:fired";
const REMINDER_CATCH_UP_EVENT: &str = "reminder:catch_up";

/// 稍后次数上限（ADR 决策 14：有限稍后；UAT-Reminder-3：连续稍后 3 次后第 4 次转 overdue）。
/// UI 在 snooze_count==3 时隐藏稍后按钮；本常量是后端防御兜底。
pub const MAX_SNOOZE_COUNT: u32 = 3;
const SNOOZE_MINUTES_ALLOWED: &[u32] = &[5, 15, 30];

#[derive(Debug, Error)]
pub enum ReminderError {
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
    #[error("reminder not found: {0}")]
    NotFound(String),
    #[error("invalid trigger: {0}")]
    InvalidTrigger(String),
    #[error("snooze count exceeded (max 3)")]
    MaxSnoozeExceeded,
    #[error("invalid snooze minutes: {0} (must be 5/15/30)")]
    InvalidSnoozeMinutes(u32),
}

impl From<sqlx::Error> for ReminderError {
    fn from(e: sqlx::Error) -> Self {
        ReminderError::Database(e.to_string())
    }
}

impl From<DbError> for ReminderError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => ReminderError::AppConfigDir(s),
            DbError::Database(s) => ReminderError::Database(s),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub trigger_type: String,
    pub trigger_spec: String,
    pub priority: String,
    pub enabled: bool,
    pub snooze_count: u32,
    pub next_fire_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInput {
    pub title: String,
    pub trigger_type: String,
    pub trigger_spec: String,
    pub priority: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInput {
    pub title: Option<String>,
    pub trigger_type: Option<String>,
    pub trigger_spec: Option<String>,
    pub priority: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiredPayload {
    pub reminder_id: String,
    pub priority: String,
    pub title: String,
    pub snooze_count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatchUpItem {
    pub reminder_id: String,
    pub title: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatchUpReport {
    pub merged_count: usize,
    pub overdue_count: usize,
}

// ============================================================================
// 公共 IPC fn — create / list / update / delete / snooze / complete
// ============================================================================

pub async fn create<R: Runtime>(
    app: &AppHandle<R>,
    input: CreateInput,
) -> Result<Reminder, ReminderError> {
    let priority = input.priority.as_deref().unwrap_or("soft").to_string();
    validate_priority(&priority)?;
    validate_trigger(&input.trigger_type, &input.trigger_spec)?;

    let id = Ulid::new().to_string();
    let now = Utc::now();
    let now_str = now.to_rfc3339();
    let next = compute_next_fire_at(&input.trigger_type, &input.trigger_spec, now)?;
    let next_str = next.map(|dt| dt.to_rfc3339());

    let mut conn = open_app_db(app).await?;
    sqlx::query(
        r#"INSERT INTO reminders
           (id, title, trigger_type, trigger_spec, priority, enabled, snooze_count, next_fire_at, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, 1, 0, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(&input.title)
    .bind(&input.trigger_type)
    .bind(&input.trigger_spec)
    .bind(&priority)
    .bind(&next_str)
    .bind(&now_str)
    .bind(&now_str)
    .execute(&mut conn)
    .await?;

    let reminder = get_with_conn(&mut conn, &id).await?;
    conn.close().await?;
    Ok(reminder)
}

pub async fn list<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<Reminder>, ReminderError> {
    let mut conn = open_app_db(app).await?;
    let rows = list_with_conn(&mut conn).await?;
    conn.close().await?;
    Ok(rows)
}

pub async fn update<R: Runtime>(
    app: &AppHandle<R>,
    id: String,
    input: UpdateInput,
) -> Result<Reminder, ReminderError> {
    let mut conn = open_app_db(app).await?;
    let mut r = get_with_conn(&mut conn, &id).await?;

    if let Some(v) = input.title {
        r.title = v;
    }
    if let Some(v) = input.trigger_type {
        r.trigger_type = v;
    }
    if let Some(v) = input.trigger_spec {
        r.trigger_spec = v;
    }
    if let Some(v) = input.priority {
        validate_priority(&v)?;
        r.priority = v;
    }
    if let Some(v) = input.enabled {
        r.enabled = v;
    }

    validate_trigger(&r.trigger_type, &r.trigger_spec)?;

    let now = Utc::now();
    let now_str = now.to_rfc3339();
    // update 改触发参数 → 重置 snooze_count 并重算 next_fire_at；enabled=false 时清空 next。
    let next = if r.enabled {
        compute_next_fire_at(&r.trigger_type, &r.trigger_spec, now)?
    } else {
        None
    };
    let next_str = next.map(|dt| dt.to_rfc3339());

    sqlx::query(
        r#"UPDATE reminders SET title=?, trigger_type=?, trigger_spec=?, priority=?,
           enabled=?, next_fire_at=?, snooze_count=0, updated_at=? WHERE id=?"#,
    )
    .bind(&r.title)
    .bind(&r.trigger_type)
    .bind(&r.trigger_spec)
    .bind(&r.priority)
    .bind(if r.enabled { 1 } else { 0 })
    .bind(&next_str)
    .bind(&now_str)
    .bind(&id)
    .execute(&mut conn)
    .await?;

    let out = get_with_conn(&mut conn, &id).await?;
    conn.close().await?;
    Ok(out)
}

pub async fn delete<R: Runtime>(app: &AppHandle<R>, id: String) -> Result<(), ReminderError> {
    let mut conn = open_app_db(app).await?;
    let res = sqlx::query("DELETE FROM reminders WHERE id=?")
        .bind(&id)
        .execute(&mut conn)
        .await?;
    if res.rows_affected() == 0 {
        conn.close().await?;
        return Err(ReminderError::NotFound(id));
    }
    // schema 无 FK，手动级联清 history。
    sqlx::query("DELETE FROM reminder_history WHERE reminder_id=?")
        .bind(&id)
        .execute(&mut conn)
        .await?;
    conn.close().await?;
    Ok(())
}

pub async fn snooze<R: Runtime>(
    app: &AppHandle<R>,
    id: String,
    minutes: u32,
) -> Result<Reminder, ReminderError> {
    if !SNOOZE_MINUTES_ALLOWED.contains(&minutes) {
        return Err(ReminderError::InvalidSnoozeMinutes(minutes));
    }
    let mut conn = open_app_db(app).await?;
    let r = get_with_conn(&mut conn, &id).await?;
    if r.snooze_count >= MAX_SNOOZE_COUNT {
        conn.close().await?;
        return Err(ReminderError::MaxSnoozeExceeded);
    }
    let now = Utc::now();
    let now_str = now.to_rfc3339();
    let next_str = (now + chrono::Duration::minutes(minutes as i64)).to_rfc3339();
    sqlx::query(
        "UPDATE reminders SET snooze_count=snooze_count+1, next_fire_at=?, updated_at=? WHERE id=?",
    )
    .bind(&next_str)
    .bind(&now_str)
    .bind(&id)
    .execute(&mut conn)
    .await?;
    sqlx::query(
        "INSERT INTO reminder_history (reminder_id, fired_at, action, snooze_count) VALUES (?, ?, 'snoozed', ?)",
    )
    .bind(&id)
    .bind(&now_str)
    .bind((r.snooze_count + 1) as i64)
    .execute(&mut conn)
    .await?;
    let out = get_with_conn(&mut conn, &id).await?;
    conn.close().await?;
    Ok(out)
}

pub async fn complete<R: Runtime>(app: &AppHandle<R>, id: String) -> Result<(), ReminderError> {
    let mut conn = open_app_db(app).await?;
    let r = get_with_conn(&mut conn, &id).await?;
    let now = Utc::now();
    let now_str = now.to_rfc3339();

    sqlx::query(
        "INSERT INTO reminder_history (reminder_id, fired_at, action, snooze_count) VALUES (?, ?, 'completed', ?)",
    )
    .bind(&id)
    .bind(&now_str)
    .bind(r.snooze_count as i64)
    .execute(&mut conn)
    .await?;

    // once → 禁用 + 清 next；recurring → 重算 next + 重置 snooze。
    match r.trigger_type.as_str() {
        "once" => {
            sqlx::query("UPDATE reminders SET enabled=0, next_fire_at=NULL, snooze_count=0, updated_at=? WHERE id=?")
                .bind(&now_str)
                .bind(&id)
                .execute(&mut conn)
                .await?;
        }
        _ => {
            let next = compute_next_fire_at(&r.trigger_type, &r.trigger_spec, now)?;
            let next_str = next.map(|dt| dt.to_rfc3339());
            sqlx::query("UPDATE reminders SET next_fire_at=?, snooze_count=0, updated_at=? WHERE id=?")
                .bind(&next_str)
                .bind(&now_str)
                .bind(&id)
                .execute(&mut conn)
                .await?;
        }
    }
    conn.close().await?;
    Ok(())
}

// ============================================================================
// Scheduler 触发路径 — find_due + fire
// ============================================================================

/// scheduler tick 调：找到所有到点且未被本 anchor 触发的 reminder。
///
/// 防重入：NOT EXISTS 子查询过滤已 fired_at >= next_fire_at 的记录，避免同 anchor
/// 在 polling + eager check 并发场景下被触发两次。
pub(crate) async fn find_due<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<String>, ReminderError> {
    let now_str = Utc::now().to_rfc3339();
    let mut conn = open_app_db(app).await?;
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT id FROM reminders r
           WHERE r.enabled=1 AND r.next_fire_at IS NOT NULL AND r.next_fire_at <= ?
             AND NOT EXISTS (
                 SELECT 1 FROM reminder_history h
                 WHERE h.reminder_id = r.id AND h.fired_at >= r.next_fire_at
             )"#,
    )
    .bind(&now_str)
    .fetch_all(&mut conn)
    .await?;
    conn.close().await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// 触发一条 reminder：写 history + 推进 next_fire_at + emit + OS notification。
///
/// 顺序：先写 history 提交（推进 anchor 防重入）→ 再 emit + notification（best-effort，
/// 失败仅 log，不 rollback——history 已经标记触发；用户已经看到对话/列表里的状态变化）。
pub(crate) async fn fire<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<(), ReminderError> {
    let mut conn = open_app_db(app).await?;
    let r = get_with_conn(&mut conn, id).await?;

    if !r.enabled {
        conn.close().await?;
        return Ok(());
    }

    let now = Utc::now();
    let now_str = now.to_rfc3339();

    // 1. 写 history：action='ignored' 是"已触发未操作"占位。用户后续点完成/稍后会写新行。
    //    与 telemetry UAT-Reminder-3 中"用户主动忽略"语义略有 overlap，但 M2 范围内本字段
    //    仅作"已触发"标记，详 #22 PR commit message 登记。
    sqlx::query(
        "INSERT INTO reminder_history (reminder_id, fired_at, action, snooze_count) VALUES (?, ?, 'ignored', ?)",
    )
    .bind(id)
    .bind(&now_str)
    .bind(r.snooze_count as i64)
    .execute(&mut conn)
    .await?;

    // 2. 推进 next_fire_at：once → 清空 + 禁用；recurring → 重算下次。
    match r.trigger_type.as_str() {
        "once" => {
            sqlx::query(
                "UPDATE reminders SET enabled=0, next_fire_at=NULL, updated_at=? WHERE id=?",
            )
            .bind(&now_str)
            .bind(id)
            .execute(&mut conn)
            .await?;
        }
        _ => {
            let next = compute_next_fire_at(&r.trigger_type, &r.trigger_spec, now)?;
            let next_str = next.map(|dt| dt.to_rfc3339());
            sqlx::query("UPDATE reminders SET next_fire_at=?, updated_at=? WHERE id=?")
                .bind(&next_str)
                .bind(&now_str)
                .bind(id)
                .execute(&mut conn)
                .await?;
        }
    }
    conn.close().await?;

    // 3. emit + OS notification（best-effort）。
    let payload = FiredPayload {
        reminder_id: id.to_string(),
        priority: r.priority.clone(),
        title: r.title.clone(),
        snooze_count: r.snooze_count,
    };
    if let Err(e) = app.emit(REMINDER_FIRED_EVENT, &payload) {
        eprintln!("[reminder] emit {REMINDER_FIRED_EVENT} failed: {e}");
    }

    // OS 通知文案 hardcode（lesson #5 AUP：不调 LLM、无安全前缀；前端展示桌宠气泡时
    // 可改用人格化文本）。
    let body = if r.priority == "hard" {
        "现在做"
    } else {
        "记得做哦"
    };
    if let Err(e) = app
        .notification()
        .builder()
        .title(format!("提醒 · {}", r.title))
        .body(body)
        .show()
    {
        eprintln!("[reminder] OS notification failed: {e}");
    }

    Ok(())
}

// ============================================================================
// 启动期 catch-up — 30min 内合并 toast / 超过标 overdue
// ============================================================================

pub async fn catch_up_overdue<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<CatchUpReport, ReminderError> {
    let now = Utc::now();
    let cutoff = now - chrono::Duration::minutes(30);
    let now_str = now.to_rfc3339();

    let mut conn = open_app_db(app).await?;

    // 查所有 enabled=1 且 next_fire_at<now 且未被记录为已触发的 reminder。
    let rows: Vec<(String, String, String, String, String, String)> = sqlx::query_as(
        r#"SELECT r.id, r.title, r.priority, r.next_fire_at, r.trigger_type, r.trigger_spec
           FROM reminders r
           WHERE r.enabled=1 AND r.next_fire_at IS NOT NULL AND r.next_fire_at < ?
             AND NOT EXISTS (
                 SELECT 1 FROM reminder_history h
                 WHERE h.reminder_id = r.id AND h.fired_at >= r.next_fire_at
             )"#,
    )
    .bind(&now_str)
    .fetch_all(&mut conn)
    .await?;

    let mut merged = Vec::new();
    let mut overdue = 0usize;

    for (id, title, priority, next_fire_at, trigger_type, trigger_spec) in rows {
        let next_dt = match DateTime::parse_from_rfc3339(&next_fire_at) {
            Ok(d) => d.with_timezone(&Utc),
            Err(_) => continue,
        };

        if next_dt < cutoff {
            // > 30min：标 overdue + 推进 anchor（once → 禁用）。
            sqlx::query(
                "INSERT INTO reminder_history (reminder_id, fired_at, action, snooze_count) VALUES (?, ?, 'overdue', 0)",
            )
            .bind(&id)
            .bind(&next_fire_at)
            .execute(&mut conn)
            .await?;

            match trigger_type.as_str() {
                "once" => {
                    sqlx::query(
                        "UPDATE reminders SET enabled=0, next_fire_at=NULL, updated_at=? WHERE id=?",
                    )
                    .bind(&now_str)
                    .bind(&id)
                    .execute(&mut conn)
                    .await?;
                }
                _ => {
                    let next = compute_next_fire_at(&trigger_type, &trigger_spec, now)
                        .ok()
                        .flatten();
                    let next_str = next.map(|dt| dt.to_rfc3339());
                    sqlx::query(
                        "UPDATE reminders SET next_fire_at=?, updated_at=? WHERE id=?",
                    )
                    .bind(&next_str)
                    .bind(&now_str)
                    .bind(&id)
                    .execute(&mut conn)
                    .await?;
                }
            }
            overdue += 1;
        } else {
            // ≤ 30min：merge 给前端展示一条合并 toast。
            merged.push(CatchUpItem {
                reminder_id: id,
                title,
                priority,
            });
        }
    }

    conn.close().await?;

    if !merged.is_empty() {
        if let Err(e) = app.emit(REMINDER_CATCH_UP_EVENT, &merged) {
            eprintln!("[reminder] emit {REMINDER_CATCH_UP_EVENT} failed: {e}");
        }
    }

    Ok(CatchUpReport {
        merged_count: merged.len(),
        overdue_count: overdue,
    })
}

// ============================================================================
// Helpers — validate / parse / compute / SQL inner
// ============================================================================

fn validate_priority(p: &str) -> Result<(), ReminderError> {
    if matches!(p, "soft" | "hard") {
        Ok(())
    } else {
        Err(ReminderError::InvalidTrigger(format!(
            "priority must be 'soft' or 'hard', got '{p}'"
        )))
    }
}

fn validate_trigger(t: &str, spec: &str) -> Result<(), ReminderError> {
    match t {
        "once" => {
            DateTime::parse_from_rfc3339(spec).map_err(|e| {
                ReminderError::InvalidTrigger(format!("once trigger_spec must be RFC3339: {e}"))
            })?;
            Ok(())
        }
        "daily" => {
            parse_daily_spec(spec)?;
            Ok(())
        }
        "weekly" | "cron" => Err(ReminderError::InvalidTrigger(format!(
            "{t} trigger not supported in M2 (only 'once' and 'daily')"
        ))),
        other => Err(ReminderError::InvalidTrigger(format!(
            "unknown trigger_type: {other}"
        ))),
    }
}

#[derive(Debug, Clone, Copy)]
enum DailySpec {
    EveryMinutes(u32),
    DailyAt { hour: u32, minute: u32 },
}

fn parse_daily_spec(spec: &str) -> Result<DailySpec, ReminderError> {
    let trimmed = spec.trim();

    if let Some(rest) = trimmed.strip_prefix("*/") {
        let n_str = rest.split_whitespace().next().ok_or_else(|| {
            ReminderError::InvalidTrigger(format!("daily */N malformed: '{spec}'"))
        })?;
        let n: u32 = n_str.parse().map_err(|_| {
            ReminderError::InvalidTrigger(format!("daily */N must be number: '{n_str}'"))
        })?;
        if n == 0 || n > 1440 {
            return Err(ReminderError::InvalidTrigger(format!(
                "daily */N must be 1..=1440, got {n}"
            )));
        }
        return Ok(DailySpec::EveryMinutes(n));
    }

    // "HH:MM"
    let mut parts = trimmed.split(':');
    let h_str = parts.next().ok_or_else(|| {
        ReminderError::InvalidTrigger(format!("daily HH:MM malformed: '{spec}'"))
    })?;
    let m_str = parts.next().ok_or_else(|| {
        ReminderError::InvalidTrigger(format!("daily HH:MM malformed: '{spec}'"))
    })?;
    let h: u32 = h_str
        .parse()
        .map_err(|_| ReminderError::InvalidTrigger(format!("daily HH hour: '{h_str}'")))?;
    let m: u32 = m_str
        .parse()
        .map_err(|_| ReminderError::InvalidTrigger(format!("daily MM minute: '{m_str}'")))?;
    if h > 23 || m > 59 {
        return Err(ReminderError::InvalidTrigger(format!(
            "daily HH:MM out of range: {h}:{m}"
        )));
    }
    Ok(DailySpec::DailyAt {
        hour: h,
        minute: m,
    })
}

fn compute_next_fire_at(
    trigger_type: &str,
    trigger_spec: &str,
    from: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, ReminderError> {
    match trigger_type {
        "once" => {
            let dt = DateTime::parse_from_rfc3339(trigger_spec)
                .map_err(|e| ReminderError::InvalidTrigger(format!("once spec parse: {e}")))?
                .with_timezone(&Utc);
            if dt > from {
                Ok(Some(dt))
            } else {
                Ok(None)
            }
        }
        "daily" => {
            let spec = parse_daily_spec(trigger_spec)?;
            match spec {
                DailySpec::EveryMinutes(n) => {
                    Ok(Some(from + chrono::Duration::minutes(n as i64)))
                }
                DailySpec::DailyAt { hour, minute } => {
                    // M2 简化：HH:MM 按 UTC 解释（中国时区用户 +8h 偏移）；
                    // follow-up #29 / M3 引入 chrono-tz + 用户时区配置后转本地。
                    let date = from.date_naive();
                    let today = Utc
                        .with_ymd_and_hms(date.year(), date.month(), date.day(), hour, minute, 0)
                        .single();
                    match today {
                        Some(dt) => {
                            if dt > from {
                                Ok(Some(dt))
                            } else {
                                Ok(Some(dt + chrono::Duration::days(1)))
                            }
                        }
                        None => Err(ReminderError::InvalidTrigger(
                            "daily compute target invalid".into(),
                        )),
                    }
                }
            }
        }
        _ => Err(ReminderError::InvalidTrigger(format!(
            "unsupported trigger_type: {trigger_type}"
        ))),
    }
}

async fn get_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Reminder, ReminderError> {
    let row: Option<(
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        Option<String>,
        String,
        String,
    )> = sqlx::query_as(
        r#"SELECT id, title, trigger_type, trigger_spec, priority, enabled, snooze_count,
                  next_fire_at, created_at, updated_at
           FROM reminders WHERE id=?"#,
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;

    row.map(|r| Reminder {
        id: r.0,
        title: r.1,
        trigger_type: r.2,
        trigger_spec: r.3,
        priority: r.4,
        enabled: r.5 != 0,
        snooze_count: r.6 as u32,
        next_fire_at: r.7,
        created_at: r.8,
        updated_at: r.9,
    })
    .ok_or_else(|| ReminderError::NotFound(id.to_string()))
}

async fn list_with_conn(conn: &mut SqliteConnection) -> Result<Vec<Reminder>, ReminderError> {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        Option<String>,
        String,
        String,
    )> = sqlx::query_as(
        r#"SELECT id, title, trigger_type, trigger_spec, priority, enabled, snooze_count,
                  next_fire_at, created_at, updated_at
           FROM reminders ORDER BY created_at DESC"#,
    )
    .fetch_all(conn)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Reminder {
            id: r.0,
            title: r.1,
            trigger_type: r.2,
            trigger_spec: r.3,
            priority: r.4,
            enabled: r.5 != 0,
            snooze_count: r.6 as u32,
            next_fire_at: r.7,
            created_at: r.8,
            updated_at: r.9,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_daily_at() {
        let s = parse_daily_spec("09:30").unwrap();
        assert!(matches!(s, DailySpec::DailyAt { hour: 9, minute: 30 }));
    }

    #[test]
    fn parse_daily_every_minutes() {
        let s = parse_daily_spec("*/30 * *").unwrap();
        assert!(matches!(s, DailySpec::EveryMinutes(30)));
    }

    #[test]
    fn parse_daily_bad_hour_rejected() {
        assert!(parse_daily_spec("24:00").is_err());
        assert!(parse_daily_spec("12:60").is_err());
        assert!(parse_daily_spec("abc").is_err());
    }

    #[test]
    fn parse_daily_every_n_bounds() {
        assert!(parse_daily_spec("*/0 * *").is_err());
        assert!(parse_daily_spec("*/1441 * *").is_err());
        assert!(parse_daily_spec("*/1 * *").is_ok());
        assert!(parse_daily_spec("*/1440 * *").is_ok());
    }

    #[test]
    fn validate_trigger_rejects_unsupported() {
        assert!(validate_trigger("weekly", "MON 09:00").is_err());
        assert!(validate_trigger("cron", "0 9 * * *").is_err());
        assert!(validate_trigger("garbage", "x").is_err());
    }

    #[test]
    fn validate_trigger_accepts_once_and_daily() {
        let future_iso = (Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        assert!(validate_trigger("once", &future_iso).is_ok());
        assert!(validate_trigger("daily", "09:00").is_ok());
        assert!(validate_trigger("daily", "*/30 * *").is_ok());
    }

    #[test]
    fn compute_next_for_once_in_future() {
        let now = Utc::now();
        let future = (now + chrono::Duration::hours(2)).to_rfc3339();
        let next = compute_next_fire_at("once", &future, now).unwrap();
        assert!(next.is_some());
        assert!(next.unwrap() > now);
    }

    #[test]
    fn compute_next_for_once_in_past_returns_none() {
        let now = Utc::now();
        let past = (now - chrono::Duration::hours(2)).to_rfc3339();
        let next = compute_next_fire_at("once", &past, now).unwrap();
        assert!(next.is_none());
    }

    #[test]
    fn compute_next_for_daily_every_minutes_adds_n() {
        let now = Utc::now();
        let next = compute_next_fire_at("daily", "*/30 * *", now).unwrap().unwrap();
        let delta = next - now;
        assert!(delta.num_minutes() >= 29 && delta.num_minutes() <= 30);
    }

    #[test]
    fn compute_next_for_daily_at_hourminute_skips_to_tomorrow_if_past() {
        let now = Utc::now();
        // 选一个肯定已经过去的时间（now - 1h），daily HH:MM 应跳到明天同时刻
        let past = now - chrono::Duration::hours(1);
        let spec = past.format("%H:%M").to_string();
        let next = compute_next_fire_at("daily", &spec, now).unwrap().unwrap();
        assert!(next > now);
        // 距离至少 22h（即明天附近的同时刻）
        let delta = next - now;
        assert!(delta.num_minutes() >= 22 * 60);
    }
}
