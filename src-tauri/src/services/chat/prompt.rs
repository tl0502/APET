// chat/prompt.rs — prompt material assembly.
//
// A2 hot path:
// - build_messages_from_profile(input): consumes SoulRuntimeProfile identity/style/examples,
//   user nickname, history, and current input.
// - SafetyPrefix is not built here. SafetyGuard.wrap_messages injects it after PromptBuilder output
//   when PrefixInjection policy is ON.
//
// Legacy compatibility:
// - extract_persona_sections(raw_md) and build_messages(persona, user_nickname, pet_nickname,
//   history, current_input) remain for tests and migration comparison, but ChatService::prepare
//   must not use raw markdown for the formal path.

use thiserror::Error;

use crate::services::llm::{ChatMessage, Role};
use crate::services::memory::MessageRecord;
use crate::services::persona::{PersonaSummary, SoulRuntimeProfile};

// SAFETY_PREFIX 已由 SafetyGuard.wrap_messages 在 build_messages 调用方 (chat::service)
// 集中注入到 system message 第一位 (Phase A0.1, Spec §6.6, ADR-006);
// 本模块不再持有 prefix const。

/// 单 section 字符上限（中文 1 字符 ≈ 1.5 token，4000 字符 ≈ 6000 token，4 节合计 ≤ ~24K token）。
/// 防恶意人格 / 用户写超长 # 性格 把 LLM context 吃光。
const MAX_SECTION_CHARS: usize = 4000;
const MAX_EXAMPLE_COUNT: usize = 3;
const MAX_EXAMPLE_CHARS: usize = 600;
const MAX_EXAMPLES_TOTAL_CHARS: usize = 1200;

/// 错误类型用于 MissingSection 定位。
const LABEL_IDENTITY: &str = "# 身份";
const LABEL_PERSONALITY: &str = "# 性格";
const LABEL_ABILITIES: &str = "# 能力";
const LABEL_RULES: &str = "# 行为规则";

