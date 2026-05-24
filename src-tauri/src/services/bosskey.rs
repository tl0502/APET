//! BossKeyService (#23-d / #42) — 摸鱼模式：Ctrl+Shift+B 一键隐藏全部窗 + 缓冲提醒 + 恢复合并提示。
//!
//! ## 范围（flows §12 Updated 2026-05-24）
//!
//! - **隐藏窗集合**：pet / chat / workspace / pomodoro（onboarding 不在内）
//! - **onboarding 期**（consent_gate=false）：静默忽略，不切换状态（ADR-019 不被绕过）
//! - **缓冲队列**：KV `boss_key_pending_reminders`（JSON 数组），不新增表（lesson #2）
//! - **崩溃恢复 KV**：`bosskey:pending=true` 标记当前在隐藏期；启动期检测 → 清 KV + 默认 show
//! - **快捷键**：Ctrl+Shift+B 默认，可通过 `bosskey_rebind` IPC 改键（同 #11 set_chat_shortcut 套路）
//!
//! ## 与既有服务的协作
//!
//! - **reminder.rs::fire**：在 emit + OS 通知前检查 `is_hidden()` → true 则 `push_pending_reminder` 入队 + return（不通知不打断 focus）
//! - **living_pet 调度器**：自由活动 tick 内已经检查 `pet_window.is_visible()` → 我们 hide 后自然跳过 wander（无需新增 bosskey 检查）
//! - **pomodoro**：计时器正常跑（独立于窗口可见性），REST 转移正常；只是 pomodoro 窗被隐藏 → 用户看不到 UI（自动达成）
//! - **主动关心** / **自动更新** / **IN_GAME**：M3+/M5 模块上线时直接查 `BossKeyState::is_hidden`，本 issue 不实现消费方
//!
//! ## 性能预算
//!
//! hide/show < 200ms（PRD §10）。当前实现：循环 4 窗 × (is_visible + hide/show + emit) ≈ <10ms 实测预期。
//! Instant 打点写 eprintln，dev 期可观测。
//!
//! ## 与 #11 ShortcutRegistry 的分工
//!
//! 不复用 ShortcutRegistry 字段（current_chat / current_workspace）—— 把 bosskey 的
//! current_shortcut + last_error 放进 BossKeyState 内聚（避免 shortcuts.rs ↔ bosskey.rs
//! 循环 import：shortcuts 注册时调 bosskey::toggle，bosskey 又依赖 shortcuts state）。
//! 复用的是「parse + on_shortcut + 槽位 + 留痕事件」的注册算法骨架。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState as GsState};
use tauri_plugin_notification::NotificationExt;

use crate::services::config;
use crate::services::consent_gate::ConsentGate;
use crate::services::window_actions::{
    CHAT_WINDOW_LABEL, PET_COMMAND_OVERLAY_LABEL, PET_REMINDER_OVERLAY_LABEL,
    PET_WINDOW_LABEL, POMODORO_WINDOW_LABEL, VISIBILITY_CHANGED_EVENT, WORKSPACE_WINDOW_LABEL,
};

pub const CONFIG_KEY_SHORTCUT_BOSSKEY: &str = "shortcut:bosskey";
pub const DEFAULT_SHORTCUT_BOSSKEY: &str = "Ctrl+Shift+B";
pub const SHORTCUT_REGISTER_FAILED_EVENT: &str = "shortcut:register-failed";
pub const BOSSKEY_TOGGLED_EVENT: &str = "boss_key:toggled";
pub const KV_BOSSKEY_PENDING: &str = "bosskey:pending";
pub const KV_PENDING_REMINDERS: &str = "boss_key_pending_reminders";

