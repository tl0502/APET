//! Mood IPC（#41）— 单 IPC + disabled_features toggle KV 包装。
//!
//! mood 状态机详见 [services/mood.rs](../services/mood.rs)；本模块仅 IPC 端封装。
//!
//! 隐私 / 锁定项：mood 全 transient 不持久（PRD line 1089）；本 IPC 仅读，不接受外部写入。
//! disabled_features 是用户偏好（持久 KV `pet:disabled_features`），与 mood 数据本身分层。

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use crate::services::mood::{Mood, MoodState};

#[derive(Serialize)]
pub struct MoodSnapshot {
    pub mood: Mood,
}

/// 读当前 mood（含 transient 合并后的"展示值"）。前端 MoodIcon 浮层 polling 用。
#[tauri::command]
pub fn mood_get(state: State<'_, MoodState>) -> MoodSnapshot {
    MoodSnapshot {
        mood: state.compute_current(),
    }
}

/// disabled_features KV 读写。值是 JSON 数组（["mood_icon","energy","free_movement"] 子集）。
/// 跨重启持久（PRD §7.9.5 验收 4 "各 feature 独立可关"）。
const DISABLED_FEATURES_KEY: &str = "pet:disabled_features";

#[tauri::command]
pub async fn mood_get_disabled_features<R: Runtime>(app: AppHandle<R>) -> Result<Vec<String>, String> {
    match crate::services::config::get(&app, DISABLED_FEATURES_KEY).await {
        Ok(Some(raw)) => serde_json::from_str(&raw).map_err(|e| {
            format!(
                "disabled_features JSON parse failed (returning empty list): {e}"
            )
        }),
        Ok(None) => Ok(vec![]),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn mood_set_disabled_features<R: Runtime>(
    app: AppHandle<R>,
    features: Vec<String>,
) -> Result<(), String> {
    let json = serde_json::to_string(&features)
        .map_err(|e| format!("serialize disabled_features failed: {e}"))?;
    crate::services::config::set(&app, DISABLED_FEATURES_KEY, &json)
        .await
        .map_err(|e| e.to_string())
}
