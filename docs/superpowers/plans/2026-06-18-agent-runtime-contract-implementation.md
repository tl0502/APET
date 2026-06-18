# Agent Runtime Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the documentation set consistent with `2026-06-18-agent-runtime-contract-design.md`: SafetyPolicy scopes default OFF, SafetyPrefix is optional, `.soul/` is an undecided source format, and SafetyPolicy implementation planning no longer contains terminal-state contradictions.

**Architecture:** This is a documentation-contract cleanup, not a runtime code change. The new runtime contract spec remains the authority; older persona, architecture, runtime, and SafetyPolicy plan docs are patched to point to it or align their active wording.

**Tech Stack:** Markdown docs, existing `docs/` conventions, PowerShell verification commands, git commits.

---

## File Structure

- Modify: `docs/persona/persona-design.md`
  - Replace old "SafetyPrefix is non-disableable" wording with SafetyPolicy-gated defaults.
  - Keep the ADR-006 text as optional prefix content, not always-on runtime behavior.
- Modify: `docs/architecture/system-architecture.md`
  - Rename active `SecurityGuard` references to `SafetyGuard + SafetyPolicy`.
  - Replace always-scan prompt flow with policy-gated prompt/runtime flow.
- Modify: `docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md`
  - Add a visible supersession note pointing to `2026-06-18-agent-runtime-contract-design.md`.
  - Patch the highest-impact active contradictions: `.soul/` as finalized first-class source format, 7-state wording, "must scan" wording, and release-gate phrasing.
- Modify: `docs/superpowers/plans/2026-05-26-safety-policy-configurable-implementation.md`
  - Revise Task 9 plan text so hard hits use a dedicated safety finalization path.
  - Revise SoftBlock plan text so dedupe uses `rule_id`.
  - Revise final status derivation so `Disabled` does not overwrite earlier stream safety hits.
- Create commits at task boundaries.

Do not modify:

- `docs/superpowers/specs/2026-06-18-agent-runtime-contract-design.md` unless a contradiction is discovered during implementation.
- Source code under `src-tauri/` or `src/`; this plan only aligns docs and implementation plans.
- `CLAUDE.md` or `.claude/`; they are intentionally preserved.

---

### Task 1: Update Persona Design Safety Wording

**Files:**
- Modify: `docs/persona/persona-design.md`

- [ ] **Step 1: Inspect current persona safety section**

Run:

```powershell
Select-String -Path docs/persona/persona-design.md -Pattern '安全前缀|不可禁用|安全规则本体|SecurityGuard' -Context 2,3
```

Expected: output includes old lines around §7.1, §7.3, §7.4, §7.5, and §8.2.

- [ ] **Step 2: Replace §7.1 prompt chain with SafetyPolicy-gated wording**

Replace the current §7.1 chain:

```markdown
每次调用 LLM 时拼装链路为:

```
[安全前缀(系统注入,用户不可见、不可禁用,版本 v1.0)]
[当前人格的 system prompt(由 .soul.md 渲染而成)]
[用户记忆(key-value 简表,含 username)]
[最近 N 轮对话历史]
[本轮用户输入]
```
```

with:

```markdown
每次调用 LLM 时按最新 Agent Runtime contract 拼装 prompt。`SafetyPrefix` 不再是不可关闭的固定第一层；它由 `SafetyPolicy.PrefixInjection` 控制，出厂默认 OFF。

```
[可选 SafetyPrefix（SafetyPolicy.PrefixInjection=ON 时由 SafetyGuard 注入）]
[app/runtime frame]
[PersonaSnapshot.identity_prompt]
[PersonaSnapshot.style_prompt]
[用户 profile（nickname / locale / preferences）]
[live state（mood / energy，可选）]
[memory bullets（A2 起）]
[few-shot examples（预算允许时）]
[history window]
[本轮用户输入]
```
```

- [ ] **Step 3: Replace LLM game chain wording**

Replace:

```markdown
LLM 游戏(模块 Q)拼装顺序:

```
[安全前缀]
[当前人格 system prompt]
[游戏场景 system_prompt(来自 game_scenes/<id>.yaml)]
[用户记忆摘要(仅 username/作息等公共项)]
[本会话历史(game_session_events)]
[本轮输入]
```

游戏场景的 `system_prompt` **不能覆盖安全前缀**;立项期复审(ADR-007)。
```