/// 隐藏窗集合（flows §12.1 Updated 2026-05-24）。
/// onboarding 不在内 —— BossKey 在 onboarding 期已被 consent_gate 拦截。
/// 2026-05-24 第二轮 pet UI 重构：追加 pet-reminder / pet-command 两 overlay；
/// hide 时若未 visible 自然跳过（show_overlay 是幂等），recover 时同步 show 让用户
/// toggle 回来后 overlay 立即可见（前提是其 content flag 仍为 true）。
pub const SNAPSHOTABLE_LABELS: &[&str] = &[
    PET_WINDOW_LABEL,
    CHAT_WINDOW_LABEL,
    WORKSPACE_WINDOW_LABEL,
    POMODORO_WINDOW_LABEL,
    PET_REMINDER_OVERLAY_LABEL,
    PET_COMMAND_OVERLAY_LABEL,
];

/// hide 前为每窗记录的快照。show 时按此恢复。
///
/// `always_on_top` 不入快照：Tauri 2.x WebviewWindow 无 `is_always_on_top` getter；
/// 实测 hide/show 不改变 AOT 属性（OS 窗口 attribute 与 visibility 正交），
/// 故无需快照（恢复时窗自然保留 hide 前的 AOT 值）。
#[derive(Clone, Debug, Serialize)]
pub struct WindowSnapshotItem {
    pub was_visible: bool,
    pub x: i32,
    pub y: i32,
}

/// 注册失败留痕 payload（emit + Mutex 双路径解决 setup 内 emit 早于 webview listener 挂载
/// 的 race，与 ShortcutRegistry.last_chat_error 同套路）。
#[derive(Serialize, Clone, Debug)]
pub struct ShortcutRegisterFailedPayload {
    /// 前端 listener 用 kind 字段分发（chat / workspace / bosskey 三类）。
    pub kind: &'static str,
    pub shortcut: String,
    pub error: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct ToggledPayload {
    pub hidden: bool,
}

/// 缓冲提醒条目。`reminder_id` 用 String（reminders.id 是 TEXT uuid）。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PendingReminder {
    pub reminder_id: String,
    pub title: String,
    pub priority: String,
}

/// 进程级状态容器。Tauri `app.manage(BossKeyState::default())` 共享。
#[derive(Default)]
pub struct BossKeyState {
    hidden: AtomicBool,
    snapshot: Mutex<HashMap<String, WindowSnapshotItem>>,
    current_shortcut: Mutex<Option<String>>,
    last_error: Mutex<Option<ShortcutRegisterFailedPayload>>,
}

impl BossKeyState {
    /// 是否在摸鱼隐藏期。reminder.rs / living_pet / M3+ 消费方查询此接口判断是否抑制行为。
    pub fn is_hidden(&self) -> bool {
        self.hidden.load(Ordering::Acquire)
    }

    /// 当前注册的快捷键字符串（None = 启动期 register 失败）。
    /// MVP 暂无消费方（前端 query 走 IPC bosskey_get_shortcut，本 issue 不上 UI rebind）。
    #[allow(dead_code)]
    pub fn current_shortcut(&self) -> Option<String> {
        self.current_shortcut.lock().clone()
    }

    /// 启动期 register 失败留痕（前端 mount 时可查询 → 提示用户改键）。
    /// MVP 暂无消费方（M3 G 设置面板上线时启用）。
    #[allow(dead_code)]
    pub fn last_register_error(&self) -> Option<ShortcutRegisterFailedPayload> {
        self.last_error.lock().clone()
    }
}

// ============================================================
// toggle / hide / show 主路径
// ============================================================

/// 内部 emit 包装：与 [`window_actions::emit_visibility_changed`] 同事件 + 同 schema，
/// 但本函数 generic over `R: Runtime`，可在 reminder.rs / scheduler 等 generic 上下文里调用。
/// （window_actions 的版本签名是 concrete `&AppHandle`，无法在 generic 路径上下文里用。）
fn emit_vis_changed<R: Runtime>(app: &AppHandle<R>, label: &str, visible: bool) {
    if let Err(e) = app.emit(
        VISIBILITY_CHANGED_EVENT,
        serde_json::json!({ "label": label, "visible": visible }),
    ) {
        eprintln!(
            "[bosskey] emit {VISIBILITY_CHANGED_EVENT} for {label}={visible} failed: {e}"
        );
    }
}

