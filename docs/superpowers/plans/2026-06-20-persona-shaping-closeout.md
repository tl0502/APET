# Persona Shaping Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Persona Workshop shaping fields produce visible runtime behavior by compiling them into natural-language `SoulRuntimeProfile.style_prompt`, aligning draft validation, and adding built-in example dialogues.

**Architecture:** Keep the runtime contract unchanged. The single back-end compile boundary remains `src-tauri/src/services/persona.rs::compile_persona_draft`; front-end work is limited to pure draft validation helpers in `src/features/persona-workshop/draft.ts`; no Vue SFC/component boundary changes are needed.

**Tech Stack:** Rust, Tauri 2, serde JSON, Vue 3 + TypeScript pure helpers, Vitest, Cargo tests.

---

## Scope Check

This plan implements `docs/superpowers/specs/2026-06-20-persona-shaping-closeout-design.md` A2-C0 only. It does not implement example preview, LLM-assisted generation, memory bullets, live state, sandbox chat, new persona creation, snapshot history UI, or import/export source format.

## File Structure

- `src-tauri/src/services/persona.rs`
  - Existing owner of Persona draft compilation, diagnostics, built-in parsing tests, and snapshot profile generation.
  - Add private shaping helper functions in this file to avoid broad refactors.
  - Add Rust tests in the existing `#[cfg(test)] mod tests`.
- `src/features/persona-workshop/draft.ts`
  - Existing pure front-end draft projection and validation helper.
  - Add `name.empty` and `capabilities.empty` validation only.
- `src/features/persona-workshop/__tests__/draft.test.ts`
  - Existing Vitest file for draft helper behavior.
  - Add validation regression test.
- `src-tauri/personas/_builtin/momo.soul.md`
- `src-tauri/personas/_builtin/joker.soul.md`
- `src-tauri/personas/_builtin/coach.soul.md`
  - Add `# 例对话` sections using the pair format already supported by A2-B.
- `docs/STATUS.md`
  - Update current session snapshot after implementation and verification.

No `.vue` components, composables, Pinia stores, IPC commands, migrations, or schema files should change.

---

### Task 1: Rust RED Tests For Shaping Prompt Compilation

**Files:**
- Modify: `src-tauri/src/services/persona.rs`

- [ ] **Step 1: Add failing tests for natural-language shaping prompt**

In `src-tauri/src/services/persona.rs`, inside `#[cfg(test)] mod tests`, add these tests after `compile_draft_falls_back_to_simple_examples_when_structured_examples_empty`:

```rust
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
        assert!(
            style.contains("回避偏好：除非用户主动要求，否则避开：空洞鼓励；连续追问私人情绪")
        );
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
```

- [ ] **Step 2: Add failing tests for slider clamp and unknown options**

In the same test module, immediately after the previous test, add:

```rust
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
        assert_eq!(compiled.runtime_profile.initiative_config["mode"], "sometimes");

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
```

- [ ] **Step 3: Run Rust tests and verify RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib compile_draft_style_prompt_explains_shaping_fields
```

Expected: FAIL because `style_prompt` still contains bare values like `warmth=4` and does not contain `# 一句话定位`.

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib compile_draft_clamps_out_of_range_tone_sliders_before_prompt
```

Expected: FAIL because `255/5` is still present or no `tone.slider.out_of_range` diagnostic exists.

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib compile_draft_uses_safe_defaults_for_unknown_shaping_options
```

Expected: FAIL because unknown options are not normalized and no `tone.option.unknown` diagnostics exist.

---

### Task 2: Rust GREEN Implementation For Shaping Prompt Compilation

**Files:**
- Modify: `src-tauri/src/services/persona.rs`

- [ ] **Step 1: Add private shaping helper code**

In `src-tauri/src/services/persona.rs`, add this block after `fn split_examples(draft: &PersonaSourceDraft) -> Vec<String>` and before `fn extract_markdown_section`:

```rust
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
        &format!("{field} 的值「{}」未识别，已按 {fallback} 处理", value.trim()),
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

fn slider_description(value: u8, low: &'static str, mid: &'static str, high: &'static str) -> &'static str {
    match value {
        0 | 1 => low,
        2 | 3 => mid,
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

fn relationship_style_description(
    value: &str,
    diagnostics: &mut Vec<PersonaDiagnostic>,
) -> String {
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

fn initiative_details(
    value: &str,
    diagnostics: &mut Vec<PersonaDiagnostic>,
) -> (String, String) {
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
        "normal" => ("normal".to_string(), "默认一到三句，先回答重点。".to_string()),
        "detailed" => (
            "detailed".to_string(),
            "可以展开说明，但先给结论，再补细节。".to_string(),
        ),
        other => {
            warn_unknown_option("speech_length", other, "normal", diagnostics);
            ("normal".to_string(), "默认一到三句，先回答重点。".to_string())
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
    let (initiative_mode, initiative_description) = initiative_details(&simple.initiative, diagnostics);
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
    parts.push(format!("# 关系与互动方式\n{}", interaction_lines.join("\n")));

    let (speech_length, speech_description) = speech_length_details(&simple.speech_length, diagnostics);
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
```

