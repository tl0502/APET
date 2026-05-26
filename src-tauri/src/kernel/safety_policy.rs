// SafetyPolicy — kernel-owned, SafetyGuardImpl 的依赖。Spec §6.6.0 (Updated 2026-05-26)。
// 4 个 scope 出厂全 OFF; ConfigKvSafetyPolicy 持 Arc<AtomicBool>×4 + boot 时同步读 KV。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafetyScope {
    /// wrap_messages 是否注入 ADR-006 prefix
    PrefixInjection,
    /// scan_user_input 是否真扫
    UserInput,
    /// scan_token 是否真扫 (mid-stream)
    StreamToken,
    /// scan_final 是否真扫 (流终全文)
    FinalOutput,
}

impl SafetyScope {
    pub fn kv_key(&self) -> &'static str {
        match self {
            Self::PrefixInjection => "safety:prefix_enabled",
            Self::UserInput => "safety:scan_user_input_enabled",
            Self::StreamToken => "safety:scan_token_enabled",
            Self::FinalOutput => "safety:scan_final_enabled",
        }
    }
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("db error: {0}")]
    Db(String),
    #[error("config error: {0}")]
    Config(String),
}

#[async_trait]
pub trait SafetyPolicy: Send + Sync {
    fn is_enabled(&self, scope: SafetyScope) -> bool;
    async fn set_enabled(&self, scope: SafetyScope, enabled: bool) -> Result<(), PolicyError>;
}

/// 测试用: ChatService / SafetyGuard 单测注入。直接 4 AtomicBool 不走 DB。
pub struct MockSafetyPolicy {
    prefix: Arc<AtomicBool>,
    user_input: Arc<AtomicBool>,
    stream_token: Arc<AtomicBool>,
    final_output: Arc<AtomicBool>,
}

impl MockSafetyPolicy {
    /// 4 个 scope 全 OFF
    pub fn all_off() -> Self {
        Self {
            prefix: Arc::new(AtomicBool::new(false)),
            user_input: Arc::new(AtomicBool::new(false)),
            stream_token: Arc::new(AtomicBool::new(false)),
            final_output: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 4 个 scope 全 ON
    pub fn all_on() -> Self {
        let s = Self::all_off();
        s.prefix.store(true, Ordering::Relaxed);
        s.user_input.store(true, Ordering::Relaxed);
        s.stream_token.store(true, Ordering::Relaxed);
        s.final_output.store(true, Ordering::Relaxed);
        s
    }

    fn slot(&self, scope: SafetyScope) -> &Arc<AtomicBool> {
        match scope {
            SafetyScope::PrefixInjection => &self.prefix,
            SafetyScope::UserInput => &self.user_input,
            SafetyScope::StreamToken => &self.stream_token,
            SafetyScope::FinalOutput => &self.final_output,
        }
    }
}

#[async_trait]
impl SafetyPolicy for MockSafetyPolicy {
    fn is_enabled(&self, scope: SafetyScope) -> bool {
        self.slot(scope).load(Ordering::Relaxed)
    }

    async fn set_enabled(&self, scope: SafetyScope, enabled: bool) -> Result<(), PolicyError> {
        self.slot(scope).store(enabled, Ordering::Relaxed);
        Ok(())
    }
}

/// 生产实施: 4 KV 持 Arc<AtomicBool>, boot 时同步读 KV 装载, 运行期 atomic 读不 hit DB。
/// 实现挂在 Task 4 — 此处仅占位结构, 暂不实现 SafetyPolicy trait, 避免空骨架编译告警。
pub struct ConfigKvSafetyPolicy {
    pub(crate) db_path: PathBuf,
    pub(crate) prefix: Arc<AtomicBool>,
    pub(crate) user_input: Arc<AtomicBool>,
    pub(crate) stream_token: Arc<AtomicBool>,
    pub(crate) final_output: Arc<AtomicBool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safety_scope_kv_key_returns_expected_const_string() {
        assert_eq!(SafetyScope::PrefixInjection.kv_key(), "safety:prefix_enabled");
        assert_eq!(SafetyScope::UserInput.kv_key(), "safety:scan_user_input_enabled");
        assert_eq!(SafetyScope::StreamToken.kv_key(), "safety:scan_token_enabled");
        assert_eq!(SafetyScope::FinalOutput.kv_key(), "safety:scan_final_enabled");
    }

    #[tokio::test]
    async fn mock_policy_all_off_returns_false_for_all_scopes() {
        let p = MockSafetyPolicy::all_off();
        assert!(!p.is_enabled(SafetyScope::PrefixInjection));
        assert!(!p.is_enabled(SafetyScope::UserInput));
        assert!(!p.is_enabled(SafetyScope::StreamToken));
        assert!(!p.is_enabled(SafetyScope::FinalOutput));
    }

    #[tokio::test]
    async fn mock_policy_all_on_returns_true_for_all_scopes() {
        let p = MockSafetyPolicy::all_on();
        assert!(p.is_enabled(SafetyScope::PrefixInjection));
        assert!(p.is_enabled(SafetyScope::UserInput));
        assert!(p.is_enabled(SafetyScope::StreamToken));
        assert!(p.is_enabled(SafetyScope::FinalOutput));
    }

    #[tokio::test]
    async fn mock_policy_set_enabled_toggles_scope() {
        let p = MockSafetyPolicy::all_off();
        p.set_enabled(SafetyScope::UserInput, true).await.unwrap();
        assert!(p.is_enabled(SafetyScope::UserInput));
        assert!(!p.is_enabled(SafetyScope::PrefixInjection));
        p.set_enabled(SafetyScope::UserInput, false).await.unwrap();
        assert!(!p.is_enabled(SafetyScope::UserInput));
    }
}