/// 快捷键 / IPC / 托盘统一入口。返回操作后的 `hidden` 值。
///
/// onboarding 期（consent_gate=false）静默返当前态（应为 false）。flows §12.4 Updated。
pub async fn toggle<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
    let gate_open = app
        .try_state::<ConsentGate>()
        .map(|g| g.is_open())
        .unwrap_or(false);
    if !gate_open {
        eprintln!("[bosskey] consent_gate=false, ignored");
        // 返回当前 hidden（理论 false：onboarding 期不应进 hidden）
        let cur = app
            .try_state::<BossKeyState>()
            .map(|s| s.is_hidden())
            .unwrap_or(false);
        return Ok(cur);
    }
    let state = app.state::<BossKeyState>();
    if state.is_hidden() {
        show(app, &state).await
    } else {
        hide(app, &state).await
    }
}

async fn hide<R: Runtime>(app: &AppHandle<R>, state: &BossKeyState) -> Result<bool, String> {
    let start = Instant::now();
    let mut snap = HashMap::new();
    for label in SNAPSHOTABLE_LABELS {
        // get_webview_window 在窗口已被外力关闭时返 None（flows §12.4 分支 2：静默忽略）
        if let Some(w) = app.get_webview_window(label) {
            let was_visible = w.is_visible().unwrap_or(false);
            let (x, y) = w.outer_position().map(|p| (p.x, p.y)).unwrap_or((0, 0));
            snap.insert(
                label.to_string(),
                WindowSnapshotItem { was_visible, x, y },
            );
            if was_visible {
                let _ = w.hide();
                emit_vis_changed(app, label, false);
            }
        }
    }
    *state.snapshot.lock() = snap;
    state.hidden.store(true, Ordering::Release);
    // 崩溃恢复 KV：标记"摸鱼期未正常 toggle 回 show"
    if let Err(e) = config::set(app, KV_BOSSKEY_PENDING, "true").await {
        eprintln!("[bosskey] set pending KV failed: {e}");
    }
    let _ = app.emit(BOSSKEY_TOGGLED_EVENT, ToggledPayload { hidden: true });
    eprintln!("[bosskey] hide done in {:?}", start.elapsed());
    Ok(true)
}

async fn show<R: Runtime>(app: &AppHandle<R>, state: &BossKeyState) -> Result<bool, String> {
    let start = Instant::now();
    let snap = std::mem::take(&mut *state.snapshot.lock());
    for label in SNAPSHOTABLE_LABELS {
        // 用顺序遍历 SNAPSHOTABLE_LABELS（而非 snap.iter()）保证 show 顺序稳定（HashMap 无序）
        let info = match snap.get(*label) {
            Some(i) => i,
            None => continue, // hide 期间被外力关闭的窗 → 跳过
        };
        if let Some(w) = app.get_webview_window(label) {
            if info.was_visible {
                let _ = w.show();
                // 位置恢复 best-effort：若 monitor 已拔 / DPI 变化，OS 会落到主屏可见区
                let _ = w.set_position(PhysicalPosition::new(info.x, info.y));
                emit_vis_changed(app, label, true);
            }
        }
    }
    state.hidden.store(false, Ordering::Release);
    if let Err(e) = config::delete(app, KV_BOSSKEY_PENDING).await {
        eprintln!("[bosskey] delete pending KV failed: {e}");
    }
    // 恢复合并提醒：≥2 → 合并通知，=1 → 单条，0 → no-op（flows §12.3）
    flush_pending_reminders(app).await;
    let _ = app.emit(BOSSKEY_TOGGLED_EVENT, ToggledPayload { hidden: false });
    eprintln!("[bosskey] show done in {:?}", start.elapsed());
    Ok(false)
}

// ============================================================
// 缓冲队列：reminder.rs 触发期生产、show / 启动恢复期消费
// ============================================================

