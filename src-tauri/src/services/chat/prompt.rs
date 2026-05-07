// chat/prompt.rs — system prompt 拼装（ADR-018 + persona-design.md §7.1 / §8.2）
//
// 三个公共函数：
// - extract_persona_sections(raw_md): 切人格 4 节
//     # 身份 / # 性格 / # 能力 / # 行为规则
//   遇到 # 离线模板 / # 反应配置 → 立即停止解析（这俩属本地代码池，塞 LLM 是噪音）。
//   遇到其他 H1（# 例对话 / # 集成 等）→ 退出当前 section 但继续扫描。
//   匹配按 H1 标题前缀（"# 身份" / "# 身份(Identity)" / "# 身份 / Identity" 全兼容）。
//   每节 ≤ MAX_SECTION_CHARS（4000 chars / ~1000 token），超出加 "[truncated]" 标记。
//   缺任一节返 PromptError::MissingSection。
//
// - build_system_message(sections, persona_name, user_nickname?, pet_nickname):
//   拼 [安全前缀(M1 占位)] + [4 节] + [昵称 bullets] + [re-anchor 末句]。
//   昵称注入按 persona-design.md §8.3：ChatService 统一注入，人格不能直读 NicknameService。
//   user_nickname=None / 空白 → 跳过 user bullet
//   pet_nickname == persona_name → 跳过 pet bullet（不显式提"你叫某某"避免冗余）
//
// - build_messages(persona, user_nickname?, pet_nickname, history, current_input):
//   返 Vec<ChatMessage>：[system] + [history user/assistant 交替] + [包装后的 current user]。
//   包装规则（防 drift inline 注入，arxiv 2402.10962 学术支持的 cheap technique）：
//     wrap_user_input(name, raw) = format!("（保持 {name} 风格）{raw}")
//   注：DB 仍存原始 input（用户在 ChatPanel 看到原文），仅 LLM 调用时包装。

use thiserror::Error;

use crate::services::llm::{ChatMessage, Role};
use crate::services::memory::MessageRecord;
use crate::services::persona::PersonaSummary;

/// M1 安全前缀占位。M3 G ADR-006 真注入时填充（通用 5 条 + 地区补充）。
const SAFETY_PREFIX_PLACEHOLDER: &str = "";

/// 单 section 字符上限（中文 1 字符 ≈ 1.5 token，4000 字符 ≈ 6000 token，4 节合计 ≤ ~24K token）。
/// 防恶意人格 / 用户写超长 # 性格 把 LLM context 吃光。
const MAX_SECTION_CHARS: usize = 4000;

/// 错误类型用于 MissingSection 定位。
const LABEL_IDENTITY: &str = "# 身份";
const LABEL_PERSONALITY: &str = "# 性格";
const LABEL_ABILITIES: &str = "# 能力";
const LABEL_RULES: &str = "# 行为规则";

#[derive(Debug, Error, PartialEq)]
pub enum PromptError {
    #[error("persona missing required section: {0}")]
    MissingSection(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonaSections {
    pub identity: String,
    pub personality: String,
    pub abilities: String,
    pub rules: String,
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
    if body.starts_with("身份") {
        SectionKind::Identity
    } else if body.starts_with("性格") {
        SectionKind::Personality
    } else if body.starts_with("能力") {
        SectionKind::Abilities
    } else if body.starts_with("行为规则") {
        SectionKind::Rules
    } else if body.starts_with("离线模板") || body.starts_with("反应配置") {
        SectionKind::Terminator
    } else {
        SectionKind::OtherH1
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

    // 1. 安全前缀（M1 占位空字符串；非空才输出）
    if !SAFETY_PREFIX_PLACEHOLDER.is_empty() {
        parts.push(SAFETY_PREFIX_PLACEHOLDER.to_string());
    }

    // 2. 角色身份
    parts.push("你是一个 AI 桌面伙伴。以下是你扮演的角色定义：".to_string());
    parts.push(sections.identity.clone());
    parts.push(sections.personality.clone());
    parts.push(sections.abilities.clone());
    parts.push(sections.rules.clone());

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

/// LLM 调用前包装当前 user 输入；DB 仍存原始 input（ChatPanel 显示原文）。
pub fn wrap_user_input(persona_name: &str, raw_input: &str) -> String {
    format!("（保持 {persona_name} 风格）{raw_input}")
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
            // 'system' 不应出现在 history（ChatService 不写 system role）；防御性 skip
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

    const MOMO_RAW: &str = include_str!("../../../personas/_builtin/momo.soul.md");

    fn make_persona(raw_markdown: &str, name: &str) -> PersonaSummary {
        PersonaSummary {
            id: "momo".to_string(),
            name: name.to_string(),
            version: "1.0.0".to_string(),
            source: "builtin".to_string(),
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
    fn build_messages_propagates_missing_section_error() {
        let bad = make_persona("# 身份\nx\n\n# 能力\ny\n\n# 行为规则\nz", "默默");
        let result = build_messages(&bad, None, "默默", &[], "hi");
        assert_eq!(result, Err(PromptError::MissingSection(LABEL_PERSONALITY)));
    }
}
