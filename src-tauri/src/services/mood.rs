//! MoodService（#41，模块 I.2）— 桌宠 mood 状态机，全 transient 不持久。
//!
//! ## 锁定项（PRD §7.9 / 决策 20 / ADR-025）
//!
//! - **全 transient 不持久**（PRD line 1073 / 1089）：永远不写 `pet_runtime_state.mood` 表；
//!   进程退出即失，启动期回 Neutral。
//! - **mood values**：`Neutral / Happy / Focused / Sleepy / Cozy / Annoyed`
//! - **优先级**（高 → 低）：`Annoyed (transient) > Focused (FOCUS 期) > Sleepy (energy<30) >
//!   Cozy (22:00-00:00 时段) > Happy (互动 10min) > Neutral`
//!
//! ## 驱动源
//!
//! - **#40 InteractionRouter**：emit `pet:interaction_reacted` 含 `mood_delta` 字段
//!   ("happy"/"annoyed"/"calm"/"neutral")；本服务 `apply_delta` 转 transient push
//! - **#28 PomodoroService**：FOCUS → `set_focused(true)`；REST/IDLE → `set_focused(false)`
//! - **#22/#28 scheduler 1s tick**（合用，每 60 个 tick 分频一次）：调 `tick_periodic(energy)`
//!   更新 base mood（cozy 时段 / energy 低 / 默认 Neutral）
//!
//! ## 实现要点
//!
//! - base + transient 分离：`base` 是稳态（time-driven），`happy_until` / `annoyed_until` 是
//!   transient（事件驱动）；`compute_current` 按优先级合并
//! - `Happy` 仅在 `base = Neutral` 时显示（focused/sleepy/cozy 都是更"重要"的 mood，
//!   不被互动 push 覆盖）
//! - `Annoyed` 5s window 压制一切，与 #40 N.3 抗议 5s revert 语义一致

use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::Timelike;
use serde::{Deserialize, Serialize};

/// 互动 push happy 持续时长（PRD §7.9.2 / issue body）。
const HAPPY_DURATION: Duration = Duration::from_secs(10 * 60);

/// 抗议 push annoyed 持续时长（与 #40 N.3 抗议 5s revert 对齐 / 决策 20）。
const ANNOYED_DURATION: Duration = Duration::from_secs(5);

/// energy 低于此阈值时 base 进 Sleepy（PRD §7.9.3 / issue body）。
const SLEEPY_ENERGY_THRESHOLD: u8 = 30;

/// Cozy 时段起始小时（22:00）。
const COZY_START_HOUR: u32 = 22;

/// Mood 标签。serialize 走 lowercase 与前端 emoji 表一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mood {
    Neutral,
    Happy,
    Focused,
    Sleepy,
    Cozy,
    Annoyed,
}

/// MoodState 进程级 state，纯内存。`app.manage(MoodState::default())` 共享。
pub struct MoodState {
    /// 时段 / FOCUS / energy 驱动的 base mood
    base: Mutex<Mood>,
    /// 互动 push happy 的失效时刻（None = 无 happy push）
    happy_until: Mutex<Option<Instant>>,
    /// 抗议 push annoyed 的失效时刻（None = 无 annoyed push）
    annoyed_until: Mutex<Option<Instant>>,
}

impl Default for MoodState {
    fn default() -> Self {
        Self {
            base: Mutex::new(Mood::Neutral),
            happy_until: Mutex::new(None),
            annoyed_until: Mutex::new(None),
        }
    }
}

impl MoodState {
    /// 计算当前展示的 mood，按优先级合并 base + transient。
    pub fn compute_current(&self) -> Mood {
        let now = Instant::now();

        // P1: Annoyed transient（最高，#40 N.3 抗议）
        if let Some(t) = *self.annoyed_until.lock().unwrap() {
            if now < t {
                return Mood::Annoyed;
            }
        }

        let base = *self.base.lock().unwrap();

        // P2-4: base 已含 Focused / Sleepy / Cozy（由 tick_periodic 维护）
        // P5: Happy transient 仅在 base = Neutral 时显示（不覆盖更"重要"的 mood）
        if matches!(base, Mood::Neutral) {
            if let Some(t) = *self.happy_until.lock().unwrap() {
                if now < t {
                    return Mood::Happy;
                }
            }
        }

        base
    }

    /// #40 InteractionRouter 调：apply mood_delta 字符串。
    pub fn apply_delta(&self, delta: &str) {
        match delta {
            "happy" => {
                *self.happy_until.lock().unwrap() = Some(Instant::now() + HAPPY_DURATION);
            }
            "annoyed" => {
                *self.annoyed_until.lock().unwrap() = Some(Instant::now() + ANNOYED_DURATION);
            }
            // "calm" / "neutral" / unknown：no-op（calm 是长按"保持平静"，不主动 push mood）
            _ => {}
        }
    }

    /// #28 PomodoroService listener 调：FOCUS 期切 Focused，FOCUS_END 恢复 Neutral。
    /// 不是 FOCUS 状态时不动 base（避免与 cozy/sleepy 抢）。
    pub fn set_focused(&self, focused: bool) {
        let mut b = self.base.lock().unwrap();
        if focused {
            *b = Mood::Focused;
        } else if matches!(*b, Mood::Focused) {
            // 仅在 base 还是 Focused 时复位，避免覆盖 tick_periodic 已经写入的其他 base
            *b = Mood::Neutral;
        }
    }

