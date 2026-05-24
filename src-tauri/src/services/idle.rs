//! IdleDetector (#23-a / #39) — Windows 用户输入空闲检测公共依赖。
//!
//! 三处下游消费方（M2-M3）：
//! - #23-c I 精力衰减（idle 超阈值 → energy 缓慢下降）
//! - #23-b N 物理交互"被冷落"判定（连续多分钟无操作 → 抗议）
//! - M3 J 主动关心模块（idle 阶梯触发主动陪伴对话）
//!
//! ## 隐私边界（PRD §7.6 / ADR-006 lock）
//!
//! 永远 **不读按键内容 / 不读应用名 / 不读窗口标题**。GetLastInputInfo 只返回
//! "距离上次输入事件的毫秒数"（u32 tick），无键值 / 无事件类型 / 无目标窗口。
//! 与未来 N.4 RAWINPUT spike（#23-e）相同边界 —— RAWINPUT 也只看事件 *存在*
//! 不看 *内容*。
//!
//! ## 实现要点
//!
//! - **GetLastInputInfo**（[windows-rs Win32_UI_Input_KeyboardAndMouse](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Input/KeyboardAndMouse/fn.GetLastInputInfo.html)）
//!   返回 `LASTINPUTINFO.dwTime` (u32 ms tick，与 `GetTickCount` 同源)。
//! - **49.7 天 wrap-around 安全**：u32 tick 在系统连续运行 49.7 天后 wrap。
//!   `now_tick.wrapping_sub(input_tick)` 得到正确差值；强转 u64 提升避免后续算术溢出。
//! - **休眠唤醒过滤**（issue body 拍板 + 2026-05-24 实证偏离方案 A）：
//!   issue body 字面说"复用 #22 WM_POWERBROADCAST hook"，**但实测**：reminder.rs
//!   的 catch-up 是启动期一次性 `catch_up_overdue` 调用（reminder.rs:631），
//!   全代码库 grep 0 命中 WM_POWERBROADCAST。改用 **tick 心跳方案**：
//!   IdleDetector 自含 5s watchdog tokio task，记录 `last_tick_at`，与上次 tick
//!   wall-clock 比较；若 > 5min 视系统休眠过 → 写 `wake_at = now`；`is_idle` 检查
//!   `now - wake_at < 30s` 时返 false。优势：不需要 unsafe Win32 subclassing /
//!   单测可控 / 与 scheduler tick 模式一致 / 平台无关。
//!
//! ## lesson #4 双 check
//!
//! `Win32_UI_Input_KeyboardAndMouse` + `Win32_System_SystemInformation` features
//! 加到 [dependencies] 主 windows crate（非 dev-deps），resolver v2 下 lib build
//! 与 test build 都能编译。

use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// watchdog tick 间隔（秒）。5s 与 reminder.scheduler 1s tick 区分（降低 CPU 唤醒频率，
/// idle 阶梯只需分钟级精度）。
const WATCHDOG_TICK_SEC: u64 = 5;

/// 视系统休眠过的 tick 间隔阈值（秒）。两次 tick wall-clock 差 > 此值 = 进程被
/// 冻结（休眠 / Hibernate / 长时调度延迟）。
const WAKE_THRESHOLD_SEC: u64 = 5 * 60;

/// 唤醒后 `is_idle` 强制返 false 的时窗（秒）。防止 wake 瞬间 LASTINPUTINFO 里
/// 仍残留休眠前的旧 tick → 被误判成超长 idle。
const WAKE_GUARD_SEC: u64 = 30;

/// IdleDetector 进程级 state。Tauri `app.manage(IdleState::default())` 共享。
pub struct IdleState {
    /// 上次检测到休眠唤醒的 Instant；None = 启动至今未检测到。
    wake_at: Mutex<Option<Instant>>,
}

impl Default for IdleState {
    fn default() -> Self {
        Self {
            wake_at: Mutex::new(None),
        }
    }
}

impl IdleState {
    /// watchdog tick 检测到长间隔时调用。**仅供 watchdog 内部 + 单测 mock 使用**。
    pub fn mark_wake(&self, at: Instant) {
        *self.wake_at.lock() = Some(at);
    }

