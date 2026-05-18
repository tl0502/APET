//! Pomodoro IPC commands（#28）— 6 命令：start / pause / resume / stop / active / today_stats。
//!
//! 命名遵循 Tauri 2.x runtime 规范：snake_case [a-zA-Z0-9_]（架构 §566 + chat_send /
//! reminder_create 等命名风格）。
//!
//! 与 reminder commands 的区别：番茄无 eager reload —— pomodoro::tick 走 KV 不查 DB 表，
//! 状态写 KV 立即可见，不需要让 scheduler 多跑一次。

use tauri::{AppHandle, Manager};

use crate::services::pomodoro::{self, ActiveSession, StartInput, StopOutput, TodayStats};

#[tauri::command]
pub async fn pomodoro_start(
    app: AppHandle,
    input: StartInput,
) -> Result<ActiveSession, String> {
    let s = pomodoro::start(&app, input).await.map_err(|e| e.to_string())?;
    // #28 follow-up 修订 #4：start 成功后自动唤起独立窗，但仅在窗当前 hidden 时 show
    // （防抢焦点）——独立窗已可见时再 show 会重新置顶 / 抢焦点，干扰用户当前操作。
    // is_visible() 是同步 OS 调用 < 1ms 可忽略。show_pomodoro 内已查 ConsentGate。
    if let Some(w) =
        app.get_webview_window(crate::services::window_actions::POMODORO_WINDOW_LABEL)
    {
        if !w.is_visible().unwrap_or(false) {
            crate::services::window_actions::show_pomodoro(&app);
        }
    }
    Ok(s)
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
