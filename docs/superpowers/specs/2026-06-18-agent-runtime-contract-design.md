---
title: Agent Runtime Contract Design
updated: 2026-06-18
related:
  - 2026-05-24-companion-agent-runtime-design.md
  - 2026-05-26-safety-policy-configurable-design.md
  - ../../persona/persona-design.md
  - ../../architecture/system-architecture.md
  - ../../decisions.md
---

# Agent Runtime Contract Design

> 2026-06-18 收口契约。本文件只定义 Agent Runtime hot path、prompt material contract、SafetyPolicy 最新口径、PersonaSnapshot/SoulRuntimeProfile 运行时边界。旧文档与本文件冲突时，以本文件为准。

## 1. Scope

本 spec 解决：

- Agent Runtime hot path
- Prompt material contract
- `PersonaSnapshot` / `SoulRuntimeProfile` 的运行时消费边界
- `SafetyPolicy` 最新口径（4 scope 出厂全 OFF）
- `.soul/` 降级为未决 source format
- 旧文档需要修正的冲突清单

本 spec 不解决：

- `.soul/` 文件结构
- 人格工坊 UI
- Tool sandbox 细节
- episodic memory / embeddings
- 完整主动陪伴策略

## 2. Current Runtime Flow

```text
SurfaceEvent
  ↓
ConversationSub.handle_user_message(surface, conv_id, input)
  ↓
ConversationRepo.read(conv_id)
  ↓
persona_snapshot_id
  ↓
PersonaSub.read_snapshot(persona_snapshot_id)
  ↓
MemorySub.read_prompt_context(conv_id, persona_snapshot_id)
  ↓
PromptBuilder.build(PromptBuildInput)
  ↓
SafetyGuard.wrap_messages(messages)
  ↓
LLMProvider.stream(messages)
  ↓
SafetyGuard scan path, policy-gated
  ↓
ConversationRepo finalize message
  ↓
StreamEvent to Surface
```

Hard rules:

- Hot path must not read active persona.
- Conversation must bind a stable `persona_snapshot_id`.
- PromptBuilder consumes runtime structs only; it must not read persona source files.
- SafetyGuard path remains mandatory, but all 4 SafetyPolicy scopes are OFF by default and disabled scopes behave as noop / always-pass.
- `.soul/` does not enter the hot path.

## 3. Prompt Material Contract

PromptBuilder takes a single runtime input object:

```rust
struct PromptBuildInput {
    runtime_profile: SoulRuntimeProfile,
    user_profile: UserPromptProfile,
    live_state: Option<LiveState>,
    memory_bullets: Vec<KvBullet>,
    history_window: Vec<MessageRecord>,
    current_input: String,
}
```

Prompt material order:

```text
[optional SafetyPrefix]
[system: app/runtime frame]
[system: persona identity]
[system: persona style]
[system: user profile]
[system: live state]
[system: memory bullets]
[few-shot examples]
[history window]
[current user input]
```

Semantics:

- `optional SafetyPrefix` is added only by `SafetyGuard.wrap_messages` when `SafetyPolicy.PrefixInjection` is ON.
- `app/runtime frame` defines app identity, output conventions, and runtime capability boundaries.
- `persona identity` comes from `SoulRuntimeProfile.identity_prompt`.
- `persona style` comes from `SoulRuntimeProfile.style_prompt`.
- `user profile` includes nickname, locale, and stable user preferences.
- `live state` includes mood / energy if the caller has a current `LiveState`.
- `memory bullets` are A2 prompt context, filtered by MemorySub and budgeted before injection.
- `few-shot examples` are optional and budget-gated.
- `history window` must eventually be token-aware; fixed N history is legacy behavior.
- `current user input` is the final user message.

SafetyPrefix is no longer an always-on or non-disableable guard. It is a policy-controlled optional prompt layer.

## 4. Soul Runtime Contract

`.soul/` is not a finalized runtime requirement. It is downgraded to an undecided source format.

Runtime depends on `SoulRuntimeProfile`, regardless of whether the source is `.soul/`, `.soul.md`, GUI schema, imported package, or another future format:

```rust
struct SoulRuntimeProfile {
    identity_prompt: String,
    style_prompt: String,
    examples: Vec<DialogueExample>,
    initiative_config: InitiativeSoulConfig,
    memory_policy: SoulMemoryPolicy,
    ui_metadata: SoulUiMetadata,
    source_kind: PersonaSourceKind,
    source_hash: String,
}
```

Consumption boundaries:

- `identity_prompt`, `style_prompt`, and `examples` may enter PromptBuilder.
- `initiative_config` is consumed by InitiativeWeights, not directly by PromptBuilder.
- `memory_policy` is consumed by MemorySub, not directly by PromptBuilder.
- `ui_metadata` is consumed by UI surfaces.
- `source_kind` and `source_hash` are for audit, cache invalidation, and snapshot comparison.

Source format rules:

- Source format changes must not change ConversationSub hot path.
- PromptBuilder must never parse source files.
- PersonaSub is the only owner of source parsing / compilation.
- A source format may not grant runtime permissions, tools, scheduler control, or direct memory writes.
- If a future source format has fields resembling `permissions`, `tools`, or `safety_prefix`, PersonaSub must reject or ignore them before producing `SoulRuntimeProfile`.

## 5. Persona Snapshot Contract

Conversation records bind `persona_snapshot_id`, not active persona.

```text
conversation.persona_snapshot_id
  ↓
PersonaSub.read_snapshot(id)
  ↓
SoulRuntimeProfile
  ↓
PromptBuildInput.runtime_profile
```

Rules:

- Active persona affects new conversations and idle / proactive defaults only.
- Existing conversations continue to use their bound snapshot until the user explicitly rebinds or forks.
- Runtime must not silently update `conversations.persona_snapshot_id`.
- Snapshot freezes persona definition only. It does not freeze SafetyPolicy, PermissionService grants, tool policy, nickname, user preferences, mood / energy, or conversation history.

## 6. SafetyPolicy Contract

Latest defaults:

| Scope | Default | Effect when OFF |
|---|---|---|
| `PrefixInjection` | OFF | `wrap_messages` returns messages unchanged |
| `UserInput` | OFF | `scan_user_input` returns Ok / always-pass |
| `StreamToken` | OFF | `scan_token` returns Pass |
| `FinalOutput` | OFF | stream finalization writes `safety_scan_status='disabled'` if no earlier safety hit forces another terminal state |

Hard boundaries:

- Subsystems must not bypass SafetyGuard and implement their own LLM input/output safety path.
- Disabled SafetyPolicy scope means noop, not an alternate path.
- `safety_scan_status='disabled'` is a valid terminal status.
- OS context privacy remains controlled by PermissionService and CI OS API denylist. Turning SafetyPolicy OFF does not grant OS context access.

`scan_token` and `scan_final` priority:

| Condition | Terminal status |
|---|---|
| `StreamToken=OFF`, `FinalOutput=OFF` | `disabled` |
| `StreamToken=OFF`, `FinalOutput=ON`, final clean | `final_ok` |
| `StreamToken=OFF`, `FinalOutput=ON`, final hit | `final_redacted` / `final_blocked` / `scan_failed` |
| `StreamToken=ON`, hard hit | `final_blocked` |
| `StreamToken=ON`, soft hit, `FinalOutput=OFF` | `stream_soft_blocked` |
| `StreamToken=ON`, soft hit, `FinalOutput=ON` | `final_redacted` / `final_blocked` / `final_ok` after final scan |
| `StreamToken=ON`, no hit, `FinalOutput=OFF` | `disabled` |

Implementation notes:

- `ScanTokenResult::SoftBlock` should include `rule_id`, not only placeholder text.
- A hard hit must not reuse the ordinary user-cancel finalization path. It needs a dedicated safety-blocked finalization path that writes fallback content, `mode='online'`, and `safety_scan_status='final_blocked'` coherently.
- `scan_final` should accept `persona_snapshot_id` or an audit context that includes it. Passing only `persona_id` is insufficient once PersonaSnapshot binding is active.