/// reminder.rs::fire 消费：隐藏期 reminder 入队（不 emit 不 OS 通知）。
/// 异常时只 eprintln 不阻断 fire 主路径（reminder DB tx 已 commit，丢一条缓冲不致命）。
pub async fn push_pending_reminder<R: Runtime>(
    app: &AppHandle<R>,
    reminder_id: String,
    title: String,
    priority: String,
) -> Result<(), String> {
    let stored = config::get(app, KV_PENDING_REMINDERS)
        .await
        .map_err(|e| e.to_string())?;
    let mut queue: Vec<PendingReminder> = stored
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    queue.push(PendingReminder {
        reminder_id,
        title,
        priority,
    });
    let serialized = serde_json::to_string(&queue).map_err(|e| e.to_string())?;
    config::set(app, KV_PENDING_REMINDERS, &serialized)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// show / 启动恢复消费：合并展示 + 清队列。
///
/// flows §12.3 阈值规则：
/// - ≥2 → 合并 OS 通知"回来了？刚才我留了 N 条提醒在这"
/// - =1 → 正常单条通知（与 reminder.rs::fire OS 通知文案对齐）
/// - 0  → no-op
async fn flush_pending_reminders<R: Runtime>(app: &AppHandle<R>) {
    let stored = match config::get(app, KV_PENDING_REMINDERS).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[bosskey] read pending reminders failed: {e}");
            return;
        }
    };
    let queue: Vec<PendingReminder> = stored
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if queue.is_empty() {
        return;
    }
    if queue.len() >= 2 {
        let body = format!("刚才我留了 {} 条提醒在这", queue.len());
        if let Err(e) = app
            .notification()
            .builder()
            .title("回来了？")
            .body(body)
            .show()
        {
            eprintln!("[bosskey] merged notification failed: {e}");
        }
    } else {
        let r = &queue[0];
        let body = if r.priority == "hard" {
            "现在做"
        } else {
            "记得做哦"
        };
        if let Err(e) = app
            .notification()
            .builder()
            .title(format!("提醒 · {}", r.title))
            .body(body)
            .show()
        {
            eprintln!("[bosskey] single reminder notification failed: {e}");
        }
    }
    if let Err(e) = config::delete(app, KV_PENDING_REMINDERS).await {
        eprintln!("[bosskey] clear pending reminders failed: {e}");
    }
}

// ============================================================
// 启动期崩溃恢复（flows §12.4 分支 3 / §8.1）
// ============================================================

/// 启动期 setup 调用：若上次进程退出时仍在隐藏期（`bosskey:pending=true` 未被 show 路径清掉）
/// → 默认恢复显示态：清 KV + 不重建 snapshot；pending reminders 合并展示让用户知道错过了什么。
///
/// 注：crash 后所有 tauri 窗按 tauri.conf 默认 visible 启动，等价于"默认 show"已自动达成，
/// 无需我们再调 w.show()。
pub async fn recover_from_crash<R: Runtime>(app: &AppHandle<R>) {
    match config::get(app, KV_BOSSKEY_PENDING).await {
        Ok(Some(_)) => {
            eprintln!("[bosskey] crash recovery: clear pending KV + flush pending reminders");
            if let Err(e) = config::delete(app, KV_BOSSKEY_PENDING).await {
                eprintln!("[bosskey] crash recovery clear KV failed: {e}");
            }
            // pending reminders 留着合并展示（用户回来期待看到他错过的）
            flush_pending_reminders(app).await;
        }
        Ok(None) => {}
        Err(e) => eprintln!("[bosskey] read pending KV failed: {e}"),
    }
}

// ============================================================
// 快捷键注册（Ctrl+Shift+B，#11 ShortcutRegistry 算法骨架复用）
// ============================================================

fn parse_shortcut(s: &str) -> Result<Shortcut, String> {
    s.parse::<Shortcut>()
        .map_err(|e| format!("解析快捷键失败 '{s}': {e}"))
}

