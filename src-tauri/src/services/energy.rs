//! EnergyService（#41，模块 I.3）— 桌宠精力衰减，全 transient 不持久。
//!
//! ## 锁定项（PRD §7.9.3 / line 1073 / ADR-025）
//!
//! - **全 transient 不持久**（PRD line 1073 "精力值(瞬态,不持久)"）：app 启动 initial=80，
//!   不读 KV / 不写 KV / 跨重启 reset
//! - **可逆 / 非养成**（PRD §7.11）：用户回来交互即恢复，无"喂养"无"流失"
//! - **idle-driven 不是 wall-clock**：休眠期间 idle_ms 不增，不被衰干
//!
//! ## 规则
//!
//! - 范围 0-100，启动 initial=80
//! - idle > 5min 后开始衰减，每多 5min -1（首次跨阈值即 -1）
//! - 任何 interaction 调 `boost()` → +5，cap 100；同时由 #39 GetLastInputInfo 自然
//!   更新 idle_ms（用户鼠标点击是 OS 级输入事件），下次 tick 看到 idle < 5min 自动重置
//!   `last_decay_at` —— 即"用户回来即恢复"语义
//!
//! ## 驱动
//!
//! - **scheduler 1s tick**（合用 #22 1s scheduler，每 60 步分频）：调 `tick_decay(idle_ms)`

use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

const INITIAL: u8 = 80;
const IDLE_THRESHOLD_MS: u64 = 5 * 60 * 1000; // 5min
const DECAY_INTERVAL: Duration = Duration::from_secs(5 * 60); // 5min
const INTERACTION_BOOST: u8 = 5;
const ENERGY_MAX: u8 = 100;

/// EnergyState 进程级，纯内存。`app.manage(EnergyState::default())` 共享。
pub struct EnergyState {
    value: AtomicU8,
    /// 上次衰减时刻；None = 当前 idle < 阈值（reset 状态）
    last_decay_at: Mutex<Option<Instant>>,
}

impl Default for EnergyState {
    fn default() -> Self {
        Self {
            value: AtomicU8::new(INITIAL),
            last_decay_at: Mutex::new(None),
        }
    }
}

impl EnergyState {
    pub fn get(&self) -> u8 {
        self.value.load(Ordering::Relaxed)
    }

    /// #40 InteractionRouter 调：+5 cap 100。
    pub fn boost(&self) {
        let _ = self
            .value
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                Some(v.saturating_add(INTERACTION_BOOST).min(ENERGY_MAX))
            });
    }

    /// scheduler 1min 分频 tick 调：根据 idle_ms 决定是否 -1。
    ///
    /// 返回 `true` 表示这个 tick 真的衰减了 1 点（用于 metrics/debug）。
    pub fn tick_decay(&self, idle_ms: u64) -> bool {
        // 用户有近期活动（idle 未过阈值）→ reset
        if idle_ms < IDLE_THRESHOLD_MS {
            *self.last_decay_at.lock().unwrap() = None;
            return false;
        }

        let now = Instant::now();
        let mut last = self.last_decay_at.lock().unwrap();

        let should_decay = match *last {
            None => true, // 首次跨阈值
            Some(t) => now.duration_since(t) >= DECAY_INTERVAL,
        };

        if should_decay {
            *last = Some(now);
            let _ = self
                .value
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                    Some(v.saturating_sub(1))
                });
            true
        } else {
            false
        }
    }

    /// 仅测试：直接设置 value（绕过衰减/boost）。
    #[cfg(test)]
    pub fn set_for_test(&self, v: u8) {
        self.value.store(v, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_is_80() {
        let s = EnergyState::default();
        assert_eq!(s.get(), INITIAL);
    }

    #[test]
    fn boost_plus_5_caps_at_100() {
        let s = EnergyState::default();
        s.set_for_test(98);
        s.boost();
        assert_eq!(s.get(), 100, "+5 但 cap 100");
        s.boost();
        assert_eq!(s.get(), 100, "已 cap 不再增");
    }

    #[test]
    fn boost_normal_path() {
        let s = EnergyState::default();
        s.set_for_test(50);
        s.boost();
        assert_eq!(s.get(), 55);
    }

    #[test]
    fn tick_decay_below_threshold_no_op_and_resets() {
        let s = EnergyState::default();
        let before = s.get();
        let decayed = s.tick_decay(1000); // 1s idle, 远未到 5min
        assert!(!decayed);
        assert_eq!(s.get(), before);
        // last_decay_at 应该是 None（reset 状态）
        assert!(s.last_decay_at.lock().unwrap().is_none());
    }

    #[test]
    fn tick_decay_first_cross_threshold_immediately_decays() {
        let s = EnergyState::default();
        s.set_for_test(50);
        // idle_ms 刚好超过 5min 阈值
        let decayed = s.tick_decay(IDLE_THRESHOLD_MS + 1);
        assert!(decayed);
        assert_eq!(s.get(), 49);
    }

    #[test]
    fn tick_decay_second_call_within_5min_no_op() {
        let s = EnergyState::default();
        s.set_for_test(50);
        s.tick_decay(IDLE_THRESHOLD_MS + 1); // 首次 -1
        // 立即第二次（last_decay_at 写入时间不到 5min）
        let decayed = s.tick_decay(IDLE_THRESHOLD_MS + 2000);
        assert!(!decayed, "5min 内不应再衰减");
        assert_eq!(s.get(), 49);
    }

    #[test]
    fn tick_decay_floor_at_0() {
        let s = EnergyState::default();
        s.set_for_test(0);
        let decayed = s.tick_decay(IDLE_THRESHOLD_MS + 1);
        assert!(decayed, "tick 触发了（虽然 value 已 floor）");
        assert_eq!(s.get(), 0, "saturating_sub 保护 floor");
    }

    #[test]
    fn user_returns_then_reset_decay_timer() {
        let s = EnergyState::default();
        s.tick_decay(IDLE_THRESHOLD_MS + 1); // 首次衰减
        assert!(s.last_decay_at.lock().unwrap().is_some());
        // 用户回来活动
        s.tick_decay(500); // idle 重置
        assert!(s.last_decay_at.lock().unwrap().is_none(), "用户回来后 reset");
    }
}
