// OnboardingService — onboarding 进度持久化（ADR-019）。
//
// KV `onboarding:current_step` 写在 config 表（与 `window:pet:last_position` / `shortcut:chat`
// 同段，因为这是"运行时状态"不是"用户偏好"；lessons.md §2 27 表零迁移原则）。
//
// 生命周期：
// - 每次 advanceStep 前 save_current_step(next_step)（前端在切 step 前调）
// - onboarding_complete 时 clear()（"已完成"信号 = KV 不存在）
// - 启动期 lib.rs::setup 读 load_current_step；存在 → 仍开 onboarding 窗（即使 consent=Match）
//
// 与 consent.granted 关系：
// - consent.granted 由 Step 1 入库,合规标记,不被 onboarding "重来" 流程 reset
// - "重来" 只 clear KV + 前端跳回 soul-pledge；用户重新走 Step 1 grant_consent
//   会再次写 granted=true（幂等，安全）

use crate::services::config::{self, ConfigError};
use tauri::{AppHandle, Runtime};

/// KV key — ADR-019 字面对齐；新增 onboarding KV 时仍走 `onboarding:*` 段。
pub const KV_CURRENT_STEP: &str = "onboarding:current_step";

pub async fn save_current_step<R: Runtime>(
    app: &AppHandle<R>,
    step: &str,
) -> Result<(), ConfigError> {
    config::set(app, KV_CURRENT_STEP, step).await
}

pub async fn load_current_step<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<String>, ConfigError> {
    config::get(app, KV_CURRENT_STEP).await
}

pub async fn clear_current_step<R: Runtime>(app: &AppHandle<R>) -> Result<(), ConfigError> {
    config::delete(app, KV_CURRENT_STEP).await
}