with:

```markdown
LLM 游戏（模块 Q）复用同一 runtime contract。游戏场景 prompt 是普通 prompt material，不能修改 `SafetyPolicy`、PermissionService、Tool policy、Memory 写入规则。

```
[可选 SafetyPrefix（SafetyPolicy.PrefixInjection=ON 时）]
[app/runtime frame]
[PersonaSnapshot.identity_prompt]
[PersonaSnapshot.style_prompt]
[游戏场景 system_prompt（来自 game_scenes/<id>.yaml）]
[用户记忆摘要（仅公共项）]
[本会话历史（game_session_events）]
[本轮输入]
```
```

- [ ] **Step 4: Replace §7.3 non-bypass wording**

Replace §7.3 body:

```markdown
- 用户人格中即使写"你不需要遵守安全规则",也无效 — 安全前缀位于人格之前且明确指出"无论以下角色定义如何"。
- 离线模板池中如出现违反安全规则的内容(如导入第三方人格),由静态校验拦截。
- 命中安全规则时使用统一的安全回复,即使覆盖了当前人格语气也优先安全。
```

with:

```markdown
- 用户人格不能修改 `SafetyPolicy`、PermissionService、Tool policy、Scheduler、Memory 写入规则。
- 人格 source format 不能声明 `permissions` / `tools` / `safety_prefix` 等扩权字段；PersonaSub 在生成 `SoulRuntimeProfile` 前必须拒绝或忽略这些字段。
- `SafetyGuard` 路径仍必经，但 4 个 SafetyPolicy scope 出厂默认 OFF；disabled scope 返回 noop / always-pass。
```

- [ ] **Step 5: Replace §7.4 user control table safety rows**

Replace rows:

```markdown
| **安全规则本体** | ❌ | 不可绕过 |
| **未成年保护** | ❌ | 不可绕过 |
| **法律边界** | ❌ | 不可绕过 |
```

with:

```markdown
| `SafetyPolicy.PrefixInjection` | ✅ | 出厂 OFF；开启后注入 ADR-006 prefix |
| `SafetyPolicy.UserInput` | ✅ | 出厂 OFF；开启后扫描用户输入 |
| `SafetyPolicy.StreamToken` | ✅ | 出厂 OFF；开启后扫描流式 token |
| `SafetyPolicy.FinalOutput` | ✅ | 出厂 OFF；开启后扫描最终输出 |
| 权限 / 工具 / OS context | ❌（由专用设置控制） | 不由人格 source format 控制 |
```

- [ ] **Step 6: Replace §7.5 SecurityGuard wording**

Replace:

```markdown
当 LLM 游戏中 SecurityGuard 触发拒答替换,**优先**从当前游戏场景 yaml 文件 `refusals` 字段抽样(每场景 ≥ 3 条),其次降级到当前人格的 `## 拒答 / Refusal` 池,最末全局兜底。
```

with:

```markdown
当 `SafetyPolicy` 对应扫描 scope 开启且 `SafetyGuard` 触发拒答替换时，**优先**从当前游戏场景 yaml 文件 `refusals` 字段抽样（每场景 ≥ 3 条），其次降级到当前人格的 `## 拒答 / Refusal` 池，最末全局兜底。
```

- [ ] **Step 7: Replace §8.2 prompt order summary**

Replace:

```markdown
正常对话:`[安全前缀] [人格] [记忆摘要(含 username)] [对话历史] [本轮输入]`

LLM 游戏:`[安全前缀] [人格] [游戏场景 system_prompt] [用户记忆摘要(仅公共项)] [游戏会话历史] [本轮输入]`
```

with:

```markdown
正常对话：`[可选 SafetyPrefix] [app/runtime frame] [PersonaSnapshot identity/style] [user profile] [live state] [memory bullets] [examples] [history window] [本轮输入]`