    /// 当前是否在 wake guard 时窗内（30s 防误判区）。
    pub fn recently_woke(&self) -> bool {
        match *self.wake_at.lock() {
            Some(t) => Instant::now().duration_since(t) < Duration::from_secs(WAKE_GUARD_SEC),
            None => false,
        }
    }
}

/// 距上次用户输入事件的毫秒数。
///
/// 实现细节：
/// - `GetLastInputInfo` 失败返 0（保守：视为"刚有输入"，不会误触下游 idle 行为）
/// - `GetTickCount` 与 `LASTINPUTINFO.dwTime` 同源（系统启动后 ms tick），单调
/// - `wrapping_sub` 保证 u32 wrap-around 后差值正确（49.7 天系统连续运行）
/// - u32 → u64 提升避免下游消费方算术溢出（threshold_ms 用 u64）
pub fn last_input_ms() -> u64 {
    // SAFETY: GetLastInputInfo / GetTickCount 都是 Windows API，传入指针指向栈上
    // 已正确初始化的 LASTINPUTINFO 结构（cbSize 必填），返回值类型由 windows-rs
    // 绑定保证。无并发或别名问题（本地变量）。
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).as_bool() {
            let now_tick = GetTickCount();
            let delta = now_tick.wrapping_sub(info.dwTime);
            delta as u64
        } else {
            0
        }
    }
}

/// 便捷封装：距上次输入 ≥ threshold_ms 且不在 wake guard 内 = idle。
///
/// 当前 #39 范围内 IPC 走 [`snapshot`]，未直接消费本函数；#23-b N 抗议（"被冷落"
/// 判定）/ #23-c I 精力衰减（idle 超 N 分钟 → energy 下降）会作真消费方。
#[allow(dead_code)]
pub fn is_idle(threshold_ms: u64, state: &IdleState) -> bool {
    if state.recently_woke() {
        return false;
    }
    last_input_ms() >= threshold_ms
}

/// 前端 IPC 返回结构（camelCase）。
///
/// - `idle_ms`：距上次输入毫秒数（u64）
/// - `is_idle`：基于 `threshold_ms` 入参 + wake guard 综合判定
/// - `recently_woke`：当前是否在唤醒 30s guard 内（前端 debug / sleepy 心情图标用）
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct IdleStateSnapshot {
    pub idle_ms: u64,
    pub is_idle: bool,
    pub recently_woke: bool,
}

/// 默认 idle 判定阈值（毫秒）。前端 `idle_get_state` 不传参时用此值。
/// 60s = 1min，是 #23-c 精力衰减的最小观察粒度。
pub const DEFAULT_IDLE_THRESHOLD_MS: u64 = 60_000;

/// 取当前 idle 快照（IPC 内部消费）。
pub fn snapshot(state: &IdleState, threshold_ms: u64) -> IdleStateSnapshot {
    let recently_woke = state.recently_woke();
    let idle_ms = last_input_ms();
    let is_idle = !recently_woke && idle_ms >= threshold_ms;
    IdleStateSnapshot {
        idle_ms,
        is_idle,
        recently_woke,
    }
}

