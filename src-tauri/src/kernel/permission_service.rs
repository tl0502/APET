// PermissionService — Context Awareness 权限网关 (Spec §2.4 / §4.2 / §8.2).
// Phase A0: DenyOnlyPermissionService 永远拒绝, 不调任何 OS API。
//
// **CI 黑名单** (Task 8 落地脚本验证): 整个 src-tauri crate 不得 import 以下符号:
//   ❌ winapi / windows-sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow
//   ❌ windows-sys::Win32::UI::WindowsAndMessaging::GetWindowTextW
//   ❌ windows-sys::Win32::Graphics::Gdi::BitBlt
//   ❌ web_sys::Navigator::media_devices (getUserMedia / MediaRecorder)
//   ❌ tauri::clipboard 任何 read 操作
// 验证脚本: scripts/ci_check_os_context_apis.sh (Task 8)。

use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::kernel::repos::{PermissionRepo, RepoError};
use crate::services::db::DbError;

#[derive(Debug, Error)]
pub enum PermissionError {
    #[error("feature disabled in Phase A0 (DenyOnly)")]
    FeatureDisabled,
    #[error("denied: scope={scope}, reason={reason}")]
    Denied { scope: String, reason: String },
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("repo error: {0}")]
    Repo(#[from] RepoError),
}

/// Context Scope (Spec §8.2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextScope {
    ForegroundAppName,
    WindowTitle,
    SelectedText,
    MicrophoneAudio,
    ScreenText,
}

impl ContextScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ForegroundAppName => "foreground_app_name",
            Self::WindowTitle => "window_title",
            Self::SelectedText => "selected_text",
            Self::MicrophoneAudio => "microphone_audio",
            Self::ScreenText => "screen_text",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantSource {
    UserSettingsToggle,
    OnboardingFlow,
    GrantBrokerUpgrade,
    SystemDefault,
}

#[derive(Debug, Clone)]
pub struct ContextValue(pub String);

/// 用于审计的调用方 ID (Spec §8.2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemId {
    PersonaSub,
    MemorySub,
    ConversationSub,
    InitiativeSub,
    ToolSub,
    LivingSub,
    Surface,
    /// Stored as `"Soul"` in audit log (Spec §8.2 wording — abbreviated to match Soul subsystem id).
    SoulOverlay,
    Boot,
}

impl SubsystemId {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PersonaSub => "PersonaSub",
            Self::MemorySub => "MemorySub",
            Self::ConversationSub => "ConversationSub",
            Self::InitiativeSub => "InitiativeSub",
            Self::ToolSub => "ToolSub",
            Self::LivingSub => "LivingSub",
            Self::Surface => "Surface",
            Self::SoulOverlay => "Soul",
            Self::Boot => "Boot",
        }
    }
}

#[async_trait]
pub trait PermissionService: Send + Sync {
    fn is_granted(&self, scope: ContextScope) -> bool;
    async fn grant(
        &self,
        scope: ContextScope,
        by_action: GrantSource,
    ) -> Result<(), PermissionError>;
    async fn revoke(
        &self,
        scope: ContextScope,
        by_action: GrantSource,
    ) -> Result<(), PermissionError>;
    async fn read_context(
        &self,
        scope: ContextScope,
        used_for: &str,
        actor: SubsystemId,
    ) -> Result<Option<ContextValue>, PermissionError>;
}

/// Phase A0 唯一实现, 永远拒绝。
/// 持 Arc<PermissionRepo> + DB 路径; read_context 每次开新连接写一条 deny 审计后返 Denied。
pub struct DenyOnlyPermissionService {
    audit_repo: Arc<PermissionRepo>,
    db_path: std::path::PathBuf,
}

impl DenyOnlyPermissionService {
    /// 构造 DenyOnly 服务。
    ///
    /// **ORDER DEPENDENCY** (Spec §11 / Task 6 boot 1-7):
    /// 调用方必须保证 `db_path` 指向的 SQLite 文件已由 plugin migrations 建好
    /// (即 Task 6 boot 步骤 3 之后) 才能调 `read_context`。`connect_at` 用
    /// `create_if_missing(false)`, DB 不存在时 audit 写失败 — 但 Denied 不变量
    /// 由 `read_context` 内部 swallow-and-eprintln 保证, 不会回退到 Ok(Some)。
    pub fn new(audit_repo: Arc<PermissionRepo>, db_path: std::path::PathBuf) -> Self {
        Self {
            audit_repo,
            db_path,
        }
    }
}

