// GrantBroker — Tool 同步授权 request/response (Spec §2.7 / §8.2, Constitution #13).
// Phase A0: trait + DenyAllGrantBroker + MockGrantBroker; 无 UI modal, 不接 ToolSub。
// Phase C: RealGrantBroker (含 UI modal + persistent cache + 真接 ToolSub)。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceId {
    Pet,
    Chat,
    Workspace,
    Tray,
}

#[derive(Debug, Clone)]
pub struct ToolArgsSummary {
    pub display_text: String, // UI 显示用 (Phase A0 仅占位)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantReason {
    FirstAccess,
    PathOutsideWhitelist,
    SensitiveOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolId(pub String);

#[derive(Debug, Clone)]
pub struct ScopeNarrowing {
    pub path_prefix: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum GrantDecision {
    AllowOnce,
    AllowSession(SessionId),
    AllowPersistent(ToolId, ScopeNarrowing),
    Deny,
    DenyAndDisable,
}

#[derive(Debug, Clone, Error)]
pub enum GrantError {
    #[error("timeout after {0:?}")]
    Timeout(Duration),
    #[error("user dismissed modal")]
    UserDismissed,
    #[error("surface unavailable: {0:?}")]
    SurfaceUnavailable(SurfaceId),
    #[error("feature disabled (Phase A0/A1/A2/B: DenyAllGrantBroker)")]
    FeatureDisabled,
}

#[async_trait]
pub trait GrantBroker: Send + Sync {
    async fn request_tool_grant(
        &self,
        surface: SurfaceId,
        tool_id: &str,
        args_summary: ToolArgsSummary,
        paths: Vec<PathBuf>,
        reason: GrantReason,
        persona_snapshot_id: &str,
    ) -> Result<GrantDecision, GrantError>;

    fn check_cached(&self, tool_id: &str, args_hash: &str) -> Option<GrantDecision>;
}

/// Phase A0/A1/A2/B 默认实现: 永远拒绝。ToolSub 不存在时永远不会被调用; 即使被调也立刻拒绝。
pub struct DenyAllGrantBroker;

#[async_trait]
impl GrantBroker for DenyAllGrantBroker {
    async fn request_tool_grant(
        &self,
        _surface: SurfaceId,
        _tool_id: &str,
        _args_summary: ToolArgsSummary,
        _paths: Vec<PathBuf>,
        _reason: GrantReason,
        _persona_snapshot_id: &str,
    ) -> Result<GrantDecision, GrantError> {
        Err(GrantError::FeatureDisabled)
    }

    fn check_cached(&self, _tool_id: &str, _args_hash: &str) -> Option<GrantDecision> {
        None
    }
}

/// 测试用: ConversationSub Phase A 测试 / ToolSub Phase C 单测时注入。
/// 可预设固定 GrantDecision 序列。
pub struct MockGrantBroker {
    decisions: Mutex<VecDeque<GrantDecision>>,
}

impl MockGrantBroker {
    pub fn new(decisions: Vec<GrantDecision>) -> Self {
        Self { decisions: Mutex::new(decisions.into()) }
    }

    pub fn empty() -> Self { Self::new(vec![]) }
}

#[async_trait]
impl GrantBroker for MockGrantBroker {
    async fn request_tool_grant(
        &self,
        _surface: SurfaceId,
        _tool_id: &str,
        _args_summary: ToolArgsSummary,
        _paths: Vec<PathBuf>,
        _reason: GrantReason,
        _persona_snapshot_id: &str,
    ) -> Result<GrantDecision, GrantError> {
        self.decisions.lock().pop_front()
            .map(Ok)
            .unwrap_or(Err(GrantError::FeatureDisabled))
    }

    fn check_cached(&self, _tool_id: &str, _args_hash: &str) -> Option<GrantDecision> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deny_all_always_returns_feature_disabled() {
        let broker = DenyAllGrantBroker;
        let result = broker.request_tool_grant(
            SurfaceId::Chat, "read", ToolArgsSummary { display_text: "test".into() },
            vec![], GrantReason::FirstAccess, "snap_1"
        ).await;
        assert!(matches!(result, Err(GrantError::FeatureDisabled)));
        assert!(broker.check_cached("read", "hash").is_none());
    }

    #[tokio::test]
    async fn mock_returns_preset_decisions_in_order() {
        let broker = MockGrantBroker::new(vec![
            GrantDecision::AllowOnce,
            GrantDecision::Deny,
        ]);
        let r1 = broker.request_tool_grant(
            SurfaceId::Chat, "read", ToolArgsSummary { display_text: "1".into() },
            vec![], GrantReason::FirstAccess, "snap_1"
        ).await;
        assert!(matches!(r1, Ok(GrantDecision::AllowOnce)));
        let r2 = broker.request_tool_grant(
            SurfaceId::Chat, "read", ToolArgsSummary { display_text: "2".into() },
            vec![], GrantReason::FirstAccess, "snap_1"
        ).await;
        assert!(matches!(r2, Ok(GrantDecision::Deny)));
        // 用完后返 FeatureDisabled
        let r3 = broker.request_tool_grant(
            SurfaceId::Chat, "read", ToolArgsSummary { display_text: "3".into() },
            vec![], GrantReason::FirstAccess, "snap_1"
        ).await;
        assert!(matches!(r3, Err(GrantError::FeatureDisabled)));
    }
}
