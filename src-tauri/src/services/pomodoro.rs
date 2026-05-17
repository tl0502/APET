//! PomodoroService（#28，模块 D）— 番茄钟 CRUD + 状态机 + drift 校准 + 三方协作 hook。
//!
//! 范围（M2，2026-05-17）:
//! - 6 IPC: start / pause / resume / stop / active / today_stats（active 给前端启动期拉一次）
//! - 运行时态走 KV `pomodoro:active_session`（JSON），不污染 pomodoro_sessions 表（lesson #2：
//!   schema 无 paused_at/accumulated_pause，规避字段缺失）
//! - 终态写表：stop / 自动 REST 结束 / 硬提醒打断 时插一行
//!   （id/focus_min/rest_min/status/started_at/ended_at），完全对齐既有 schema
//! - 默认配置 KV `pomodoro:default_config`：用户带参 start 后记忆，下次不带参用上次
//! - drift 校准：每个 tick check `now - last_tick > 10s` 视为系统休眠，phase_planned_end 后推
//!   sleep_ms（休眠时长不计入番茄进度，PRD §7.4）。1s tick + 9s 容差。
//!
//! 状态机:
//!   IDLE → FOCUS → REST → IDLE（auto 转换 + drift 补偿）
//!   FOCUS ⇄ PAUSED_F；REST ⇄ PAUSED_R（手动暂停 / 继续）
//!   stop 任何运行态 → IDLE：在 FOCUS 段按 effective_elapsed/focus_total 30% 阈值判
//!   completed/cancelled；在 REST 段必 completed。
//!
//! 协作 hook（commit 3 接入）:
//! - `is_focus_active(app) -> bool`：reminder.fire / living_pet.wander 入口前置检查
//! - `push_soft_buffer(app, payload)`：FOCUS 期软提醒入 KV；REST 启动时 flush
//! - `cancel_by_hard_interrupt(app, reminder_id)`：FOCUS 期硬提醒立即终止番茄
//!
//! 事务策略:
//! - tick（scheduler 1s spawn task）与 IPC（pause/resume/stop）可能并发 → tick 内部
//!   load+save 包在 conn.begin() 事务里；IPC 自身互斥（Tauri command 串行入口）所以不上事务。
//! - 这样 tick 不会覆盖 IPC 的中间状态（如 pause 写 PAUSED_F 后 tick load 仍看到 Focus 旧值）。

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Connection, SqliteConnection};
use tauri::{AppHandle, Emitter, Runtime};
use thiserror::Error;
use ulid::Ulid;

use crate::services::config;
use crate::services::db::{open_app_db, DbError};

// ============================================================================
// 常量
// ============================================================================

const KEY_ACTIVE: &str = "pomodoro:active_session";
const KEY_DEFAULT_CONFIG: &str = "pomodoro:default_config";
/// FOCUS 期软提醒缓冲；reminder.rs commit 3 写、本模块 transition_auto 读。
pub const KEY_SOFT_BUFFER: &str = "pomodoro:focus_soft_buffer";

const EVENT_TICK: &str = "pomodoro:tick";
const EVENT_STATE_CHANGED: &str = "pomodoro:state_changed";
const EVENT_FOCUS_STARTED: &str = "pomodoro:focus_started";
const EVENT_FOCUS_ENDED: &str = "pomodoro:focus_ended";
const EVENT_REST_STARTED: &str = "pomodoro:rest_started";
const EVENT_REST_ENDED: &str = "pomodoro:rest_ended";
/// Reminder 模块 commit 3 在 ReminderPanel 端 listen 此事件。
const EVENT_BUFFER_FLUSH: &str = "reminder:buffer_flush";

pub const MIN_FOCUS_MIN: u32 = 5;
pub const MAX_FOCUS_MIN: u32 = 90;
pub const MIN_REST_MIN: u32 = 1;
pub const MAX_REST_MIN: u32 = 30;
pub const DEFAULT_FOCUS_MIN: u32 = 25;
pub const DEFAULT_REST_MIN: u32 = 5;
pub const MIN_COMPLETE_RATIO: f64 = 0.30;

const TICK_INTERVAL_SEC: i64 = 1;
const SLEEP_THRESHOLD_SEC: i64 = 10;