    /// scheduler 1min 分频 tick 调：根据时段 + energy 更新 base mood。
    /// FOCUS 期 base 不动（pomodoro listener 控制）。
    pub fn tick_periodic(&self, energy: u8) {
        let mut b = self.base.lock().unwrap();

        if matches!(*b, Mood::Focused) {
            return;
        }

        // base 优先级：Sleepy > Cozy > Neutral
        *b = compute_base_mood(energy, current_hour_local());
    }

    /// 测试 + IPC 暴露：仅返回 base，不含 transient。
    #[allow(dead_code)]
    pub fn base(&self) -> Mood {
        *self.base.lock().unwrap()
    }
}

/// 22:00-00:00 (含 22 与 23 两个 hour，不含 0)
fn is_cozy_time(hour: u32) -> bool {
    hour >= COZY_START_HOUR
}

fn current_hour_local() -> u32 {
    chrono::Local::now().hour()
}

/// 抽离方便单测（不依赖当前时间）。
fn compute_base_mood(energy: u8, hour: u32) -> Mood {
    if energy < SLEEPY_ENERGY_THRESHOLD {
        Mood::Sleepy
    } else if is_cozy_time(hour) {
        Mood::Cozy
    } else {
        Mood::Neutral
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_neutral() {
        let s = MoodState::default();
        assert_eq!(s.compute_current(), Mood::Neutral);
    }

    #[test]
    fn happy_push_shows_when_base_neutral() {
        let s = MoodState::default();
        s.apply_delta("happy");
        assert_eq!(s.compute_current(), Mood::Happy);
    }

    #[test]
    fn happy_push_does_not_override_focused() {
        let s = MoodState::default();
        s.set_focused(true);
        s.apply_delta("happy");
        assert_eq!(s.compute_current(), Mood::Focused, "focused 优先于 happy");
    }

    #[test]
    fn annoyed_overrides_everything() {
        let s = MoodState::default();
        s.set_focused(true);
        s.apply_delta("happy");
        s.apply_delta("annoyed");
        assert_eq!(s.compute_current(), Mood::Annoyed, "annoyed 最高优先");
    }

    #[test]
    fn annoyed_expires_after_5s_window() {
        let s = MoodState::default();
        // 直接写 annoyed_until 已经过期的时间点
        *s.annoyed_until.lock().unwrap() = Some(Instant::now() - Duration::from_millis(100));
        assert_ne!(s.compute_current(), Mood::Annoyed, "过期后不再 annoyed");
    }

    #[test]
    fn happy_expires_after_10min_window() {
        let s = MoodState::default();
        *s.happy_until.lock().unwrap() = Some(Instant::now() - Duration::from_millis(100));
        assert_eq!(s.compute_current(), Mood::Neutral, "过期后回 neutral");
    }

    #[test]
    fn calm_and_neutral_delta_no_op() {
        let s = MoodState::default();
        s.apply_delta("calm");
        s.apply_delta("neutral");
        s.apply_delta("unknown_label");
        assert_eq!(s.compute_current(), Mood::Neutral);
        assert!(s.happy_until.lock().unwrap().is_none());
        assert!(s.annoyed_until.lock().unwrap().is_none());
    }

    #[test]
    fn set_focused_then_unfocused_returns_to_neutral() {
        let s = MoodState::default();
        s.set_focused(true);
        assert_eq!(s.compute_current(), Mood::Focused);
        s.set_focused(false);
        assert_eq!(s.compute_current(), Mood::Neutral);
    }

    #[test]
    fn set_focused_false_does_not_overwrite_non_focused_base() {
        let s = MoodState::default();
        // base 由 tick_periodic 推到 Sleepy（energy 低）
        s.tick_periodic(20);
        assert_eq!(s.base(), Mood::Sleepy);
        // pomodoro 发 FOCUS_END，但 base 不是 Focused —— 不应覆盖 Sleepy
        s.set_focused(false);
        assert_eq!(s.base(), Mood::Sleepy);
    }

    #[test]
    fn tick_periodic_low_energy_sleepy() {
        let s = MoodState::default();
        s.tick_periodic(20);
        assert_eq!(s.base(), Mood::Sleepy);
    }

    #[test]
    fn tick_periodic_focus_skips() {
        let s = MoodState::default();
        s.set_focused(true);
        s.tick_periodic(20); // 即使 energy 低，FOCUS 期也不动
        assert_eq!(s.base(), Mood::Focused);
    }

    #[test]
    fn compute_base_mood_priority() {
        // energy < 30 → Sleepy（高优于 cozy 时段）
        assert_eq!(compute_base_mood(20, 23), Mood::Sleepy);
        // energy ≥ 30 + 22:00-00:00 → Cozy
        assert_eq!(compute_base_mood(50, 22), Mood::Cozy);
        assert_eq!(compute_base_mood(50, 23), Mood::Cozy);
        // energy ≥ 30 + 非 cozy 时段 → Neutral
        assert_eq!(compute_base_mood(80, 14), Mood::Neutral);
        // 边界：00:00 不算 cozy（COZY_START_HOUR=22 单向）
        assert_eq!(compute_base_mood(80, 0), Mood::Neutral);
        // 边界：21:59 不算 cozy
        assert_eq!(compute_base_mood(80, 21), Mood::Neutral);
    }
}
