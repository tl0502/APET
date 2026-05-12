//! ConsentGate AppState — onboarding 是否已完成的运行时状态缓存。
//!
//! 设计：写极少（仅 `commands::window::onboarding_complete` 翻 true 一次），读密集
//! （window_actions 每条 show/toggle 路径 + shortcuts handler 都过）。AtomicBool 无锁、
//! 比 Mutex<bool> 更轻；Tauri State 要求 Send+Sync，AtomicBool 满足。
//!
//! 初值由 `lib.rs::setup` 根据 consent + onboarding KV 决定（与 setup 阶段"是否进 pet
//! 主态"的分支判定结果一致，避免后续每条路径都重复查 DB）。
//!
//! 不需要"翻 false"的路径：M1 NeedReconsent 走重启路径，撤回同意 / 删除账号是 M3
//! 数据治理范围。
use std::sync::atomic::{AtomicBool, Ordering};

/// Onboarding 已完成的进程级闸门。
///
/// `is_open() == true` 即等价于 setup 路由判定的"可进 pet 主态"
/// （consent.granted && version OK && onboarding KV 不存在）。
pub struct ConsentGate(AtomicBool);

impl ConsentGate {
    pub fn new(open: bool) -> Self {
        Self(AtomicBool::new(open))
    }

    pub fn is_open(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn open(&self) {
        self.0.store(true, Ordering::Release)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_false_is_closed() {
        let g = ConsentGate::new(false);
        assert!(!g.is_open());
    }

    #[test]
    fn new_true_is_open() {
        let g = ConsentGate::new(true);
        assert!(g.is_open());
    }

    #[test]
    fn open_flips_closed_gate() {
        let g = ConsentGate::new(false);
        g.open();
        assert!(g.is_open());
    }

    #[test]
    fn open_is_idempotent() {
        let g = ConsentGate::new(true);
        g.open();
        g.open();
        assert!(g.is_open());
    }
}
