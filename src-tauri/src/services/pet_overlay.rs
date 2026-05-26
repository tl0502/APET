//! Pet Overlay 协作模块（2026-05-24 pet UI 重构第二轮）。
//!
//! ## 职责
//! - 位置算法：reminder overlay 锚 pet 上方居中（不够上空间 → 下方），command overlay 锚
//!   pet 右下偏侧（右侧不够 → 左侧）；clamp 到当前 monitor 内 + 16px margin
//! - 拖动协作：pet WindowEvent::Moved 触发立即 hide 两个 overlay；200ms 无新 Moved
//!   → "settled" → 重算 anchor + reposition + show（若 overlay 内部有 content）
//! - 内容跟踪：listen `pet-reminder:active/idle`（前端 watch bubbleCount emit）维护
//!   atomic flag；listen `pet:contextmenu:request-open/close`（pet 主窗 emit）控
//!   command overlay。settled 时只 show flag=true 的 overlay
//!
//! ## 锁定项
//! - 不动 reminder / interaction / bosskey 业务逻辑
//! - consent_gate 边界由 reminder scheduler / interaction enabled 守卫，本模块不再 gate

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Listener, LogicalPosition, Manager, Runtime};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::services::window_actions::{
    PET_COMMAND_OVERLAY_LABEL, PET_REMINDER_OVERLAY_LABEL, PET_WINDOW_LABEL,
    VISIBILITY_CHANGED_EVENT,
};

const REMINDER_W: f64 = 320.0;
const REMINDER_H: f64 = 280.0;
const COMMAND_W: f64 = 140.0;
const COMMAND_H: f64 = 196.0;
const GAP: f64 = 8.0;
const SCREEN_MARGIN: f64 = 16.0;
const SETTLED_DEBOUNCE: Duration = Duration::from_millis(200);

/// 进程级 state：overlay content flag + 拖动 settled debouncer cancel token。
#[derive(Default)]
pub struct PetOverlayState {
    reminder_has_content: AtomicBool,
    command_is_open: AtomicBool,
    settled_token: Mutex<Option<CancellationToken>>,
}

impl PetOverlayState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// lib.rs setup 调一次：manage state + 注册 4 个 listener。
pub fn setup<R: Runtime>(app: &AppHandle<R>) {
    app.manage(PetOverlayState::new());

    {
        let app_handle = app.clone();
        app.listen("pet-reminder:active", move |_| {
            if let Some(state) = app_handle.try_state::<PetOverlayState>() {
                state.reminder_has_content.store(true, Ordering::Release);
            }
            reposition_overlay(&app_handle, PET_REMINDER_OVERLAY_LABEL);
            show_overlay(&app_handle, PET_REMINDER_OVERLAY_LABEL);
        });
    }
    {
        let app_handle = app.clone();
        app.listen("pet-reminder:idle", move |_| {
            if let Some(state) = app_handle.try_state::<PetOverlayState>() {
                state.reminder_has_content.store(false, Ordering::Release);
            }
            hide_overlay(&app_handle, PET_REMINDER_OVERLAY_LABEL);
        });
    }
    {
        let app_handle = app.clone();
        app.listen("pet:contextmenu:request-open", move |_| {
            if let Some(state) = app_handle.try_state::<PetOverlayState>() {
                state.command_is_open.store(true, Ordering::Release);
            }
            reposition_overlay(&app_handle, PET_COMMAND_OVERLAY_LABEL);
            show_overlay(&app_handle, PET_COMMAND_OVERLAY_LABEL);
        });
    }
    {
        let app_handle = app.clone();
        app.listen("pet:contextmenu:request-close", move |_| {
            if let Some(state) = app_handle.try_state::<PetOverlayState>() {
                state.command_is_open.store(false, Ordering::Release);
            }
            hide_overlay(&app_handle, PET_COMMAND_OVERLAY_LABEL);
        });
    }
    // 2026-05-26 Bug 2 修：tray 内部 closeAll（点 pill 后 emit close 关）只 emit
    // closed-ack 同步 pet App.vue 的 ref，但不告知 Rust → 窗口仍 visible → 最后一条
    // window:visibility-changed=true 留存 → reminder bubble stuck dim 40%。
    // 镜像 request-close 行为：closed-ack 到达即 hide 窗 + clear state，覆盖所有关闭路径。
    {
        let app_handle = app.clone();
        app.listen("pet:contextmenu:closed-ack", move |_| {
            if let Some(state) = app_handle.try_state::<PetOverlayState>() {
                state.command_is_open.store(false, Ordering::Release);
            }
            hide_overlay(&app_handle, PET_COMMAND_OVERLAY_LABEL);
        });
    }
}

