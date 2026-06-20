// H.1 PersonaService MVP — 加载内置 momo + 写入 SQLite
//
// 范围(plan H.1):
// - parse_persona(&str) 通用解析器(接受任意 .soul.md 字符串,内置 / 用户文件 / 导入复用)
// - seed_builtin(&AppHandle) 启动入口 — 把 include_str! 编译进 binary 的 momo 写入 personas / persona_snapshots
//
// 设计:
// - frontmatter 用 gray_matter (yaml feature) 解析 → 反序列化到 PersonaFrontmatter
// - 必填字段检查(id / name / version / schema_version)用空字符串/0 哨兵 + 显式 MissingField 错误,
//   而不是依赖 serde missing field 错误(那个文案是英文,不便 UI 展示)
// - schema_version 仅接受 1 或 2(persona-design v1.0 §2.3 至少向后兼容前 1 个 schema)
// - markdown 区段不切分,直接把 raw markdown 存 persona_snapshots.content;切分推到 B.2 拼 system prompt 时按需做
//
// DB:
// - tauri-plugin-sql 2.4 的 DbPool 公共方法被注释掉(看 wrapper.rs 行 37-64),Rust 端无法借用 plugin 的 Pool
// - 自己开 sqlx::SqliteConnection 短期连接,做完 drop;DB 路径与 plugin 一致(<app_config>/aipet.db)
// - personas 走 ON CONFLICT(id) DO UPDATE(idempotent),persona_snapshots 走 (persona_id, version) 唯一性守卫

use chrono::Utc;
use gray_matter::engine::YAML;
use gray_matter::Matter;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{Connection, Transaction};
use tauri::{AppHandle, Runtime};
use thiserror::Error;

use crate::services::db::{open_app_db, DbError};

/// 内置人格,编译期注入(与 migrations/001_init.sql 的 include_str! 同款)。
/// ADR-009 Accepted:默默 / 阿吉 / 教官。
const MOMO_RAW: &str = include_str!("../../personas/_builtin/momo.soul.md");
const JOKER_RAW: &str = include_str!("../../personas/_builtin/joker.soul.md");
const COACH_RAW: &str = include_str!("../../personas/_builtin/coach.soul.md");

/// 内置 file_path 标识 — 用 `<bundled>:` 前缀区别于用户人格(后者填真实 APPDATA 路径)
const MOMO_BUNDLED_PATH: &str = "<bundled>:_builtin/momo.soul.md";
const JOKER_BUNDLED_PATH: &str = "<bundled>:_builtin/joker.soul.md";
const COACH_BUNDLED_PATH: &str = "<bundled>:_builtin/coach.soul.md";

/// 启动期内置 seed 清单。顺序约定 momo 第一,因为首启默认 active=momo
/// (flows §1.2 Step 2 "跳过 → 默认默默"语义)。
const BUILTIN_SEEDS: &[(&str, &str, bool)] = &[
    (MOMO_RAW, MOMO_BUNDLED_PATH, true),
    (JOKER_RAW, JOKER_BUNDLED_PATH, false),
    (COACH_RAW, COACH_BUNDLED_PATH, false),
];

const SUPPORTED_SCHEMAS: &[u32] = &[1, 2];
const WORKSHOP_FILE_PATH_PREFIX: &str = "<workshop>:";
const PERSONA_DANGEROUS_FIELDS: &[&str] = &[
    "permissions",
    "tools",
    "safety_prefix",
    "system_prefix",
    "clipboard",
    "screen_capture",
];

#[derive(Debug, Deserialize, Default)]
pub struct PersonaFrontmatter {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    // MVP 只消费上述 4 个必填字段;author / voice_pack / accessories / tone_profile 等会被
    // gray_matter Pod 解析,但 PersonaFrontmatter struct 不声明这些字段 —— serde 默认忽略多余字段。
    // H.2/H.3 GUI 工坊接入时再扩展 struct。
}

#[derive(Debug)]
pub struct ParsedPersona {
    pub frontmatter: PersonaFrontmatter,
    pub raw_markdown: String,
}

#[derive(Debug, Error)]
pub enum PersonaError {
    #[error("frontmatter parse failed: {0}")]
    FrontmatterParse(String),
    #[error("schema_version unsupported: got {0}, want one of {1:?}")]
    UnsupportedSchema(u32, &'static [u32]),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("database error: {0}")]
    Database(String),
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("snapshot not found: {0}")]
    SnapshotNotFound(i64),
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl From<sqlx::Error> for PersonaError {
    fn from(e: sqlx::Error) -> Self {
        PersonaError::Database(e.to_string())
    }
}

impl From<DbError> for PersonaError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::AppConfigDir(s) => PersonaError::AppConfigDir(s),
            DbError::Database(s) => PersonaError::Database(s),
        }
    }
}

impl From<serde_json::Error> for PersonaError {
    fn from(e: serde_json::Error) -> Self {
        PersonaError::Serialization(e.to_string())
    }
}

/// PersonaService 对外契约（与前端 src/types/persona.ts::PersonaSummary 对齐）。
///
/// raw_markdown 是去掉 frontmatter 后的纯 markdown 正文，供 ChatService 拼 system prompt。
#[derive(Debug, Serialize)]
pub struct PersonaSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: String,
    pub snapshot_id: String,
    pub raw_markdown: String,
}