## 7. Prompt Builder Interface

Recommended trait boundary:

```rust
trait PromptBuilder: Send + Sync {
    fn build(&self, input: PromptBuildInput, budget: PromptBudget) -> Result<Vec<ChatMessage>>;
}
```

PromptBuilder owns:

- Material ordering
- Token budget allocation
- Example inclusion / exclusion
- Memory bullet inclusion / exclusion
- App/runtime frame placement

PromptBuilder does not own:

- Persona source parsing
- Snapshot rebinding
- SafetyPolicy decisions
- Permission grants
- Tool execution
- Memory writes

## 8. Error Handling

| Failure | Handling |
|---|---|
| `ConversationRepo.read(conv_id)` fails | Return chat error; do not call LLM |
| `persona_snapshot_id` missing | Migration bug; block conversation and surface repair path |
| `PersonaSub.read_snapshot` fails | Return chat error; do not fallback to active persona |
| `MemorySub.read_prompt_context` fails | Degrade by omitting memory; log error; continue if conversation and snapshot are valid |
| PromptBuilder over budget | Drop examples first, then memory bullets, then shrink history window |
| `SafetyGuard.wrap_messages` fails | Return chat error if PrefixInjection is ON; noop if disabled path cannot fail |
| `scan_token` hard hit | Dedicated `final_blocked` finalization path |
| `scan_final` disabled | Write `disabled` unless earlier stream safety state has higher priority |

## 9. Tests / Verification

Required tests for implementation:

- PromptBuilder does not read active persona.
- PromptBuilder receives `SoulRuntimeProfile`, not source file paths.
- Conversation hot path reads by `persona_snapshot_id`.
- Existing conversation is unaffected by active persona switch.
- `PrefixInjection=OFF` produces no SafetyPrefix.
- `PrefixInjection=ON` inserts SafetyPrefix before app/runtime frame.
- All 4 SafetyPolicy scopes default OFF.
- `scan_token=ON`, `scan_final=OFF`, hard hit ends as `final_blocked`.
- `scan_token=ON`, `scan_final=OFF`, soft hit ends as `stream_soft_blocked`.
- Safety hard hit does not produce `mode='cancelled'`.
- `scan_final` audit context includes `persona_snapshot_id`.

## 10. Superseded / To Update

The following older statements are superseded by this spec:

- `docs/persona/persona-design.md`: "SafetyPrefix is user-invisible and non-disableable" is obsolete. Replace with SafetyPolicy-gated PrefixInjection default OFF.
- `docs/persona/persona-design.md`: "安全规则本体不可控" should be narrowed to "persona source cannot grant tools / permissions / safety prefix control"; SafetyPolicy is user-configurable.
- `docs/architecture/system-architecture.md`: `SecurityGuard` naming is legacy. Use `SafetyGuard + SafetyPolicy`.
- `docs/architecture/system-architecture.md`: "LLM output always scans" is obsolete. Scanning is policy-gated and default OFF.
- `docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md`: residual `7-state` wording is obsolete where not explicitly marked as historical; current FSM is 8-state with `disabled`.
- `docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md`: `.soul/` as a finalized first-class source format is downgraded. Runtime contract depends on `SoulRuntimeProfile`; source format is undecided.
- `docs/superpowers/plans/2026-05-26-safety-policy-configurable-implementation.md`: hard hit must not reuse ordinary cancel path.
- `docs/superpowers/plans/2026-05-26-safety-policy-configurable-implementation.md`: soft block dedupe must use `rule_id`.
- `docs/superpowers/plans/2026-05-26-safety-policy-configurable-implementation.md`: `scan_final` should receive `persona_snapshot_id` or equivalent audit context.

## 11. Next Steps

1. Update the superseded docs listed in §10.
2. Revise the SafetyPolicy implementation plan for terminal-state priority and hard-hit finalization.
3. Create a separate source-format design only when `.soul/` / `.soul.md` / GUI schema decisions are ready.
4. Start A1 only after `SoulRuntimeProfile` and PersonaSnapshot contracts are implemented or explicitly stubbed.
