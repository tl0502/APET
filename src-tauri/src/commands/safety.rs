use serde::{Deserialize, Serialize};
use tauri::State;

use crate::kernel::safety_policy::SafetyScope;
use crate::kernel::Kernel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SafetyScopeIpc {
    PrefixInjection,
    UserInput,
    StreamToken,
    FinalOutput,
}

impl SafetyScopeIpc {
    fn to_kernel_scope(self) -> SafetyScope {
        match self {
            Self::PrefixInjection => SafetyScope::PrefixInjection,
            Self::UserInput => SafetyScope::UserInput,
            Self::StreamToken => SafetyScope::StreamToken,
            Self::FinalOutput => SafetyScope::FinalOutput,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyPolicySnapshot {
    pub prefix_injection: bool,
    pub user_input: bool,
    pub stream_token: bool,
    pub final_output: bool,
}

#[tauri::command]
pub fn safety_policy_get(kernel: State<'_, Kernel>) -> SafetyPolicySnapshot {
    let policy = &kernel.safety_policy;
    SafetyPolicySnapshot {
        prefix_injection: policy.is_enabled(SafetyScope::PrefixInjection),
        user_input: policy.is_enabled(SafetyScope::UserInput),
        stream_token: policy.is_enabled(SafetyScope::StreamToken),
        final_output: policy.is_enabled(SafetyScope::FinalOutput),
    }
}

#[tauri::command]
pub async fn safety_policy_set(
    kernel: State<'_, Kernel>,
    scope: SafetyScopeIpc,
    enabled: bool,
) -> Result<(), String> {
    kernel
        .safety_policy
        .set_enabled(scope.to_kernel_scope(), enabled)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safety_scope_ipc_maps_to_kernel_scope() {
        assert_eq!(
            SafetyScopeIpc::PrefixInjection.to_kernel_scope(),
            crate::kernel::safety_policy::SafetyScope::PrefixInjection
        );
        assert_eq!(
            SafetyScopeIpc::UserInput.to_kernel_scope(),
            crate::kernel::safety_policy::SafetyScope::UserInput
        );
        assert_eq!(
            SafetyScopeIpc::StreamToken.to_kernel_scope(),
            crate::kernel::safety_policy::SafetyScope::StreamToken
        );
        assert_eq!(
            SafetyScopeIpc::FinalOutput.to_kernel_scope(),
            crate::kernel::safety_policy::SafetyScope::FinalOutput
        );
    }

    #[test]
    fn safety_policy_snapshot_serializes_camel_case() {
        let snapshot = SafetyPolicySnapshot {
            prefix_injection: true,
            user_input: false,
            stream_token: true,
            final_output: false,
        };

        let json = serde_json::to_value(snapshot).unwrap();

        assert_eq!(json["prefixInjection"], true);
        assert_eq!(json["userInput"], false);
        assert_eq!(json["streamToken"], true);
        assert_eq!(json["finalOutput"], false);
    }
}