- [ ] **Step 2: Replace bare `style_prompt` construction**

In `compile_persona_draft`, replace the current `let style_prompt = format!` block that emits `warmth={}` / `playfulness={}` / `speech_length={}` with:

```rust
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
```

- [ ] **Step 3: Normalize `initiative_config`**

In the `SoulRuntimeProfile` construction inside `compile_persona_draft`, replace:

```rust
        initiative_config: json!({ "mode": draft.simple.initiative.as_str() }),
```

with:

```rust
        initiative_config: json!({ "mode": shaping_prompt.initiative_mode.as_str() }),
```

- [ ] **Step 4: Run Rust tests and verify GREEN**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib compile_draft_style_prompt_explains_shaping_fields
cargo test --manifest-path src-tauri/Cargo.toml --lib compile_draft_clamps_out_of_range_tone_sliders_before_prompt
cargo test --manifest-path src-tauri/Cargo.toml --lib compile_draft_uses_safe_defaults_for_unknown_shaping_options
```

Expected: all three commands PASS.

- [ ] **Step 5: Format Rust code**

Run:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml
```

Expected: command exits 0 and only Rust formatting changes appear.

- [ ] **Step 6: Commit Rust shaping prompt work**

Run:

```powershell
git add -- src-tauri/src/services/persona.rs
git commit -m "feat: strengthen persona shaping prompt"
```

Expected: commit succeeds and includes only `src-tauri/src/services/persona.rs`.

---

### Task 3: TypeScript RED Test For Draft Validation Alignment

**Files:**
- Modify: `src/features/persona-workshop/__tests__/draft.test.ts`

- [ ] **Step 1: Add failing validation test**

In `src/features/persona-workshop/__tests__/draft.test.ts`, add this test after `validates required identity and behavior rules`:

```ts
  test('validates name and capabilities as frontend errors', () => {
    const draft = createPersonaDraft(persona)
    const broken = {
      ...draft,
      simple: {
        ...draft.simple,
        name: '   ',
      },
      structured: {
        ...draft.structured,
        capabilities: '   ',
      },
    }

    const diagnostics = validatePersonaDraft(broken)

    expect(diagnostics).toContainEqual({
      code: 'name.empty',
      severity: 'error',
      message: '名字不能为空',
    })
    expect(diagnostics).toContainEqual({
      code: 'capabilities.empty',
      severity: 'error',
      message: '能力不能为空',
    })
  })
```

- [ ] **Step 2: Run Vitest and verify RED**

Run:

```powershell
pnpm vitest run src/features/persona-workshop/__tests__/draft.test.ts
```

Expected: FAIL because `name.empty` and `capabilities.empty` are not returned yet.

---

### Task 4: TypeScript GREEN Implementation For Draft Validation

**Files:**
- Modify: `src/features/persona-workshop/draft.ts`

- [ ] **Step 1: Add frontend validation checks**

In `validatePersonaDraft`, insert these checks immediately after `const diagnostics: PersonaDiagnostic[] = []`:

```ts
  if (!draft.simple.name.trim()) {
    diagnostics.push({ code: 'name.empty', severity: 'error', message: '名字不能为空' })
  }
```

Then insert this check after the existing `personality.empty` check and before `rules.do.empty`:

```ts
  if (!draft.structured.capabilities.trim()) {
    diagnostics.push({ code: 'capabilities.empty', severity: 'error', message: '能力不能为空' })
  }
```

- [ ] **Step 2: Run Vitest and verify GREEN**

Run:

```powershell
pnpm vitest run src/features/persona-workshop/__tests__/draft.test.ts
```

Expected: PASS for all draft helper tests.

- [ ] **Step 3: Run TypeScript typecheck**

Run:

```powershell
pnpm typecheck
```

Expected: PASS with no TypeScript errors.

- [ ] **Step 4: Commit frontend validation work**

Run:

```powershell
git add -- src/features/persona-workshop/draft.ts src/features/persona-workshop/__tests__/draft.test.ts
git commit -m "fix: align persona draft validation"
```

Expected: commit succeeds and includes only the draft helper and its test file.

---

### Task 5: Built-In Persona Examples

**Files:**
- Modify: `src-tauri/src/services/persona.rs`
- Modify: `src-tauri/personas/_builtin/momo.soul.md`
- Modify: `src-tauri/personas/_builtin/joker.soul.md`
- Modify: `src-tauri/personas/_builtin/coach.soul.md`

- [ ] **Step 1: Add failing built-in examples test**

In `src-tauri/src/services/persona.rs`, inside the existing test module, add this test after `parse_three_builtins_succeed`:

```rust
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
```

- [ ] **Step 2: Run Rust test and verify RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib builtin_personas_compile_runtime_examples
```

Expected: FAIL because the built-in `.soul.md` files do not yet contain `# 例对话`.