/// PersonaService list 契约：onboarding Step 2 / 设置面板列表用。
///
/// 不含 raw_markdown 字段——picker 只显示 id/name + active 高亮，正文按需 `persona_load(id)` 拉。
#[derive(Debug, Serialize)]
pub struct PersonaListItem {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaSimpleDraft {
    pub name: String,
    pub tagline: String,
    pub relationship_style: String,
    pub warmth: u8,
    pub playfulness: u8,
    pub formality: u8,
    pub proactivity: u8,
    pub brevity: u8,
    pub speech_length: String,
    pub initiative: String,
    pub dislikes: Vec<String>,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaStructuredDraft {
    pub identity: String,
    pub personality: String,
    pub capabilities: String,
    pub rules_do: Vec<String>,
    pub rules_dont: Vec<String>,
    pub offline_templates: String,
    pub reactions: String,
    pub examples: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaSourceDraft {
    pub persona_id: String,
    pub version: String,
    pub source: String,
    pub simple: PersonaSimpleDraft,
    pub structured: PersonaStructuredDraft,
    #[allow(dead_code)]
    pub source_text: String,
    pub preserved_unknown_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PersonaDiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonaDiagnostic {
    pub code: String,
    pub severity: PersonaDiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonaDraftValidationResult {
    pub diagnostics: Vec<PersonaDiagnostic>,
    pub blocking: bool,
    pub token_estimate: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonaSaveResult {
    pub persona_id: String,
    pub snapshot_id: String,
    pub version: String,
    pub activated: bool,
    pub diagnostics: Vec<PersonaDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulRuntimeProfile {
    pub identity_prompt: String,
    #[serde(default)]
    pub capabilities_prompt: String,
    pub style_prompt: String,
    pub examples: Vec<String>,
    pub initiative_config: serde_json::Value,
    pub memory_policy: serde_json::Value,
    pub ui_metadata: serde_json::Value,
    pub source_kind: String,
    pub source_hash: String,
}

#[derive(Debug, Clone)]
struct CompiledPersonaDraft {
    source_text: String,
    runtime_profile: SoulRuntimeProfile,
    diagnostics: Vec<PersonaDiagnostic>,
    source_hash: String,
    token_estimate: u32,
}

#[derive(Debug, Error)]
pub enum PersonaLookupError {
    #[error(transparent)]
    Db(#[from] PersonaError),
    #[error("persona not found: {0}")]
    NotFound(String),
}

impl From<DbError> for PersonaLookupError {
    fn from(e: DbError) -> Self {
        PersonaLookupError::Db(e.into())
    }
}

impl From<sqlx::Error> for PersonaLookupError {
    fn from(e: sqlx::Error) -> Self {
        PersonaLookupError::Db(e.into())
    }
}

/// 解析任意 .soul.md 字符串为 ParsedPersona。
///
/// 不依赖 IO — 内置走 const,用户走 fs::read_to_string,后续 H.2 import 走拖拽 payload。
pub fn parse_persona(content: &str) -> Result<ParsedPersona, PersonaError> {
    let matter = Matter::<YAML>::new();
    let parsed = matter.parse(content);

    let pod = parsed.data.ok_or_else(|| {
        PersonaError::FrontmatterParse("missing frontmatter block (no `---` delimiters?)".into())
    })?;

    let frontmatter: PersonaFrontmatter = pod
        .deserialize()
        .map_err(|e| PersonaError::FrontmatterParse(e.to_string()))?;

    if frontmatter.schema_version == 0 {
        return Err(PersonaError::MissingField("schema_version"));
    }
    if !SUPPORTED_SCHEMAS.contains(&frontmatter.schema_version) {
        return Err(PersonaError::UnsupportedSchema(
            frontmatter.schema_version,
            SUPPORTED_SCHEMAS,
        ));
    }
    if frontmatter.id.is_empty() {
        return Err(PersonaError::MissingField("id"));
    }
    if frontmatter.name.is_empty() {
        return Err(PersonaError::MissingField("name"));
    }
    if frontmatter.version.is_empty() {
        return Err(PersonaError::MissingField("version"));
    }

    Ok(ParsedPersona {
        frontmatter,
        raw_markdown: parsed.content,
    })
}

fn diagnostic(code: &str, severity: PersonaDiagnosticSeverity, message: &str) -> PersonaDiagnostic {
    PersonaDiagnostic {
        code: code.to_string(),
        severity,
        message: message.to_string(),
    }
}

fn has_blocking_diagnostics(diagnostics: &[PersonaDiagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|d| d.severity == PersonaDiagnosticSeverity::Error)
}

fn sanitize_bullets(items: &[String]) -> Vec<String> {
    items
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn join_section(parts: &mut Vec<String>, heading: &str, body: &str) {
    parts.push(format!("{heading}\n{}", body.trim()));
}

fn project_draft_to_source(draft: &PersonaSourceDraft) -> String {
    let rules_do = sanitize_bullets(&draft.structured.rules_do);
    let rules_dont = sanitize_bullets(&draft.structured.rules_dont);
    let mut parts = Vec::new();

    join_section(&mut parts, "# 身份", &draft.structured.identity);
    join_section(&mut parts, "# 性格", &draft.structured.personality);
    join_section(&mut parts, "# 能力", &draft.structured.capabilities);

    let rules = format!(
        "## Do\n{}\n\n## Don't\n{}",
        rules_do
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n"),
        rules_dont
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    join_section(&mut parts, "# 行为规则", &rules);
    join_section(
        &mut parts,
        "# 离线模板",
        &draft.structured.offline_templates,
    );
    join_section(&mut parts, "# 反应配置", &draft.structured.reactions);

    if !draft.structured.examples.trim().is_empty() {
        join_section(&mut parts, "# 例对话", &draft.structured.examples);
    }
    if !draft.preserved_unknown_text.trim().is_empty() {
        parts.push(draft.preserved_unknown_text.trim().to_string());
    }

    parts
        .into_iter()
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn source_hash(source_text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source_text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn estimate_tokens(source_text: &str) -> u32 {
    source_text.chars().count().div_ceil(3) as u32
}

#[derive(Debug, Clone, PartialEq)]
struct PersonaExamplePair {
    user: String,
    assistant: String,
}

fn strip_speaker_prefix(line: &str) -> Option<(&str, &str)> {
    let normalized = line.trim().trim_start_matches("- ").trim();
    let split_at = normalized.find('：').or_else(|| normalized.find(':'))?;
    let (speaker, rest) = normalized.split_at(split_at);
    let text = rest.trim_start_matches('：').trim_start_matches(':').trim();
    if speaker.trim().is_empty() {
        return None;
    }
    Some((speaker.trim(), text))
}

fn normalize_example_text(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_example_pairs(markdown: &str) -> Vec<PersonaExamplePair> {
    let mut pairs = Vec::new();
    let mut current: Option<PersonaExamplePair> = None;
    let mut target: Option<&'static str> = None;

    for raw_line in markdown.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }

        if let Some((speaker, text)) = strip_speaker_prefix(line) {
            if speaker == "用户" {
                if let Some(pair) = current.take() {
                    pairs.push(pair);
                }
                current = Some(PersonaExamplePair {
                    user: text.to_string(),
                    assistant: String::new(),
                });
                target = Some("user");
                continue;
            }

            if let Some(pair) = current.as_mut() {
                pair.assistant = text.to_string();
                target = Some("assistant");
                continue;
            }
        }

        if let (Some(pair), Some(target)) = (current.as_mut(), target) {
            match target {
                "user" => {
                    pair.user.push('\n');
                    pair.user.push_str(line.trim());
                }
                "assistant" => {
                    pair.assistant.push('\n');
                    pair.assistant.push_str(line.trim());
                }
                _ => {}
            }
        }
    }

    if let Some(pair) = current {
        pairs.push(pair);
    }
    pairs
}

fn complete_example_pairs(pairs: Vec<PersonaExamplePair>) -> Vec<PersonaExamplePair> {
    pairs
        .into_iter()
        .map(|pair| PersonaExamplePair {
            user: normalize_example_text(&pair.user),
            assistant: normalize_example_text(&pair.assistant),
        })
        .filter(|pair| !pair.user.is_empty() && !pair.assistant.is_empty())
        .take(5)
        .collect()
}

fn pair_to_runtime_example(pair: PersonaExamplePair, assistant_name: &str) -> String {
    let assistant_name = assistant_name.trim();
    let assistant_name = if assistant_name.is_empty() {
        "助手"
    } else {
        assistant_name
    };
    format!(
        "用户：{}\n{}：{}",
        pair.user, assistant_name, pair.assistant
    )
}

fn split_examples(draft: &PersonaSourceDraft) -> Vec<String> {
    let assistant_name = draft.simple.name.trim();
    let structured_pairs = parse_example_pairs(&draft.structured.examples);
    let complete_structured = complete_example_pairs(structured_pairs);
    if !complete_structured.is_empty() {
        return complete_structured
            .into_iter()
            .map(|pair| pair_to_runtime_example(pair, assistant_name))
            .collect();
    }

    let simple_pairs = draft
        .simple
        .examples
        .iter()
        .flat_map(|example| parse_example_pairs(example))
        .collect::<Vec<_>>();
    complete_example_pairs(simple_pairs)
        .into_iter()
        .map(|pair| pair_to_runtime_example(pair, assistant_name))
        .collect()
}

struct ShapingPrompt {
    text: String,
    initiative_mode: String,
}

fn warn_unknown_option(
    field: &str,
    value: &str,
    fallback: &str,
    diagnostics: &mut Vec<PersonaDiagnostic>,
) {
    diagnostics.push(diagnostic(
        "tone.option.unknown",
        PersonaDiagnosticSeverity::Warning,
        &format!(
            "{field} 的值「{}」未识别，已按 {fallback} 处理",
            value.trim()
        ),
    ));
}

fn clamp_tone_slider(
    field: &str,
    label: &str,
    value: u8,
    diagnostics: &mut Vec<PersonaDiagnostic>,
) -> u8 {
    if value <= 5 {
        return value;
    }

    diagnostics.push(diagnostic(
        "tone.slider.out_of_range",
        PersonaDiagnosticSeverity::Warning,
        &format!("{field}（{label}）应在 0-5 之间，已按 5 处理"),
    ));
    5
}

fn slider_description(
    value: u8,
    low: &'static str,
    mid: &'static str,
    high: &'static str,
) -> &'static str {
    match value {
        0..=2 => low,
        3 => mid,
        _ => high,
    }
}

fn tone_line(
    field: &str,
    label: &str,
    value: u8,
    low: &'static str,
    mid: &'static str,
    high: &'static str,
    diagnostics: &mut Vec<PersonaDiagnostic>,
) -> String {
    let clamped = clamp_tone_slider(field, label, value, diagnostics);
    format!(
        "- {label} {clamped}/5：{}",
        slider_description(clamped, low, mid, high)
    )
}

fn relationship_style_description(value: &str, diagnostics: &mut Vec<PersonaDiagnostic>) -> String {
    match value.trim() {
        "companion" => "陪伴型搭档，优先站在用户身边一起想办法。".to_string(),
        "buddy" => "朋友型损友，可以轻松互动，但不贬低用户。".to_string(),
        "coach" => "教练型伙伴，目标清晰，提醒直接但不刻薄。".to_string(),
        "custom" => "自定义关系风格，优先遵守身份、性格和行为规则中的具体描述。".to_string(),
        other => {
            warn_unknown_option("relationshipStyle", other, "companion", diagnostics);
            "陪伴型搭档，优先站在用户身边一起想办法。".to_string()
        }
    }
}

fn initiative_details(value: &str, diagnostics: &mut Vec<PersonaDiagnostic>) -> (String, String) {
    match value.trim() {
        "quiet" => (
            "quiet".to_string(),
            "尽量等待用户开口，只做必要回应。".to_string(),
        ),
        "sometimes" => (
            "sometimes".to_string(),
            "偶尔主动推进话题，但不连续催促。".to_string(),
        ),
        "often" => (
            "often".to_string(),
            "更愿意主动推进话题和下一步，但不连续催促。".to_string(),
        ),
        other => {
            warn_unknown_option("initiative", other, "sometimes", diagnostics);
            (
                "sometimes".to_string(),
                "偶尔主动推进话题，但不连续催促。".to_string(),
            )
        }
    }
}

fn speech_length_details(
    value: &str,
    diagnostics: &mut Vec<PersonaDiagnostic>,
) -> (String, String) {
    match value.trim() {
        "short" => ("short".to_string(), "默认短句，必要时再展开。".to_string()),
        "normal" => (
            "normal".to_string(),
            "默认一到三句，先回答重点。".to_string(),
        ),
        "detailed" => (
            "detailed".to_string(),
            "可以展开说明，但先给结论，再补细节。".to_string(),
        ),
        other => {
            warn_unknown_option("speech_length", other, "normal", diagnostics);
            (
                "normal".to_string(),
                "默认一到三句，先回答重点。".to_string(),
            )
        }
    }
}

fn build_shaping_prompt(
    simple: &PersonaSimpleDraft,
    diagnostics: &mut Vec<PersonaDiagnostic>,
) -> ShapingPrompt {
    let mut parts = Vec::new();

    let tagline = simple.tagline.trim();
    if !tagline.is_empty() {
        parts.push(format!("# 一句话定位\n{tagline}"));
    }

    let relationship = relationship_style_description(&simple.relationship_style, diagnostics);
    let (initiative_mode, initiative_description) =
        initiative_details(&simple.initiative, diagnostics);
    let dislikes = sanitize_bullets(&simple.dislikes);
    let mut interaction_lines = vec![
        format!("- 关系风格：{relationship}"),
        format!("- 主动性：{initiative_description}"),
    ];
    if !dislikes.is_empty() {
        interaction_lines.push(format!(
            "- 回避偏好：除非用户主动要求，否则避开：{}",
            dislikes.join("；")
        ));
    }
    parts.push(format!(
        "# 关系与互动方式\n{}",
        interaction_lines.join("\n")
    ));

    let (speech_length, speech_description) =
        speech_length_details(&simple.speech_length, diagnostics);
    let tone_lines = vec![
        tone_line(
            "warmth",
            "温暖度",
            simple.warmth,
            "语气偏冷静，少做情绪延展。",
            "语气温和，适度承接用户情绪。",
            "语气偏温暖，会简短承接用户情绪。",
            diagnostics,
        ),
        tone_line(
            "playfulness",
            "俏皮度",
            simple.playfulness,
            "基本不玩梗，保持端正。",
            "可以轻微调侃，但不频繁玩梗。",
            "可以频繁使用轻松调侃和小玩笑，但不伤人。",
            diagnostics,
        ),
        tone_line(
            "formality",
            "正式度",
            simple.formality,
            "使用朋友口吻，避免商务腔。",
            "保持自然礼貌，不太随意也不太正式。",
            "表达更正式克制，减少口语化玩笑。",
            diagnostics,
        ),
        tone_line(
            "proactivity",
            "主动度",
            simple.proactivity,
            "少主动推进，优先回应用户已经提出的内容。",
            "偶尔补一个下一步建议，但不抢话题。",
            "会主动推进话题或下一步，但不连续催促。",
            diagnostics,
        ),
        tone_line(
            "brevity",
            "简洁度",
            simple.brevity,
            "允许适度解释，必要时用两三句说清。",
            "默认简洁，复杂问题再分点说明。",
            "优先短句和结论，避免长段铺陈。",
            diagnostics,
        ),
        format!("- 回复长度 {speech_length}：{speech_description}"),
    ];
    parts.push(format!("# 语气参数\n{}", tone_lines.join("\n")));

    ShapingPrompt {
        text: parts.join("\n\n"),
        initiative_mode,
    }
}

fn extract_markdown_section(markdown: &str, label: &str) -> String {
    let lines = markdown.lines().collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|line| line.trim().starts_with(&format!("# {label}")));
    let Some(start) = start else {
        return String::new();
    };
    let mut body = Vec::new();
    for line in lines.iter().skip(start + 1) {
        if line.starts_with("# ") && !line.starts_with("## ") {
            break;
        }
        body.push(*line);
    }
    body.join("\n").trim().to_string()
}

fn extract_markdown_list_items(markdown: &str) -> Vec<String> {
    markdown
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("- "))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn split_markdown_rules(rules: &str) -> (Vec<String>, Vec<String>) {
    let do_index = rules.find("## Do");
    let dont_index = rules.find("## Don't");
    let do_text = do_index
        .map(|idx| &rules[idx..dont_index.unwrap_or(rules.len())])
        .unwrap_or("");
    let dont_text = dont_index.map(|idx| &rules[idx..]).unwrap_or("");
    (
        extract_markdown_list_items(do_text),
        extract_markdown_list_items(dont_text),
    )
}

fn compile_parsed_persona(parsed: &ParsedPersona, source: &str) -> CompiledPersonaDraft {
    let rules = extract_markdown_section(&parsed.raw_markdown, "行为规则");
    let (rules_do, rules_dont) = split_markdown_rules(&rules);
    let examples = extract_markdown_section(&parsed.raw_markdown, "例对话");
    let draft = PersonaSourceDraft {
        persona_id: parsed.frontmatter.id.clone(),
        version: parsed.frontmatter.version.clone(),
        source: source.to_string(),
        simple: PersonaSimpleDraft {
            name: parsed.frontmatter.name.clone(),
            tagline: extract_markdown_section(&parsed.raw_markdown, "身份")
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or_default()
                .trim()
                .to_string(),
            relationship_style: "companion".to_string(),
            warmth: 3,
            playfulness: 3,
            formality: 2,
            proactivity: 3,
            brevity: 4,
            speech_length: "short".to_string(),
            initiative: "sometimes".to_string(),
            dislikes: rules_dont.iter().take(3).cloned().collect(),
            examples: extract_markdown_list_items(&examples),
        },
        structured: PersonaStructuredDraft {
            identity: extract_markdown_section(&parsed.raw_markdown, "身份"),
            personality: extract_markdown_section(&parsed.raw_markdown, "性格"),
            capabilities: extract_markdown_section(&parsed.raw_markdown, "能力"),
            rules_do,
            rules_dont,
            offline_templates: extract_markdown_section(&parsed.raw_markdown, "离线模板"),
            reactions: extract_markdown_section(&parsed.raw_markdown, "反应配置"),
            examples,
        },
        source_text: parsed.raw_markdown.clone(),
        preserved_unknown_text: String::new(),
    };
    compile_persona_draft(&draft)
}

fn compile_persona_draft(draft: &PersonaSourceDraft) -> CompiledPersonaDraft {
    let source_text = project_draft_to_source(draft);
    let source_hash = source_hash(&source_text);
    let token_estimate = estimate_tokens(&source_text);
    let rules_do = sanitize_bullets(&draft.structured.rules_do);
    let rules_dont = sanitize_bullets(&draft.structured.rules_dont);
    let mut diagnostics = Vec::new();

    if draft.simple.name.trim().is_empty() {
        diagnostics.push(diagnostic(
            "name.empty",
            PersonaDiagnosticSeverity::Error,
            "名字不能为空",
        ));
    }
    if draft.structured.identity.trim().is_empty() {
        diagnostics.push(diagnostic(
            "identity.empty",
            PersonaDiagnosticSeverity::Error,
            "身份不能为空",
        ));
    }
    if draft.structured.personality.trim().is_empty() {
        diagnostics.push(diagnostic(
            "personality.empty",
            PersonaDiagnosticSeverity::Error,
            "性格不能为空",
        ));
    }
    if draft.structured.capabilities.trim().is_empty() {
        diagnostics.push(diagnostic(
            "capabilities.empty",
            PersonaDiagnosticSeverity::Error,
            "能力不能为空",
        ));
    }
    if rules_do.is_empty() && rules_dont.is_empty() {
        diagnostics.push(diagnostic(
            "rules.empty",
            PersonaDiagnosticSeverity::Error,
            "至少需要 1 条行为规则",
        ));
    } else {
        if rules_do.is_empty() {
            diagnostics.push(diagnostic(
                "rules.do.empty",
                PersonaDiagnosticSeverity::Warning,
                "建议至少写 1 条 Do 规则",
            ));
        }
        if rules_dont.is_empty() {
            diagnostics.push(diagnostic(
                "rules.dont.empty",
                PersonaDiagnosticSeverity::Warning,
                "建议至少写 1 条 Don't 规则",
            ));
        }
    }

    let source_lower = source_text.to_ascii_lowercase();
    for field in PERSONA_DANGEROUS_FIELDS {
        if source_lower.contains(field) {
            diagnostics.push(diagnostic(
                "source.dangerous_field",
                PersonaDiagnosticSeverity::Error,
                &format!("源码包含不允许的人格字段：{field}"),
            ));
        }
    }

    if token_estimate > 1200 {
        diagnostics.push(diagnostic(
            "budget.high",
            PersonaDiagnosticSeverity::Warning,
            "人格定义偏长，会挤压聊天历史",
        ));
    }

    let examples = split_examples(draft);
    let structured_pairs = parse_example_pairs(&draft.structured.examples);
    let simple_pairs = draft
        .simple
        .examples
        .iter()
        .flat_map(|example| parse_example_pairs(example))
        .collect::<Vec<_>>();
    let has_any_example_pair = !structured_pairs.is_empty() || !simple_pairs.is_empty();
    if structured_pairs
        .iter()
        .chain(simple_pairs.iter())
        .any(|pair| pair.user.trim().is_empty() || pair.assistant.trim().is_empty())
    {
        diagnostics.push(diagnostic(
            "examples.partial",
            PersonaDiagnosticSeverity::Warning,
            "存在未写完整的示例对话，保存时会跳过不完整样本",
        ));
    }
    if examples.is_empty() && !has_any_example_pair {
        diagnostics.push(diagnostic(
            "examples.empty",
            PersonaDiagnosticSeverity::Warning,
            "建议补充 1-3 条示例对话；没有示例时，AI 只能靠身份与规则判断语气。",
        ));
    }

    let shaping_prompt = build_shaping_prompt(&draft.simple, &mut diagnostics);
    let style_prompt = vec![
        draft.structured.personality.trim().to_string(),
        format!(
            "# 行为规则\n## Do\n{}\n\n## Don't\n{}",
            rules_do
                .iter()
                .map(|item| format!("- {item}"))
                .collect::<Vec<_>>()
                .join("\n"),
            rules_dont
                .iter()
                .map(|item| format!("- {item}"))
                .collect::<Vec<_>>()
                .join("\n")
        ),
        shaping_prompt.text,
    ]
    .into_iter()
    .map(|part| part.trim().to_string())
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("\n\n");

    let runtime_profile = SoulRuntimeProfile {
        identity_prompt: draft.structured.identity.trim().to_string(),
        capabilities_prompt: draft.structured.capabilities.trim().to_string(),
        style_prompt,
        examples,
        initiative_config: json!({ "mode": shaping_prompt.initiative_mode.as_str() }),
        memory_policy: json!({ "mode": "default" }),
        ui_metadata: json!({
            "name": draft.simple.name.trim(),
            "source": draft.source.as_str(),
            "version": draft.version.trim(),
            "relationshipStyle": draft.simple.relationship_style.as_str(),
            "tagline": draft.simple.tagline.trim(),
            "dislikes": draft.simple.dislikes.clone(),
        }),
        source_kind: "legacy_soul_md".to_string(),
        source_hash: source_hash.clone(),
    };

    CompiledPersonaDraft {
        source_text,
        runtime_profile,
        diagnostics,
        source_hash,
        token_estimate,
    }
}

pub fn validate_draft(draft: &PersonaSourceDraft) -> PersonaDraftValidationResult {
    let compiled = compile_persona_draft(draft);
    PersonaDraftValidationResult {
        blocking: has_blocking_diagnostics(&compiled.diagnostics),
        diagnostics: compiled.diagnostics,
        token_estimate: compiled.token_estimate,
    }
}

/// 启动入口:解析所有内置人格 → 依次 UPSERT personas + idempotent INSERT persona_snapshots。
///
/// 调用方在 lib.rs setup 阶段用 `tauri::async_runtime::block_on` 同步等(冷启 50-200ms,
/// 避免前端 persona_load("momo") 与 seed 的 race);失败仅 eprintln 到 stderr 不阻塞启动。
///
/// 默认 active:仅 momo 首次 INSERT 时 is_active=1,其余 0;ON CONFLICT 路径**不动** is_active,
/// 保证用户在 Step 2 / 设置切到 joker/coach 后,后续重启 seed 不抹掉用户的选择。
pub async fn seed_builtin<R: Runtime>(app: &AppHandle<R>) -> Result<(), PersonaError> {
    let mut conn = open_app_db(app).await?;
    for (raw, file_path, set_active) in BUILTIN_SEEDS {
        let parsed = parse_persona(raw)?;
        seed_persona_with_conn(&mut conn, &parsed, "builtin", file_path, *set_active).await?;
    }
    conn.close().await?;
    Ok(())
}

/// 读取 persona + 最新 snapshot，拼成 PersonaSummary。NotFound 时返回 NotFound 变体（前端可定向提示）。
pub async fn load_persona<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
) -> Result<PersonaSummary, PersonaLookupError> {
    let mut conn = open_app_db(app).await?;
    let summary = load_persona_with_conn(&mut conn, id).await?;
    conn.close().await.map_err(PersonaError::from)?;
    Ok(summary)
}

/// 读取当前 active persona（personas WHERE is_active = 1 LIMIT 1）。
///
/// #13 ChatService 拼 system prompt 时用：用户没指定时直接拿当前激活人格。
/// 没有 active 行（首启 seed 之前 / 全部停用）→ NotFound("active persona")。
pub async fn load_active_persona<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<PersonaSummary, PersonaLookupError> {
    let mut conn = open_app_db(app).await?;
    let summary = load_active_persona_with_conn(&mut conn).await?;
    conn.close().await.map_err(PersonaError::from)?;
    Ok(summary)
}

/// 列出所有人格 summary（不含 raw_markdown）。onboarding Step 2 / 设置面板列表用。
///
/// 排序：is_active DESC（active 优先）、id ASC（稳定次序），让 picker 默认聚焦当前 active 卡片。
pub async fn list_personas<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<PersonaListItem>, PersonaError> {
    let mut conn = open_app_db(app).await?;
    let items = list_personas_with_conn(&mut conn).await?;
    conn.close().await?;
    Ok(items)
}

pub(crate) async fn list_personas_with_conn(
    conn: &mut sqlx::SqliteConnection,
) -> Result<Vec<PersonaListItem>, PersonaError> {
    let rows: Vec<(String, String, String, String, i64)> = sqlx::query_as(
        "SELECT id, name, version, source, is_active FROM personas \
         ORDER BY is_active DESC, id ASC",
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, name, version, source, is_active)| PersonaListItem {
            id,
            name,
            version,
            source,
            is_active: is_active != 0,
        })
        .collect())
}

/// B1 修复：with_conn 变体，让 ChatService::prepare 把多个 service 调用串到同一条 conn 上，
/// 把单 chat_send 的 open/close 周期从 8 次降到 2 次（prepare 1 + run_stream 收尾 1）。
pub(crate) async fn load_active_persona_with_conn(
    conn: &mut sqlx::SqliteConnection,
) -> Result<PersonaSummary, PersonaLookupError> {
    let active: Option<(String, Option<i64>)> =
        sqlx::query_as("SELECT id, active_snapshot_id FROM personas WHERE is_active = 1 LIMIT 1")
            .fetch_optional(&mut *conn)
            .await
            .map_err(PersonaError::from)?;
    let (id, snapshot_id) =
        active.ok_or_else(|| PersonaLookupError::NotFound("active persona".to_string()))?;
    if let Some(snapshot_id) = snapshot_id {
        return load_persona_snapshot_with_conn(conn, snapshot_id).await;
    }
    let latest = latest_snapshot_id_with_conn(conn, &id)
        .await?
        .ok_or_else(|| PersonaLookupError::NotFound(format!("active snapshot for {id}")))?;
    load_persona_snapshot_with_conn(conn, latest).await
}

pub(crate) async fn load_persona_with_conn(
    conn: &mut sqlx::SqliteConnection,
    id: &str,
) -> Result<PersonaSummary, PersonaLookupError> {
    let row: Option<(String, String, String, String)> =
        sqlx::query_as("SELECT id, name, version, source FROM personas WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
    let (pid, name, version, source) =
        row.ok_or_else(|| PersonaLookupError::NotFound(id.to_string()))?;
    let mut snap: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT id, version, content FROM persona_snapshots \
         WHERE persona_id = ? AND version = ? \
         ORDER BY id DESC LIMIT 1",
    )
    .bind(&pid)
    .bind(&version)
    .fetch_optional(&mut *conn)
    .await?;
    if snap.is_none() {
        snap = sqlx::query_as(
            "SELECT id, version, content FROM persona_snapshots \
             WHERE persona_id = ? \
             ORDER BY id DESC LIMIT 1",
        )
        .bind(&pid)
        .fetch_optional(&mut *conn)
        .await?;
    }
    Ok(PersonaSummary {
        id: pid,
        name,
        version: snap
            .as_ref()
            .map(|(_, version, _)| version.clone())
            .unwrap_or(version),
        source,
        snapshot_id: snap
            .as_ref()
            .map(|(id, _, _)| id.to_string())
            .unwrap_or_default(),
        raw_markdown: snap.map(|(_, _, c)| c).unwrap_or_default(),
    })
}

pub(crate) async fn load_persona_snapshot_with_conn(
    conn: &mut sqlx::SqliteConnection,
    snapshot_id: i64,
) -> Result<PersonaSummary, PersonaLookupError> {
    let row: Option<(String, String, String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT p.id, p.name, s.version, p.source, CAST(s.id AS TEXT), s.content
        FROM persona_snapshots s
        INNER JOIN personas p ON p.id = s.persona_id
        WHERE s.id = ?
        LIMIT 1
        "#,
    )
    .bind(snapshot_id)
    .fetch_optional(&mut *conn)
    .await?;
    let (id, name, version, source, snapshot_id, raw_markdown) =
        row.ok_or_else(|| PersonaLookupError::NotFound(format!("snapshot {snapshot_id}")))?;
    Ok(PersonaSummary {
        id,
        name,
        version,
        source,
        snapshot_id,
        raw_markdown,
    })
}

pub(crate) async fn load_persona_for_conversation_with_conn(
    conn: &mut sqlx::SqliteConnection,
    conversation_id: &str,
) -> Result<PersonaSummary, PersonaLookupError> {
    let row: Option<(String, Option<i64>)> = sqlx::query_as(
        "SELECT persona_id, persona_snapshot_id FROM conversations WHERE id = ? AND archived = 0",
    )
    .bind(conversation_id)
    .fetch_optional(&mut *conn)
    .await?;
    let (persona_id, snapshot_id) =
        row.ok_or_else(|| PersonaLookupError::NotFound(format!("conversation {conversation_id}")))?;
    let snapshot_id = snapshot_id.ok_or_else(|| {
        PersonaLookupError::NotFound(format!(
            "conversation {conversation_id} missing persona snapshot binding for {persona_id}"
        ))
    })?;
    load_persona_snapshot_with_conn(conn, snapshot_id).await
}

async fn latest_snapshot_id_with_conn(
    conn: &mut sqlx::SqliteConnection,
    persona_id: &str,
) -> Result<Option<i64>, PersonaError> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM persona_snapshots WHERE persona_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(persona_id)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row.map(|(id,)| id))
}

/// 把目标 persona 设为 active（其他全部清零）。NotFound 时报 PersonaLookupError::NotFound。
pub async fn activate_persona<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
) -> Result<(), PersonaLookupError> {
    let mut conn = open_app_db(app).await?;
    activate_persona_with_conn(&mut conn, id).await?;
    conn.close().await.map_err(PersonaError::from)?;
    Ok(())
}

pub(crate) async fn activate_persona_with_conn(
    conn: &mut sqlx::SqliteConnection,
    id: &str,
) -> Result<(), PersonaLookupError> {
    let snapshot_id = latest_snapshot_id_with_conn(conn, id)
        .await?
        .ok_or_else(|| PersonaLookupError::NotFound(format!("snapshot for {id}")))?;
    let mut tx = conn.begin().await?;
    sqlx::query("UPDATE personas SET is_active = 0")
        .execute(&mut *tx)
        .await?;
    let result = sqlx::query(
        "UPDATE personas SET is_active = 1, active_snapshot_id = ?, updated_at = ? WHERE id = ?",
    )
    .bind(snapshot_id)
    .bind(Utc::now().to_rfc3339())
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() == 0 {
        return Err(PersonaLookupError::NotFound(id.to_string()));
    }
    tx.commit().await?;
    Ok(())
}

pub async fn save_draft<R: Runtime>(
    app: &AppHandle<R>,
    draft: PersonaSourceDraft,
    activate: bool,
) -> Result<PersonaSaveResult, PersonaError> {
    let mut conn = open_app_db(app).await?;
    let result = save_draft_with_conn(&mut conn, draft, activate).await?;
    conn.close().await?;
    Ok(result)
}

pub(crate) async fn save_draft_with_conn(
    conn: &mut sqlx::SqliteConnection,
    draft: PersonaSourceDraft,
    activate: bool,
) -> Result<PersonaSaveResult, PersonaError> {
    let compiled = compile_persona_draft(&draft);
    if has_blocking_diagnostics(&compiled.diagnostics) {
        return Err(PersonaError::Validation(
            compiled
                .diagnostics
                .iter()
                .filter(|d| d.severity == PersonaDiagnosticSeverity::Error)
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }

    let persona_id = draft.persona_id.trim();
    if persona_id.is_empty() {
        return Err(PersonaError::Validation("persona id 不能为空".to_string()));
    }
    let version = next_available_version_with_conn(conn, persona_id, draft.version.trim()).await?;
    let now = Utc::now().to_rfc3339();
    let runtime_profile_json = serde_json::to_string(&compiled.runtime_profile)?;
    let file_path = format!("{WORKSHOP_FILE_PATH_PREFIX}{persona_id}.soul.md");

    let mut tx = conn.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO personas
            (id, name, version, source, file_path, is_active, created_at, updated_at, active_snapshot_id)
        VALUES (?, ?, ?, 'user', ?, 0, ?, ?, NULL)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            version = excluded.version,
            source = excluded.source,
            file_path = excluded.file_path,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(persona_id)
    .bind(draft.simple.name.trim())
    .bind(&version)
    .bind(&file_path)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    let snapshot_id =
        insert_snapshot(&mut tx, persona_id, &version, &compiled.source_text, &now).await?;
    insert_snapshot_profile(
        &mut tx,
        snapshot_id,
        persona_id,
        &runtime_profile_json,
        &compiled.source_hash,
        &now,
    )
    .await?;

    if activate {
        activate_snapshot_in_tx(&mut tx, snapshot_id, &now).await?;
    }

    tx.commit().await?;

    Ok(PersonaSaveResult {
        persona_id: persona_id.to_string(),
        snapshot_id: snapshot_id.to_string(),
        version,
        activated: activate,
        diagnostics: compiled.diagnostics,
    })
}

pub async fn activate_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    snapshot_id: i64,
) -> Result<(), PersonaError> {
    let mut conn = open_app_db(app).await?;
    activate_snapshot_with_conn(&mut conn, snapshot_id).await?;
    conn.close().await?;
    Ok(())
}

pub(crate) async fn activate_snapshot_with_conn(
    conn: &mut sqlx::SqliteConnection,
    snapshot_id: i64,
) -> Result<(), PersonaError> {
    let now = Utc::now().to_rfc3339();
    let mut tx = conn.begin().await?;
    activate_snapshot_in_tx(&mut tx, snapshot_id, &now).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn get_snapshot_profile<R: Runtime>(
    app: &AppHandle<R>,
    snapshot_id: i64,
) -> Result<SoulRuntimeProfile, PersonaError> {
    let mut conn = open_app_db(app).await?;
    let profile = get_snapshot_profile_with_conn(&mut conn, snapshot_id).await?;
    conn.close().await?;
    Ok(profile)
}

pub(crate) async fn get_snapshot_profile_with_conn(
    conn: &mut sqlx::SqliteConnection,
    snapshot_id: i64,
) -> Result<SoulRuntimeProfile, PersonaError> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT runtime_profile_json FROM persona_snapshot_profiles WHERE snapshot_id = ?",
    )
    .bind(snapshot_id)
    .fetch_optional(&mut *conn)
    .await?;
    let json = row
        .map(|(json,)| json)
        .ok_or(PersonaError::SnapshotNotFound(snapshot_id))?;
    Ok(serde_json::from_str(&json)?)
}

async fn next_available_version_with_conn(
    conn: &mut sqlx::SqliteConnection,
    persona_id: &str,
    requested: &str,
) -> Result<String, PersonaError> {
    let base = if requested.trim().is_empty() {
        "1.0.0"
    } else {
        requested.trim()
    };
    let mut candidate = base.to_string();
    loop {
        let exists: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM persona_snapshots WHERE persona_id = ? AND version = ? LIMIT 1",
        )
        .bind(persona_id)
        .bind(&candidate)
        .fetch_optional(&mut *conn)
        .await?;
        if exists.is_none() {
            return Ok(candidate);
        }
        candidate = bump_patch_version(&candidate);
    }
}

fn bump_patch_version(version: &str) -> String {
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        return format!("{version}.1");
    }
    let major = parts[0].parse::<u64>();
    let minor = parts[1].parse::<u64>();
    let patch = parts[2].parse::<u64>();
    match (major, minor, patch) {
        (Ok(major), Ok(minor), Ok(patch)) => format!("{major}.{minor}.{}", patch + 1),
        _ => format!("{version}.1"),
    }
}

async fn insert_snapshot(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    persona_id: &str,
    version: &str,
    content: &str,
    created_at: &str,
) -> Result<i64, PersonaError> {
    let result = sqlx::query(
        "INSERT INTO persona_snapshots (persona_id, version, content, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(persona_id)
    .bind(version)
    .bind(content)
    .bind(created_at)
    .execute(tx.as_mut())
    .await?;
    Ok(result.last_insert_rowid())
}

async fn insert_snapshot_profile(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    snapshot_id: i64,
    persona_id: &str,
    runtime_profile_json: &str,
    source_hash: &str,
    created_at: &str,
) -> Result<(), PersonaError> {
    sqlx::query(
        r#"
        INSERT INTO persona_snapshot_profiles
            (snapshot_id, persona_id, runtime_profile_json, source_hash, created_at)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(snapshot_id) DO UPDATE SET
            runtime_profile_json = excluded.runtime_profile_json,
            source_hash = excluded.source_hash
        "#,
    )
    .bind(snapshot_id)
    .bind(persona_id)
    .bind(runtime_profile_json)
    .bind(source_hash)
    .bind(created_at)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

async fn activate_snapshot_in_tx(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    snapshot_id: i64,
    now: &str,
) -> Result<(), PersonaError> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT persona_id, version FROM persona_snapshots WHERE id = ?")
            .bind(snapshot_id)
            .fetch_optional(tx.as_mut())
            .await?;
    let (persona_id, version) = row.ok_or(PersonaError::SnapshotNotFound(snapshot_id))?;

    sqlx::query("UPDATE personas SET is_active = 0")
        .execute(tx.as_mut())
        .await?;
    let result = sqlx::query(
        "UPDATE personas SET is_active = 1, active_snapshot_id = ?, version = ?, updated_at = ? WHERE id = ?",
    )
    .bind(snapshot_id)
    .bind(&version)
    .bind(now)
    .bind(&persona_id)
    .execute(tx.as_mut())
    .await?;
    if result.rows_affected() == 0 {
        return Err(PersonaError::Validation(format!(
            "snapshot {snapshot_id} belongs to missing persona {persona_id}"
        )));
    }
    Ok(())
}

/// Inner helper(2026-05-04 test-coverage):接 SqliteConnection,不依赖 AppHandle。
///
/// 与 prod `seed_builtin` 完全等价的 SQL 语义:begin tx → UPSERT persona → INSERT snapshot → commit。
///
/// `set_active`:仅在 **首次 INSERT** 时写入 is_active 字段;ON CONFLICT 路径(persona id 已存在)
/// 不更新 is_active,保护用户跑步切换过的 active 状态不被后续 seed 抹掉。
pub(crate) async fn seed_persona_with_conn(
    conn: &mut sqlx::SqliteConnection,
    parsed: &ParsedPersona,
    source: &str,
    file_path: &str,
    set_active: bool,
) -> Result<(), PersonaError> {
    let mut tx = conn.begin().await?;
    upsert_persona(&mut tx, parsed, source, file_path, set_active).await?;
    let snapshot_id = insert_snapshot_if_new(&mut tx, &parsed.frontmatter.id, parsed).await?;
    let compiled = compile_parsed_persona(parsed, source);
    let runtime_profile_json = serde_json::to_string(&compiled.runtime_profile)?;
    let now = Utc::now().to_rfc3339();
    insert_snapshot_profile(
        &mut tx,
        snapshot_id,
        &parsed.frontmatter.id,
        &runtime_profile_json,
        &compiled.source_hash,
        &now,
    )
    .await?;
    sqlx::query(
        "UPDATE personas SET active_snapshot_id = COALESCE(active_snapshot_id, ?) WHERE id = ?",
    )
    .bind(snapshot_id)
    .bind(&parsed.frontmatter.id)
    .execute(tx.as_mut())
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn upsert_persona(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    parsed: &ParsedPersona,
    source: &str,
    file_path: &str,
    set_active: bool,
) -> Result<(), PersonaError> {
    let now = Utc::now().to_rfc3339();
    let is_active_value: i64 = if set_active { 1 } else { 0 };
    sqlx::query(
        r#"
        INSERT INTO personas (id, name, version, source, file_path, is_active, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            name = CASE WHEN personas.source = 'user' AND personas.file_path LIKE '<workshop>:%' THEN personas.name ELSE excluded.name END,
            version = CASE WHEN personas.source = 'user' AND personas.file_path LIKE '<workshop>:%' THEN personas.version ELSE excluded.version END,
            file_path = CASE WHEN personas.source = 'user' AND personas.file_path LIKE '<workshop>:%' THEN personas.file_path ELSE excluded.file_path END,
            updated_at = CASE WHEN personas.source = 'user' AND personas.file_path LIKE '<workshop>:%' THEN personas.updated_at ELSE excluded.updated_at END
        "#,
    )
    .bind(&parsed.frontmatter.id)
    .bind(&parsed.frontmatter.name)
    .bind(&parsed.frontmatter.version)
    .bind(source)
    .bind(file_path)
    .bind(is_active_value)
    .bind(&now)
    .bind(&now)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

async fn insert_snapshot_if_new(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    persona_id: &str,
    parsed: &ParsedPersona,
) -> Result<i64, PersonaError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO persona_snapshots (persona_id, version, content, created_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(persona_id, version) DO NOTHING
        "#,
    )
    .bind(persona_id)
    .bind(&parsed.frontmatter.version)
    .bind(&parsed.raw_markdown)
    .bind(&now)
    .execute(tx.as_mut())
    .await?;
    let row: (i64,) = sqlx::query_as(
        "SELECT id FROM persona_snapshots WHERE persona_id = ? AND version = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(persona_id)
    .bind(&parsed.frontmatter.version)
    .fetch_one(tx.as_mut())
    .await?;
    Ok(row.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_workshop_draft(persona_id: &str, name: &str, version: &str) -> PersonaSourceDraft {
        PersonaSourceDraft {
            persona_id: persona_id.to_string(),
            version: version.to_string(),
            source: "builtin".to_string(),
            simple: PersonaSimpleDraft {
                name: name.to_string(),
                tagline: "安静陪伴型伙伴".to_string(),
                relationship_style: "companion".to_string(),
                warmth: 3,
                playfulness: 2,
                formality: 2,
                proactivity: 3,
                brevity: 4,
                speech_length: "short".to_string(),
                initiative: "sometimes".to_string(),
                dislikes: vec!["空洞鼓励".to_string()],
                examples: vec!["用户：你好\n默默：我在。".to_string()],
            },
            structured: PersonaStructuredDraft {
                identity: format!("你叫{name}，是一个安静的桌面伙伴。"),
                personality: "- 温和\n- 克制".to_string(),
                capabilities: "- 陪用户整理想法\n- 提醒用户休息".to_string(),
                rules_do: vec!["用第二人称回应".to_string()],
                rules_dont: vec!["不空洞鼓励".to_string()],
                offline_templates: "## 拒答 / Refusal\n- 这个我现在不适合处理。".to_string(),
                reactions: "- click.head: 轻声回应".to_string(),
                examples: "- 用户：你好\n  默默：我在。".to_string(),
            },
            source_text: String::new(),
            preserved_unknown_text: String::new(),
        }
    }

    #[test]
    fn parse_momo_succeeds() {
        let parsed = parse_persona(MOMO_RAW).expect("momo should parse");
        assert_eq!(parsed.frontmatter.id, "momo");
        assert_eq!(parsed.frontmatter.name, "默默");
        assert_eq!(parsed.frontmatter.version, "1.0.0");
        assert_eq!(parsed.frontmatter.schema_version, 2);
        assert!(!parsed.raw_markdown.is_empty());
    }

    #[test]
    fn parse_invalid_yaml_fails() {
        let bad = "---\nid: : : broken\n---\n# 身份\nx";
        let result = parse_persona(bad);
        assert!(matches!(result, Err(PersonaError::FrontmatterParse(_))));
    }

    #[test]
    fn parse_missing_id_fails() {
        let no_id = "---\nschema_version: 2\nname: 默默\nversion: 1.0.0\n---\n# 身份\nx";
        let result = parse_persona(no_id);
        assert!(matches!(result, Err(PersonaError::MissingField("id"))));
    }

    #[test]
    fn parse_unsupported_schema_fails() {
        let future = "---\nschema_version: 999\nid: x\nname: x\nversion: 1.0.0\n---\n# 身份\nx";
        let result = parse_persona(future);
        assert!(matches!(
            result,
            Err(PersonaError::UnsupportedSchema(999, _))
        ));
    }

    #[test]
    fn parse_schema_v1_compatible() {
        let v1 = "---\nschema_version: 1\nid: legacy\nname: 老人格\nversion: 0.9.0\n---\n# 身份\nx";
        let parsed = parse_persona(v1).expect("schema v1 must still parse");
        assert_eq!(parsed.frontmatter.schema_version, 1);
        assert_eq!(parsed.frontmatter.id, "legacy");
    }

    #[test]
    fn parse_strips_frontmatter_from_raw() {
        let parsed = parse_persona(MOMO_RAW).expect("momo should parse");
        assert!(
            !parsed.raw_markdown.contains("schema_version:"),
            "raw_markdown must not contain frontmatter"
        );
        assert!(
            parsed.raw_markdown.contains("# 身份"),
            "raw_markdown must contain # 身份 heading"
        );
    }

    // ===== DB 集成测试(2026-05-04 test-coverage P0)=====

    use crate::services::test_db::fresh_db;

    #[tokio::test]
    async fn seed_builtin_writes_personas_row() {
        let (_dir, mut conn) = fresh_db().await;
        let parsed = parse_persona(MOMO_RAW).unwrap();
        seed_persona_with_conn(&mut conn, &parsed, "builtin", MOMO_BUNDLED_PATH, true)
            .await
            .unwrap();

        // personas 表应有 momo 一行,is_active = 1
        let row: (String, String, String, String, String, i64) = sqlx::query_as(
            "SELECT id, name, version, source, file_path, is_active FROM personas WHERE id = ?",
        )
        .bind(&parsed.frontmatter.id)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(row.0, "momo");
        assert_eq!(row.1, "默默");
        assert_eq!(row.2, "1.0.0");
        assert_eq!(row.3, "builtin");
        assert_eq!(row.4, MOMO_BUNDLED_PATH);
        assert_eq!(row.5, 1, "seeded persona must be active");
    }

    #[tokio::test]
    async fn seed_builtin_writes_persona_snapshot() {
        let (_dir, mut conn) = fresh_db().await;
        let parsed = parse_persona(MOMO_RAW).unwrap();
        seed_persona_with_conn(&mut conn, &parsed, "builtin", MOMO_BUNDLED_PATH, true)
            .await
            .unwrap();

        // persona_snapshots 表应有 (momo, 1.0.0) 一行
        let row: (String, String) = sqlx::query_as(
            "SELECT persona_id, version FROM persona_snapshots WHERE persona_id = ?",
        )
        .bind(&parsed.frontmatter.id)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(row.0, "momo");
        assert_eq!(row.1, "1.0.0");
    }

    #[tokio::test]
    async fn seed_builtin_writes_snapshot_profile_and_active_snapshot() {
        let (_dir, mut conn) = fresh_db().await;
        let parsed = parse_persona(MOMO_RAW).unwrap();
        seed_persona_with_conn(&mut conn, &parsed, "builtin", MOMO_BUNDLED_PATH, true)
            .await
            .unwrap();

        let snapshot_id: i64 =
            sqlx::query_scalar("SELECT active_snapshot_id FROM personas WHERE id = 'momo'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        let profile = get_snapshot_profile_with_conn(&mut conn, snapshot_id)
            .await
            .unwrap();

        assert_eq!(profile.source_kind, "legacy_soul_md");
        assert!(profile.source_hash.starts_with("sha256:"));
        assert!(profile.ui_metadata["name"]
            .as_str()
            .unwrap()
            .contains("默默"));
    }

    #[tokio::test]
    async fn load_persona_summary_includes_snapshot_id() {
        let (_dir, mut conn) = fresh_db().await;
        let parsed = parse_persona(MOMO_RAW).unwrap();
        seed_persona_with_conn(&mut conn, &parsed, "builtin", MOMO_BUNDLED_PATH, true)
            .await
            .unwrap();

        let expected_id: i64 = sqlx::query_scalar(
            "SELECT id FROM persona_snapshots WHERE persona_id = ? AND version = ?",
        )
        .bind(&parsed.frontmatter.id)
        .bind(&parsed.frontmatter.version)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        let summary = load_persona_with_conn(&mut conn, &parsed.frontmatter.id)
            .await
            .unwrap();

        assert_eq!(summary.snapshot_id, expected_id.to_string());
    }

    #[tokio::test]
    async fn seed_builtin_is_idempotent() {
        // 关键:启动时 seed 多次(用户重启 / dev tooling reload)不能重复插入 snapshot
        let (_dir, mut conn) = fresh_db().await;
        let parsed = parse_persona(MOMO_RAW).unwrap();

        // 跑 3 次 seed
        for _ in 0..3 {
            seed_persona_with_conn(&mut conn, &parsed, "builtin", MOMO_BUNDLED_PATH, true)
                .await
                .unwrap();
        }

        // personas 仍然只有 1 行(UPSERT)
        let persona_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM personas WHERE id = 'momo'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(persona_count.0, 1, "personas idempotent under repeat seed");

        // persona_snapshots 也只有 1 行(ON CONFLICT(persona_id, version) DO NOTHING + 002 unique idx)
        let snap_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM persona_snapshots WHERE persona_id = 'momo' AND version = '1.0.0'",
        )
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(
            snap_count.0, 1,
            "snapshot must be deduped by (persona_id, version)"
        );
    }

    #[tokio::test]
    async fn persona_snapshot_unique_index_blocks_direct_duplicate_insert() {
        // 防御:002 migration 的 UNIQUE INDEX 必须能挡住绕过 ON CONFLICT 的直接 INSERT
        // (例如未来 H.2 import flow 用了 INSERT 不带 ON CONFLICT)
        let (_dir, mut conn) = fresh_db().await;
        let parsed = parse_persona(MOMO_RAW).unwrap();
        seed_persona_with_conn(&mut conn, &parsed, "builtin", MOMO_BUNDLED_PATH, true)
            .await
            .unwrap();

        // 直接 INSERT 同 (persona_id, version) — 不走 ON CONFLICT,期待 unique 索引报错
        let result = sqlx::query(
            "INSERT INTO persona_snapshots (persona_id, version, content, created_at) \
             VALUES ('momo', '1.0.0', 'duplicate', '2026-05-04')",
        )
        .execute(&mut conn)
        .await;
        assert!(
            result.is_err(),
            "direct duplicate INSERT must violate idx_persona_snapshots_unique_persona_version"
        );
    }

    #[test]
    fn validate_draft_rejects_dangerous_source_fields() {
        let mut draft = valid_workshop_draft("momo", "默默", "1.0.0");
        draft.preserved_unknown_text = "# tools\n- screen_capture".to_string();

        let result = validate_draft(&draft);

        assert!(result.blocking);
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.code == "source.dangerous_field"));
    }

    #[test]
    fn compile_draft_keeps_structured_example_pair_as_one_runtime_example() {
        let draft = valid_workshop_draft("momo", "默默", "1.0.0");

        let compiled = compile_persona_draft(&draft);

        assert_eq!(compiled.runtime_profile.examples.len(), 1);
        assert_eq!(
            compiled.runtime_profile.examples[0],
            "用户：你好\n默默：我在。"
        );
    }

    #[test]
    fn compile_draft_prefers_structured_examples_over_simple_examples() {
        let mut draft = valid_workshop_draft("momo", "默默", "1.0.0");
        draft.simple.examples = vec!["用户：simple\n默默：simple reply".to_string()];
        draft.structured.examples = "- 用户：structured\n  默默：structured reply".to_string();

        let compiled = compile_persona_draft(&draft);

        assert_eq!(
            compiled.runtime_profile.examples,
            vec!["用户：structured\n默默：structured reply".to_string()]
        );
    }

    #[test]
    fn compile_draft_falls_back_to_simple_examples_when_structured_examples_empty() {
        let mut draft = valid_workshop_draft("momo", "默默", "1.0.0");
        draft.structured.examples = String::new();
        draft.simple.examples = vec!["用户：simple\n默默：simple reply".to_string()];

        let compiled = compile_persona_draft(&draft);

        assert_eq!(
            compiled.runtime_profile.examples,
            vec!["用户：simple\n默默：simple reply".to_string()]
        );
    }

    #[test]
    fn compile_draft_style_prompt_explains_shaping_fields() {
        let mut draft = valid_workshop_draft("momo", "默默", "1.0.0");
        draft.simple.tagline = "安静但可靠".to_string();
        draft.simple.relationship_style = "companion".to_string();
        draft.simple.warmth = 4;
        draft.simple.playfulness = 5;
        draft.simple.formality = 1;
        draft.simple.proactivity = 4;
        draft.simple.brevity = 2;
        draft.simple.speech_length = "detailed".to_string();
        draft.simple.initiative = "often".to_string();
        draft.simple.dislikes = vec!["空洞鼓励".to_string(), "连续追问私人情绪".to_string()];

        let compiled = compile_persona_draft(&draft);
        let style = &compiled.runtime_profile.style_prompt;

        assert!(style.contains("# 一句话定位"));
        assert!(style.contains("安静但可靠"));
        assert!(style.contains("# 关系与互动方式"));
        assert!(style.contains("关系风格：陪伴型搭档"));
        assert!(style.contains("主动性：更愿意主动推进话题"));
        assert!(style.contains("回避偏好：除非用户主动要求，否则避开：空洞鼓励；连续追问私人情绪"));
        assert!(style.contains("# 语气参数"));
        assert!(style.contains("温暖度 4/5：语气偏温暖"));
        assert!(style.contains("俏皮度 5/5：可以频繁使用轻松调侃"));
        assert!(style.contains("正式度 1/5：使用朋友口吻"));
        assert!(style.contains("主动度 4/5：会主动推进话题"));
        assert!(style.contains("简洁度 2/5：允许适度解释"));
        assert!(style.contains("回复长度 detailed：可以展开说明"));
        assert!(!style.contains("warmth=4"));
        assert_eq!(compiled.runtime_profile.initiative_config["mode"], "often");
    }

    #[test]
    fn compile_draft_includes_capabilities_as_runtime_contract() {
        let mut draft = valid_workshop_draft("momo", "默默", "1.0.0");
        draft.structured.capabilities = "- 帮用户拆任务\n- 提醒用户休息".to_string();

        let compiled = compile_persona_draft(&draft);

        assert_eq!(
            compiled.runtime_profile.capabilities_prompt,
            "- 帮用户拆任务\n- 提醒用户休息"
        );
        assert!(
            !compiled
                .runtime_profile
                .style_prompt
                .contains("帮用户拆任务"),
            "capabilities should stay separate from style/rules"
        );
    }

    #[test]
    fn runtime_profile_deserializes_legacy_snapshot_without_capabilities_prompt() {
        let raw = serde_json::json!({
            "identity_prompt": "你叫默默。",
            "style_prompt": "# 风格\n- 短句",
            "examples": [],
            "initiative_config": { "mode": "sometimes" },
            "memory_policy": { "mode": "default" },
            "ui_metadata": { "name": "默默" },
            "source_kind": "legacy_soul_md",
            "source_hash": "sha256:old"
        });

        let profile: SoulRuntimeProfile = serde_json::from_value(raw).unwrap();

        assert_eq!(profile.capabilities_prompt, "");
    }

    #[test]
    fn compile_draft_clamps_out_of_range_tone_sliders_before_prompt() {
        let mut draft = valid_workshop_draft("momo", "默默", "1.0.0");
        draft.simple.warmth = 255;
        draft.simple.proactivity = 9;

        let compiled = compile_persona_draft(&draft);
        let style = &compiled.runtime_profile.style_prompt;

        assert!(style.contains("温暖度 5/5"));
        assert!(style.contains("主动度 5/5"));
        assert!(!style.contains("255/5"));
        assert!(!style.contains("9/5"));

        let warning_count = compiled
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == "tone.slider.out_of_range"
                    && diagnostic.severity == PersonaDiagnosticSeverity::Warning
            })
            .count();
        assert_eq!(warning_count, 2);
    }

    #[test]
    fn compile_draft_uses_safe_defaults_for_unknown_shaping_options() {
        let mut draft = valid_workshop_draft("momo", "默默", "1.0.0");
        draft.simple.relationship_style = "stranger".to_string();
        draft.simple.speech_length = "verbose".to_string();
        draft.simple.initiative = "pushy".to_string();

        let compiled = compile_persona_draft(&draft);
        let style = &compiled.runtime_profile.style_prompt;

        assert!(style.contains("关系风格：陪伴型搭档"));
        assert!(style.contains("主动性：偶尔主动推进话题"));
        assert!(style.contains("回复长度 normal：默认一到三句"));
        assert_eq!(
            compiled.runtime_profile.initiative_config["mode"],
            "sometimes"
        );

        let warning_count = compiled
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == "tone.option.unknown"
                    && diagnostic.severity == PersonaDiagnosticSeverity::Warning
            })
            .count();
        assert_eq!(warning_count, 3);
    }

