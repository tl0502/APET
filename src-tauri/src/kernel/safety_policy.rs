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
pub struct ConfigKvSafetyPolicy {
    pub(crate) db_path: PathBuf,
    pub(crate) prefix: Arc<AtomicBool>,
    pub(crate) user_input: Arc<AtomicBool>,
    pub(crate) stream_token: Arc<AtomicBool>,
    pub(crate) final_output: Arc<AtomicBool>,
}

impl ConfigKvSafetyPolicy {
    /// Boot 期同步阻塞读 4 个 KV，缺失或解析失败 fallback false (出厂状态 = 全 OFF)。
    /// DB 连接失败 → fallback 全 OFF + eprintln warning (保守原则: 安全功能默认 off 与"零 overhead 起步"语义一致)。
    pub async fn load_from_kv(db_path: &std::path::Path) -> Result<Self, PolicyError> {
        let prefix = read_kv_bool_or_false(db_path, SafetyScope::PrefixInjection).await;
        let user_input = read_kv_bool_or_false(db_path, SafetyScope::UserInput).await;
        let stream_token = read_kv_bool_or_false(db_path, SafetyScope::StreamToken).await;
        let final_output = read_kv_bool_or_false(db_path, SafetyScope::FinalOutput).await;
        Ok(Self {
            db_path: db_path.to_path_buf(),
            prefix: Arc::new(AtomicBool::new(prefix)),
            user_input: Arc::new(AtomicBool::new(user_input)),
            stream_token: Arc::new(AtomicBool::new(stream_token)),
            final_output: Arc::new(AtomicBool::new(final_output)),
        })
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

/// 内部 helper: 读单个 KV bool, 任何失败 fallback false + eprintln warning。
async fn read_kv_bool_or_false(db_path: &std::path::Path, scope: SafetyScope) -> bool {
    match crate::services::db::connect_at(db_path).await {
        Ok(mut conn) => match crate::services::config::get_with_conn(&mut conn, scope.kv_key()).await {
            Ok(Some(s)) => match s.trim().parse::<bool>() {
                Ok(b) => b,
                Err(_) => {
                    eprintln!(
                        "[safety_policy] KV {} value {:?} is not a valid bool, fallback to false",
                        scope.kv_key(),
                        s
                    );
                    false
                }
            },
            Ok(None) => false, // KV 不存在 = 出厂状态 = OFF
            Err(e) => {
                eprintln!(
                    "[safety_policy] config::get_with_conn failed for {}: {}, fallback to false",
                    scope.kv_key(),
                    e
                );
                false
            }
        },
        Err(e) => {
            eprintln!(
                "[safety_policy] connect_at failed for {}: {}, fallback to false (Denied invariant: 保守不开扫描)",
                scope.kv_key(),
                e
            );
            false
        }
    }
}

#[async_trait]
impl SafetyPolicy for ConfigKvSafetyPolicy {
    fn is_enabled(&self, scope: SafetyScope) -> bool {
        self.slot(scope).load(Ordering::Relaxed)
    }

    /// 先写 DB 成功后才更新内存 AtomicBool, 保证 DB 与内存一致。
    /// DB 失败时内存不变, 返 Err 给 IPC caller (UI toast)。
    async fn set_enabled(&self, scope: SafetyScope, enabled: bool) -> Result<(), PolicyError> {
        let mut conn = crate::services::db::connect_at(&self.db_path)
            .await
            .map_err(|e| PolicyError::Db(e.to_string()))?;
        let now = chrono::Utc::now().to_rfc3339();
        crate::services::config::set_with_conn(&mut conn, scope.kv_key(), &enabled.to_string(), &now)
            .await
            .map_err(|e| PolicyError::Config(e.to_string()))?;
        self.slot(scope).store(enabled, Ordering::Relaxed);
        Ok(())
    }
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

    #[tokio::test]
    async fn config_kv_policy_falls_back_to_all_off_when_db_empty() {
        let (_dir, _conn) = crate::services::test_db::fresh_db().await;
        let db_path = _dir.path().join("aipet.db");

        let policy = ConfigKvSafetyPolicy::load_from_kv(&db_path).await.unwrap();
        assert!(!policy.is_enabled(SafetyScope::PrefixInjection));
        assert!(!policy.is_enabled(SafetyScope::UserInput));
        assert!(!policy.is_enabled(SafetyScope::StreamToken));
        assert!(!policy.is_enabled(SafetyScope::FinalOutput));
    }

    #[tokio::test]
    async fn config_kv_policy_set_enabled_updates_both_db_and_memory() {
        let (_dir, _conn) = crate::services::test_db::fresh_db().await;
        let db_path = _dir.path().join("aipet.db");

        let policy = ConfigKvSafetyPolicy::load_from_kv(&db_path).await.unwrap();
        // 初始全 OFF
        assert!(!policy.is_enabled(SafetyScope::FinalOutput));

        // 翻 FinalOutput ON
        policy.set_enabled(SafetyScope::FinalOutput, true).await.unwrap();
        assert!(policy.is_enabled(SafetyScope::FinalOutput));

        // Reload from KV 应仍是 true (DB 持久化生效)
        let policy2 = ConfigKvSafetyPolicy::load_from_kv(&db_path).await.unwrap();
        assert!(policy2.is_enabled(SafetyScope::FinalOutput));
        assert!(!policy2.is_enabled(SafetyScope::PrefixInjection));
    }

    #[tokio::test]
    async fn config_kv_policy_handles_invalid_bool_string_as_false() {
        let (_dir, mut conn) = crate::services::test_db::fresh_db().await;
        let db_path = _dir.path().join("aipet.db");

        // 手动写一个非法 bool 字符串
        let now = chrono::Utc::now().to_rfc3339();
        crate::services::config::set_with_conn(
            &mut conn,
            SafetyScope::UserInput.kv_key(),
            "not-a-bool",
            &now,
        )
        .await
        .unwrap();

        let policy = ConfigKvSafetyPolicy::load_from_kv(&db_path).await.unwrap();
        assert!(!policy.is_enabled(SafetyScope::UserInput));
    }
}