// ============================================================================
// Error
// ============================================================================

#[derive(Debug, Error)]
pub enum PomodoroError {
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
    #[error("no active pomodoro session")]
    NoActive,
    #[error("session already active (phase={0})")]
    AlreadyActive(String),
    #[error("invalid focus_min: {0} (must be 5..=90)")]
    InvalidFocusMin(u32),
    #[error("invalid rest_min: {0} (must be 1..=30)")]
    InvalidRestMin(u32),
    #[error("invalid phase transition: from {0} to {1}")]
    InvalidPhaseTransition(String, String),
    #[error("kv serialize: {0}")]
    Serialize(String),
}

impl From<sqlx::Error> for PomodoroError {
    fn from(e: sqlx::Error) -> Self {
        PomodoroError::Database(e.to_string())
    }
}

impl From<DbError> for PomodoroError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => PomodoroError::AppConfigDir(s),
            DbError::Database(s) => PomodoroError::Database(s),
        }
    }
}

impl From<config::ConfigError> for PomodoroError {
    fn from(e: config::ConfigError) -> Self {
        match e {
            config::ConfigError::AppConfigDir(s) => PomodoroError::AppConfigDir(s),
            config::ConfigError::Database(s) => PomodoroError::Database(s),
        }
    }
}

impl From<serde_json::Error> for PomodoroError {
    fn from(e: serde_json::Error) -> Self {
        PomodoroError::Serialize(e.to_string())
    }
}

// ============================================================================
// 类型
// ============================================================================

/// 番茄当前 phase。IDLE 不存 KV（无 active session 等价 IDLE）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Phase {
    Focus,
    PausedF,
    Rest,
    PausedR,
}