#[async_trait]
impl PermissionService for DenyOnlyPermissionService {
    fn is_granted(&self, _: ContextScope) -> bool {
        false
    }

    async fn grant(&self, _: ContextScope, _: GrantSource) -> Result<(), PermissionError> {
        Err(PermissionError::FeatureDisabled)
    }

    async fn revoke(&self, _: ContextScope, _: GrantSource) -> Result<(), PermissionError> {
        Err(PermissionError::FeatureDisabled)
    }

    async fn read_context(
        &self,
        scope: ContextScope,
        used_for: &str,
        actor: SubsystemId,
    ) -> Result<Option<ContextValue>, PermissionError> {
        // **DenyOnly 不变量**: 无论 audit 写入是否成功, 必须返回 Denied。
        // 审计失败仅 eprintln 留痕, 绝不传播为 PermissionError::Db / PermissionError::Repo
        // (避免 caller 误以为不是 Denied 状态而 log-and-continue, 击穿 Phase A0 deny gate)。
        match crate::services::db::connect_at(&self.db_path).await {
            Ok(mut conn) => {
                if let Err(e) = self
                    .audit_repo
                    .append_denied(&mut conn, scope.as_str(), actor.as_str(), used_for, None)
                    .await
                {
                    eprintln!(
                        "[kernel.permission_service] audit write failed (scope={}, actor={}): {} \
                         — Denied invariant preserved",
                        scope.as_str(),
                        actor.as_str(),
                        e
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "[kernel.permission_service] connect_at failed for audit (scope={}, actor={}): {} \
                     — Denied invariant preserved",
                    scope.as_str(),
                    actor.as_str(),
                    e
                );
            }
        }
        Err(PermissionError::Denied {
            scope: scope.as_str().to_string(),
            reason: "Phase A0: DenyOnly".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deny_only_is_granted_returns_false_for_all_scopes() {
        let repo = Arc::new(PermissionRepo::new());
        let svc = DenyOnlyPermissionService::new(repo, std::path::PathBuf::from(":memory:"));
        assert!(!svc.is_granted(ContextScope::ForegroundAppName));
        assert!(!svc.is_granted(ContextScope::WindowTitle));
        assert!(!svc.is_granted(ContextScope::SelectedText));
        assert!(!svc.is_granted(ContextScope::MicrophoneAudio));
        assert!(!svc.is_granted(ContextScope::ScreenText));
    }

    #[tokio::test]
    async fn deny_only_grant_returns_feature_disabled() {
        let repo = Arc::new(PermissionRepo::new());
        let svc = DenyOnlyPermissionService::new(repo, std::path::PathBuf::from(":memory:"));
        let result = svc
            .grant(ContextScope::ForegroundAppName, GrantSource::UserSettingsToggle)
            .await;
        assert!(matches!(result, Err(PermissionError::FeatureDisabled)));
    }

    // read_context audit 写入测试在 Task 6 集成 lib.rs setup 后做 (需要真 DB path)

    #[tokio::test]
    async fn deny_only_read_context_returns_denied_even_when_db_missing() {
        // Pin Phase A0 invariant: DB unavailable → still Denied, never Db / Repo error
        let repo = Arc::new(PermissionRepo::new());
        let bogus_path = std::path::PathBuf::from("/__definitely_does_not_exist__/no.db");
        let svc = DenyOnlyPermissionService::new(repo, bogus_path);
        let result = svc
            .read_context(
                ContextScope::ForegroundAppName,
                "test_proactive_eval",
                SubsystemId::InitiativeSub,
            )
            .await;
        match result {
            Err(PermissionError::Denied { scope, .. }) => {
                assert_eq!(scope, "foreground_app_name");
            }
            other => panic!("expected Denied (invariant), got {:?}", other),
        }
    }
}
