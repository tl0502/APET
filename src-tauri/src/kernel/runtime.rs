// Kernel — Phase A0 5 件套总聚合 (Spec §4.2 + §6.2 Boot 序列)。
// lib.rs setup hook 调用 Kernel::boot 完成 Boot 1-7。

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

use crate::kernel::crypto::{CryptoService, DpapiCryptoService};
use crate::kernel::grant_broker::{DenyAllGrantBroker, GrantBroker};
use crate::kernel::lifecycle_manager::{LifecycleManager, LifecycleState, TransitionError};
use crate::kernel::permission_service::{DenyOnlyPermissionService, PermissionService};
use crate::kernel::repos::{PermissionRepo, SecretRepo};
use crate::kernel::safety_guard::{SafetyError, SafetyGuard, SafetyGuardImpl};
use crate::kernel::safety_policy::{ConfigKvSafetyPolicy, PolicyError, SafetyPolicy};
use crate::kernel::state_store::StateStore;

/// Phase A0 boot 时编译嵌入的 ADR-006 prefix; runtime 不可篡改。
/// 路径相对 runtime.rs (src-tauri/src/kernel/runtime.rs) 上行 3 级到项目根。
const SAFETY_PREFIX: &str = include_str!("../../../assets/safety/prefix_v1.txt");

#[derive(Debug, Error)]
pub enum BootError {
    #[error("safety guard load failed: {0}")]
    Safety(#[from] SafetyError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("transition: {0}")]
    Transition(#[from] TransitionError),
    #[error("safety policy load failed: {0}")]
    SafetyPolicy(#[from] PolicyError),
}

/// Phase A0 Kernel 总句柄 — 注入到 Tauri State, 供 commands 用。
pub struct Kernel {
    pub state_store: Arc<StateStore>,
    pub safety_policy: Arc<dyn SafetyPolicy>,
    pub safety_guard: Arc<dyn SafetyGuard>,
    pub permission_service: Arc<dyn PermissionService>,
    pub grant_broker: Arc<dyn GrantBroker>,
    pub crypto: Arc<dyn CryptoService>,
    pub secret_repo: Arc<SecretRepo>,
    pub lifecycle: Arc<LifecycleManager>,
}

impl Kernel {
    /// Boot 1-7 序列。db_path 由调用方提供 (Tauri AppHandle.app_config_dir + aipet.db)。
    ///
    /// Boot steps:
    /// 1. MigrationService — 由 tauri-plugin-sql 自动执行 (lib.rs setup 之前)
    /// 2. open_app_db — 由 services::db 已有 (此处不持 Pool, 每次 commands acquire)
    /// 3. SafetyGuard (compile-time prefix)
    /// 4. PermissionService (DenyOnly)
    /// 5. GrantBroker (DenyAll, Phase A0 无 Tool)
    /// 6. CryptoService + SecretRepo
    /// 7. LifecycleManager → Live
    pub fn boot(db_path: PathBuf) -> Result<Self, BootError> {
        // Boot 3a: SafetyPolicy (4 KV, 出厂全 OFF)
        let safety_policy: Arc<dyn SafetyPolicy> = Arc::new(tauri::async_runtime::block_on(
            ConfigKvSafetyPolicy::load_from_kv(&db_path),
        )?);

        // Boot 3b: SafetyGuard (compile-time prefix + policy dependency)
        let safety_guard: Arc<dyn SafetyGuard> = Arc::new(SafetyGuardImpl::from_text_with_policy(
            SAFETY_PREFIX,
            Arc::clone(&safety_policy),
        )?);

        // Boot 4: PermissionService (DenyOnly)
        let permission_repo = Arc::new(PermissionRepo::new());
        let permission_service: Arc<dyn PermissionService> = Arc::new(
            DenyOnlyPermissionService::new(permission_repo, db_path.clone()),
        );

        // Boot 5: GrantBroker (DenyAll, Phase A0 无 Tool)
        let grant_broker: Arc<dyn GrantBroker> = Arc::new(DenyAllGrantBroker);

        // Boot 6: CryptoService + SecretRepo
        let crypto: Arc<dyn CryptoService> = Arc::new(DpapiCryptoService);
        let secret_repo = Arc::new(SecretRepo::new(Arc::clone(&crypto)));

        // Boot 7: LifecycleManager → Live
        let lifecycle = Arc::new(LifecycleManager::new());
        lifecycle.transition(LifecycleState::Live)?;

        // StateStore (Repository 注册中心, Phase A0 不持 Pool)
        let state_store = Arc::new(StateStore::new());

        Ok(Self {
            state_store,
            safety_policy,
            safety_guard,
            permission_service,
            grant_broker,
            crypto,
            secret_repo,
            lifecycle,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_produces_live_lifecycle_and_all_components() {
        let db_path = std::env::temp_dir().join("boot_test_db.sqlite");
        let kernel =
            Kernel::boot(db_path).expect("Kernel::boot should succeed with embedded prefix");
        assert_eq!(kernel.lifecycle.current_state(), LifecycleState::Live);
        // 不检查具体类型, 但确保 Arc 字段非空 (compile 即证明; runtime sanity)
        assert!(Arc::strong_count(&kernel.lifecycle) >= 1);
    }
}