/// pet WindowEvent::Moved 触发：立即 hide 两个 overlay + 调度 settled task。
pub fn on_pet_moved<R: Runtime>(app: &AppHandle<R>) {
    hide_overlay(app, PET_REMINDER_OVERLAY_LABEL);
    hide_overlay(app, PET_COMMAND_OVERLAY_LABEL);

    let new_token = CancellationToken::new();
    let token_clone = new_token.clone();
    if let Some(state) = app.try_state::<PetOverlayState>() {
        if let Ok(mut guard) = state.settled_token.lock() {
            if let Some(old) = guard.replace(new_token) {
                old.cancel();
            }
        }
    }

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::select! {
            _ = token_clone.cancelled() => return,
            _ = sleep(SETTLED_DEBOUNCE) => {}
        }
        on_pet_settled(&app_clone);
    });
}

fn on_pet_settled<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<PetOverlayState>() else {
        return;
    };
    if state.reminder_has_content.load(Ordering::Acquire) {
        reposition_overlay(app, PET_REMINDER_OVERLAY_LABEL);
        show_overlay(app, PET_REMINDER_OVERLAY_LABEL);
    }
    if state.command_is_open.load(Ordering::Acquire) {
        reposition_overlay(app, PET_COMMAND_OVERLAY_LABEL);
        show_overlay(app, PET_COMMAND_OVERLAY_LABEL);
    }
}

pub fn reposition_overlay<R: Runtime>(app: &AppHandle<R>, label: &str) {
    let Some(pet) = app.get_webview_window(PET_WINDOW_LABEL) else { return };
    let Some(overlay) = app.get_webview_window(label) else { return };
    let Ok(Some(monitor)) = pet.current_monitor() else { return };
    let scale = monitor.scale_factor();
    let mon_pos = monitor.position().to_logical::<f64>(scale);
    let mon_size = monitor.size().to_logical::<f64>(scale);
    let Ok(pet_phys_pos) = pet.outer_position() else { return };
    let pet_pos = pet_phys_pos.to_logical::<f64>(scale);
    let Ok(pet_phys_size) = pet.outer_size() else { return };
    let pet_size = pet_phys_size.to_logical::<f64>(scale);

    // P6（2026-05-25）：reminder overlay 使用实际窗口大小定位（前端 ResizeObserver 动态调整高度）。
    // command overlay 仍用固定常量（tray 尺寸固定）。
    // 实际尺寸 < 10px（初始/隐藏态）时 fallback 到常量防止定位异常。
    let (w, h) = if label == PET_REMINDER_OVERLAY_LABEL {
        let phys = overlay.outer_size().unwrap_or_default();
        let actual_w = (phys.width as f64) / scale;
        let actual_h = (phys.height as f64) / scale;
        (
            if actual_w > 10.0 { actual_w } else { REMINDER_W },
            if actual_h > 10.0 { actual_h } else { REMINDER_H },
        )
    } else {
        (COMMAND_W, COMMAND_H)
    };

    let (x, y, placement_opt) = if label == PET_REMINDER_OVERLAY_LABEL {
        let (x, y, placement) = compute_reminder_anchor(
            pet_pos.x, pet_pos.y, pet_size.width, pet_size.height,
            mon_pos.x, mon_pos.y, mon_size.width, mon_size.height, w, h,
        );
        (x, y, Some(placement))
    } else {
        let (x, y) = compute_command_anchor(
            pet_pos.x, pet_pos.y, pet_size.width, pet_size.height,
            mon_pos.x, mon_pos.y, mon_size.width, mon_size.height, w, h,
        );
        (x, y, None)
    };

    match overlay.set_position(LogicalPosition::new(x, y)) {
        Ok(()) => {
            // reminder overlay 定位成功后通知 pet 窗口当前 placement 方向（spec 2026-05-25-pet-reminder-card-stack §5）。
            // command overlay 不发，pet glance 只跟 reminder 联动。
            if let Some(placement) = placement_opt {
                emit_reminder_placement(app, placement);
            }
        }
        Err(e) => {
            eprintln!("[pet_overlay] set_position {label} failed: {e}");
        }
    }
}

fn compute_reminder_anchor(
    pet_x: f64, pet_y: f64, pet_w: f64, pet_h: f64,
    mon_x: f64, mon_y: f64, mon_w: f64, mon_h: f64,
    w: f64, h: f64,
) -> (f64, f64, &'static str) {
    let target_x = pet_x + pet_w / 2.0 - w / 2.0;
    let target_y_above = pet_y - h - GAP;
    let (target_y, placement) = if target_y_above < mon_y + SCREEN_MARGIN {
        (pet_y + pet_h + GAP, "below")
    } else {
        (target_y_above, "above")
    };
    let (x, y) = clamp_to_monitor(target_x, target_y, w, h, mon_x, mon_y, mon_w, mon_h);
    (x, y, placement)
}

fn compute_command_anchor(
    pet_x: f64, pet_y: f64, pet_w: f64, pet_h: f64,
    mon_x: f64, mon_y: f64, mon_w: f64, mon_h: f64,
    w: f64, h: f64,
) -> (f64, f64) {
    let target_y = pet_y + pet_h * 0.45;
    let target_x_right = pet_x + pet_w + GAP;
    let target_x = if target_x_right + w > mon_x + mon_w - SCREEN_MARGIN {
        pet_x - w - GAP
    } else {
        target_x_right
    };
    clamp_to_monitor(target_x, target_y, w, h, mon_x, mon_y, mon_w, mon_h)
}

