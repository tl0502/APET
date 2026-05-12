// 桌宠窗口位置持久化 + 多屏还原（#10）
//
// 存储：sqlite `config` 表 KV，key=`CONFIG_KEY_PET_POSITION`，value=JSON
//   注：issue #10 字面是 `pet_runtime_state.last_position` 字段，但项目 schema 走"27 表
//   一次建零迁移"原则（D5），pet_runtime_state 实际未保留 last_position 列。改用现有
//   config 表（"运行时配置走此表"，与 active_conversation_id 同表）。语义零损失。
//
// 还原策略（lib.rs setup 阶段调 apply_initial_position）：
// - 有 last_position + monitor 在场 → 还原 (logical_x, logical_y)，按目标 monitor 逻辑边界
//   裁剪到 16px 安全边距内
// - 副屏被拔了 / 没记录 → fallback 主屏右下角偏左 80px（PRD §7.1 默认值）
//
// 写入策略（lib.rs on_window_event Moved 调 save_debounced）：
// - WindowEvent::Moved 高频触发（每像素一次），用 200ms debounce 节流
// - 持有 Mutex<Option<JoinHandle>>：每次新事件 abort 上次未触达的 spawn，再新 spawn
// - 偏离 issue body "pointerup 200ms 防抖 + Moved 双保险"：startDragging 系统级拖动
//   不会冒泡 pointerup 到 webview（OS 接管 mouse），单 Moved + debounce 已足够可靠

use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tauri::{
    AppHandle, LogicalPosition, Manager, Monitor, PhysicalPosition, Runtime, WebviewWindow,
};

use crate::services::config;
use crate::services::window_actions::PET_WINDOW_LABEL;

/// `config` 表 key：桌宠窗口最后位置（JSON 序列化的 LastPosition）
pub const CONFIG_KEY_PET_POSITION: &str = "window:pet:last_position";