- [ ] **Step 3: Add momo examples**

In `src-tauri/personas/_builtin/momo.soul.md`, insert this section after `## 庆祝 / Celebration` and before `# 反应配置`:

```markdown
# 例对话

- 用户：我今天特别累，什么都不想做。
  默默：那就先不做。把肩膀放下来，我陪你缓一会儿。
- 用户：我是不是太废了？
  默默：不是。你只是累了，先别拿最糟的话评价自己。
- 用户：提醒我该休息了。
  默默：去喝口水，站起来一下。回来再继续。
```

- [ ] **Step 4: Add joker examples**

In `src-tauri/personas/_builtin/joker.soul.md`, insert this section after `## 庆祝 / Celebration` and before `# 反应配置`:

```markdown
# 例对话

- 用户：我刚刚把任务做完了！
  阿吉：!!!这不得记入史册？来，世界冠军先坐下喝水。
- 用户：我今天很低落。
  阿吉：收到，梗先收起来。来一个虚拟抱抱，我在这儿。
- 用户：我又想摸鱼。
  阿吉：可以摸五分钟，小摸怡情，大摸明天哭。计时开始？
```

- [ ] **Step 5: Add coach examples**

In `src-tauri/personas/_builtin/coach.soul.md`, insert this section after `## 庆祝 / Celebration` and before `# 反应配置`:

```markdown
# 例对话

- 用户：我不知道从哪开始。
  教官：列三项。先做最小的那项。
- 用户：我今天状态不行。
  教官：明白。降级目标，不停摆。
- 用户：完成了。
  教官：完成。记录。下一项。
```

- [ ] **Step 6: Run Rust test and verify GREEN**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib builtin_personas_compile_runtime_examples
```

Expected: PASS.

- [ ] **Step 7: Commit built-in examples**

Run:

```powershell
git add -- src-tauri/src/services/persona.rs src-tauri/personas/_builtin/momo.soul.md src-tauri/personas/_builtin/joker.soul.md src-tauri/personas/_builtin/coach.soul.md
git commit -m "chore: add builtin persona examples"
```

Expected: commit succeeds and includes only the Rust built-in test plus the three built-in persona files.

---

### Task 6: Final Verification And Status Sync

**Files:**
- Modify: `docs/STATUS.md`

- [ ] **Step 1: Run focused Rust persona suite**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib persona::tests
```

Expected: PASS for all `services::persona::tests::*` tests.

- [ ] **Step 2: Run focused front-end draft suite**

Run:

```powershell
pnpm vitest run src/features/persona-workshop/__tests__/draft.test.ts
```

Expected: PASS.

- [ ] **Step 3: Run front-end typecheck**

Run:

```powershell
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 4: Inspect git diff**

Run:

```powershell
git status --short
git diff --stat HEAD
```

Expected: only `docs/STATUS.md` is uncommitted at this point. If any code files remain uncommitted, stop and inspect before continuing.

- [ ] **Step 5: Update STATUS.md current snapshot**

In `docs/STATUS.md`, update the `当前状态` section to reflect this implementation:

```markdown
- **当前 session 在做**：A2-C0 人格塑形收口已完成：工坊滑杆/tagline/relationshipStyle/dislikes/initiative 编译为自然语言 `SoulRuntimeProfile.style_prompt`；异常滑杆 clamp + warning；前端 name/capabilities 校验与 Rust blocking 口径对齐；momo/joker/coach 内置 `# 例对话` 补齐。
- **下一步**：进入 A2-C 示例预览 / LLM 辅助生成评估，或回到 M2 [#23] 物理交互 + 心情/精力 + 摸鱼。
- **阻塞**：无
```

Keep the existing milestone progress sections unchanged unless they already mention A2-C0 in a way that is now stale.

- [ ] **Step 6: Commit STATUS.md**

Run:

```powershell
git add -- docs/STATUS.md
git commit -m "docs: update persona shaping status"
```

Expected: commit succeeds and includes only `docs/STATUS.md`.

- [ ] **Step 7: Final clean tree check**

Run:

```powershell
git status --short
```

Expected: no output.

---

## Self-Review

- Spec coverage:
  - Natural-language prompt: Task 1 + Task 2.
  - Slider clamp and diagnostics: Task 1 + Task 2.
  - Unknown enum safe defaults: Task 1 + Task 2.
  - Front-end validation alignment: Task 3 + Task 4.
  - Built-in examples: Task 5.
  - Verification and status snapshot: Task 6.
- Type consistency:
  - Rust uses existing `PersonaSimpleDraft` fields: `relationship_style`, `speech_length`, `initiative`.
  - TypeScript uses existing `PersonaSimpleDraft` / `PersonaStructuredDraft` camelCase fields.
  - Diagnostic codes introduced by tests match implementation: `tone.slider.out_of_range`, `tone.option.unknown`, `name.empty`, `capabilities.empty`.
- Component boundary check:
  - No SFCs change. Vue component data flow, props/emits, and composables remain untouched.
