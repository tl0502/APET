//! 公共 Scheduler 抽象 — 5s polling，DB 驱动单 tokio task。
//!
//! 设计取舍（M2 极简）:
//! - 用 5s tick polling DB（不引入 tokio::sync::Notify / BinaryHeap），节点数量小（用户级
//!   reminder 通常 <10 条），polling 5s 精度足够，提醒间隔本来就是分钟级。代码 ~70 行 vs
//!   Notify+Heap 方案 ~150 行的 trade-off。
//! - reminder.create/update/snooze/complete 等 IPC 写后立即调 `reload_reminders()` 让其
//!   多走一次 find_due+fire（≈ "eager check"），不等下一个 5s tick。fire 内有防重入
//!   （`reminder_history.fired_at >= reminders.next_fire_at` 的 NOT EXISTS 过滤），eager
//!   与 polling 同时跑也安全。
//! - TimerKind variant 给 #28 番茄 / M3 IdleDetector 占位；本 issue 只 dispatch Reminder。
//!
//! 复用边界:
//! - #28 番茄会复用 polling task：tick 时检查 pomodoro KV 是否到点。
//! - M3 IdleDetector 同 polling 模式：tick 时调 GetLastInputInfo 比较。
//!
//! ConsentGate 门禁：onboarding 未完成时跳过 tick（参考 living_pet.rs::start_scheduler）。

use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};

use crate::services::consent_gate::ConsentGate;
use crate::services::reminder;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind {
    Reminder,
    Pomodoro, // #28 follow-up
    Idle,     // M3 follow-up
}

const TICK_INTERVAL_SEC: u64 = 5;

/// 启动期 spawn 调度 task。lib.rs::setup 调用一次即可，task 与进程同生命周期。
///
/// dev 期实测：5s 间隔已经够短，没有像 living_pet 那样另开 env var 缩短的必要。
pub fn start<R: Runtime + 'static>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(TICK_INTERVAL_SEC)).await;

            // ConsentGate 门禁：onboarding 未完成时跳过（参考 living_pet.rs::start_scheduler）。
            // gate 关闭期间用户还在 onboarding 窗，发提醒只会惊吓。
            let gate_open = app
                .try_state::<ConsentGate>()
                .map(|g| g.is_open())
                .unwrap_or(false);
            if !gate_open {
                continue;
            }

            // 查 + dispatch
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
        }
    });
}

/// IPC 写后调（create/update/snooze/complete 等）：立刻 check 一次 find_due+fire，
/// 不等下一个 5s tick。fire 内有防重入，与 polling 并发安全。
///
/// 启动期 lib.rs::setup 在 catch_up_overdue 后也调一次，让首发 reminder 不必等 5s。
pub async fn reload_reminders<R: Runtime>(app: &AppHandle<R>) -> Result<(), reminder::ReminderError> {
    let ids = reminder::find_due(app).await?;
    for id in ids {
        if let Err(e) = reminder::fire(app, &id).await {
            eprintln!("[scheduler] eager fire {id} failed: {e}");
        }
    }
    Ok(())
}