LLM 游戏：`[可选 SafetyPrefix] [app/runtime frame] [PersonaSnapshot identity/style] [game scene prompt] [公共记忆摘要] [game history] [本轮输入]`
```

- [ ] **Step 8: Verify persona doc no longer carries active obsolete wording**

Run:

```powershell
Select-String -Path docs/persona/persona-design.md -Pattern '不可禁用|安全规则本体|SecurityGuard|安全前缀始终|不能覆盖安全前缀'
```

Expected: no matches.

- [ ] **Step 9: Commit persona doc update**

Run:

```powershell
git add -- docs/persona/persona-design.md
git commit -m "docs: align persona prompt safety contract"
```

Expected: commit succeeds with only `docs/persona/persona-design.md`.

---

### Task 2: Update System Architecture SafetyGuard Wording

**Files:**
- Modify: `docs/architecture/system-architecture.md`

- [ ] **Step 1: Inspect current architecture references**

Run:

```powershell
Select-String -Path docs/architecture/system-architecture.md -Pattern 'SecurityGuard|二次扫描|实时扫描|安全前缀注入|安全前缀始终' -Context 2,3
```

Expected: output includes the service map, §8.2, game flow, test target, and milestone rows.

- [ ] **Step 2: Replace service table row**

Replace:

```markdown
| **SecurityGuard**(主进程,占位) | 安全前缀注入、内容过滤(M1 占位,M3 G 真注入,ADR-006) | 内部,被 ChatService / LLMGameRunner 调用 | M3 G |
```

with:

```markdown
| **SafetyGuard + SafetyPolicy**(主进程,kernel) | `SafetyGuard` 是 LLM 输入/输出必经路径；`SafetyPolicy` 控制 PrefixInjection / UserInput / StreamToken / FinalOutput 4 scope，出厂全 OFF | 内部,被 ChatService / LLMGameRunner 调用 | Phase A0+ |
```

- [ ] **Step 3: Replace dependency flow block**

Replace:

```markdown
ChatService → SecurityGuard + PersonaService + MemoryService + NicknameService → LLMProvider
LLMGameRunner → SecurityGuard + PersonaService + LLMProvider(prompt 含 game_scenes/<id>.yaml)
```

with:

```markdown
ChatService / ConversationSub → PersonaSnapshot + Memory prompt context → PromptBuilder → SafetyGuard.wrap_messages(policy-gated) → LLMProvider
LLMGameRunner → same runtime contract + game_scenes/<id>.yaml prompt material → SafetyGuard.wrap_messages(policy-gated) → LLMProvider
```

- [ ] **Step 4: Replace §8.2 heading and body**

Replace heading:

```markdown
### 8.2 安全前缀注入(SecurityGuard)
```

with:

```markdown
### 8.2 Agent Runtime prompt contract（SafetyGuard + SafetyPolicy）
```

Replace the §8.2 prompt-order paragraphs through the refusal bullets with:

```markdown
最新 prompt / runtime contract 以 [Agent Runtime Contract Design](../superpowers/specs/2026-06-18-agent-runtime-contract-design.md) 为准。

正常对话拼装顺序：

```
[可选 SafetyPrefix（SafetyPolicy.PrefixInjection=ON 时）]
[app/runtime frame]
[PersonaSnapshot.identity_prompt]
[PersonaSnapshot.style_prompt]
[user profile]
[live state]
[memory bullets]
[few-shot examples]
[history window]
[本轮用户输入]
```

LLM 游戏拼装顺序：

```
[可选 SafetyPrefix（SafetyPolicy.PrefixInjection=ON 时）]
[app/runtime frame]
[PersonaSnapshot.identity_prompt]
[PersonaSnapshot.style_prompt]
[game scene system_prompt]
[公共记忆摘要]
[game session history]
[本轮输入]
```

`SafetyPrefix` 不再是 always-on 层。`SafetyPolicy` 4 scope 出厂默认 OFF；开启对应 scope 后，`SafetyGuard` 才执行 prefix 注入或输入/输出扫描。游戏场景 prompt 不能修改 SafetyPolicy、PermissionService、Tool policy 或 Memory 写入规则。

命中扫描规则时：

- 正常对话 → 由 `SafetyGuard` 产生替换内容，并通过 `StreamEvent::ReplaceMessage` 覆盖 UI。
- LLM 游戏 → 优先用 `game_scenes/<id>.yaml.refusals`（每场景 ≥ 3 条人格化拒答），否则降级到人格 refusal 池，最末全局兜底。
```

- [ ] **Step 5: Replace LLM game flow scan wording**

Replace:

```markdown
 ├── 流式输出 → SecurityGuard 实时扫描
 │    ├── 命中违禁 → 替换为 game_scenes/story_relay.yaml.refusals 抽样(人格化拒答)
 │    └── 通过 → 输出