/// 启动期 spawn watchdog tick task。`lib.rs::setup` 在 `app.manage(IdleState::default())`
/// 之后调用一次；task 与进程同生命周期（无 cancel 路径）。
///
/// 5s tick wall-clock 间隔检测：若 > `WAKE_THRESHOLD_SEC` (5min) 视休眠过 → mark wake_at。
/// 正常运行下两次 tick 间隔 ~5s，远低于阈值 → 不写 wake_at。
pub fn start_watchdog<R: Runtime + 'static>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let mut last_tick = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_secs(WATCHDOG_TICK_SEC)).await;
            let now = Instant::now();
            if now.duration_since(last_tick) > Duration::from_secs(WAKE_THRESHOLD_SEC) {
                if let Some(state) = app.try_state::<IdleState>() {
                    state.mark_wake(now);
                }
            }
            last_tick = now;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    /// `last_input_ms` 调用语义验证。
    ///
    /// issue body 原写"sleep 200ms → 验证返回值 in [180, 350]"，**实测在开发机
    /// 跑会偶发失败**：测试期间用户主机后台可能有真实输入事件（鼠标微动 / 切窗等），
    /// LASTINPUTINFO.dwTime 被持续刷新 → after 反而很小（如 15ms）。issue body
    /// 假定"测试机完全静默"，不可靠。改自适应两段式：
    /// - **静默路径**（after ≥ 150）：测试期间无新输入 → 验证 `after - before` 累计 ≥ 100ms
    /// - **活跃路径**（after < 150）：测试期间有输入 → 仅验证调用不 panic + 返回值合理
    /// - 通用：返回值不超过 30day = 2.6e9 ms（u32 wrap 边界以内）
    #[test]
    fn last_input_ms_within_ms_window() {
        let before = last_input_ms();
        sleep(Duration::from_millis(200));
        let after = last_input_ms();

        // 通用边界：30 day 是任何合理桌面会话上限（u32 49.7 day wrap 之前）
        const SANITY_UPPER: u64 = 30 * 24 * 60 * 60 * 1000;
        assert!(before < SANITY_UPPER, "before={before} 越界");
        assert!(after < SANITY_UPPER, "after={after} 越界");

        if after >= 150 {
            // 静默路径：sleep 200ms 内无新输入 → after 至少累计了 100ms
            // （留 100ms 抖动余量；OS 调度 / GetTickCount 精度 10-16ms 都吸收）
            assert!(
                after >= before.saturating_add(100),
                "static-path: expected after >= before+100, before={before} after={after}"
            );
        }
        // 活跃路径（after < 150）：测试期间有输入事件刷新 dwTime → 跳过严格断言，
        // 仅证明 last_input_ms 调用本身无 panic + 返回值在合理 u32 区间。
    }

    #[test]
    fn is_idle_true_after_threshold() {
        let state = IdleState::default();
        // threshold = 0 必然 ≥ idle_ms（任何 last_input 都 ≥ 0）
        assert!(is_idle(0, &state));
    }

    #[test]
    fn is_idle_false_under_threshold() {
        let state = IdleState::default();
        // threshold = u64::MAX 永远不可能达到 → 不 idle
        assert!(!is_idle(u64::MAX, &state));
    }

    #[test]
    fn recently_woke_suppresses_idle() {
        let state = IdleState::default();
        // 未 mark wake → recently_woke = false → is_idle 走正常路径
        assert!(!state.recently_woke());
        assert!(is_idle(0, &state));

        // mark wake = now → recently_woke = true → is_idle 强制返 false
        state.mark_wake(Instant::now());
        assert!(state.recently_woke());
        assert!(!is_idle(0, &state));
    }

    /// wake_at 超过 30s 后 recently_woke 应转 false。
    /// 模拟方式：mark 一个"31s 前"的 Instant（用 checked_sub 防 underflow，
    /// 系统启动 < 31s 时跳过测试）。
    #[test]
    fn recently_woke_expires_after_guard_window() {
        let state = IdleState::default();
        let now = Instant::now();
        if let Some(past) = now.checked_sub(Duration::from_secs(WAKE_GUARD_SEC + 1)) {
            state.mark_wake(past);
            assert!(
                !state.recently_woke(),
                "wake_at 超过 {WAKE_GUARD_SEC}s 应过期 → recently_woke=false"
            );
        }
        // checked_sub 失败（CI 启动早期）→ 跳过该断言，测试视为通过
    }

    /// u32 → u64 提升不 panic。模拟 GetTickCount wrap-around：
    /// 直接调 last_input_ms 不会真触发 wrap，但 wrapping_sub 算术在编译期保证不 panic。
    /// 本测试只验证 last_input_ms 返回值类型为 u64 且可参与算术（不溢出）。
    #[test]
    fn wrap_around_u32_to_u64_no_panic() {
        let ms = last_input_ms();
        // u64 加法不溢出
        let _doubled = ms.saturating_add(ms);
        // 与 u64::MAX 比较不 panic
        assert!(ms < u64::MAX);
    }

    /// snapshot 三字段一致性：is_idle = !recently_woke && idle_ms >= threshold
    #[test]
    fn snapshot_consistency() {
        let state = IdleState::default();

        // threshold=0 + 未 wake → is_idle=true, recently_woke=false
        let snap = snapshot(&state, 0);
        assert!(snap.is_idle);
        assert!(!snap.recently_woke);

        // mark wake → is_idle=false, recently_woke=true
        state.mark_wake(Instant::now());
        let snap = snapshot(&state, 0);
        assert!(!snap.is_idle);
        assert!(snap.recently_woke);
    }
}