#[derive(Debug, Error, PartialEq)]
pub enum PromptError {
    #[error("persona missing required section: {0}")]
    MissingSection(&'static str),
    #[error("runtime profile missing required field: {0}")]
    EmptyProfileField(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonaSections {
    pub identity: String,
    pub personality: String,
    pub abilities: String,
    pub rules: String,
}

#[derive(Debug, Clone, Copy)]
pub struct PromptBuildInput<'a> {
    pub runtime_profile: &'a SoulRuntimeProfile,
    pub persona_name: &'a str,
    pub user_nickname: Option<&'a str>,
    pub pet_nickname: &'a str,
    pub history: &'a [MessageRecord],
    pub current_input: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SectionKind {
    Identity,
    Personality,
    Abilities,
    Rules,
    /// # 离线模板 / # 反应配置 — 立即终止解析
    Terminator,
    /// 其他 H1（# 例对话 / # 集成 等）— 退出当前累积，继续扫描
    OtherH1,
    /// 非 H1 行（正文 / ## H2 / 列表 / 空行 等）— 累积到当前 section
    NotH1,
}

fn classify_line(line: &str) -> SectionKind {
    // H1 = "# " 开头但**不是** "## "（"## ".starts_with("# ") = false 因为第二字符是 '#' 不是空格）
    if !line.starts_with("# ") {
        return SectionKind::NotH1;
    }
    let body = line.trim_start_matches('#').trim();
    if matches_section(body, "身份") {
        SectionKind::Identity
    } else if matches_section(body, "性格") {
        SectionKind::Personality
    } else if matches_section(body, "能力") {
        SectionKind::Abilities
    } else if matches_section(body, "行为规则") {
        SectionKind::Rules
    } else if matches_section(body, "离线模板") || matches_section(body, "反应配置") {
        SectionKind::Terminator
    } else {
        SectionKind::OtherH1
    }
}

/// B3 修复：精确 token 匹配，避免 `# 身份证认证` 这种 H1 被误判为 Identity 段。
///
/// 接受形式（按 .soul.md 实际写法穷举）：
/// - 单 token：`# 身份`
/// - 后跟空格+英文/补充：`# 身份 / Identity`、`# 身份 - 详细`
/// - 紧挨括号注释：`# 身份(Identity)`、`# 身份（说明）`
/// - 紧挨斜杠并列：`# 身份/Identity`
///
/// 不接受 `# 身份证`、`# 性格使然` 等扩展词（rest 起始字符是中文 / 英文字母而非分隔符）。
fn matches_section(body: &str, kw: &str) -> bool {
    if let Some(rest) = body.strip_prefix(kw) {
        rest.is_empty()
            || rest.starts_with(' ')
            || rest.starts_with('\t')
            || rest.starts_with('/')
            || rest.starts_with('(')
            || rest.starts_with('（')
            || rest.starts_with('-')
    } else {
        false
    }
}

pub fn extract_persona_sections(raw_md: &str) -> Result<PersonaSections, PromptError> {
    let mut identity = String::new();
    let mut personality = String::new();
    let mut abilities = String::new();
    let mut rules = String::new();
    let mut current: Option<SectionKind> = None;

    for line in raw_md.lines() {
        match classify_line(line) {
            SectionKind::Identity => {
                current = Some(SectionKind::Identity);
                identity.push_str(line);
                identity.push('\n');
            }
            SectionKind::Personality => {
                current = Some(SectionKind::Personality);
                personality.push_str(line);
                personality.push('\n');
            }
            SectionKind::Abilities => {
                current = Some(SectionKind::Abilities);
                abilities.push_str(line);
                abilities.push('\n');
            }
            SectionKind::Rules => {
                current = Some(SectionKind::Rules);
                rules.push_str(line);
                rules.push('\n');
            }
            SectionKind::Terminator => break,
            SectionKind::OtherH1 => {
                current = None;
            }
            SectionKind::NotH1 => match current {
                Some(SectionKind::Identity) => {
                    identity.push_str(line);
                    identity.push('\n');
                }
                Some(SectionKind::Personality) => {
                    personality.push_str(line);
                    personality.push('\n');
                }
                Some(SectionKind::Abilities) => {
                    abilities.push_str(line);
                    abilities.push('\n');
                }
                Some(SectionKind::Rules) => {
                    rules.push_str(line);
                    rules.push('\n');
                }
                _ => {}
            },
        }
    }

    let identity = truncate_section(identity);
    let personality = truncate_section(personality);
    let abilities = truncate_section(abilities);
    let rules = truncate_section(rules);

    if identity.trim().is_empty() {
        return Err(PromptError::MissingSection(LABEL_IDENTITY));
    }
    if personality.trim().is_empty() {
        return Err(PromptError::MissingSection(LABEL_PERSONALITY));
    }
    if abilities.trim().is_empty() {
        return Err(PromptError::MissingSection(LABEL_ABILITIES));
    }
    if rules.trim().is_empty() {
        return Err(PromptError::MissingSection(LABEL_RULES));
    }

    Ok(PersonaSections {
        identity,
        personality,
        abilities,
        rules,
    })
}

fn truncate_section(s: String) -> String {
    let trimmed = s.trim_end().to_string();
    if trimmed.chars().count() <= MAX_SECTION_CHARS {
        return trimmed;
    }
    let truncated: String = trimmed.chars().take(MAX_SECTION_CHARS).collect();
    format!("{truncated}\n[truncated]")
}

pub fn build_system_message(
    sections: &PersonaSections,
    persona_name: &str,
    user_nickname: Option<&str>,
    pet_nickname: &str,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    // 1. 安全前缀: 由 SafetyGuard.wrap_messages 在 ChatService 集中注入 (Phase A0.1, Spec §6.6)
    //    此处不再 push, 避免与 prefix 注入路径冲突。

    // 2. 角色身份
    parts.push("你是一个 AI 桌面伙伴。以下是你扮演的角色定义：".to_string());
    parts.push(sections.identity.clone());
    parts.push(sections.personality.clone());
    parts.push(sections.abilities.clone());
    parts.push(sections.rules.clone());

    // C8：告知 LLM 用户消息可能带 wrap_user_input 包装的「（保持 X 风格）」前缀，
    // 让它视为系统引导而不是用户内容，回复中也不复读这个前缀。
    parts.push(format!(
        "（系统说明）用户消息可能带「（保持 {persona_name} 风格）」前缀作为系统引导，请视为指令而非用户内容；回复中不要复读这个前缀。"
    ));

    // 3. 当前会话上下文（昵称注入；persona-design.md §8.3 ChatService 统一注入）
    let mut nickname_bullets: Vec<String> = Vec::new();
    if let Some(nick) = user_nickname.map(str::trim).filter(|s| !s.is_empty()) {
        nickname_bullets.push(format!("- 用户希望你称他为「{nick}」"));
    }
    if pet_nickname != persona_name {
        nickname_bullets.push(format!(
            "- 你的人格名是「{persona_name}」，但用户给你起了昵称「{pet_nickname}」，回应时优先用这个昵称",
        ));
    }
    if !nickname_bullets.is_empty() {
        parts.push(format!("# 当前会话上下文\n{}", nickname_bullets.join("\n")));
    }

    // 4. Re-anchor 末句（防 personality drift）
    parts.push(format!(
        "保持上述身份与性格设定。回复偏离 {persona_name} 的语气时，立即回到原风格。"
    ));

    parts.join("\n\n")
}

fn build_profile_system_message(
    profile: &SoulRuntimeProfile,
    persona_name: &str,
    user_nickname: Option<&str>,
    pet_nickname: &str,
) -> Result<String, PromptError> {
    let identity = profile.identity_prompt.trim();
    if identity.is_empty() {
        return Err(PromptError::EmptyProfileField("identity_prompt"));
    }

    let style = profile.style_prompt.trim();
    if style.is_empty() {
        return Err(PromptError::EmptyProfileField("style_prompt"));
    }

    let mut parts = Vec::new();
    parts.push(
        "你是一个 AI 桌面伙伴。你必须保持当前人格快照定义，不要声称拥有未授予的系统权限、工具权限或屏幕/剪贴板读取能力。"
            .to_string(),
    );
    parts.push(format!(
        "以下是当前人格快照：\n\n# 身份\n{identity}\n\n# 风格与规则\n{style}"
    ));

    parts.push(format!(
        "（系统说明）用户消息可能带「（保持 {persona_name} 风格）」前缀作为系统引导，请视为指令而非用户内容；回复中不要复读这个前缀。"
    ));

    let mut nickname_bullets = Vec::new();
    if let Some(nick) = user_nickname.map(str::trim).filter(|s| !s.is_empty()) {
        nickname_bullets.push(format!("- 用户希望你称他为「{nick}」"));
    }
    if pet_nickname != persona_name {
        nickname_bullets.push(format!(
            "- 你的人格名是「{persona_name}」，但用户给你起了昵称「{pet_nickname}」，回应时优先用这个昵称",
        ));
    }
    if !nickname_bullets.is_empty() {
        parts.push(format!("# 当前会话上下文\n{}", nickname_bullets.join("\n")));
    }

    parts.push(format!(
        "保持上述身份与性格设定。回复偏离 {persona_name} 的语气时，立即回到原风格。"
    ));

    Ok(parts.join("\n\n"))
}

fn truncate_example(example: &str) -> String {
    let trimmed = example.trim();
    if trimmed.chars().count() <= MAX_EXAMPLE_CHARS {
        return trimmed.to_string();
    }
    let truncated: String = trimmed.chars().take(MAX_EXAMPLE_CHARS).collect();
    format!("{truncated}\n[truncated]")
}

fn build_examples_message(examples: &[String]) -> Option<String> {
    let mut selected = Vec::new();
    let mut total_chars = 0usize;

    for example in examples
        .iter()
        .map(|example| truncate_example(example))
        .filter(|example| !example.trim().is_empty())
        .take(MAX_EXAMPLE_COUNT)
    {
        let example_chars = example.chars().count();
        if total_chars + example_chars > MAX_EXAMPLES_TOTAL_CHARS {
            break;
        }
        total_chars += example_chars;
        selected.push(example);
    }

    if selected.is_empty() {
        return None;
    }

    Some(format!(
        "以下是这个人格的示例对话。它们用于校准语气，不代表当前会话事实：\n\n{}",
        selected
            .iter()
            .enumerate()
            .map(|(idx, example)| format!("## 示例 {}\n{}", idx + 1, example))
            .collect::<Vec<_>>()
            .join("\n\n")
    ))
}

/// LLM 调用前包装当前 user 输入；DB 仍存原始 input（ChatPanel 显示原文）。
pub fn wrap_user_input(persona_name: &str, raw_input: &str) -> String {
    format!("（保持 {persona_name} 风格）{raw_input}")
}

pub fn build_messages_from_profile(
    input: PromptBuildInput<'_>,
) -> Result<Vec<ChatMessage>, PromptError> {
    let system = build_profile_system_message(
        input.runtime_profile,
        input.persona_name,
        input.user_nickname,
        input.pet_nickname,
    )?;

    let mut messages = vec![ChatMessage::text(Role::System, system)];

    if let Some(examples) = build_examples_message(&input.runtime_profile.examples) {
        messages.push(ChatMessage::text(Role::System, examples));
    }

    for record in input.history {
        let role = match record.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "system" => Role::System,
            _ => continue,
        };
        messages.push(ChatMessage::text(role, record.content.clone()));
    }

    let wrapped = wrap_user_input(input.persona_name, input.current_input);
    messages.push(ChatMessage::text(Role::User, wrapped));

    Ok(messages)
}

pub fn build_messages(
    persona: &PersonaSummary,
    user_nickname: Option<&str>,
    pet_nickname: &str,
    history: &[MessageRecord],
    current_input: &str,
) -> Result<Vec<ChatMessage>, PromptError> {
    let sections = extract_persona_sections(&persona.raw_markdown)?;
    let system = build_system_message(&sections, &persona.name, user_nickname, pet_nickname);

    let mut messages = vec![ChatMessage::text(Role::System, system)];

    // 历史按 created_at 升序（list_messages_by_conversation 已升序）
    for record in history {
        let role = match record.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            // 'system' 用于 NicknameService 的转场注入消息（如"用户改称呼为 X"）。
            // 让 LLM 真的看到——研究文献（Persona Drift, arxiv 2402.10962）证实：
            // 仅靠开头 system prompt + 末尾 anchor 在长 history 下会被稀释；
            // history 中段插入的 system 通知能直接重置话术，避免旧昵称污染。
            "system" => Role::System,
            // 未知 role：防御性 skip
            _ => continue,
        };
        messages.push(ChatMessage::text(role, record.content.clone()));
    }

    let wrapped = wrap_user_input(&persona.name, current_input);
    messages.push(ChatMessage::text(Role::User, wrapped));

    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::llm::ContentPart;
    use crate::services::persona::SoulRuntimeProfile;

    const MOMO_RAW: &str = include_str!("../../../personas/_builtin/momo.soul.md");

    fn make_persona(raw_markdown: &str, name: &str) -> PersonaSummary {
        PersonaSummary {
            id: "momo".to_string(),
            name: name.to_string(),
            version: "1.0.0".to_string(),
            source: "builtin".to_string(),
            snapshot_id: "1".to_string(),
            raw_markdown: raw_markdown.to_string(),
        }
    }

    fn message_text(msg: &ChatMessage) -> String {
        msg.content
            .iter()
            .filter_map(|p| match p {
                ContentPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn make_profile(examples: Vec<String>) -> SoulRuntimeProfile {
        SoulRuntimeProfile {
            identity_prompt: "你叫默默，是一个安静的桌面伙伴。".to_string(),
            style_prompt: "# 风格\n- 句子短\n- 不空洞鼓励".to_string(),
            examples,
            initiative_config: serde_json::json!({ "mode": "sometimes" }),
            memory_policy: serde_json::json!({ "mode": "default" }),
            ui_metadata: serde_json::json!({ "name": "默默" }),
            source_kind: "legacy_soul_md".to_string(),
            source_hash: "sha256:test".to_string(),
        }
    }

    #[test]
    fn build_messages_from_profile_uses_identity_and_style_without_raw_markdown() {
        let profile = make_profile(Vec::new());
        let history = Vec::new();

        let messages = build_messages_from_profile(PromptBuildInput {
            runtime_profile: &profile,
            persona_name: "默默",
            user_nickname: Some("Tong"),
            pet_nickname: "默默",
            history: &history,
            current_input: "你好",
        })
        .unwrap();

        let system = message_text(&messages[0]);
        assert!(system.contains("当前人格快照"));
        assert!(system.contains("你叫默默，是一个安静的桌面伙伴。"));
        assert!(system.contains("# 风格"));
        assert!(system.contains("用户希望你称他为「Tong」"));
        assert!(
            !system.contains("# 能力"),
            "profile path must not require legacy markdown section headings"
        );
    }

    #[test]
    fn build_messages_from_profile_inserts_examples_before_history() {
        let profile = make_profile(vec!["用户：你好\n默默：我在。".to_string()]);
        let history = vec![MessageRecord {
            id: "01HISTORY000000000000000001".to_string(),
            conversation_id: "01CONV000000000000000001".to_string(),
            role: "user".to_string(),
            content: "历史消息".to_string(),
            mode: "online".to_string(),
            created_at: "2026-06-20T00:00:00Z".to_string(),
        }];

        let messages = build_messages_from_profile(PromptBuildInput {
            runtime_profile: &profile,
            persona_name: "默默",
            user_nickname: None,
            pet_nickname: "默默",
            history: &history,
            current_input: "继续",
        })
        .unwrap();

        assert_eq!(messages[0].role, Role::System);
        assert_eq!(messages[1].role, Role::System);
        assert_eq!(messages[2].role, Role::User);
        assert!(message_text(&messages[1]).contains("示例对话"));
        assert!(message_text(&messages[1]).contains("用户：你好"));
        assert_eq!(message_text(&messages[2]), "历史消息");
    }

    #[test]
    fn build_messages_from_profile_skips_empty_examples() {
        let profile = make_profile(Vec::new());
        let history = Vec::new();

        let messages = build_messages_from_profile(PromptBuildInput {
            runtime_profile: &profile,
            persona_name: "默默",
            user_nickname: None,
            pet_nickname: "默默",
            history: &history,
            current_input: "ok",
        })
        .unwrap();

        assert_eq!(messages.len(), 2, "system + current user only");
        assert!(!messages.iter().any(|m| message_text(m).contains("示例对话")));
    }

    #[test]
    fn build_messages_from_profile_truncates_example_budget() {
        let long = "甲".repeat(700);
        let profile = make_profile(vec![
            long,
            "用户：第二条\n默默：第二条。".to_string(),
            "用户：第三条\n默默：第三条。".to_string(),
            "用户：第四条\n默默：第四条。".to_string(),
        ]);
        let history = Vec::new();

        let messages = build_messages_from_profile(PromptBuildInput {
            runtime_profile: &profile,
            persona_name: "默默",
            user_nickname: None,
            pet_nickname: "默默",
            history: &history,
            current_input: "ok",
        })
        .unwrap();

        let examples = message_text(&messages[1]);
        assert!(examples.contains("[truncated]"));
        assert!(examples.contains("第二条"));
        assert!(examples.contains("第三条"));
        assert!(!examples.contains("第四条"), "only first 3 examples should be injected");
    }

    #[test]
    fn build_messages_from_profile_rejects_empty_identity_or_style() {
        let mut profile = make_profile(Vec::new());
        profile.identity_prompt = " ".to_string();
        let history = Vec::new();

        let result = build_messages_from_profile(PromptBuildInput {
            runtime_profile: &profile,
            persona_name: "默默",
            user_nickname: None,
            pet_nickname: "默默",
            history: &history,
            current_input: "ok",
        });

        assert_eq!(result, Err(PromptError::EmptyProfileField("identity_prompt")));
    }

    // momo.soul.md frontmatter 在 PersonaService::parse_persona 已被 gray_matter 剥离 →
    // raw_markdown 是去 frontmatter 的纯 markdown 正文。但 include_str! 拿到的是带 frontmatter
    // 的原文，本地测试需要先剥一下（用一个 marker 简化）。
    fn momo_body() -> String {
        let mut sep_count = 0;
        let mut start = 0;
        for (i, line) in MOMO_RAW.lines().enumerate() {
            if line.trim() == "---" {
                sep_count += 1;
                if sep_count == 2 {
                    start = i + 1;
                    break;
                }
            }
        }
        MOMO_RAW.lines().skip(start).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn extract_momo_sections_returns_all_four() {
        let body = momo_body();
        let sections = extract_persona_sections(&body).expect("momo sections must parse");

        assert!(sections.identity.contains("# 身份"));
        assert!(sections.identity.contains("默默"));
        assert!(sections.personality.contains("# 性格"));
        assert!(sections.personality.contains("慵懒"));
        assert!(sections.abilities.contains("# 能力"));
        assert!(sections.abilities.contains("安静地陪伴"));
        assert!(sections.rules.contains("# 行为规则"));
        assert!(sections.rules.contains("## Do"));
        assert!(sections.rules.contains("## Don't"));
    }

    #[test]
    fn extract_stops_at_offline_templates_section() {
        let body = momo_body();
        let sections = extract_persona_sections(&body).expect("must parse");

        let combined = format!(
            "{}{}{}{}",
            sections.identity, sections.personality, sections.abilities, sections.rules
        );
        assert!(
            !combined.contains("# 离线模板"),
            "# 离线模板 不应混入 LLM section（属于本地代码池）"
        );
        assert!(
            !combined.contains("# 反应配置"),
            "# 反应配置 不应混入 LLM section（属于物理交互模块）"
        );
        assert!(
            !combined.contains("click.head"),
            "反应配置 yaml 内容不应泄漏到 LLM section"
        );
        assert!(
            !combined.contains("嘿~ 又见面啦"),
            "离线模板内容不应泄漏到 LLM section"
        );
    }

    #[test]
    fn missing_personality_returns_error() {
        let raw = "# 身份\n你叫小明。\n\n# 能力\n- 测试\n\n# 行为规则\n## Do\n- 守规则";
        let result = extract_persona_sections(raw);
        assert_eq!(result, Err(PromptError::MissingSection(LABEL_PERSONALITY)));
    }

    #[test]
    fn missing_identity_returns_error() {
        let raw = "# 性格\n- 慵懒\n\n# 能力\n- x\n\n# 行为规则\n- y";
        let result = extract_persona_sections(raw);
        assert_eq!(result, Err(PromptError::MissingSection(LABEL_IDENTITY)));
    }

    #[test]
    fn other_h1_sections_are_skipped_not_terminate() {
        // # 集成 / # 例对话 等其他 H1 不应中断扫描；目标 4 节即使排在它们之后也能解析
        let raw = "# 集成\n- mcp_servers: []\n\n# 身份\n你叫测试。\n\n# 性格\n- 慵懒\n\n# 能力\n- 测试\n\n# 行为规则\n## Do\n- 守规则";
        let sections = extract_persona_sections(raw).expect("must parse despite # 集成 在前");
        assert!(sections.identity.contains("你叫测试"));
        assert!(sections.personality.contains("慵懒"));
    }

    #[test]
    fn truncate_appends_marker_when_over_limit() {
        let huge = "x".repeat(MAX_SECTION_CHARS + 100);
        let truncated = truncate_section(huge);
        assert!(truncated.contains("[truncated]"));
        assert!(
            truncated.chars().count() <= MAX_SECTION_CHARS + "\n[truncated]".chars().count() + 1
        );
    }

    // ===== B3：精确 token 匹配，避免扩展词被误判 =====

    #[test]
    fn h1_extension_words_are_not_classified_as_target_sections() {
        // # 身份证认证 / # 性格使然 等扩展词不能命中目标段
        let raw =
            "# 身份证认证\n你需要的证件\n\n# 身份\n你是测试。\n\n# 性格\n- 慵懒\n\n# 能力\n- 测试\n\n# 行为规则\n- 守规则";
        let sections = extract_persona_sections(raw).expect("must parse");
        assert!(
            !sections.identity.contains("身份证认证"),
            "# 身份证认证 不应被吸收进 # 身份"
        );
        assert!(!sections.identity.contains("证件"));
        assert!(sections.identity.contains("你是测试"));
    }

    #[test]
    fn h1_with_paren_or_slash_still_matches() {
        // 带英文 / 括号注释的 H1 仍应正确分类
        let raw = "# 身份(Identity)\nA\n\n# 性格 / Personality\n- B\n\n# 能力（说明）\n- C\n\n# 行为规则\n- D";
        let sections = extract_persona_sections(raw).expect("must parse");
        assert!(sections.identity.contains("# 身份(Identity)"));
        assert!(sections.identity.contains('A'));
        assert!(sections.personality.contains("# 性格 / Personality"));
        assert!(sections.abilities.contains("# 能力（说明）"));
    }

    #[test]
    fn classify_extension_word_returns_other_h1() {
        assert_eq!(classify_line("# 身份证认证"), SectionKind::OtherH1);
        assert_eq!(classify_line("# 性格使然"), SectionKind::OtherH1);
        assert_eq!(classify_line("# 能力者"), SectionKind::OtherH1);
    }

    #[test]
    fn classify_target_token_with_separators_returns_target() {
        assert_eq!(classify_line("# 身份"), SectionKind::Identity);
        assert_eq!(classify_line("# 身份 / Identity"), SectionKind::Identity);
        assert_eq!(classify_line("# 身份(说明)"), SectionKind::Identity);
        assert_eq!(classify_line("# 身份（注释）"), SectionKind::Identity);
        assert_eq!(classify_line("# 身份/Identity"), SectionKind::Identity);
        assert_eq!(classify_line("# 身份-详细"), SectionKind::Identity);
    }

    #[test]
    fn truncate_no_marker_when_under_limit() {
        let small = "x".repeat(100);
        let result = truncate_section(small);
        assert!(!result.contains("[truncated]"));
    }

    #[test]
    fn h2_sub_sections_dont_terminate() {
        // ## Do / ## Don't 是 # 行为规则 的子节，不能误判为 OtherH1
        let raw = "# 身份\n你叫测试。\n\n# 性格\n- 慵懒\n\n# 能力\n- x\n\n# 行为规则\n## Do\n- 用第二人称\n## Don't\n- 不长篇大论";
        let sections = extract_persona_sections(raw).expect("must parse");
        assert!(sections.rules.contains("## Do"));
        assert!(sections.rules.contains("## Don't"));
        assert!(sections.rules.contains("用第二人称"));
        assert!(sections.rules.contains("不长篇大论"));
    }

    #[test]
    fn build_system_with_both_nicknames() {
        let body = momo_body();
        let sections = extract_persona_sections(&body).unwrap();
        let system = build_system_message(&sections, "默默", Some("Tong"), "小默");

        assert!(system.contains("# 身份"));
        assert!(system.contains("# 性格"));
        assert!(system.contains("# 能力"));
        assert!(system.contains("# 行为规则"));
        assert!(system.contains("# 当前会话上下文"));
        assert!(system.contains("用户希望你称他为「Tong」"));
        assert!(system.contains("人格名是「默默」"));
        assert!(system.contains("起了昵称「小默」"));
        assert!(
            system.contains("保持上述身份与性格设定"),
            "末句 re-anchor 必须存在"
        );
    }

    #[test]
    fn build_system_skips_user_bullet_when_no_user_nickname() {
        let body = momo_body();
        let sections = extract_persona_sections(&body).unwrap();
        let system = build_system_message(&sections, "默默", None, "小默");
        assert!(!system.contains("用户希望你称他为"));
        assert!(system.contains("起了昵称「小默」"));
    }

    #[test]
    fn build_system_skips_pet_bullet_when_pet_eq_persona_name() {
        let body = momo_body();
        let sections = extract_persona_sections(&body).unwrap();
        let system = build_system_message(&sections, "默默", Some("Tong"), "默默");
        assert!(system.contains("用户希望你称他为「Tong」"));
        assert!(
            !system.contains("起了昵称"),
            "pet_nickname == persona_name 时应跳过 pet bullet"
        );
    }

    #[test]
    fn build_system_skips_context_section_entirely_when_both_empty() {
        let body = momo_body();
        let sections = extract_persona_sections(&body).unwrap();
        let system = build_system_message(&sections, "默默", None, "默默");
        assert!(!system.contains("# 当前会话上下文"));
        assert!(system.contains("保持上述身份与性格设定"));
    }

    #[test]
    fn build_system_treats_blank_user_nickname_as_none() {
        let body = momo_body();
        let sections = extract_persona_sections(&body).unwrap();
        let system = build_system_message(&sections, "默默", Some("   "), "默默");
        assert!(!system.contains("用户希望你称他为"));
    }

    #[test]
    fn build_system_includes_wrap_prefix_guidance_with_persona_name() {
        // C8：system message 必须告诉 LLM"（保持 X 风格）"前缀是引导词
        let body = momo_body();
        let sections = extract_persona_sections(&body).unwrap();
        let system = build_system_message(&sections, "默默", None, "默默");
        assert!(
            system.contains("保持 默默 风格"),
            "system message 应包含 wrap 前缀模板"
        );
        assert!(
            system.contains("不要复读这个前缀"),
            "system message 应明确要求 LLM 不复读引导词"
        );
    }

    #[test]
    fn wrap_user_input_format_is_stable() {
        assert_eq!(wrap_user_input("默默", "你好"), "（保持 默默 风格）你好");
        assert_eq!(wrap_user_input("阿吉", ""), "（保持 阿吉 风格）");
    }

    fn make_record(role: &str, content: &str) -> MessageRecord {
        MessageRecord {
            id: format!("01TEST{role}"),
            conversation_id: "c1".to_string(),
            role: role.to_string(),
            content: content.to_string(),
            mode: "online".to_string(),
            created_at: "2026-05-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn build_messages_orders_system_history_then_wrapped_input() {
        let body = momo_body();
        let persona = make_persona(&body, "默默");
        let history = vec![
            make_record("user", "上轮问题"),
            make_record("assistant", "上轮回答"),
        ];
        let messages =
            build_messages(&persona, Some("Tong"), "小默", &history, "新一轮问题").unwrap();

        assert_eq!(messages.len(), 4, "system + 2 history + 1 wrapped current");
        assert_eq!(messages[0].role, Role::System);
        assert_eq!(messages[1].role, Role::User);
        assert_eq!(message_text(&messages[1]), "上轮问题");
        assert_eq!(messages[2].role, Role::Assistant);
        assert_eq!(message_text(&messages[2]), "上轮回答");
        assert_eq!(messages[3].role, Role::User);
        assert_eq!(
            message_text(&messages[3]),
            "（保持 默默 风格）新一轮问题",
            "当前 user 必须被 wrap_user_input 包装"
        );
    }

    #[test]
    fn build_messages_skips_unknown_role_in_history() {
        let body = momo_body();
        let persona = make_persona(&body, "默默");
        let history = vec![
            make_record("user", "u1"),
            make_record("admin", "should-be-skipped"),
            make_record("assistant", "a1"),
        ];
        let messages = build_messages(&persona, None, "默默", &history, "ok").unwrap();
        assert_eq!(messages.len(), 4, "unknown role 'admin' 必须 skip");
        assert_eq!(message_text(&messages[1]), "u1");
        assert_eq!(message_text(&messages[2]), "a1");
    }

    #[test]
    fn build_messages_propagates_system_role_in_history() {
        // role='system' 是 NicknameService 转场注入消息，必须传到 LLM 重置话术。
        // 测试覆盖 [system_prompt] + [user, system 转场, assistant] + [wrapped_user] 顺序。
        let body = momo_body();
        let persona = make_persona(&body, "默默");
        let history = vec![
            make_record("user", "你好"),
            make_record(
                "system",
                "「系统通知」用户希望你之后称呼TA「Bob」（之前是「Alice」）。",
            ),
            make_record("assistant", "好的，Alice"),
        ];
        let messages = build_messages(&persona, Some("Bob"), "默默", &history, "再见").unwrap();
        assert_eq!(messages.len(), 5, "[sys] + 3 history + 1 wrapped current");
        assert_eq!(messages[0].role, Role::System, "leading system prompt");
        assert_eq!(messages[1].role, Role::User);
        assert_eq!(message_text(&messages[1]), "你好");
        assert_eq!(
            messages[2].role,
            Role::System,
            "history system 转场必须保留为 Role::System 给 LLM"
        );
        assert!(message_text(&messages[2]).contains("Bob"), "转场内容透传");
        assert_eq!(messages[3].role, Role::Assistant);
        assert_eq!(message_text(&messages[3]), "好的，Alice");
        assert_eq!(messages[4].role, Role::User);
        assert_eq!(message_text(&messages[4]), "（保持 默默 风格）再见");
    }

    #[test]
    fn build_messages_propagates_missing_section_error() {
        let bad = make_persona("# 身份\nx\n\n# 能力\ny\n\n# 行为规则\nz", "默默");
        let result = build_messages(&bad, None, "默默", &[], "hi");
        assert_eq!(result, Err(PromptError::MissingSection(LABEL_PERSONALITY)));
    }
}
