// LifecycleManager — Spec §6.1 / §8.2。Phase A0 仅 5 顶层 state, 不含 Live sub-state。

use parking_lot::RwLock;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Booting,
    Live,
    Suspending,
    Waking,
    ShuttingDown,
}

#[derive(Debug, Error)]
pub enum TransitionError {
    #[error("invalid transition: {from:?} → {to:?}")]
    Invalid { from: LifecycleState, to: LifecycleState },
}

pub struct LifecycleManager {
    state: Arc<RwLock<LifecycleState>>,
}

impl LifecycleManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(LifecycleState::Booting)),
        }
    }

    pub fn current_state(&self) -> LifecycleState {
        *self.state.read()
    }

    pub fn transition(&self, to: LifecycleState) -> Result<(), TransitionError> {
        let mut state = self.state.write();
        let from = *state;
        let valid = matches!(
            (from, to),
            (LifecycleState::Booting, LifecycleState::Live)
                | (LifecycleState::Live, LifecycleState::Suspending)
                | (LifecycleState::Suspending, LifecycleState::Waking)
                | (LifecycleState::Waking, LifecycleState::Live)
                | (LifecycleState::Live, LifecycleState::ShuttingDown)
        );
        if !valid {
            return Err(TransitionError::Invalid { from, to });
        }
        *state = to;
        Ok(())
    }
}

impl Default for LifecycleManager {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_booting() {
        let mgr = LifecycleManager::new();
        assert_eq!(mgr.current_state(), LifecycleState::Booting);
    }

    #[test]
    fn booting_to_live_is_valid() {
        let mgr = LifecycleManager::new();
        mgr.transition(LifecycleState::Live).unwrap();
        assert_eq!(mgr.current_state(), LifecycleState::Live);
    }

    #[test]
    fn booting_directly_to_suspending_is_invalid() {
        let mgr = LifecycleManager::new();
        let result = mgr.transition(LifecycleState::Suspending);
        assert!(matches!(result, Err(TransitionError::Invalid { .. })));
    }

    #[test]
    fn suspend_wake_resume_cycle() {
        let mgr = LifecycleManager::new();
        mgr.transition(LifecycleState::Live).unwrap();
        mgr.transition(LifecycleState::Suspending).unwrap();
        mgr.transition(LifecycleState::Waking).unwrap();
        mgr.transition(LifecycleState::Live).unwrap();
        assert_eq!(mgr.current_state(), LifecycleState::Live);
    }
}