fn clamp_to_monitor(
    x: f64, y: f64, w: f64, h: f64,
    mon_x: f64, mon_y: f64, mon_w: f64, mon_h: f64,
) -> (f64, f64) {
    let cx = x.max(mon_x + SCREEN_MARGIN).min(mon_x + mon_w - w - SCREEN_MARGIN);
    let cy = y.max(mon_y + SCREEN_MARGIN).min(mon_y + mon_h - h - SCREEN_MARGIN);
    (cx, cy)
}

fn show_overlay<R: Runtime>(app: &AppHandle<R>, label: &str) {
    let Some(overlay) = app.get_webview_window(label) else { return };
    if matches!(overlay.is_visible(), Ok(true)) { return; }
    let _ = overlay.show();
    emit_visibility_changed_generic(app, label, true);
}

fn hide_overlay<R: Runtime>(app: &AppHandle<R>, label: &str) {
    let Some(overlay) = app.get_webview_window(label) else { return };
    if matches!(overlay.is_visible(), Ok(false)) { return; }
    let _ = overlay.hide();
    emit_visibility_changed_generic(app, label, false);
}

/// 与 window_actions::emit_visibility_changed 同款 payload；本模块 generic R 兼容 MockRuntime。
fn emit_visibility_changed_generic<R: Runtime>(app: &AppHandle<R>, label: &str, visible: bool) {
    use tauri::Emitter;
    if let Err(e) = app.emit(
        VISIBILITY_CHANGED_EVENT,
        serde_json::json!({ "label": label, "visible": visible }),
    ) {
        eprintln!(
            "[pet_overlay] emit {VISIBILITY_CHANGED_EVENT} for {label}={visible} failed: {e}"
        );
    }
}

/// 把 reminder overlay 当前 placement（`"above"` / `"below"`）通知 pet 窗口；
/// 前端 usePetGlance 监听此事件，决定下一次 reminder:fired 时 head bone 抬头还是低头
/// （spec 2026-05-25-pet-reminder-card-stack §5）。
fn emit_reminder_placement<R: Runtime>(app: &AppHandle<R>, placement: &'static str) {
    use tauri::Emitter;
    if let Err(e) = app.emit_to(
        PET_WINDOW_LABEL,
        "pet-reminder:placement",
        serde_json::json!({ "direction": placement }),
    ) {
        eprintln!("[pet_overlay] emit pet-reminder:placement={placement} failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reminder_anchor_default_above() {
        let (x, y, placement) = compute_reminder_anchor(
            500.0, 400.0, 320.0, 320.0,
            0.0, 0.0, 1920.0, 1080.0,
            REMINDER_W, REMINDER_H,
        );
        // pet 中心 660 - 320/2 = 500
        assert!((x - 500.0).abs() < 0.1, "上方居中 x={x}");
        // pet_y 400 - 280 - 8 = 112
        assert!((y - 112.0).abs() < 0.1, "上方 y={y}");
        assert_eq!(placement, "above", "上方 placement={placement}");
    }

    #[test]
    fn reminder_anchor_fallback_below_when_no_top_space() {
        let (_, y, placement) = compute_reminder_anchor(
            500.0, 30.0, 320.0, 320.0,
            0.0, 0.0, 1920.0, 1080.0,
            REMINDER_W, REMINDER_H,
        );
        // 顶部空间不够（30 - 280 - 8 = -258 < 0+16）→ pet 下方：30 + 320 + 8 = 358
        assert!((y - 358.0).abs() < 0.1, "下方 y={y}");
        assert_eq!(placement, "below", "下方 placement={placement}");
    }

    #[test]
    fn command_anchor_default_right() {
        let (x, _) = compute_command_anchor(
            500.0, 400.0, 320.0, 320.0,
            0.0, 0.0, 1920.0, 1080.0,
            COMMAND_W, COMMAND_H,
        );
        // pet_x 500 + pet_w 320 + 8 = 828
        assert!((x - 828.0).abs() < 0.1, "右侧 x={x}");
    }

    #[test]
    fn command_anchor_fallback_left_when_no_right_space() {
        // pet 紧贴屏幕右边 → 右侧不足 → 左侧
        let (x, _) = compute_command_anchor(
            1900.0, 400.0, 16.0, 320.0,
            0.0, 0.0, 1920.0, 1080.0,
            COMMAND_W, COMMAND_H,
        );
        // 期望 x = 1900 - 140 - 8 = 1752（COMMAND_W=140 since 2026-05-26）
        assert!((x - 1752.0).abs() < 0.1, "左侧 x={x}");
    }

    #[test]
    fn clamp_keeps_within_monitor_with_margin() {
        let (x, y) = clamp_to_monitor(
            -100.0, -100.0, 320.0, 280.0,
            0.0, 0.0, 1920.0, 1080.0,
        );
        assert_eq!(x, SCREEN_MARGIN);
        assert_eq!(y, SCREEN_MARGIN);
    }
}
