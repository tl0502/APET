---
title: Persona Runtime Profile PromptBuilder 设计
updated: 2026-06-20
related:
  - 2026-06-18-agent-runtime-contract-design.md
  - 2026-06-19-persona-snapshot-minimal-closure-design.md
  - ../../persona/persona-design.md
---

# Persona Runtime Profile PromptBuilder 设计

## 0. 结论

A2-A 第一刀只做运行时最小闭环：

```text
conversation.persona_snapshot_id
  -> persona_snapshot_profiles.runtime_profile_json
  -> SoulRuntimeProfile
  -> PromptBuilder
  -> SafetyGuard.wrap_messages
  -> LLMProvider
```

Chat 不再从 `PersonaSummary.raw_markdown` / `.soul.md` 解析人格四段来拼 prompt。`.soul.md` 继续作为 source / audit / 兼容存储，但不是正式聊天 hot path 的 prompt 输入。

本切片同时让 `SoulRuntimeProfile.examples` 进入 prompt，解决当前“缺少示例对话” warning 没有运行时意义的问题。

## 1. 目标

- 让人格工坊保存出的 `SoulRuntimeProfile.identity_prompt` 和 `style_prompt` 成为 Chat prompt 的权威人格材料。
- 让 `SoulRuntimeProfile.examples` 在预算允许时作为 few-shot examples 注入 history 之前。
- 保持 A1 已落地的 session persona stability：旧会话继续使用自己的 `persona_snapshot_id`。
- 保持 SafetyGuard 位置不变：PromptBuilder 输出 messages 后，再由 SafetyGuard 注入可选 safety prefix / 执行安全扫描。
- 为后续 memory bullets、live state、试聊沙盒留下清晰接口，但不在本切片实现。

## 2. 非目标

- 不实现 memory bullets。
- 不实现 mood / energy live state 注入。
- 不实现试聊沙盒。
- 不重做 persona-inspector UI。
- 不实现示例对话结构化 speaker-pair 编辑器。
- 不实现 source mode 可编辑。
- 不实现快照历史和恢复 UI。
- 不定义 `.soul/` 或 `.soulpack` 最终格式。

## 3. 当前状态

A1 已完成：

- `personas.active_snapshot_id`
- `persona_snapshot_profiles.runtime_profile_json`
- `conversations.persona_snapshot_id`
- Workshop validate / save / save-and-activate
- Chat 按 conversation snapshot 读取 `PersonaSummary`

仍未完成：

- `chat/prompt.rs::build_messages` 仍调用 `extract_persona_sections(&persona.raw_markdown)`。
- `SoulRuntimeProfile.examples` 已保存但未被 Chat 消费。
- `persona_get_snapshot_profile` 已有 IPC，但 ChatService 内部没有走 profile hot path。

## 4. 数据与接口

### 4.1 SoulRuntimeProfile

沿用 A1 DTO：

```rust
pub struct SoulRuntimeProfile {
    pub identity_prompt: String,
    pub style_prompt: String,
    pub examples: Vec<String>,
    pub initiative_config: serde_json::Value,
    pub memory_policy: serde_json::Value,
    pub ui_metadata: serde_json::Value,
    pub source_kind: String,
    pub source_hash: String,
}
```

A2-A 只消费：

- `identity_prompt`
- `style_prompt`
- `examples`
- `ui_metadata.name` 可作为 persona name fallback

暂不消费：

- `initiative_config`
- `memory_policy`
- `source_kind`
- `source_hash`

### 4.2 PromptBuildInput

新增运行时输入对象：

```rust
pub struct PromptBuildInput<'a> {
    pub runtime_profile: &'a SoulRuntimeProfile,
    pub persona_name: &'a str,
    pub user_nickname: Option<&'a str>,
    pub pet_nickname: &'a str,
    pub history: &'a [MessageRecord],
    pub current_input: &'a str,
}
```

本切片先不引入 trait object。保持函数式边界即可：

```rust
pub fn build_messages_from_profile(input: PromptBuildInput<'_>) -> Result<Vec<ChatMessage>, PromptError>
```

旧的 `build_messages(persona, user_nickname, pet_nickname, history, current_input)` 可以保留给测试或迁移对照，但 ChatService hot path 必须切到 `build_messages_from_profile`。

## 5. Prompt 拼装

顺序：

```text
[system] app/runtime frame
[system] persona identity
[system] persona style
[system] user profile / nickname
[few-shot] examples
[history] history window
[user] current input with re-anchor wrapper
```

### 5.1 App/runtime frame

固定 system 文案：

```text
你是一个 AI 桌面伙伴。你必须保持当前人格快照定义，不要声称拥有未授予的系统权限、工具权限或屏幕/剪贴板读取能力。
```

SafetyPrefix 不在这里写。SafetyPrefix 仍只由 `SafetyGuard.wrap_messages` 在 policy 开启时注入。

### 5.2 Persona identity / style

