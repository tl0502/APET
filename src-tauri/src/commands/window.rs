// 窗口控制 IPC commands
//
// 当前窗口语义：
// - settings show/hide（#9）
// - pet 位置 get/save（#10；与后端 Moved 自动保存路径独立，前端可主动覆写）
// - chat show/hide/toggle（#14；接 #11 全局快捷键 + 关闭按钮 / ESC）
// - onboarding_complete（#16；"我懂了"路径切窗 + emit step-done 给 #17 状态机用）

use crate::services::consent_gate::ConsentGate;
use crate::services::onboarding;
use crate::services::window_actions::{
    hide_chat, hide_onboarding, hide_settings, show_chat, show_pet, show_settings, toggle_chat,
};
use crate::services::window_state::{self, LastPosition};
use tauri::{AppHandle, Emitter, Manager};

#[tauri::command]
pub async fn settings_show(app: AppHandle) -> Result<(), String> {
    show_settings(&app);
    Ok(())
}

#[tauri::command]
pub async fn settings_hide(app: AppHandle) -> Result<(), String> {
    hide_settings(&app);
    Ok(())
}

#[tauri::command]
pub async fn get_pet_position(app: AppHandle) -> Result<Option<LastPosition>, String> {
    window_state::load_pet_position(&app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_pet_position(app: AppHandle, pos: LastPosition) -> Result<(), String> {
    window_state::set_pet_position(&app, &pos)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn chat_show(app: AppHandle) -> Result<(), String> {
    show_chat(&app);
    Ok(())
}

#[tauri::command]
pub async fn chat_hide(app: AppHandle) -> Result<(), String> {
    hide_chat(&app);
    Ok(())
}

#[tauri::command]
pub async fn chat_toggle(app: AppHandle) -> Result<(), String> {
    toggle_chat(&app);
    Ok(())
}

/// issue #16："我懂了"路径切窗 IPC。
///
/// 调用契约：前端 SoulPledgeView 在 `consent_grant` 成功后调本命令，由后端统一完成
/// 1. hide onboarding 窗口（保留 webview 供 #17 状态机后续 Step 2-6 复用）
/// 2. show pet 窗口（startup 期为它 hide 过；现在用户已同意 → 进入主态）
/// 3. emit 全局事件 `onboarding:step-done` { step: "soul-pledge" } 给 #17 监听
///
/// 后端做切窗而不让前端自行切的理由：
/// - 切窗涉及两个不同窗口（onboarding hide + pet show），前端跨窗口编排需要绕一圈
///   getAll() / find by label；后端 AppHandle 直接拿 webview window 更简洁
/// - 与 settings_show / chat_show 等同款风格（窗口可见性统一从 Rust 侧管控）
/// - 事件 emit 也需要 AppHandle.emit；前端在 onboarding webview 里 emit 只能发到本窗，
///   pet webview 收不到——后端 emit 是全局广播，所有 webview 都能监听
#[tauri::command]
pub async fn onboarding_complete(app: AppHandle) -> Result<(), String> {
    // ADR-019：先 clear KV `onboarding:current_step`（"已完成" 信号 = KV 不存在）。
    // 失败 → 不阻断切窗：用户体验上 onboarding 已经完成，下次启动多弹一次"继续/重来/退出"
    // 比"切窗失败困在 onboarding"更友好。eprintln 暴露给开发期。
    if let Err(e) = onboarding::clear_current_step(&app).await {
        eprintln!("[onboarding_complete] clear_current_step failed (non-fatal): {e}");
    }
    // #21 M1 收尾：往 active conversation 写一条 system 转场消息（与 nickname_announcement
    // 同款 System Prompt Inconsistency 治理套路）。失败仅 eprintln 不阻断主路径——掉这条
    // system 行只会让 LLM 首句问候稍微平淡，不应让 onboarding 完成卡住。
    if let Err(e) =
        crate::services::onboarding_announcement::inject_onboarding_complete(&app).await
    {
        eprintln!("[onboarding_complete] inject announcement failed (non-fatal): {e}");
    }
    // #21 锁死边界：翻 ConsentGate 必须在 show_pet 之前——show_pet 内部 gate 检查通过
    // 才会真正显示 pet 窗。顺序反了会被 show_pet 回路引导回 onboarding（gate 仍是 false
    // 时它会改调 show_onboarding），陷入"刚 hide 又 show"的逻辑回环。
    app.state::<ConsentGate>().open();
    hide_onboarding(&app);
    show_pet(&app);
    // 事件 payload 用对象而非裸字符串，给 #17 状态机扩展空间（如 step 字段 + reconsent 标记）。
    // emit 失败仅 eprintln 不返 Err：到此为止所有 side-effect（clear KV / inject / gate.open /
    // hide onboarding / show pet）都已成功，IPC 返 Err 会让前端 catch toast 在已 hidden 的
    // onboarding 窗里渲染（用户看不到）+ 误把"已成功的主态切换"标记为失败。emit 是给 #17
    // 状态机的辅助广播，M1 无消费方，丢一次不影响主流程。
    if let Err(e) = app.emit("onboarding:step-done", serde_json::json!({ "step": "soul-pledge" })) {
        eprintln!("[onboarding_complete] emit step-done failed (non-fatal): {e}");
    }
    Ok(())
}
