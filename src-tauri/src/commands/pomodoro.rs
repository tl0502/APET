//! Pomodoro IPC commands（#28）— 6 命令：start / pause / resume / stop / active / today_stats。
//!
//! 命名遵循 Tauri 2.x runtime 规范：snake_case [a-zA-Z0-9_]（架构 §566 + chat_send /
//! reminder_create 等命名风格）。
//!
//! 与 reminder commands 的区别：番茄无 eager reload —— pomodoro::tick 走 KV 不查 DB 表，
//! 状态写 KV 立即可见，不需要让 scheduler 多跑一次。

use tauri::AppHandle;

use crate::services::pomodoro::{self, ActiveSession, StartInput, StopOutput, TodayStats};

#[tauri::command]
pub async fn pomodoro_start(
    app: AppHandle,
    input: StartInput,
) -> Result<ActiveSession, String> {
    // #33 phase E：删除 start 后自动 show 浮窗逻辑。issue#33 明文 "浮窗仅托盘'番茄...'手开"，
    // FOCUS 期协作（hard 打断 / soft 缓冲 / LivingPet wander 跳过）完全依赖后端 PomodoroService
    // 状态机，与浮窗 visible 与否无关 —— 浮窗 hidden 时 phase 仍按 tick 推进，listener 仍挂
    // （webview 不销毁），用户主动开浮窗时立即视觉对齐。
    pomodoro::start(&app, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pomodoro_pause(app: AppHandle) -> Result<ActiveSession, String> {
    pomodoro::pause(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pomodoro_resume(app: AppHandle) -> Result<ActiveSession, String> {
    pomodoro::resume(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pomodoro_stop(app: AppHandle) -> Result<StopOutput, String> {
    pomodoro::stop(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pomodoro_active(app: AppHandle) -> Result<Option<ActiveSession>, String> {
    pomodoro::active(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pomodoro_today_stats(app: AppHandle) -> Result<TodayStats, String> {
    pomodoro::today_stats(&app).await.map_err(|e| e.to_string())
}
