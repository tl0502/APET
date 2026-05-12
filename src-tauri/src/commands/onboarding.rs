// onboarding 进度 IPC commands（#21, ADR-019）
//
// - onboarding_save_step(step)：advanceStep 前写 KV `onboarding:current_step`
// - onboarding_load_step()：启动期前端 onMounted 读续接状态
// - onboarding_reset()：「重来」按钮调；clear KV，不动 consent.granted
//
// onboarding_complete 仍在 commands/window.rs（核心动作是切窗），其内部已加 clear KV 调用。

use crate::services::onboarding;
use tauri::AppHandle;

#[tauri::command]
pub async fn onboarding_save_step(app: AppHandle, step: String) -> Result<(), String> {
    onboarding::save_current_step(&app, &step)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn onboarding_load_step(app: AppHandle) -> Result<Option<String>, String> {
    onboarding::load_current_step(&app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn onboarding_reset(app: AppHandle) -> Result<(), String> {
    onboarding::clear_current_step(&app)
        .await
        .map_err(|e| e.to_string())
}