```

with:

```markdown
 ├── 流式输出 → SafetyGuard scan path（SafetyPolicy.StreamToken / FinalOutput 开启时）
 │    ├── 命中规则 → 替换为 game_scenes/story_relay.yaml.refusals 抽样（人格化拒答）
 │    └── 未启用或通过 → 输出
```

- [ ] **Step 6: Replace test target and milestone references**

Replace:

```markdown
核心服务(Persona / Chat / Task / SecurityGuard)≥ 70%
```

with:

```markdown
核心服务(Persona / Chat / Task / SafetyGuard / SafetyPolicy)≥ 70%
```

Replace milestone row fragment:

```markdown
LLM Provider(OpenAI 兼容)、SecurityGuard、MigrationService
```

with:

```markdown
LLM Provider(OpenAI 兼容)、SafetyGuard + SafetyPolicy、MigrationService
```

- [ ] **Step 7: Verify architecture doc no longer uses active SecurityGuard wording**

Run:

```powershell
Select-String -Path docs/architecture/system-architecture.md -Pattern 'SecurityGuard|二次扫描|实时扫描|安全前缀始终'
```

Expected: no matches.

- [ ] **Step 8: Commit architecture doc update**

Run:

```powershell
git add -- docs/architecture/system-architecture.md
git commit -m "docs: align architecture with runtime contract"
```

Expected: commit succeeds with only `docs/architecture/system-architecture.md`.

---

### Task 3: Patch Runtime v3 Spec Superseded Wording

**Files:**
- Modify: `docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md`

- [ ] **Step 1: Add supersession note after the title**

Insert after:

```markdown
# Companion Agent Runtime v3 — Revised MVP Architecture Spec
```

this block:

```markdown
> **Supersession note (2026-06-18)**: Agent Runtime hot path, prompt material contract, SafetyPolicy defaults, and Persona source-format boundaries are superseded by [2026-06-18-agent-runtime-contract-design.md](2026-06-18-agent-runtime-contract-design.md). This v3 spec remains useful for historical context and broader subsystem mapping, but active implementation must follow the 2026-06-18 runtime contract when conflicts exist.
```

- [ ] **Step 2: Replace v2-to-v3 Soul terminology row**

Replace row:

```markdown
| 1 | Soul 术语统一 | `.soul.md` / `.soul/` / `.soulpack` 残留混用 | **`.soul/` 一等格式** / `.soulpack` 分发 / `.soul.md` legacy 输入;§2.6.1 Soul Package Terminology |
```

with:

```markdown
| 1 | Soul 术语统一 | `.soul.md` / `.soul/` / `.soulpack` 残留混用 | **Superseded 2026-06-18**: runtime only depends on `SoulRuntimeProfile`; `.soul/` is an undecided source format, not an active runtime requirement |
```

- [ ] **Step 3: Replace project philosophy Soul source statement**

Replace:

```markdown
1. **用户自主人格**: 角色定义采用 **`.soul/` 多文件包**作为底层一等格式 (manifest+identity+style+initiative+memory+examples);用户可分享的 `.soulpack` 是 zip 分发格式;`.soul.md` 是 legacy / simple 模式输入, 由 SoulCompiler 转换为默认 `.soul/` 布局。参考 OpenClaw 开源项目验证可行。用户可编辑、可分享、可从零创建。详见 §2.6 与 ADR-028。
```

with:

```markdown
1. **用户自主人格**: 运行时只依赖 `SoulRuntimeProfile` / `PersonaSnapshot`；`.soul/`、`.soul.md`、GUI schema、imported package 都只是候选 source format。`.soul/` 不再是已拍板的一等格式，后续需独立 source-format spec 决定。
```

- [ ] **Step 4: Replace §2.6 source package opening**

Replace:

```markdown
.soul/ package (源, 用户编辑) — 一等格式
├── manifest.toml           ← schema / id / name / version / author
├── identity.md             ← LLM 用 system prompt 来源
├── style.toml              ← 语气 / 口头禅 / 表情 token (ToneShaper 用)
├── initiative.toml         ← proactivity / quiet hours 默认 (InitiativeWeights 用)
├── memory.toml             ← KV preference / scope 偏好 (MemoryPolicy 用)
├── examples.md             ← few-shot 对话样本
└── (P1) assets/ voice/ outfits/ games/
```

with:

```markdown
persona source format (未决; examples: .soul/ / .soul.md / GUI schema / imported package)
          ↓ PersonaSub-owned parser / compiler
