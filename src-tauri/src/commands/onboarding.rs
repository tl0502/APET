// onboarding 进度 IPC commands（#21, ADR-019）
//
// - onboarding_save_step(step)：advanceStep 前写 KV `onboarding:current_step`
// - onboarding_load_step()：启动期前端 onMounted 读续接状态
//
// 「重来」按钮的"清 KV"路径已废:实测后改为 saveOnboardingStep('soul-pledge')（写而非清),
// 避免 consent.granted=true + KV 不存在被启动期错认为"已完成 onboarding" → SoulPledge 误跳。
// 详 ADR-019 Updated 2026-05-12 + OnboardingApp.vue::onResumeRestart 注释。
// service 层 `clear_current_step` 保留:onboarding_complete / setup 路径仍消费。
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