    #[test]
    fn validate_draft_warns_for_partial_examples_without_blocking() {
        let mut draft = valid_workshop_draft("momo", "默默", "1.0.0");
        draft.simple.examples.clear();
        draft.structured.examples = "- 用户：只有用户".to_string();

        let result = validate_draft(&draft);

        assert!(!result.blocking);
        assert!(!result
            .diagnostics
            .iter()
            .any(|d| d.code == "examples.empty"));
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.code == "examples.partial"));
    }

    #[tokio::test]
    async fn save_draft_creates_snapshot_and_profile() {
        let (_dir, mut conn) = fresh_db().await;
        let draft = valid_workshop_draft("momo", "默默", "1.0.0");

        let result = save_draft_with_conn(&mut conn, draft, false).await.unwrap();

        assert_eq!(result.persona_id, "momo");
        assert_eq!(result.version, "1.0.0");
        assert!(!result.activated);

        let snapshot_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM persona_snapshots WHERE persona_id = 'momo'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        let profile_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM persona_snapshot_profiles WHERE snapshot_id = ?",
        )
        .bind(result.snapshot_id.parse::<i64>().unwrap())
        .fetch_one(&mut conn)
        .await
        .unwrap();

        assert_eq!(snapshot_count, 1);
        assert_eq!(profile_count, 1);
    }

    #[tokio::test]
    async fn repeated_save_bumps_patch_version_on_conflict() {
        let (_dir, mut conn) = fresh_db().await;
        let draft = valid_workshop_draft("momo", "默默", "1.0.0");

        let first = save_draft_with_conn(&mut conn, draft.clone(), false)
            .await
            .unwrap();
        let second = save_draft_with_conn(&mut conn, draft, false).await.unwrap();

        assert_eq!(first.version, "1.0.0");
        assert_eq!(second.version, "1.0.1");
    }

    #[tokio::test]
    async fn save_and_activate_keeps_exactly_one_active_persona() {
        let (_dir, mut conn) = fresh_db().await;
        let momo = valid_workshop_draft("momo", "默默", "1.0.0");
        let joker = valid_workshop_draft("joker", "阿吉", "1.0.0");

        save_draft_with_conn(&mut conn, momo, true).await.unwrap();
        let active_joker = save_draft_with_conn(&mut conn, joker, true).await.unwrap();

        let active_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM personas WHERE is_active = 1")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        let row: (String, i64) =
            sqlx::query_as("SELECT id, active_snapshot_id FROM personas WHERE is_active = 1")
                .fetch_one(&mut conn)
                .await
                .unwrap();

        assert_eq!(active_count, 1);
        assert_eq!(row.0, "joker");
        assert_eq!(row.1.to_string(), active_joker.snapshot_id);
    }

    #[tokio::test]
    async fn activate_snapshot_switches_active_snapshot() {
        let (_dir, mut conn) = fresh_db().await;
        let draft = valid_workshop_draft("momo", "默默", "1.0.0");
        let first = save_draft_with_conn(&mut conn, draft.clone(), true)
            .await
            .unwrap();
        let second = save_draft_with_conn(&mut conn, draft, true).await.unwrap();

        activate_snapshot_with_conn(&mut conn, first.snapshot_id.parse::<i64>().unwrap())
            .await
            .unwrap();

        let active_snapshot_id: i64 =
            sqlx::query_scalar("SELECT active_snapshot_id FROM personas WHERE id = 'momo'")
                .fetch_one(&mut conn)
                .await
                .unwrap();

        assert_ne!(first.snapshot_id, second.snapshot_id);
        assert_eq!(active_snapshot_id.to_string(), first.snapshot_id);
    }

    #[tokio::test]
    async fn conversation_load_keeps_bound_snapshot_after_active_changes() {
        let (_dir, mut conn) = fresh_db().await;
        let draft = valid_workshop_draft("momo", "默默", "1.0.0");
        let first = save_draft_with_conn(&mut conn, draft.clone(), true)
            .await
            .unwrap();
        let second = save_draft_with_conn(&mut conn, draft, true).await.unwrap();
        let conv_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        sqlx::query(
            "INSERT INTO conversations \
             (id, persona_id, persona_snapshot_id, started_at, last_activity_at) \
             VALUES (?, 'momo', ?, '2026-06-19T00:00:00Z', '2026-06-19T00:00:00Z')",
        )
        .bind(conv_id)
        .bind(first.snapshot_id.parse::<i64>().unwrap())
        .execute(&mut conn)
        .await
        .unwrap();

        activate_snapshot_with_conn(&mut conn, second.snapshot_id.parse::<i64>().unwrap())
            .await
            .unwrap();
        let persona = load_persona_for_conversation_with_conn(&mut conn, conv_id)
            .await
            .unwrap();

        assert_eq!(persona.snapshot_id, first.snapshot_id);
        assert_eq!(persona.version, first.version);
    }

    #[tokio::test]
    async fn seed_different_versions_keeps_history_in_snapshots() {
        // 用户改 .soul.md 的 version 字段:同 persona_id 不同 version 应在 snapshots 累积历史
        let (_dir, mut conn) = fresh_db().await;
        let v1 = parse_persona(
            "---\nschema_version: 2\nid: momo\nname: 默默\nversion: 1.0.0\n---\n# 身份\nv1",
        )
        .unwrap();
        let v2 = parse_persona(
            "---\nschema_version: 2\nid: momo\nname: 默默\nversion: 1.1.0\n---\n# 身份\nv2",
        )
        .unwrap();

        seed_persona_with_conn(&mut conn, &v1, "user", "/tmp/momo.v1.md", true)
            .await
            .unwrap();
        seed_persona_with_conn(&mut conn, &v2, "user", "/tmp/momo.v2.md", true)
            .await
            .unwrap();

        // personas 表 UPSERT 后只有 1 行 momo,name/version 是 v2 的
        let persona_row: (String, String) =
            sqlx::query_as("SELECT name, version FROM personas WHERE id = 'momo'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(persona_row.1, "1.1.0", "personas.version reflects latest");

        // persona_snapshots 应有 2 行历史(1.0.0 + 1.1.0)
        let snap_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM persona_snapshots WHERE persona_id = 'momo'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(
            snap_count.0, 2,
            "snapshots accumulate per-version history (audit trail)"
        );
    }

    // ===== 3 内置人格 seed 语义（ADR-009 / #21 Step 2 前置）=====

    #[test]
    fn parse_all_three_builtins_succeed() {
        // 编译期 include_str! 进来的 3 个 .soul.md 必须都能 parse + 字段对得上
        let momo = parse_persona(MOMO_RAW).expect("momo should parse");
        assert_eq!(momo.frontmatter.id, "momo");
        assert_eq!(momo.frontmatter.name, "默默");

        let joker = parse_persona(JOKER_RAW).expect("joker should parse");
        assert_eq!(joker.frontmatter.id, "joker");
        assert_eq!(joker.frontmatter.name, "阿吉");

        let coach = parse_persona(COACH_RAW).expect("coach should parse");
        assert_eq!(coach.frontmatter.id, "coach");
        assert_eq!(coach.frontmatter.name, "教官");
    }

    #[test]
    fn builtin_personas_compile_runtime_examples() {
        for (id, raw) in [
            ("momo", MOMO_RAW),
            ("joker", JOKER_RAW),
            ("coach", COACH_RAW),
        ] {
            let parsed = parse_persona(raw).expect("builtin persona should parse");
            let compiled = compile_parsed_persona(&parsed, "builtin");

            assert!(
                !compiled.runtime_profile.examples.is_empty(),
                "{id} should include runtime examples"
            );
            assert!(
                compiled
                    .runtime_profile
                    .examples
                    .iter()
                    .all(|example| example.contains("用户：")),
                "{id} examples should include user turns"
            );
        }
    }

    /// 模拟 seed_builtin 在 fresh_db 上的语义。同 prod 路径,不依赖 AppHandle。
    async fn seed_all_builtins(conn: &mut sqlx::SqliteConnection) {
        for (raw, file_path, set_active) in BUILTIN_SEEDS {
            let parsed = parse_persona(raw).unwrap();
            seed_persona_with_conn(conn, &parsed, "builtin", file_path, *set_active)
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn seed_three_builtins_only_momo_active() {
        let (_dir, mut conn) = fresh_db().await;
        seed_all_builtins(&mut conn).await;

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM personas")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(count.0, 3, "all 3 builtins should be seeded");

        let active_id: (String,) = sqlx::query_as("SELECT id FROM personas WHERE is_active = 1")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(active_id.0, "momo", "momo is default active on first seed");

        let active_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM personas WHERE is_active = 1")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(active_count.0, 1, "exactly one active persona");
    }

    #[tokio::test]
    async fn repeat_seed_preserves_user_active_choice() {
        // 防御:用户切到 joker 后,后续 seed_builtin(重启)不能抹掉这个选择
        let (_dir, mut conn) = fresh_db().await;
        seed_all_builtins(&mut conn).await;

        // 用户在 Step 2 / 设置面板切到 joker
        activate_persona_with_conn(&mut conn, "joker")
            .await
            .unwrap();

        // 再 seed 一遍(模拟重启)
        seed_all_builtins(&mut conn).await;

        let active_id: (String,) = sqlx::query_as("SELECT id FROM personas WHERE is_active = 1")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(
            active_id.0, "joker",
            "user's active choice must survive re-seed"
        );
    }

    #[tokio::test]
    async fn repeat_seed_preserves_user_modified_builtin_metadata() {
        let (_dir, mut conn) = fresh_db().await;
        seed_all_builtins(&mut conn).await;

        let mut draft = valid_workshop_draft("momo", "小默", "1.0.0");
        draft.source = "builtin".to_string();
        let saved = save_draft_with_conn(&mut conn, draft, true).await.unwrap();

        assert_eq!(saved.persona_id, "momo");
        assert_eq!(saved.version, "1.0.1");

        seed_all_builtins(&mut conn).await;

        let row: (String, String, String, String) = sqlx::query_as(
            "SELECT name, version, source, file_path FROM personas WHERE id = 'momo'",
        )
        .fetch_one(&mut conn)
        .await
        .unwrap();

        assert_eq!(row.0, "小默");
        assert_eq!(row.1, "1.0.1");
        assert_eq!(row.2, "user");
        assert_eq!(row.3, "<workshop>:momo.soul.md");
    }

    #[tokio::test]
    async fn list_personas_returns_three_active_first() {
        let (_dir, mut conn) = fresh_db().await;
        seed_all_builtins(&mut conn).await;

        let items = list_personas_with_conn(&mut conn).await.unwrap();
        assert_eq!(items.len(), 3, "list returns all 3 builtins");
        // is_active DESC 排序 → momo（active=true）在第一位
        assert_eq!(items[0].id, "momo");
        assert!(items[0].is_active);
        // 后两位按 id ASC（coach < joker）
        assert_eq!(items[1].id, "coach");
        assert!(!items[1].is_active);
        assert_eq!(items[2].id, "joker");
        assert!(!items[2].is_active);
    }
}
