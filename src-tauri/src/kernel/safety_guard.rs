// SafetyGuard — SafetyPolicy-gated prompt wrapping and scan scopes.
// Phase A0 implements PrefixInjection, UserInput, StreamToken, and FinalOutput.
// Disabled scopes are noops; scan terminal state includes `disabled`.

use std::sync::Arc;

use thiserror::Error;

use crate::kernel::safety_policy::{MockSafetyPolicy, SafetyPolicy, SafetyScope};
use crate::services::llm::{ChatMessage, ContentPart, Role};

#[derive(Debug, Error)]
pub enum SafetyError {
    #[error("safety prefix asset missing: {0}")]
    PrefixMissing(String),
    #[error("scan rule load failed: {0}")]
    #[allow(dead_code)] // Phase A0: forward hook for P1 dynamic rule loading
    ScanRuleLoad(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// 流式 token scan 决策 (Scan Scope #2)。
#[derive(Debug, Clone, PartialEq)]
pub enum ScanTokenResult {
    Pass,
    /// soft hit: 替换最近 N 个**字符**(非字节) 为占位, stream 继续。
    /// `replace_last_n` 是 char 数 (不是 byte / grapheme),
    /// 调用方按 `s.chars().rev().take(n)` 计算定位再切片回写。
    /// Phase A0 占位串为 `[审核中…]`, 6 chars = `['[', '审', '核', '中', '…', ']']`
    /// (UTF-8 ≈ 14 bytes), `replace_last_n=8` 是粗略上下文窗口非精确长度。
    SoftBlock {
        rule_id: String,
        replace_last_n: usize,
        placeholder: String,
    },
    HardEnd {
        rule_id: String,
    },
}

/// 终态 scan 决策 (Scan Scope #1 user input / #3 final text)。
#[derive(Debug, Clone, PartialEq)]
pub enum ScanFinalResult {
    Ok,
    Redacted {
        redacted_text: String,
        rule_ids: Vec<String>,
    },
    Blocked {
        rule_ids: Vec<String>,
        fallback: String,
    },
    ScanFailed {
        reason: String,
        fallback: String,
    },
}

/// SafetyGuard trait — kernel-owned, subsystem 无法构造, 仅经 Boot 时 SafetyGuardImpl::load。
pub trait SafetyGuard: Send + Sync {
    /// SafetyPolicy 状态透传，供上层按 scope 派生终态。
    fn is_enabled(&self, scope: SafetyScope) -> bool;

    /// 出方向: prompt → LLM, PrefixInjection ON 时注入 system message 第一位。
    fn wrap_messages(&self, messages: Vec<ChatMessage>, locale: Locale) -> Vec<ChatMessage>;

    /// 入方向: 流式 token chunk 增量扫 (Scope #2)。
    ///
    /// **Phase A0 约定**: 实现使用 substring 匹配整个 `accumulated`,
    /// 同一规则在一次流中会重复命中。调用方 (ChatService) 必须按 `rule_id`
    /// 去重: 同 stream 同 rule_id 已 SoftBlock/HardEnd 过, 不再触发 FSM 转换,
    /// 否则会在重复 token 上震荡。
    ///
    /// `partial` 是本次 token chunk, `accumulated` 是流到目前为止的全文,
    /// `finished` 标识是否是流终态 token; Phase A0 行为同 false,
    /// P1 评估是否分流端 token 与流终 token 不同 rule 集。
    ///
    // TODO(P1): 切换为 trailing-window 扫 (仅扫尾部 N chars), 避开线性增长。
    fn scan_token(&self, partial: &str, accumulated: &str, finished: bool) -> ScanTokenResult;

    /// 入方向: 流终态全文扫 (Scope #3 LLM final)。
    fn scan_final(&self, full_text: &str, persona_snapshot_id: &str) -> ScanFinalResult;

    /// 入方向: 用户输入扫 (Scope #1, 防 prompt injection)。
    fn scan_user_input(&self, text: &str) -> ScanFinalResult;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Locale {
    ZhCn,
    EnUs,
}

const FALLBACK_REFUSAL: &str = "这个我现在没法陪你聊,要不我们换个话题?";
const SCAN_TOKEN_WINDOW_CHARS: usize = 64;

/// Phase A0 实现: prefix 从 assets/safety/prefix_v1.txt 加载, scan 用静态黑词表。
pub struct SafetyGuardImpl {
    prefix: String,
    /// 黑词表 (Phase A0 简单 substring 匹配, P1 评估 regex/classifier)
    hard_blocklist: Vec<&'static str>,
    soft_blocklist: Vec<&'static str>,
    policy: Arc<dyn SafetyPolicy>,
}

impl SafetyGuardImpl {
    pub fn load(prefix_path: &std::path::Path) -> Result<Self, SafetyError> {
        let prefix = std::fs::read_to_string(prefix_path)?;
        Self::from_text(&prefix).map_err(|e| match e {
            // 把 inline "<empty inline prefix>" 误差替换回真实 path, 保留 load 历史行为。
            SafetyError::PrefixMissing(_) => {
                SafetyError::PrefixMissing(prefix_path.display().to_string())
            }
            other => other,
        })
    }

    pub fn from_text(prefix: &str) -> Result<Self, SafetyError> {
        Self::from_text_with_policy(prefix, Arc::new(MockSafetyPolicy::all_on()))
    }

    /// Boot 路径: prefix 编译时 include_str! 嵌入, policy 由 Kernel::boot 注入。
    pub fn from_text_with_policy(
        prefix: &str,
        policy: Arc<dyn SafetyPolicy>,
    ) -> Result<Self, SafetyError> {
        if prefix.trim().is_empty() {
            return Err(SafetyError::PrefixMissing(
                "<empty inline prefix>".to_string(),
            ));
        }
        Ok(Self {
            prefix: prefix.to_string(),
            hard_blocklist: vec!["自杀", "自残"],
            soft_blocklist: vec!["违法", "违禁"],
            policy,
        })
    }

    fn scan_text(&self, full_text: &str) -> ScanFinalResult {
        let mut hit_rules = Vec::new();
        for rule in &self.hard_blocklist {
            if full_text.contains(rule) {
                hit_rules.push(rule.to_string());
            }
        }
        if !hit_rules.is_empty() {
            return ScanFinalResult::Blocked {
                rule_ids: hit_rules,
                fallback: FALLBACK_REFUSAL.to_string(),
            };
        }
        let mut soft_hit = Vec::new();
        let mut redacted = full_text.to_string();
        for rule in &self.soft_blocklist {
            if redacted.contains(rule) {
                redacted = redacted.replace(rule, "***");
                soft_hit.push(rule.to_string());
            }
        }
        if !soft_hit.is_empty() {
            return ScanFinalResult::Redacted {
                redacted_text: redacted,
                rule_ids: soft_hit,
            };
        }
        ScanFinalResult::Ok
    }
}

impl SafetyGuard for SafetyGuardImpl {
    fn is_enabled(&self, scope: SafetyScope) -> bool {
        self.policy.is_enabled(scope)
    }

    fn wrap_messages(&self, mut messages: Vec<ChatMessage>, _locale: Locale) -> Vec<ChatMessage> {
        if !self.policy.is_enabled(SafetyScope::PrefixInjection) {
            return messages;
        }
        // Phase A0: locale 暂未分流; P1 评估 zh-CN / en-US 切不同 prefix 文件 (asset 多语言)。
        match messages.first_mut() {
            Some(first) if first.role == Role::System => {
                // T3-7：把 prefix 合并进首个 Text part，而不是 insert 成独立的第二个 part。
                // 插成第二个 part 会让 serialize_message 把 system 的 content 从 string 降级成
                // array —— 部分 OpenAI 兼容端点 / 老模型不接受 array system content（兼容雷），
                // 且 array 与 string 字节形状不同会打断前缀缓存。合并后仍是单 Text part → string。
                // 仅当首个 part 是 Text 时可合并；否则（罕见：system 以非文本 part 起头）退回插入。
                match first.content.first_mut() {
                    Some(ContentPart::Text { text }) => {
                        *text = format!("{}\n\n{}", self.prefix, text);
                    }
                    _ => first.content.insert(
                        0,
                        ContentPart::Text {
                            text: format!("{}\n\n", self.prefix),
                        },
                    ),
                }
            }
            _ => {
                let new_system = ChatMessage::text(Role::System, self.prefix.clone());
                messages.insert(0, new_system);
            }
        }
        messages
    }

    fn scan_token(&self, _partial: &str, accumulated: &str, _finished: bool) -> ScanTokenResult {
        if !self.policy.is_enabled(SafetyScope::StreamToken) {
            return ScanTokenResult::Pass;
        }
        let target = trailing_chars(accumulated, SCAN_TOKEN_WINDOW_CHARS);
        for rule in &self.hard_blocklist {
            if target.contains(rule) {
                return ScanTokenResult::HardEnd {
                    rule_id: rule.to_string(),
                };
            }
        }
        for rule in &self.soft_blocklist {
            if target.contains(rule) {
                return ScanTokenResult::SoftBlock {
                    rule_id: rule.to_string(),
                    replace_last_n: 8,
                    placeholder: "[审核中…]".to_string(),
                };
            }
        }
        ScanTokenResult::Pass
    }

    fn scan_final(&self, full_text: &str, _persona_snapshot_id: &str) -> ScanFinalResult {
        if !self.policy.is_enabled(SafetyScope::FinalOutput) {
            return ScanFinalResult::Ok;
        }
        self.scan_text(full_text)
    }

    fn scan_user_input(&self, text: &str) -> ScanFinalResult {
        if !self.policy.is_enabled(SafetyScope::UserInput) {
            return ScanFinalResult::Ok;
        }
        self.scan_text(text)
    }
}

fn trailing_chars(s: &str, n: usize) -> &str {
    let char_count = s.chars().count();
    if char_count <= n {
        return s;
    }
    let start_byte = s
        .char_indices()
        .nth(char_count - n)
        .map(|(idx, _)| idx)
        .unwrap_or(0);
    &s[start_byte..]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::safety_policy::{MockSafetyPolicy, SafetyPolicy, SafetyScope};
    use std::sync::Arc;

    fn make_guard() -> SafetyGuardImpl {
        SafetyGuardImpl {
            prefix: "TEST_PREFIX".to_string(),
            hard_blocklist: vec!["自杀"],
            soft_blocklist: vec!["违禁"],
            policy: Arc::new(MockSafetyPolicy::all_on()),
        }
    }

    fn make_guard_with_policy_all_off() -> SafetyGuardImpl {
        let policy = Arc::new(MockSafetyPolicy::all_off()) as Arc<dyn SafetyPolicy>;
        SafetyGuardImpl::from_text_with_policy("TEST_PREFIX", policy).unwrap()
    }

    fn make_guard_with_policy_all_on() -> SafetyGuardImpl {
        let policy = Arc::new(MockSafetyPolicy::all_on()) as Arc<dyn SafetyPolicy>;
        SafetyGuardImpl::from_text_with_policy("TEST_PREFIX", policy).unwrap()
    }

    #[test]
    fn is_enabled_delegates_to_policy_for_all_scopes() {
        let guard_off = make_guard_with_policy_all_off();
        let guard_on = make_guard_with_policy_all_on();
        for scope in [
            SafetyScope::PrefixInjection,
            SafetyScope::UserInput,
            SafetyScope::StreamToken,
            SafetyScope::FinalOutput,
        ] {
            assert!(!guard_off.is_enabled(scope));
            assert!(guard_on.is_enabled(scope));
        }
    }

    #[test]
    fn policy_off_makes_all_safety_methods_noop() {
        let guard = make_guard_with_policy_all_off();
        let user_msg = ChatMessage::text(Role::User, "hi");

        let wrapped = guard.wrap_messages(vec![user_msg.clone()], Locale::ZhCn);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0].role, Role::User);
        assert_eq!(guard.scan_user_input("自杀"), ScanFinalResult::Ok);
        assert_eq!(guard.scan_token("", "自杀", false), ScanTokenResult::Pass);
        assert_eq!(guard.scan_final("自杀方法", "snap_1"), ScanFinalResult::Ok);
    }

    #[test]
    fn user_input_scope_scans_even_when_final_output_scope_is_off() {
        let policy = Arc::new(MockSafetyPolicy::all_off()) as Arc<dyn SafetyPolicy>;
        tauri::async_runtime::block_on(policy.set_enabled(SafetyScope::UserInput, true)).unwrap();
        let guard = SafetyGuardImpl::from_text_with_policy("TEST_PREFIX", policy).unwrap();

        let result = guard.scan_user_input("自杀");

        assert!(matches!(result, ScanFinalResult::Blocked { .. }));
    }

    #[test]
    fn scan_token_soft_block_includes_rule_id() {
        let guard = make_guard_with_policy_all_on();
        let result = guard.scan_token("", "教我违禁的", false);
        match result {
            ScanTokenResult::SoftBlock {
                rule_id,
                placeholder,
                ..
            } => {
                assert_eq!(rule_id, "违禁");
                assert_eq!(placeholder, "[审核中…]");
            }
            other => panic!("expected SoftBlock with rule_id, got {:?}", other),
        }
    }

    #[test]
    fn scan_token_trailing_window_skips_old_hits() {
        let guard = make_guard_with_policy_all_on();
        let mut accumulated = "自杀".to_string();
        accumulated.push_str(&"x".repeat(100));

        assert_eq!(
            guard.scan_token("", &accumulated, false),
            ScanTokenResult::Pass
        );
    }

    #[test]
    fn scan_token_trailing_window_catches_recent_hits() {
        let guard = make_guard_with_policy_all_on();
        let mut accumulated = "x".repeat(100);
        accumulated.push_str(&"y".repeat(60));
        accumulated.push_str("自杀");

        let result = guard.scan_token("", &accumulated, false);

        assert!(matches!(result, ScanTokenResult::HardEnd { rule_id } if rule_id == "自杀"));
    }

    #[test]
    fn wrap_messages_inserts_prefix_as_first_system() {
        let guard = make_guard();
        let user_msg = ChatMessage::text(Role::User, "hi");
        let wrapped = guard.wrap_messages(vec![user_msg], Locale::ZhCn);
        assert_eq!(wrapped.len(), 2);
        assert_eq!(wrapped[0].role, Role::System);
        assert!(
            matches!(&wrapped[0].content[0], ContentPart::Text { text } if text == "TEST_PREFIX")
        );
    }

    #[test]
    fn wrap_messages_prepends_to_existing_system() {
        let guard = make_guard();
        let sys = ChatMessage::text(Role::System, "you are momo");
        let wrapped = guard.wrap_messages(vec![sys], Locale::ZhCn);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0].role, Role::System);
        // T3-7：prefix 合并进同一个 Text part，不新增 part。
        assert_eq!(wrapped[0].content.len(), 1);
        assert!(
            matches!(&wrapped[0].content[0], ContentPart::Text { text } if text.starts_with("TEST_PREFIX"))
        );
    }

    #[test]
    fn wrap_messages_keeps_system_content_as_string_not_array() {
        // T3-7 回归守卫：prefix 注入后，system 消息经 serialize_message 必须仍是 string content
        // （而非 array）——否则触发 OpenAI 兼容端点/老模型兼容雷 + 打断前缀缓存。
        use crate::services::llm::openai::serialize_message;
        let guard = make_guard();
        let sys = ChatMessage::text(Role::System, "you are momo");
        let wrapped = guard.wrap_messages(vec![sys], Locale::ZhCn);
        let v = serialize_message(&wrapped[0]);
        assert!(
            v["content"].is_string(),
            "system content 应为 string，实际: {}",
            v["content"]
        );
        assert_eq!(v["content"], "TEST_PREFIX\n\nyou are momo");
    }

    #[test]
    fn scan_token_returns_pass_for_clean_text() {
        let guard = make_guard();
        assert_eq!(guard.scan_token("h", "hello", false), ScanTokenResult::Pass);
    }

    #[test]
    fn scan_token_returns_hard_end_for_hard_block_word() {
        let guard = make_guard();
        let result = guard.scan_token("", "我想自杀", false);
        assert!(matches!(result, ScanTokenResult::HardEnd { .. }));
    }

    #[test]
    fn scan_token_returns_soft_block_for_soft_word() {
        let guard = make_guard();
        let result = guard.scan_token("", "教我违禁的", false);
        match result {
            ScanTokenResult::SoftBlock { placeholder, .. } => {
                assert_eq!(placeholder, "[审核中…]");
            }
            _ => panic!("expected SoftBlock"),
        }
    }

    #[test]
    fn scan_final_returns_ok_for_clean_text() {
        let guard = make_guard();
        assert_eq!(guard.scan_final("hello", "snap_1"), ScanFinalResult::Ok);
    }

    #[test]
    fn scan_final_returns_blocked_for_hard_hit() {
        let guard = make_guard();
        let result = guard.scan_final("自杀方法", "snap_1");
        match result {
            ScanFinalResult::Blocked { rule_ids, fallback } => {
                assert!(rule_ids.contains(&"自杀".to_string()));
                assert!(!fallback.is_empty());
            }
            _ => panic!("expected Blocked, got {:?}", result),
        }
    }

    #[test]
    fn scan_final_returns_redacted_for_soft_hit() {
        let guard = make_guard();
        let result = guard.scan_final("教我违禁知识", "snap_1");
        match result {
            ScanFinalResult::Redacted {
                redacted_text,
                rule_ids,
            } => {
                assert!(redacted_text.contains("***"));
                assert!(!redacted_text.contains("违禁"));
                assert!(rule_ids.contains(&"违禁".to_string()));
            }
            _ => panic!("expected Redacted, got {:?}", result),
        }
    }

    #[test]
    fn scan_user_input_uses_same_rules() {
        let guard = make_guard();
        assert!(matches!(
            guard.scan_user_input("自杀"),
            ScanFinalResult::Blocked { .. }
        ));
    }

    #[test]
    fn scan_final_prefers_hard_block_when_both_hit() {
        let guard = make_guard();
        let result = guard.scan_final("自杀和违禁混合", "snap_1");
        match result {
            ScanFinalResult::Blocked { rule_ids, .. } => {
                assert!(rule_ids.contains(&"自杀".to_string()));
            }
            other => panic!("expected Blocked (hard precedence), got {:?}", other),
        }
    }

    #[test]
    fn scan_token_finished_flag_currently_does_not_change_outcome() {
        let guard = make_guard();
        let a = guard.scan_token("", "hello", false);
        let b = guard.scan_token("", "hello", true);
        assert_eq!(a, b);
    }

    #[test]
    fn load_reads_prefix_from_file() {
        let tmp = std::env::temp_dir().join(format!("test_prefix_{}.txt", ulid::Ulid::new()));
        std::fs::write(&tmp, "MY_TEST_PREFIX_CONTENT").unwrap();
        let guard = SafetyGuardImpl::load(&tmp).unwrap();
        assert_eq!(guard.prefix, "MY_TEST_PREFIX_CONTENT");
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn load_fails_on_empty_file() {
        let tmp = std::env::temp_dir().join(format!("test_empty_{}.txt", ulid::Ulid::new()));
        std::fs::write(&tmp, "").unwrap();
        let result = SafetyGuardImpl::load(&tmp);
        assert!(matches!(result, Err(SafetyError::PrefixMissing(_))));
        std::fs::remove_file(&tmp).ok();
    }
}
