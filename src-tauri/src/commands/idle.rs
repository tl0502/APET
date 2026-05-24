//! IdleDetector IPC (#23-a / #39) — 单 IPC 命令返回 idle 快照。
//!
//! 隐私边界：永远不读按键内容 / 应用名 / 窗口标题（PRD §7.6 / ADR-006 lock）。
//! 详细实现见 [services::idle](crate::services::idle)。

use tauri::State;

use crate::services::idle::{snapshot, IdleState, IdleStateSnapshot, DEFAULT_IDLE_THRESHOLD_MS};

/// 前端 query idle 状态。`thresholdMs` 缺省走 [`DEFAULT_IDLE_THRESHOLD_MS`] (60s)。
///
/// 返回 camelCase 字段：`{ idleMs, isIdle, recentlyWoke }`。
#[tauri::command]
pub async fn idle_get_state(
    state: State<'_, IdleState>,
    threshold_ms: Option<u64>,
) -> Result<IdleStateSnapshot, String> {
    Ok(snapshot(
        &state,
        threshold_ms.unwrap_or(DEFAULT_IDLE_THRESHOLD_MS),
    ))
}