`identity_prompt` 和 `style_prompt` 进入 system message。A2-A 可以合并成一个 system message，避免过度改变 provider 行为：

```text
你是一个 AI 桌面伙伴。以下是当前人格快照：

# 身份
{identity_prompt}

# 风格与规则
{style_prompt}
```

### 5.3 User profile

沿用旧 nickname 注入逻辑：

- 有 user nickname：加入“用户希望你称他为「{user_nickname}」”
- `pet_nickname != persona_name`：加入“你的人格名是「{persona_name}」，但用户给你起了昵称「{pet_nickname}」”

A2-A 不引入更多 user profile 字段。

### 5.4 Examples

`examples` 用 few-shot message 注入，位置在 history 之前。

每个 example 当前先按自由文本处理：

```text
以下是这个人格的示例对话。它们用于校准语气，不代表当前会话事实：

{example}
```

实现可以先把所有 examples 合并为一个 system message，避免解析 speaker pair。预算规则：

- 最多注入 3 条。
- 单条超过 600 chars 时截断并加 `[truncated]`。
- examples 总长度超过 1200 chars 时停止继续添加。
- 若 examples 为空，直接跳过，不阻塞聊天。

### 5.5 History / current input

保留旧行为：

- history 仍按 `MessageRecord.role` 映射到 `Role::User` / `Role::Assistant` / `Role::System`。
- 未知 role 跳过。
- 当前输入仍用 `wrap_user_input(persona_name, raw_input)` 包装，保留 drift re-anchor。

## 6. ChatService 数据流

当前 ChatService 已做：

```text
active persona
  -> active_snapshot_id
  -> ensure/create conversation
  -> load_persona_for_conversation_with_conn(conversation_id)
  -> build_messages(persona.raw_markdown, user_nickname, history, current_input)
```

A2-A 改为：

```text
active persona
  -> active_snapshot_id
  -> ensure/create conversation
  -> load_persona_for_conversation_with_conn(conversation_id)
  -> parse persona.snapshot_id
  -> get_snapshot_profile_with_conn(snapshot_id)
  -> build_messages_from_profile(PromptBuildInput)
```

`load_persona_for_conversation_with_conn` 仍保留，因为它提供 persona id/name/version/source/snapshot id，且继续验证 conversation snapshot binding。

## 7. 错误处理

| 场景 | 处理 |
|---|---|
| conversation 缺 `persona_snapshot_id` | 保持 A1 行为：返回 repair error，不 fallback active persona |
| snapshot id 解析失败 | 返回 ChatError::Persona |
| `persona_snapshot_profiles` 缺 profile | 返回 ChatError::Persona，不 fallback raw markdown |
| profile JSON 解析失败 | 返回 ChatError::Persona，不调用 LLM |
| identity/style 为空 | PromptBuilder 返回 PromptError，不调用 LLM |
| examples 为空 | 跳过 examples，聊天继续 |
| examples 超预算 | 截断或丢弃超出部分 |

缺 profile 不回退 legacy markdown 是硬要求。否则 A2 上线后仍可能静默走旧路径，用户调 simple slider / examples 时会误判保存无效。

## 8. 测试

Rust tests：

- `build_messages_from_profile` 不需要 raw markdown，缺 markdown section 不影响 profile path。
- `identity_prompt` 和 `style_prompt` 出现在 system message。
- examples 出现在 history 之前。
- examples 为空时不报错。
- examples 超长时按预算截断。
- 当前 user input 仍带 persona re-anchor wrapper。
- ChatService prepare 使用 `get_snapshot_profile_with_conn`，缺 profile 时返回错误。
- 切 active snapshot 后，已有 conversation 继续使用旧 snapshot profile。
- SafetyGuard 注入仍发生在 PromptBuilder 输出之后。

不要求本切片新增 Vue 测试，因为 UI 不变。

## 9. 验收

- 正式聊天 hot path 不再调用 `extract_persona_sections(raw_markdown)`。
- 保存并激活一个带 examples 的人格后，新会话 prompt 会包含 examples。
- 已有会话在 active persona / active snapshot 切换后仍使用原 snapshot profile。
- 删除或缺失 `persona_snapshot_profiles` 行时，聊天明确失败，不静默回退。
- `cargo test --lib services::chat::prompt` 通过。
- `cargo test --lib services::persona` 通过。
- `cargo test --lib services::chat` 或目标 chat prepare tests 通过。

## 10. 后续切片

A2-B：示例对话 UI

- persona-inspector 增加示例对话编辑区域。
- 支持 2-5 条示例，仍可投影到 `# 例对话`。
- 校验从 warning 文案改为更清楚的“影响语气稳定，不影响保存”。

A2-C：PromptBuilder 扩展材料

- memory bullets。
- mood / energy live state。
- token-aware history window。

A2-D：试聊沙盒

- draft / temporary snapshot 试聊。
- 不写正式记忆。
- 不污染正式 conversation list。