const PET_WINDOW_LOGICAL_W: f64 = 320.0;
const PET_WINDOW_LOGICAL_H: f64 = 320.0;
/// 公开供 living_pet 等模块复用,确保 pet 窗口尺寸单一真相源（避免 320 magic number 漂移）。
pub const PET_LOGICAL_W: f64 = PET_WINDOW_LOGICAL_W;
pub const PET_LOGICAL_H: f64 = PET_WINDOW_LOGICAL_H;
/// 主屏右下角偏左/偏上 80px（PRD §7.1 首次启动默认）
const DEFAULT_OFFSET_FROM_BOTTOM_RIGHT: f64 = 80.0;
/// 边界裁剪安全边距（让窗口任意一边都距 monitor 边 16px）
const SAFE_MARGIN: f64 = 16.0;
/// Moved 防抖时长
const SAVE_DEBOUNCE_MS: u64 = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastPosition {
    pub monitor_id: String,
    pub logical_x: f64,
    pub logical_y: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum WindowStateError {
    #[error("config service error: {0}")]
    Config(#[from] config::ConfigError),
    #[error("json (de)serialize: {0}")]
    Json(#[from] serde_json::Error),
}

/// monitor 标识：优先 name() 非空；否则用 position() 作 fallback id（"x,y"）。
fn monitor_id(monitor: &Monitor) -> String {
    if let Some(name) = monitor.name() {
        if !name.is_empty() {
            return name.to_string();
        }
    }
    let pos: &PhysicalPosition<i32> = monitor.position();
    format!("@{},{}", pos.x, pos.y)
}

/// 直接以给定 LastPosition 写 config（IPC 入口给前端用）。
pub async fn set_pet_position<R: Runtime>(
    app: &AppHandle<R>,
    pos: &LastPosition,
) -> Result<(), WindowStateError> {
    let serialized = serde_json::to_string(pos)?;
    config::set(app, CONFIG_KEY_PET_POSITION, &serialized).await?;
    Ok(())
}

/// 从 webview 当前 outer_position + current_monitor 推导 LastPosition（不写 DB）。
pub fn compute_position_from_window<R: Runtime>(
    window: &WebviewWindow<R>,
) -> Result<LastPosition, String> {
    let physical = window
        .outer_position()
        .map_err(|e| format!("outer_position: {e}"))?;
    let monitor = window
        .current_monitor()
        .map_err(|e| format!("current_monitor: {e}"))?
        .ok_or_else(|| "current_monitor returned None".to_string())?;
    let scale = monitor.scale_factor();
    let logical = LogicalPosition::<f64>::from_physical(physical, scale);
    Ok(LastPosition {
        monitor_id: monitor_id(&monitor),
        logical_x: logical.x,
        logical_y: logical.y,
    })
}

/// 把窗口当前 outer_position（physical）转 logical，结合所在 monitor 的 id 写 config。
/// （后端 Moved handler 与前端 IPC 都走此 helper）
pub async fn save_pet_position<R: Runtime>(
    window: &WebviewWindow<R>,
) -> Result<(), WindowStateError> {
    let pos = compute_position_from_window(window)
        .map_err(|e| WindowStateError::Config(config::ConfigError::Database(e)))?;
    set_pet_position(window.app_handle(), &pos).await
}

pub async fn load_pet_position<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<LastPosition>, WindowStateError> {
    let raw = config::get(app, CONFIG_KEY_PET_POSITION).await?;
    match raw {
        None => Ok(None),
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
    }
}

/// 在给定 monitor 的逻辑边界内 clamp 到安全边距内。
fn clamp_into_monitor(monitor: &Monitor, logical_x: f64, logical_y: f64) -> (f64, f64) {
    let scale = monitor.scale_factor();
    let origin = monitor.position().to_logical::<f64>(scale);
    let size = monitor.size().to_logical::<f64>(scale);
    let min_x = origin.x + SAFE_MARGIN;
    let min_y = origin.y + SAFE_MARGIN;
    let max_x = origin.x + size.width - PET_WINDOW_LOGICAL_W - SAFE_MARGIN;
    let max_y = origin.y + size.height - PET_WINDOW_LOGICAL_H - SAFE_MARGIN;
    let cx = logical_x.clamp(min_x.min(max_x), min_x.max(max_x));
    let cy = logical_y.clamp(min_y.min(max_y), min_y.max(max_y));
    (cx, cy)
}

/// 主屏右下角偏左偏上 80px（PRD §7.1 默认值），window 不在主屏时也用主屏。
fn fallback_default<R: Runtime>(app: &AppHandle<R>) -> Option<(f64, f64)> {
    let primary = app.primary_monitor().ok().flatten()?;
    let scale = primary.scale_factor();
    let origin = primary.position().to_logical::<f64>(scale);
    let size = primary.size().to_logical::<f64>(scale);
    let x = origin.x + size.width - PET_WINDOW_LOGICAL_W - DEFAULT_OFFSET_FROM_BOTTOM_RIGHT;
    let y = origin.y + size.height - PET_WINDOW_LOGICAL_H - DEFAULT_OFFSET_FROM_BOTTOM_RIGHT;
    Some((x, y))
}

/// 启动期调用：读 last_position → 找 monitor 是否还在 → set_position 还原 / fallback。
pub fn apply_initial_position<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = app
        .get_webview_window(PET_WINDOW_LABEL)
        .ok_or_else(|| format!("pet window '{PET_WINDOW_LABEL}' not found"))?;

    // 同步等读 DB（启动期，主线程，时序简单可控；DB 已被 plugin preload 创建）
    let last = tauri::async_runtime::block_on(load_pet_position(app))
        .map_err(|e| format!("load last position: {e}"))?;

    let monitors = window
        .available_monitors()
        .map_err(|e| format!("available_monitors: {e}"))?;

    let (logical_x, logical_y) = match last {
        Some(p) => match monitors.iter().find(|m| monitor_id(m) == p.monitor_id) {
            Some(monitor) => clamp_into_monitor(monitor, p.logical_x, p.logical_y),
            // monitor 已不在场（如副屏被拔），fallback 到主屏默认位置
            None => fallback_default(app).ok_or("no primary monitor")?,
        },
        None => fallback_default(app).ok_or("no primary monitor")?,
    };

    window
        .set_position(LogicalPosition::new(logical_x, logical_y))
        .map_err(|e| format!("set_position: {e}"))
}

/// 防抖锁：同一窗口最多有一个待触发的 save spawn；新 Moved 事件抢占替换。
#[derive(Default)]
pub struct SaveDebouncer {
    pending: Mutex<Option<JoinHandle<()>>>,
}

impl SaveDebouncer {
    pub fn schedule<R: Runtime>(&self, window: WebviewWindow<R>) {
        let mut slot = match self.pending.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if let Some(prev) = slot.take() {
            prev.abort();
        }
        let handle = tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(SAVE_DEBOUNCE_MS)).await;
            if let Err(e) = save_pet_position(&window).await {
                eprintln!("[window_state] save_pet_position failed: {e}");
            }
        });
        *slot = Some(handle);
    }
}
