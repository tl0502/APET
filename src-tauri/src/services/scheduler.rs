//! 公共 Scheduler 抽象 — 1s polling，DB 驱动单 tokio task。
//!
//! 设计取舍（M2 极简）:
//! - 用 1s tick polling DB（不引入 tokio::sync::Notify / BinaryHeap），节点数量小（用户级
//!   reminder 通常 <10 条）；1s 精度对 reminder（分钟级触发）富余，对 pomodoro（秒级倒计时
//!   显示）刚好。代码 ~80 行 vs Notify+Heap 方案 ~150 行的 trade-off。
//! - tick handler 内串行：① reminder::find_due+fire（防重入由 reminder_history NOT EXISTS 保证）
//!   ② pomodoro::tick（drift 校准 + 自动 FOCUS→REST→IDLE 转换 + emit pomodoro:tick）。
//! - 共用 task：1s 间隔同时驱动两个 service，省 spawn 第二个 task。
//! - reminder.create/update/snooze/complete 等 IPC 写后立即调 `reload_reminders()` 让其
//!   多走一次 find_due+fire（≈ "eager check"），不等下一个 1s tick。fire 内防重入（
//!   `reminder_history.fired_at >= reminders.next_fire_at` NOT EXISTS），eager 与 polling
//!   并发安全。
//! - TimerKind variant 给 #29 待办 / M3 IdleDetector 占位；本 issue 已 dispatch Reminder + Pomodoro。
//!
//! 复用边界:
//! - #28 番茄已复用 tick handler 内的 pomodoro::tick 调用。
//! - M3 IdleDetector 同 polling 模式：tick 时调 GetLastInputInfo 比较。
//!
//! ConsentGate 门禁：onboarding 未完成时跳过 tick（参考 living_pet.rs::start_scheduler）。

use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};

use crate::services::consent_gate::ConsentGate;
use crate::services::{pomodoro, reminder};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind {
    Reminder,
    Pomodoro, // #28 已 dispatch
    Idle,     // M3 follow-up
}

const TICK_INTERVAL_SEC: u64 = 1;

/// 启动期 spawn 调度 task。lib.rs::setup 调用一次即可，task 与进程同生命周期。
///
/// 1s tick 同时驱动 reminder + pomodoro：
/// - reminder 走 NOT EXISTS 子查询 + indexed enabled/next_fire_at，单次 < 1ms；1Hz 无压力。
/// - pomodoro 走 KV 单条 SELECT + 事务包 load/save；1Hz 无压力。
pub fn start<R: Runtime + 'static>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(TICK_INTERVAL_SEC)).await;

            // ConsentGate 门禁：onboarding 未完成时跳过（参考 living_pet.rs::start_scheduler）。
            // gate 关闭期间用户还在 onboarding 窗，发提醒只会惊吓；番茄态也不应推进。
            let gate_open = app
                .try_state::<ConsentGate>()
                .map(|g| g.is_open())
                .unwrap_or(false);
            if !gate_open {
                continue;
            }

            // ① reminder dispatch（保留 #22 原有逻辑）
            match reminder::find_due(&app).await {
                Ok(ids) => {
                    for id in ids {
                        if let Err(e) = reminder::fire(&app, &id).await {
                            eprintln!("[scheduler] reminder fire {id} failed: {e}");
                        }
                    }
                }
                Err(e) => eprintln!("[scheduler] find_due failed: {e}"),
            }

            // ② pomodoro tick（#28）：drift 校准 + 自动转换 + emit tick
            if let Err(e) = pomodoro::tick(&app).await {
                eprintln!("[scheduler] pomodoro tick failed: {e}");
            }
        }
    });
}

/// IPC 写后调（reminder.create/update/snooze/complete 等）：立刻 check 一次 find_due+fire，
/// 不等下一个 1s tick。fire 内有防重入，与 polling 并发安全。
///
/// 启动期 lib.rs::setup 在 catch_up_overdue 后也调一次，让首发 reminder 不必等 1s。
pub async fn reload_reminders<R: Runtime>(app: &AppHandle<R>) -> Result<(), reminder::ReminderError> {
    let ids = reminder::find_due(app).await?;
    for id in ids {
        if let Err(e) = reminder::fire(app, &id).await {
            eprintln!("[scheduler] eager fire {id} failed: {e}");
        }
    }
    Ok(())
}