impl Phase {
    fn is_paused(&self) -> bool {
        matches!(self, Phase::PausedF | Phase::PausedR)
    }
    /// FOCUS / PAUSED_F 都算"FOCUS 段"，参与 30% 阈值与 reminder buffer。
    fn is_focus_like(&self) -> bool {
        matches!(self, Phase::Focus | Phase::PausedF)
    }
    fn as_str(&self) -> &'static str {
        match self {
            Phase::Focus => "FOCUS",
            Phase::PausedF => "PAUSED_F",
            Phase::Rest => "REST",
            Phase::PausedR => "PAUSED_R",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSession {
    pub session_id: String,
    pub phase: Phase,
    pub focus_min: u32,
    pub rest_min: u32,
    /// FOCUS 第一次启动时刻；写 pomodoro_sessions.started_at 用。
    pub session_started_at: String,
    /// 当前 phase 起点（pause→resume 不变；FOCUS→REST 时刷新）。
    pub phase_started_at: String,
    /// 主时间戳：phase 计划结束。pause/drift 补偿都后推此值。
    pub phase_planned_end: String,
    pub last_tick_at: String,
    pub pause_started_at: Option<String>,
    /// 本 phase 累计暂停毫秒（FOCUS→REST 时清零）。
    pub accumulated_pause_ms: i64,
    /// 本 phase 累计 drift 补偿毫秒（FOCUS→REST 时清零）。
    pub sleep_compensation_ms: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartInput {
    pub focus_min: Option<u32>,
    pub rest_min: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultConfig {
    pub focus_min: u32,
    pub rest_min: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopOutput {
    pub status: String, // 'completed' | 'cancelled'
    pub completion_ratio: f64,
    pub focus_min_actual: f64,
    pub session_id: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayStats {
    pub completed: u32,
    pub cancelled: u32,
    pub total_focus_min: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TickPayload {
    pub session_id: String,
    pub phase: Phase,
    pub remaining_ms: i64,
    pub focus_min: u32,
    pub rest_min: u32,
}

// ============================================================================
// 公共 IPC: start / pause / resume / stop / active / today_stats
// ============================================================================

pub async fn start<R: Runtime>(
    app: &AppHandle<R>,
    input: StartInput,
) -> Result<ActiveSession, PomodoroError> {
    if let Some(existing) = load_active(app).await? {
        return Err(PomodoroError::AlreadyActive(existing.phase.as_str().into()));
    }
    let cfg = resolve_default_config(app).await?;
    let focus_min = input.focus_min.unwrap_or(cfg.focus_min);
    let rest_min = input.rest_min.unwrap_or(cfg.rest_min);
    validate_focus_min(focus_min)?;
    validate_rest_min(rest_min)?;

    // 仅当用户带参时记忆为默认（不带参 = 沿用记忆，无需回写）。
    if input.focus_min.is_some() || input.rest_min.is_some() {
        save_default_config(app, &DefaultConfig { focus_min, rest_min }).await?;
    }

    let now = Utc::now();
    let session_id = Ulid::new().to_string();
    let phase_planned_end = now + Duration::minutes(focus_min as i64);
    let session = ActiveSession {
        session_id: session_id.clone(),
        phase: Phase::Focus,
        focus_min,
        rest_min,
        session_started_at: now.to_rfc3339(),
        phase_started_at: now.to_rfc3339(),
        phase_planned_end: phase_planned_end.to_rfc3339(),
        last_tick_at: now.to_rfc3339(),
        pause_started_at: None,
        accumulated_pause_ms: 0,
        sleep_compensation_ms: 0,
    };
    save_active(app, &session).await?;

    emit_state_changed(app, Some(session.phase));
    let _ = app.emit(
        EVENT_FOCUS_STARTED,
        serde_json::json!({ "sessionId": session.session_id, "focusMin": focus_min }),
    );
    Ok(session)
}

pub async fn pause<R: Runtime>(app: &AppHandle<R>) -> Result<ActiveSession, PomodoroError> {
    let mut s = load_active(app).await?.ok_or(PomodoroError::NoActive)?;
    let now = Utc::now();
    let new_phase = match s.phase {
        Phase::Focus => Phase::PausedF,
        Phase::Rest => Phase::PausedR,
        _ => {
            return Err(PomodoroError::InvalidPhaseTransition(
                s.phase.as_str().into(),
                "paused".into(),
            ))
        }
    };
    s.phase = new_phase;
    s.pause_started_at = Some(now.to_rfc3339());
    save_active(app, &s).await?;
    emit_state_changed(app, Some(s.phase));
    Ok(s)
}

pub async fn resume<R: Runtime>(app: &AppHandle<R>) -> Result<ActiveSession, PomodoroError> {
    let mut s = load_active(app).await?.ok_or(PomodoroError::NoActive)?;
    let now = Utc::now();
    let (new_phase, pause_started_str) = match (s.phase, s.pause_started_at.clone()) {
        (Phase::PausedF, Some(ps)) => (Phase::Focus, ps),
        (Phase::PausedR, Some(ps)) => (Phase::Rest, ps),
        _ => {
            return Err(PomodoroError::InvalidPhaseTransition(
                s.phase.as_str().into(),
                "resumed".into(),
            ))
        }
    };
    let pause_start = parse_rfc3339(&pause_started_str)?;
    let pause_ms = (now - pause_start).num_milliseconds().max(0);
    s.accumulated_pause_ms += pause_ms;
    // 关键：phase_planned_end 后推暂停时长，倒计时延续。
    let planned_end = parse_rfc3339(&s.phase_planned_end)?;
    s.phase_planned_end = (planned_end + Duration::milliseconds(pause_ms)).to_rfc3339();
    s.phase = new_phase;
    s.pause_started_at = None;
    s.last_tick_at = now.to_rfc3339();
    save_active(app, &s).await?;
    emit_state_changed(app, Some(s.phase));
    Ok(s)
}

pub async fn stop<R: Runtime>(app: &AppHandle<R>) -> Result<StopOutput, PomodoroError> {
    let s = load_active(app).await?.ok_or(PomodoroError::NoActive)?;
    let now = Utc::now();
    let (status, completion_ratio, focus_min_actual) = compute_completion(&s, now)?;

    let mut conn = open_app_db(app).await?;
    insert_session_row(&mut conn, &s, &status, &now.to_rfc3339()).await?;
    conn.close().await?;

    delete_active(app).await?;
    let _ = config::delete(app, KEY_SOFT_BUFFER).await; // best-effort 清 buffer

    // 按 phase 分别 emit 终态事件（FOCUS 段 stop → focus_ended；REST 段 stop → rest_ended）
    match s.phase {
        Phase::Focus | Phase::PausedF => {
            let _ = app.emit(
                EVENT_FOCUS_ENDED,
                serde_json::json!({
                    "sessionId": s.session_id,
                    "completed": status == "completed",
                }),
            );
        }
        Phase::Rest | Phase::PausedR => {
            let _ = app.emit(
                EVENT_REST_ENDED,
                serde_json::json!({ "sessionId": s.session_id }),
            );
        }
    }
    emit_state_changed(app, None);

    Ok(StopOutput {
        status,
        completion_ratio,
        focus_min_actual,
        session_id: s.session_id,
    })
}

pub async fn active<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<ActiveSession>, PomodoroError> {
    load_active(app).await
}

pub async fn today_stats<R: Runtime>(app: &AppHandle<R>) -> Result<TodayStats, PomodoroError> {
    let today_start = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| PomodoroError::Database("today_start construct failed".into()))?;
    let cutoff = DateTime::<Utc>::from_naive_utc_and_offset(today_start, Utc).to_rfc3339();

    let mut conn = open_app_db(app).await?;
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT status, focus_min FROM pomodoro_sessions
           WHERE started_at >= ?"#,
    )
    .bind(&cutoff)
    .fetch_all(&mut conn)
    .await?;
    conn.close().await?;

    let mut stats = TodayStats::default();
    for (status, focus_min) in rows {
        match status.as_str() {
            "completed" => {
                stats.completed += 1;
                stats.total_focus_min += focus_min as u32;
            }
            "cancelled" => stats.cancelled += 1,
            _ => {}
        }
    }
    Ok(stats)
}

// ============================================================================
// Scheduler tick — drift 校准 + 自动转换
// ============================================================================

/// scheduler 每秒调一次。事务包裹 load→drift→save 防 IPC 竞态覆盖。
pub(crate) async fn tick<R: Runtime>(app: &AppHandle<R>) -> Result<(), PomodoroError> {
    let mut conn = open_app_db(app).await?;
    let mut tx = conn.begin().await?;

    let raw = config::get_with_conn(&mut tx, KEY_ACTIVE).await?;
    let Some(json) = raw else {
        tx.rollback().await?;
        conn.close().await?;
        return Ok(());
    };
    let mut s: ActiveSession = serde_json::from_str(&json)?;
    if s.phase.is_paused() {
        // PAUSED 期间不更新 last_tick_at（resume 时会重置），不消费 tick
        tx.rollback().await?;
        conn.close().await?;
        return Ok(());
    }

    let now = Utc::now();
    let last = parse_rfc3339(&s.last_tick_at)?;
    let delta = now - last;

    // drift 校准：delta > 10s 视为系统休眠/进程暂停。把 phase_planned_end 后推（休眠时长不计入）。
    if delta.num_seconds() > SLEEP_THRESHOLD_SEC {
        let sleep_ms = (delta - Duration::seconds(TICK_INTERVAL_SEC))
            .num_milliseconds()
            .max(0);
        let planned_end = parse_rfc3339(&s.phase_planned_end)?;
        s.phase_planned_end = (planned_end + Duration::milliseconds(sleep_ms)).to_rfc3339();
        s.sleep_compensation_ms += sleep_ms;
    }
    s.last_tick_at = now.to_rfc3339();

    let planned_end = parse_rfc3339(&s.phase_planned_end)?;
    let remaining = planned_end - now;
    let transition_needed = remaining.num_milliseconds() <= 0;

    // 先把 drift 校准 + last_tick_at 提交（用 with_conn 复用事务）
    let serialized = serde_json::to_string(&s)?;
    config::set_with_conn(&mut tx, KEY_ACTIVE, &serialized, &now.to_rfc3339()).await?;
    tx.commit().await?;
    conn.close().await?;

    if transition_needed {
        transition_auto(app, &mut s, now).await?;
        // transition_auto FOCUS→REST 后 s.phase=Rest active 仍在；REST→IDLE 后 active 已删
        if s.phase == Phase::Rest {
            emit_tick(app, &s, now);
        }
    } else {
        emit_tick(app, &s, now);
    }
    Ok(())
}

fn emit_tick<R: Runtime>(app: &AppHandle<R>, s: &ActiveSession, now: DateTime<Utc>) {
    let remaining = match parse_rfc3339(&s.phase_planned_end) {
        Ok(pe) => (pe - now).num_milliseconds().max(0),
        Err(_) => 0,
    };
    let _ = app.emit(
        EVENT_TICK,
        TickPayload {
            session_id: s.session_id.clone(),
            phase: s.phase,
            remaining_ms: remaining,
            focus_min: s.focus_min,
            rest_min: s.rest_min,
        },
    );
}

/// FOCUS → REST：切 phase + 清 pause/drift 累积 + flush soft buffer。
/// REST → IDLE：写表 + 清 active KV。
async fn transition_auto<R: Runtime>(
    app: &AppHandle<R>,
    s: &mut ActiveSession,
    now: DateTime<Utc>,
) -> Result<(), PomodoroError> {
    match s.phase {
        Phase::Focus => {
            s.phase = Phase::Rest;
            s.phase_started_at = now.to_rfc3339();
            s.phase_planned_end = (now + Duration::minutes(s.rest_min as i64)).to_rfc3339();
            s.last_tick_at = now.to_rfc3339();
            s.pause_started_at = None;
            s.accumulated_pause_ms = 0;
            s.sleep_compensation_ms = 0;
            save_active(app, s).await?;
            emit_state_changed(app, Some(s.phase));
            let _ = app.emit(
                EVENT_FOCUS_ENDED,
                serde_json::json!({ "sessionId": s.session_id, "completed": true }),
            );
            let _ = app.emit(
                EVENT_REST_STARTED,
                serde_json::json!({ "sessionId": s.session_id, "restMin": s.rest_min }),
            );
            flush_soft_buffer(app).await?;
        }
        Phase::Rest => {
            let mut conn = open_app_db(app).await?;
            insert_session_row(&mut conn, s, "completed", &now.to_rfc3339()).await?;
            conn.close().await?;
            delete_active(app).await?;
            let _ = app.emit(
                EVENT_REST_ENDED,
                serde_json::json!({ "sessionId": s.session_id }),
            );
            emit_state_changed(app, None);
        }
        _ => {}
    }
    Ok(())
}

async fn flush_soft_buffer<R: Runtime>(app: &AppHandle<R>) -> Result<(), PomodoroError> {
    if let Some(json) = config::get(app, KEY_SOFT_BUFFER).await? {
        if let Ok(items) = serde_json::from_str::<serde_json::Value>(&json) {
            if items.as_array().map(|a| !a.is_empty()).unwrap_or(false) {
                let _ = app.emit(EVENT_BUFFER_FLUSH, &items);
            }
        }
        let _ = config::delete(app, KEY_SOFT_BUFFER).await;
    }
    Ok(())
}

// ============================================================================
// 协作 hook（reminder.rs / living_pet.rs 调用 — commit 3 接入）
// ============================================================================

pub async fn is_focus_active<R: Runtime>(app: &AppHandle<R>) -> bool {
    match load_active(app).await {
        Ok(Some(s)) => s.phase.is_focus_like(),
        _ => false,
    }
}

/// FOCUS 期软提醒入 buffer（reminder.rs::fire 调用）；REST 启动时 flush 合并。
pub async fn push_soft_buffer<R: Runtime>(
    app: &AppHandle<R>,
    payload: serde_json::Value,
) -> Result<(), PomodoroError> {
    let raw = config::get(app, KEY_SOFT_BUFFER)
        .await?
        .unwrap_or_else(|| "[]".to_string());
    let mut arr: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap_or_default();
    arr.push(payload);
    let serialized = serde_json::to_string(&arr)?;
    config::set(app, KEY_SOFT_BUFFER, &serialized).await?;
    Ok(())
}

/// FOCUS 期硬提醒打断：写 cancelled 行 + 清 active + emit focus_ended {interruptedBy}。
/// reminder.fire 后续仍走正常 emit fired + OS notification（让用户看到提醒）。
pub async fn cancel_by_hard_interrupt<R: Runtime>(
    app: &AppHandle<R>,
    reminder_id: &str,
) -> Result<(), PomodoroError> {
    let Some(s) = load_active(app).await? else {
        return Ok(());
    };
    if !s.phase.is_focus_like() {
        return Ok(());
    }
    let now = Utc::now();
    let mut conn = open_app_db(app).await?;
    insert_session_row(&mut conn, &s, "cancelled", &now.to_rfc3339()).await?;
    conn.close().await?;
    delete_active(app).await?;
    let _ = config::delete(app, KEY_SOFT_BUFFER).await;
    let _ = app.emit(
        EVENT_FOCUS_ENDED,
        serde_json::json!({
            "sessionId": s.session_id,
            "completed": false,
            "interruptedBy": reminder_id,
        }),
    );
    emit_state_changed(app, None);
    Ok(())
}

// ============================================================================
// 启动期清残留（lib.rs::setup 调用）
// ============================================================================

pub async fn discard_orphan_active<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(), PomodoroError> {
    if config::get(app, KEY_ACTIVE).await?.is_some() {
        let _ = config::delete(app, KEY_ACTIVE).await;
        eprintln!("[pomodoro] discarded orphan active_session on startup");
    }
    let _ = config::delete(app, KEY_SOFT_BUFFER).await;
    Ok(())
}

// ============================================================================
// Helpers — parse / validate / KV r/w / SQL inner / compute / emit
// ============================================================================

fn parse_rfc3339(s: &str) -> Result<DateTime<Utc>, PomodoroError> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| PomodoroError::Database(format!("parse rfc3339 '{s}': {e}")))
}

fn validate_focus_min(n: u32) -> Result<(), PomodoroError> {
    if (MIN_FOCUS_MIN..=MAX_FOCUS_MIN).contains(&n) {
        Ok(())
    } else {
        Err(PomodoroError::InvalidFocusMin(n))
    }
}

fn validate_rest_min(n: u32) -> Result<(), PomodoroError> {
    if (MIN_REST_MIN..=MAX_REST_MIN).contains(&n) {
        Ok(())
    } else {
        Err(PomodoroError::InvalidRestMin(n))
    }
}

async fn resolve_default_config<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<DefaultConfig, PomodoroError> {
    if let Some(raw) = config::get(app, KEY_DEFAULT_CONFIG).await? {
        if let Ok(cfg) = serde_json::from_str::<DefaultConfig>(&raw) {
            // 反序列化后还要走 validate（KV 可能被人工改坏 / schema 变化）
            if validate_focus_min(cfg.focus_min).is_ok() && validate_rest_min(cfg.rest_min).is_ok()
            {
                return Ok(cfg);
            }
            eprintln!("[pomodoro] KV default_config out of range, fallback to 25/5: {raw}");
        }
    }
    Ok(DefaultConfig {
        focus_min: DEFAULT_FOCUS_MIN,
        rest_min: DEFAULT_REST_MIN,
    })
}

async fn save_default_config<R: Runtime>(
    app: &AppHandle<R>,
    cfg: &DefaultConfig,
) -> Result<(), PomodoroError> {
    let s = serde_json::to_string(cfg)?;
    config::set(app, KEY_DEFAULT_CONFIG, &s).await?;
    Ok(())
}

async fn load_active<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<ActiveSession>, PomodoroError> {
    match config::get(app, KEY_ACTIVE).await? {
        None => Ok(None),
        Some(raw) => {
            let s: ActiveSession = serde_json::from_str(&raw)?;
            Ok(Some(s))
        }
    }
}

async fn save_active<R: Runtime>(
    app: &AppHandle<R>,
    s: &ActiveSession,
) -> Result<(), PomodoroError> {
    let json = serde_json::to_string(s)?;
    config::set(app, KEY_ACTIVE, &json).await?;
    Ok(())
}

async fn delete_active<R: Runtime>(app: &AppHandle<R>) -> Result<(), PomodoroError> {
    config::delete(app, KEY_ACTIVE).await?;
    Ok(())
}

async fn insert_session_row(
    conn: &mut SqliteConnection,
    s: &ActiveSession,
    status: &str,
    ended_at: &str,
) -> Result<(), PomodoroError> {
    sqlx::query(
        r#"INSERT INTO pomodoro_sessions (id, focus_min, rest_min, status, started_at, ended_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&s.session_id)
    .bind(s.focus_min as i64)
    .bind(s.rest_min as i64)
    .bind(status)
    .bind(&s.session_started_at)
    .bind(ended_at)
    .execute(conn)
    .await?;
    Ok(())
}

/// stop 时算完成比例 + 状态。
/// - FOCUS / PAUSED_F: 按 effective_elapsed/focus_total 30% 阈值判 completed/cancelled
/// - REST / PAUSED_R: 必 completed（FOCUS 已完成）
fn compute_completion(
    s: &ActiveSession,
    now: DateTime<Utc>,
) -> Result<(String, f64, f64), PomodoroError> {
    if !s.phase.is_focus_like() {
        return Ok(("completed".into(), 1.0, s.focus_min as f64));
    }
    let focus_total_ms = s.focus_min as i64 * 60_000;
    let phase_start = parse_rfc3339(&s.phase_started_at)?;
    let raw_elapsed = (now - phase_start).num_milliseconds();
    // 在 PAUSED_F 内 stop 时，当前未结算的 pause 时长也要扣
    let current_pause_ms = match s.pause_started_at.as_ref() {
        Some(ps) => (now - parse_rfc3339(ps)?).num_milliseconds().max(0),
        None => 0,
    };
    let effective =
        (raw_elapsed - s.accumulated_pause_ms - s.sleep_compensation_ms - current_pause_ms).max(0);
    let ratio = effective as f64 / focus_total_ms as f64;
    let focus_min_actual = effective as f64 / 60_000.0;
    let status = if ratio >= MIN_COMPLETE_RATIO {
        "completed"
    } else {
        "cancelled"
    };
    Ok((status.into(), ratio, focus_min_actual))
}

fn emit_state_changed<R: Runtime>(app: &AppHandle<R>, phase: Option<Phase>) {
    let payload = match phase {
        Some(p) => serde_json::json!({ "phase": p }),
        None => serde_json::json!({ "phase": serde_json::Value::Null }),
    };
    let _ = app.emit(EVENT_STATE_CHANGED, payload);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_active(focus_min: u32, rest_min: u32) -> ActiveSession {
        let now = Utc::now();
        ActiveSession {
            session_id: "01TEST".into(),
            phase: Phase::Focus,
            focus_min,
            rest_min,
            session_started_at: now.to_rfc3339(),
            phase_started_at: now.to_rfc3339(),
            phase_planned_end: (now + Duration::minutes(focus_min as i64)).to_rfc3339(),
            last_tick_at: now.to_rfc3339(),
            pause_started_at: None,
            accumulated_pause_ms: 0,
            sleep_compensation_ms: 0,
        }
    }

    #[test]
    fn validates_focus_range_5_90() {
        assert!(validate_focus_min(4).is_err());
        assert!(validate_focus_min(5).is_ok());
        assert!(validate_focus_min(90).is_ok());
        assert!(validate_focus_min(91).is_err());
    }

    #[test]
    fn validates_rest_range_1_30() {
        assert!(validate_rest_min(0).is_err());
        assert!(validate_rest_min(1).is_ok());
        assert!(validate_rest_min(30).is_ok());
        assert!(validate_rest_min(31).is_err());
    }

    #[test]
    fn stop_below_30_percent_marks_cancelled() {
        let mut s = make_active(25, 5);
        // mock: phase_started_at 在 4 分钟前 (16%)
        s.phase_started_at = (Utc::now() - Duration::minutes(4)).to_rfc3339();
        let (status, ratio, focus_min_actual) = compute_completion(&s, Utc::now()).unwrap();
        assert_eq!(status, "cancelled");
        assert!(ratio < MIN_COMPLETE_RATIO);
        assert!((3.9..=4.1).contains(&focus_min_actual));
    }

    #[test]
    fn stop_at_30_percent_marks_completed() {
        let mut s = make_active(25, 5);
        // mock: phase_started_at 在 8 分钟前 (32%)
        s.phase_started_at = (Utc::now() - Duration::minutes(8)).to_rfc3339();
        let (status, ratio, _) = compute_completion(&s, Utc::now()).unwrap();
        assert_eq!(status, "completed");
        assert!(ratio >= MIN_COMPLETE_RATIO);
    }

    #[test]
    fn stop_in_rest_marks_completed_regardless() {
        let mut s = make_active(25, 5);
        s.phase = Phase::Rest;
        s.phase_started_at = (Utc::now() - Duration::seconds(10)).to_rfc3339();
        let (status, _, focus_min_actual) = compute_completion(&s, Utc::now()).unwrap();
        assert_eq!(status, "completed");
        assert_eq!(focus_min_actual, 25.0);
    }

    #[test]
    fn stop_in_paused_f_accounts_current_pause_in_effective() {
        let mut s = make_active(25, 5);
        s.phase = Phase::PausedF;
        // 9 分钟前开始,1 分钟前暂停 → effective ≈ 8 分钟,ratio=32% completed
        s.phase_started_at = (Utc::now() - Duration::minutes(9)).to_rfc3339();
        s.pause_started_at = Some((Utc::now() - Duration::minutes(1)).to_rfc3339());
        let (status, ratio, _) = compute_completion(&s, Utc::now()).unwrap();
        assert_eq!(status, "completed");
        assert!(ratio >= 0.30 && ratio < 0.34);
    }

    #[test]
    fn stop_subtracts_accumulated_pause_ms() {
        let mut s = make_active(25, 5);
        // 10 分钟前开始 → raw 10 min；accumulated 3 min → effective 7 min → 28% cancelled
        s.phase_started_at = (Utc::now() - Duration::minutes(10)).to_rfc3339();
        s.accumulated_pause_ms = 3 * 60 * 1000;
        let (status, ratio, _) = compute_completion(&s, Utc::now()).unwrap();
        assert_eq!(status, "cancelled");
        assert!(ratio < MIN_COMPLETE_RATIO);
    }

    #[test]
    fn stop_subtracts_sleep_compensation_ms() {
        let mut s = make_active(25, 5);
        // 12 分钟前开始,4 分钟休眠 → effective 8 min → 32% completed
        s.phase_started_at = (Utc::now() - Duration::minutes(12)).to_rfc3339();
        s.sleep_compensation_ms = 4 * 60 * 1000;
        let (status, ratio, _) = compute_completion(&s, Utc::now()).unwrap();
        assert_eq!(status, "completed");
        assert!(ratio >= 0.30);
    }

    /// drift 公式核心校验：last_tick=now-30s 应推 phase_planned_end ~29s（即 delta-1s）。
    /// 这里直接验证算式（绕过 app+DB tick）。
    #[test]
    fn drift_formula_within_30s_pushes_29s() {
        let now = Utc::now();
        let last = now - Duration::seconds(30);
        let delta = now - last;
        assert!(delta.num_seconds() > SLEEP_THRESHOLD_SEC);
        let sleep_ms = (delta - Duration::seconds(TICK_INTERVAL_SEC))
            .num_milliseconds()
            .max(0);
        assert!(sleep_ms >= 28_500 && sleep_ms <= 29_500);
    }

    #[test]
    fn drift_formula_within_threshold_does_not_trigger() {
        let now = Utc::now();
        let last = now - Duration::seconds(5);
        let delta = now - last;
        assert!(delta.num_seconds() <= SLEEP_THRESHOLD_SEC);
    }

    #[test]
    fn phase_serializes_screaming_snake() {
        assert_eq!(serde_json::to_string(&Phase::Focus).unwrap(), r#""FOCUS""#);
        assert_eq!(
            serde_json::to_string(&Phase::PausedF).unwrap(),
            r#""PAUSED_F""#
        );
        assert_eq!(serde_json::to_string(&Phase::Rest).unwrap(), r#""REST""#);
        assert_eq!(
            serde_json::to_string(&Phase::PausedR).unwrap(),
            r#""PAUSED_R""#
        );
    }

    #[test]
    fn phase_classification_helpers() {
        assert!(Phase::Focus.is_focus_like());
        assert!(Phase::PausedF.is_focus_like());
        assert!(!Phase::Rest.is_focus_like());
        assert!(!Phase::PausedR.is_focus_like());
        assert!(Phase::PausedF.is_paused());
        assert!(Phase::PausedR.is_paused());
        assert!(!Phase::Focus.is_paused());
        assert!(!Phase::Rest.is_paused());
    }

    #[test]
    fn default_config_serializes_camel_case() {
        let cfg = DefaultConfig {
            focus_min: 30,
            rest_min: 6,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"focusMin\":30"));
        assert!(json.contains("\"restMin\":6"));
    }
}
