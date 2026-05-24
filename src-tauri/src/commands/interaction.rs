//! Interaction IPC commands（#40，模块 N）— 3 命令。
//!
//! 命名：snake_case [a-zA-Z0-9_]（Tauri 2.x runtime + 已有命名风格）。
//! 注册位置：lib.rs invoke_handler「#23-b N InteractionRouter」段（epic #23 强制约定）。
//!
//! lesson #10：`#[tauri::command] async fn` 链路全程 async + await，不在内部 block_on。
//! record_drag_count 内部 spawn 5s revert task，是 fire-and-forget 不 await，符合规则。

use tauri::{AppHandle, State};

use crate::services::energy::EnergyState;
use crate::services::interaction::{
    self, InteractionState, ReactionEntry,
};
use crate::services::mood::MoodState;

#[tauri::command]
pub async fn interaction_dispatch(
    app: AppHandle,
    state: State<'_, InteractionState>,
    mood_state: State<'_, MoodState>,
    energy_state: State<'_, EnergyState>,
    event: String,
    hitbox: String,
) -> Result<ReactionEntry, String> {
    interaction::dispatch(
        &app,
        state.inner(),
        mood_state.inner(),
        energy_state.inner(),
        &event,
        &hitbox,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn interaction_record_drag_count(
    app: AppHandle,
    state: State<'_, InteractionState>,
    mood_state: State<'_, MoodState>,
    window: String,
    count: u32,
) -> Result<usize, String> {
    Ok(interaction::record_drag_count(
        &app,
        state.inner(),
        mood_state.inner(),
        &window,
        count,
    ))
}

#[tauri::command]
pub async fn interaction_reset_drag_state(
    state: State<'_, InteractionState>,
    window: String,
) -> Result<(), String> {
    interaction::reset_drag_state(state.inner(), &window);
    Ok(())
}