```

Keep the later `SoulRuntimeProfile` and `PersonaSnapshot` diagram lines, because the runtime contract still depends on those outputs.

- [ ] **Step 5: Replace §2.6.1 terminology table**

Replace the `.soul/`, `.soulpack`, `.soul.md`, and `SoulPackage` rows with:

```markdown
| **Persona source format** | 未决源格式；可能是 `.soul/`、`.soul.md`、GUI schema、imported package 或其他格式 | 未决 | PersonaSub 解析 / 编译 |
| **`.soul/` 目录** | 候选源格式之一；不再作为 2026-06-18 后的运行时前提 | Candidate | 后续 source-format spec 再定 |
| **`.soul.md` 单文件** | 候选源格式之一；可作为 simple / legacy 输入保留 | Candidate | 后续 source-format spec 再定 |
| **Imported package** | 候选分发 / 导入格式；是否继续使用 `.soulpack` 后续再定 | Candidate | 后续 source-format spec 再定 |
| **SoulRuntimeProfile** | Source format 编译后的运行时 profile，PromptBuilder / ToneShaper / InitiativeWeights / MemorySub 消费它 | Active runtime contract | PersonaSub 产 / runtime 消费 |
```

- [ ] **Step 6: Replace 7-state active wording in core rows**

Replace these active phrases wherever they describe current state:

```text
SafetyGuard 7-state FSM
safety_scan_status 7-state 枚举
7-state FSM 单测覆盖
```

with:

```text
SafetyGuard 8-state FSM（含 disabled）
safety_scan_status 8-state 枚举
8-state FSM 单测覆盖
```

Run this verification afterward:

```powershell
Select-String -Path docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md -Pattern '7-state|7 状态|7-state FSM'
```

Expected: matches only in historical "Updated 2026-05-26: 7-state -> 8-state" descriptions, not active implementation rows.

- [ ] **Step 7: Replace must-scan active wording**

Replace:

```markdown
1. **streaming → final_***: stream finish (`FinishReason::Stop / Length / ContentFilter / Error / Unknown`) 触发 scan_final;**不论 reason 必扫**
```

with:

```markdown
1. **streaming → final_***: stream finish (`FinishReason::Stop / Length / ContentFilter / Error / Unknown`) enters the `SafetyGuard.scan_final` path; if `SafetyPolicy.FinalOutput` is OFF, the path returns noop / always-pass and final status follows the 2026-06-18 priority table.
```

Replace:

```markdown
4. SafetyGuard.scan_final(summary) 必扫 (Constitution #1)
```

with:

```markdown
4. SafetyGuard.scan_final(summary) path is required; actual scan is SafetyPolicy-gated.
```

Replace:

```markdown
SafetyGuard.scan_final 必扫 user input + SoulValidator 拒绝 manifest 出现 permissions/tools 字段
```

with:

```markdown
SafetyGuard scan path is SafetyPolicy-gated; PersonaSub still rejects source fields that try to grant permissions/tools/safety_prefix control.
```

- [ ] **Step 8: Replace Phase A0 release-gate wording**

Replace:

```markdown
**Phase A0 是任何对外分发版本前的 hard gate** — 即使 A1/A2 推迟, A0 也必须先落 (DPAPI secrets / safety prefix / 7-state FSM 是产品发布的 P0 条件)。
```

with:

```markdown
**Phase A0 是任何对外分发版本前的 hard gate** — 即使 A1/A2 推迟, A0 也必须先落（DPAPI secrets / SafetyGuard path completeness / SafetyPolicy default OFF + configurable scopes / 8-state FSM 是产品发布的 P0 条件）。
```

- [ ] **Step 9: Verify runtime spec active contradictions are cleared**

Run:

```powershell
Select-String -Path docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md -Pattern '必扫|`.soul/` 一等|底层一等格式|safety_scan_status 7-state|SafetyGuard 7-state FSM'
```

Expected: no active contradiction matches. Historical or supersession-note matches are acceptable only if they explicitly say `Superseded` or `Updated 2026-05-26`.

- [ ] **Step 10: Commit runtime spec update**

Run:

```powershell
git add -- docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md
git commit -m "docs: supersede runtime v3 prompt contract drift"
```

Expected: commit succeeds with only the runtime v3 spec.

---

### Task 4: Revise SafetyPolicy Implementation Plan Contradictions

**Files:**
- Modify: `docs/superpowers/plans/2026-05-26-safety-policy-configurable-implementation.md`

- [ ] **Step 1: Add a supersession note after the plan title**

Insert after:

```markdown
# SafetyPolicy 可配置化 Implementation Plan
```

this block:

```markdown
> **Revision note (2026-06-18)**: Terminal-state priority, hard-hit finalization, SoftBlock dedupe, and `scan_final` audit context are revised by [../specs/2026-06-18-agent-runtime-contract-design.md](../specs/2026-06-18-agent-runtime-contract-design.md). Execute the revised Task 9 wording in this file, not the original cancel-path approach.
```

- [ ] **Step 2: Replace disabled-state description**

Replace:

```markdown
| **`disabled`** (新, Updated 2026-05-26) | **scan_final OFF**, ChatService 流末显式写入 | LLM 原文 |
```

with:

```markdown
| **`disabled`** (Updated 2026-06-18) | `scan_final OFF` 且没有更高优先级的 stream safety hit | LLM 原文 |
```

- [ ] **Step 3: Replace cross-scope table**

Replace the `scan_token × scan_final` table with:

```markdown
| `scan_token` | `scan_final` | 流末状态写入 |
|---|---|---|
| OFF | OFF | `disabled` |
| OFF | ON | `final_ok` / `final_redacted` / `final_blocked` / `scan_failed` |
| ON | ON | hard hit → `final_blocked`; soft hit → final scan 后决定 `final_ok` / `final_redacted` / `final_blocked`; scan failure → `scan_failed` |
| ON | OFF | hard hit → `final_blocked`; soft hit → `stream_soft_blocked`; no hit → `disabled` |
```

- [ ] **Step 4: Replace Scan Scope Matrix hard-hit wording**

Replace:

```markdown
hard hit → 强制 finish + scan_final
```

with:

```markdown
hard hit → dedicated safety-blocked finalization path, write `final_blocked`
```

- [ ] **Step 5: Replace Task 9.3 final status derivation snippet**

Replace the original final-status derivation block:

```rust
let final_status = if !self.safety_guard.is_enabled(SafetyScope::FinalOutput) {
    SafetyScanStatus::Disabled
} else {
    match &scan {
        ScanFinalResult::Ok => SafetyScanStatus::FinalOk,
        ScanFinalResult::Redacted { .. } => SafetyScanStatus::FinalRedacted,
        ScanFinalResult::Blocked { .. } => SafetyScanStatus::FinalBlocked,
        ScanFinalResult::ScanFailed { .. } => SafetyScanStatus::ScanFailed,
    }
};
```

with:

```rust
let prior_stream_status = stream_safety_state.lock().terminal_hint();
let final_status = match prior_stream_status {
    Some(SafetyScanStatus::FinalBlocked) => SafetyScanStatus::FinalBlocked,
    Some(SafetyScanStatus::StreamSoftBlocked)
        if !self.safety_guard.is_enabled(SafetyScope::FinalOutput) =>
    {
        SafetyScanStatus::StreamSoftBlocked
    }
    _ if !self.safety_guard.is_enabled(SafetyScope::FinalOutput) => {
        SafetyScanStatus::Disabled
    }
    _ => match &scan {
        ScanFinalResult::Ok => SafetyScanStatus::FinalOk,
        ScanFinalResult::Redacted { .. } => SafetyScanStatus::FinalRedacted,
        ScanFinalResult::Blocked { .. } => SafetyScanStatus::FinalBlocked,
        ScanFinalResult::ScanFailed { .. } => SafetyScanStatus::ScanFailed,
    },
};
```

Also add this explanatory paragraph immediately below the snippet:

```markdown
`Disabled` is only the terminal status when no earlier stream safety hit has higher priority. A hard hit always wins as `final_blocked`; a soft hit with `FinalOutput=OFF` remains `stream_soft_blocked`.
```

- [ ] **Step 6: Replace scan_final audit context snippet**

Replace:

```rust
self.safety_guard.scan_final(&collected, &persona.id)
```

with:

```rust
self.safety_guard.scan_final(&collected, &persona_snapshot_id)
```

Add this paragraph:

```markdown
`persona_snapshot_id` is required for audit and for Session Persona Stability. Passing only `persona.id` loses the stable conversation binding.
```

- [ ] **Step 7: Replace SoftBlock result snippet**

Replace:

```rust
crate::kernel::safety_guard::ScanTokenResult::SoftBlock {
    replace_last_n,
    placeholder,
} => {
    // rule_id dedupe (用 placeholder 串作 dedupe key 因为 SoftBlock 不带 rule_id)
```

with:

```rust
crate::kernel::safety_guard::ScanTokenResult::SoftBlock {
    rule_id,
    replace_last_n,
    placeholder,
} => {
    // rule_id dedupe; placeholder text is display content, not identity
```

Replace:

```rust
if seen.contains(&placeholder) {
    return; // 已 SoftBlock 过, 跳过避免震荡
}
seen.insert(placeholder.clone());
```

with:

```rust
if seen.contains(&rule_id) {
    return; // 已 SoftBlock 过, 跳过避免震荡
}
seen.insert(rule_id.clone());
stream_safety_state.lock().mark_soft_blocked(rule_id.clone());
```

- [ ] **Step 8: Replace HardEnd cancel-path snippet**

Replace the original hard-hit block:

```rust
// Hard hit → cancel stream + 标记 final_blocked
cancel_token_for_cb.cancel();
// 实际的 DB 写在 Cancelled 分支收尾时统一处理
// (run_stream Err(Cancelled) 路径会 update 'cancelled' mode,
//  此处仅借 cancel_token 触发它)
// 但 safety_scan_status 要写 FinalBlocked (而非 'cancelled' 的 cancelled 状态),
// 用 spawn 异步写避免 closure 阻塞:
```

with:

```rust
// Hard hit → dedicated safety-blocked finalization.
// Do not reuse user-cancel finalization: that path writes mode='cancelled'
// and races with safety_scan_status='final_blocked'.
stream_safety_state.lock().mark_hard_blocked(rule_id.clone());
cancel_token_for_cb.cancel_for_safety();
```

Add this paragraph below the snippet:

```markdown
Execution must distinguish user cancellation from safety hard stop. If the current `CancellationToken` cannot encode a reason, introduce a small per-stream `StreamStopReason` state owned by ChatService and read it during finalization.
```

- [ ] **Step 9: Add stream safety state helper to the plan**

Insert before Task 9.6:

```markdown
Before wiring `on_delta`, define per-message stream safety state:

```rust
#[derive(Debug, Default)]
struct StreamSafetyState {
    hard_blocked_rule: Option<String>,
    soft_blocked_rules: std::collections::HashSet<String>,
}

impl StreamSafetyState {
    fn mark_soft_blocked(&mut self, rule_id: String) {
        self.soft_blocked_rules.insert(rule_id);
    }

    fn mark_hard_blocked(&mut self, rule_id: String) {
        self.hard_blocked_rule = Some(rule_id);
    }

    fn terminal_hint(&self) -> Option<SafetyScanStatus> {
        if self.hard_blocked_rule.is_some() {
            Some(SafetyScanStatus::FinalBlocked)
        } else if !self.soft_blocked_rules.is_empty() {
            Some(SafetyScanStatus::StreamSoftBlocked)
        } else {
            None
        }
    }
}
```
```

- [ ] **Step 10: Verify old contradiction phrases are gone from SafetyPolicy plan**

Run:

```powershell
Select-String -Path docs/superpowers/plans/2026-05-26-safety-policy-configurable-implementation.md -Pattern 'SoftBlock 不带 rule_id|实际的 DB 写在 Cancelled 分支|scan_final policy off → 直接写|&persona.id'
```

Expected: no matches.

- [ ] **Step 11: Commit SafetyPolicy plan revision**

Run:

```powershell
git add -- docs/superpowers/plans/2026-05-26-safety-policy-configurable-implementation.md
git commit -m "docs: revise safety policy implementation contract"
```

Expected: commit succeeds with only the SafetyPolicy implementation plan.

---

### Task 5: Cross-Document Verification

**Files:**
- Read: `docs/persona/persona-design.md`
- Read: `docs/architecture/system-architecture.md`
- Read: `docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md`
- Read: `docs/superpowers/plans/2026-05-26-safety-policy-configurable-implementation.md`
- Read: `docs/superpowers/specs/2026-06-18-agent-runtime-contract-design.md`

- [ ] **Step 1: Verify active old SecurityGuard naming is cleared**

Run:

```powershell
Select-String -Path docs/persona/persona-design.md,docs/architecture/system-architecture.md -Pattern 'SecurityGuard'
```

Expected: no matches.

- [ ] **Step 2: Verify non-disableable SafetyPrefix wording is cleared**

Run:

```powershell
Select-String -Path docs/persona/persona-design.md,docs/architecture/system-architecture.md,docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md -Pattern '不可禁用|不可关闭|always-on|永远第一位'
```

Expected: no active matches. Historical matches are acceptable only when the same line says `obsolete`, `Superseded`, or `Updated 2026-05-26`.

- [ ] **Step 3: Verify `.soul/` active-first-class wording is cleared**

Run:

```powershell
Select-String -Path docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md -Pattern '`.soul/` 一等|底层一等格式|作为底层一等格式|一等格式'
```

Expected: no active matches. A supersession note is acceptable only if it says `.soul/` is undecided or candidate.

- [ ] **Step 4: Verify runtime contract links exist**

Run:

```powershell
Select-String -Path docs/persona/persona-design.md,docs/architecture/system-architecture.md,docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md,docs/superpowers/plans/2026-05-26-safety-policy-configurable-implementation.md -Pattern '2026-06-18-agent-runtime-contract-design.md'
```

Expected: at least 3 matches: architecture doc, runtime v3 spec, SafetyPolicy plan. Persona doc may reference the concept without a direct link; direct link is preferred if added.

- [ ] **Step 5: Verify git status only contains unrelated pre-existing files or is clean**

Run:

```powershell
git status --short
```

Expected: no modified files from Tasks 1-4. Pre-existing unrelated files may remain, such as `AGENTS.md`, `docs/agent-memory.md`, or earlier workflow docs.

- [ ] **Step 6: Record verification outcome**

Append this short note to the final session summary, not to `STATUS.md` unless the user asks for sync-status:

```markdown
Verification:
- persona safety wording aligned
- architecture SafetyGuard/SafetyPolicy wording aligned
- runtime v3 spec superseded where it conflicted with 2026-06-18 contract
- SafetyPolicy implementation plan revised for hard-hit finalization, SoftBlock rule_id, and terminal-state priority
```

---

## Self-Review

### 1. Spec Coverage

| Contract spec requirement | Plan coverage |
|---|---|
| SafetyPrefix optional, PrefixInjection default OFF | Task 1, Task 2, Task 3 |
| SafetyGuard path mandatory but disabled scopes noop | Task 1, Task 2, Task 4 |
| `.soul/` downgraded to undecided source format | Task 3 |
| Runtime depends on `SoulRuntimeProfile` / `PersonaSnapshot` | Task 1, Task 2, Task 3 |
| Old persona-design wording must be fixed | Task 1 |
| Old architecture `SecurityGuard` wording must be fixed | Task 2 |
| Runtime v3 spec residual `7-state`, must-scan, `.soul/` contradictions must be fixed | Task 3 |
| SafetyPolicy implementation plan hard-hit cancel race must be fixed | Task 4 |
| SoftBlock dedupe must use `rule_id` | Task 4 |
| `scan_final` audit context should use `persona_snapshot_id` | Task 4 |
| Cross-document verification | Task 5 |

### 2. Placeholder Scan

This plan contains no placeholder markers, vague validation instructions, or unspecified test-writing steps. Each task names exact files, replacement snippets, verification commands, expected outputs, and commit commands.

### 3. Type / Term Consistency

- The plan consistently uses `SafetyGuard + SafetyPolicy`, not active `SecurityGuard`.
- The plan consistently treats `.soul/` as candidate / undecided source format.
- The plan consistently names `SoulRuntimeProfile`, `PersonaSnapshot`, `persona_snapshot_id`, and `SafetyScanStatus`.
- The plan consistently treats `Disabled` as a terminal status only when no earlier stream safety hit has higher priority.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-18-agent-runtime-contract-implementation.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