/// lib.rs::setup 调用一次。从 config 读 KV（无则用默认）→ on_shortcut 绑定 → 失败 emit + 留痕。
///
/// 失败不阻断启动：用户可走托盘菜单"摸鱼"项手动 toggle（flows §12.4 分支 1）。
/// 本 issue 不上托盘菜单项（issue body "不做"列表第 1 行：M3 G 设置面板才上 rebind UI），
/// 但保留 last_error 给未来 UI 消费。
pub fn register_bosskey_on_startup<R: Runtime>(app: &AppHandle<R>) {
    let stored = tauri::async_runtime::block_on(config::get(app, CONFIG_KEY_SHORTCUT_BOSSKEY))
        .unwrap_or(None);
    let shortcut_str = stored.unwrap_or_else(|| DEFAULT_SHORTCUT_BOSSKEY.to_string());
    match register_internal(app, &shortcut_str) {
        Ok(()) => {
            eprintln!("[bosskey] registered shortcut: {shortcut_str}");
            clear_last_error(app);
        }
        Err(e) => {
            eprintln!("[bosskey] register failed for '{shortcut_str}': {e}");
            let payload = ShortcutRegisterFailedPayload {
                kind: "bosskey",
                shortcut: shortcut_str,
                error: e,
            };
            set_last_error(app, payload.clone());
            let _ = app.emit(SHORTCUT_REGISTER_FAILED_EVENT, payload);
        }
    }
}

fn register_internal<R: Runtime>(app: &AppHandle<R>, shortcut_str: &str) -> Result<(), String> {
    let shortcut = parse_shortcut(shortcut_str)?;
    // 与 shortcuts.rs::register_internal 同款：用 on_shortcut 而非 plugin 全局 with_handler
    // （per-shortcut handler 在 Windows 上更可靠；详 shortcuts.rs:141 注释）。
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state() != GsState::Pressed {
                return;
            }
            // 早 gate 检查（与 shortcuts.rs::handle_shortcut_pressed 同款），避免无谓 spawn
            let gate_open = app
                .try_state::<ConsentGate>()
                .map(|g| g.is_open())
                .unwrap_or(false);
            if !gate_open {
                eprintln!("[bosskey] shortcut suppressed (onboarding not complete)");
                return;
            }
            // toggle 是 async（内部 await config::set/delete），handler 是同步闭包 → spawn 异步任务。
            // AppHandle Clone 廉价（Arc 内部）；spawn 不阻塞 handler 返回，符合 plugin handler 约定。
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = toggle(&app_handle).await {
                    eprintln!("[bosskey] toggle from shortcut failed: {e}");
                }
            });
        })
        .map_err(|e| e.to_string())?;
    if let Some(state) = app.try_state::<BossKeyState>() {
        *state.current_shortcut.lock() = Some(shortcut_str.to_string());
    }
    Ok(())
}

fn unregister_current<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let state = app.state::<BossKeyState>();
    let prev = state.current_shortcut.lock().take();
    if let Some(p) = prev {
        let s = parse_shortcut(&p)?;
        app.global_shortcut()
            .unregister(s)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// IPC `bosskey_rebind` 调用：unregister 旧 + register 新 + 落 config。
/// 失败时旧已 unregister 但新 register 失败 → 失去旧快捷键；前端可重新调 rebind 兜底（M1 范围不做 tx）。
pub async fn rebind<R: Runtime>(app: &AppHandle<R>, new_shortcut: &str) -> Result<(), String> {
    let _ = parse_shortcut(new_shortcut)?;
    unregister_current(app)?;
    register_internal(app, new_shortcut)?;
    config::set(app, CONFIG_KEY_SHORTCUT_BOSSKEY, new_shortcut)
        .await
        .map_err(|e| e.to_string())?;
    clear_last_error(app);
    Ok(())
}

fn set_last_error<R: Runtime>(app: &AppHandle<R>, payload: ShortcutRegisterFailedPayload) {
    if let Some(state) = app.try_state::<BossKeyState>() {
        *state.last_error.lock() = Some(payload);
    }
}

fn clear_last_error<R: Runtime>(app: &AppHandle<R>) {
    if let Some(state) = app.try_state::<BossKeyState>() {
        *state.last_error.lock() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_not_hidden() {
        let s = BossKeyState::default();
        assert!(!s.is_hidden());
        assert!(s.current_shortcut().is_none());
        assert!(s.last_register_error().is_none());
    }

    #[test]
    fn hidden_flag_round_trips() {
        let s = BossKeyState::default();
        s.hidden.store(true, Ordering::Release);
        assert!(s.is_hidden());
        s.hidden.store(false, Ordering::Release);
        assert!(!s.is_hidden());
    }

    /// snapshot Mutex 存取语义。
    #[test]
    fn snapshot_mutex_stores_window_items() {
        let s = BossKeyState::default();
        {
            let mut snap = s.snapshot.lock();
            snap.insert(
                PET_WINDOW_LABEL.to_string(),
                WindowSnapshotItem {
                    was_visible: true,
                    x: 100,
                    y: 200,
                },
            );
        }
        let snap = s.snapshot.lock();
        let item = snap.get(PET_WINDOW_LABEL).unwrap();
        assert!(item.was_visible);
        assert_eq!(item.x, 100);
        assert_eq!(item.y, 200);
    }

    #[test]
    fn pending_reminder_json_roundtrip() {
        let r = PendingReminder {
            reminder_id: "abc-123".to_string(),
            title: "喝水".to_string(),
            priority: "soft".to_string(),
        };
        let s = serde_json::to_string(&r).unwrap();
        let back: PendingReminder = serde_json::from_str(&s).unwrap();
        assert_eq!(back.reminder_id, "abc-123");
        assert_eq!(back.title, "喝水");
        assert_eq!(back.priority, "soft");
    }

    /// 队列 JSON 数组 round-trip + 阈值边界（0 / 1 / 2 三档判定走的就是 queue.len()）。
    #[test]
    fn pending_reminders_queue_json_roundtrip() {
        let q = vec![
            PendingReminder {
                reminder_id: "1".to_string(),
                title: "a".to_string(),
                priority: "soft".to_string(),
            },
            PendingReminder {
                reminder_id: "2".to_string(),
                title: "b".to_string(),
                priority: "hard".to_string(),
            },
        ];
        let s = serde_json::to_string(&q).unwrap();
        let back: Vec<PendingReminder> = serde_json::from_str(&s).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].priority, "soft");
        assert_eq!(back[1].priority, "hard");
    }

    /// 损坏 JSON 容错：从 KV 读到非数组字符串时降级为空队列（unwrap_or_default）。
    /// 避免一次脏数据让用户的所有 reminder 都丢。
    #[test]
    fn corrupt_queue_json_degrades_to_empty() {
        let stored: Option<String> = Some("not a json array".to_string());
        let queue: Vec<PendingReminder> = stored
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        assert!(queue.is_empty());
    }

    /// SNAPSHOTABLE_LABELS 含 6 个目标窗（pet/chat/workspace/pomodoro 主体 4 + pet-reminder/pet-command
    /// overlay 2），不含 onboarding。flows §12.1 Updated 2026-05-24 锁定的边界。
    #[test]
    fn snapshotable_labels_match_flows_12_1() {
        assert_eq!(SNAPSHOTABLE_LABELS.len(), 6);
        assert!(SNAPSHOTABLE_LABELS.contains(&PET_WINDOW_LABEL));
        assert!(SNAPSHOTABLE_LABELS.contains(&CHAT_WINDOW_LABEL));
        assert!(SNAPSHOTABLE_LABELS.contains(&WORKSPACE_WINDOW_LABEL));
        assert!(SNAPSHOTABLE_LABELS.contains(&POMODORO_WINDOW_LABEL));
        assert!(SNAPSHOTABLE_LABELS.contains(&PET_REMINDER_OVERLAY_LABEL));
        assert!(SNAPSHOTABLE_LABELS.contains(&PET_COMMAND_OVERLAY_LABEL));
        // onboarding 不应在集合内（consent_gate 已前置拦截）
        assert!(!SNAPSHOTABLE_LABELS.contains(&"onboarding"));
    }
}
