---
title: Persona Snapshot Minimal Closure 设计
updated: 2026-06-19
related:
  - 2026-06-18-persona-workshop-design.md
  - 2026-06-18-agent-runtime-contract-design.md
  - 2026-05-24-companion-agent-runtime-design.md
---

# Persona Snapshot Minimal Closure 设计

## 0. 结论

A1 第一刀做最小可用闭环：

```text
Persona Workshop draft
  -> validate
  -> compile
  -> persona_snapshots.content
  -> persona_snapshot_profiles.runtime_profile_json
  -> activate snapshot
  -> new conversation binds persona_snapshot_id
```

本阶段不拍板 `.soul/`、`.soulpack` 或人格市场。source format 继续作为候选输入；运行时边界先落到 `PersonaSnapshot` 和 `SoulRuntimeProfile` 的存储与绑定。

## 1. 目标

产品目标：

- 用户在 Persona Workshop 修改人格后，可以保存为新快照。
- 用户可以保存并激活该快照。
- 新会话使用激活快照。
- 旧会话保持原绑定，不被静默重绑。

技术目标：

- `persona_snapshots.content` 继续存 legacy source text，便于兼容当前 prompt parser。
- 新增 `persona_snapshot_profiles` 存编译后的 `SoulRuntimeProfile` JSON，为后续 PromptBuilder 迁移做准备。
- `conversations` 新增 `persona_snapshot_id`，建立 session persona stability。
- Chat A1 仍可从 snapshot content 构建 prompt，但不得为已有会话重新读取 active persona。

非目标：

- 不实现 `.soul/` 多文件格式。
- 不实现 `.soulpack` 导入导出。
- 不实现人格市场。
- 不做完整 PromptBuilder runtime-profile 消费迁移。
- 不实现快照历史 UI 和恢复 UI。

## 2. 当前状态

现有后端：

- `personas` 存人格元信息和 `is_active`。
- `persona_snapshots` 存 `(persona_id, version, content, created_at)`，其中 `content` 是去 frontmatter 后的 markdown 正文。
- `PersonaRepo` 仍是 A0 stub。
- Chat prepare 当前先读 active persona，再按 active persona 创建或校验 conversation。
- `conversations` 当前只有 `persona_id`，没有稳定的 `persona_snapshot_id`。

现有前端：

- Persona Workshop 已有角色卡舞台和 Inspector Drawer。
- `PersonaSourceDraft` 能从现有人格 raw markdown 生成并 validate。
- “验证 / 试聊 / 保存快照”按钮仍是 disabled。

## 3. 数据模型

### 3.1 personas

新增字段：

```sql
ALTER TABLE personas ADD COLUMN active_snapshot_id INTEGER;
```

语义：

- `is_active=1` 表示当前 active persona。
- `active_snapshot_id` 表示该 persona 当前激活的快照。
- 首次 migration 后，每个 persona 的 `active_snapshot_id` 回填为该 persona 当前 version 对应的最新 snapshot id。
- 激活一个 snapshot 时，必须同时保证 `personas.is_active` 只有该 snapshot 所属 persona 为 1。

### 3.2 persona_snapshots

保留现有表和唯一索引：

```sql
persona_snapshots(id INTEGER PRIMARY KEY AUTOINCREMENT)
persona_snapshots(persona_id, version) UNIQUE
```

A1 保存用户草稿时不复用 `(persona_id, version)`。为了允许同一 persona 多次保存，保存逻辑需要生成递增 patch version，例如：

```text
1.0.0 -> 1.0.1 -> 1.0.2
```

如果用户 draft 已带更高版本，则使用用户版本；如果与现有 `(persona_id, version)` 冲突，则自动 bump patch version。

### 3.3 persona_snapshot_profiles

新增表：

```sql
CREATE TABLE persona_snapshot_profiles (
  snapshot_id INTEGER PRIMARY KEY,
  persona_id TEXT NOT NULL,
  runtime_profile_json TEXT NOT NULL,
  source_hash TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (snapshot_id) REFERENCES persona_snapshots(id) ON DELETE CASCADE,
  FOREIGN KEY (persona_id) REFERENCES personas(id) ON DELETE CASCADE
);

CREATE INDEX idx_persona_snapshot_profiles_persona
  ON persona_snapshot_profiles(persona_id, created_at DESC);
```

`runtime_profile_json` 是 A1 的 `SoulRuntimeProfile` 序列化结果：

```json
{
  "identity_prompt": "...",
  "style_prompt": "...",
  "examples": [],
  "initiative_config": {
    "mode": "default"
  },
  "memory_policy": {
    "mode": "default"
  },
  "ui_metadata": {
    "name": "默默",
    "source": "user",
    "version": "1.0.1"
  },
  "source_kind": "legacy_soul_md",
  "source_hash": "sha256:..."
}
```

### 3.4 conversations

新增字段：

```sql
ALTER TABLE conversations ADD COLUMN persona_snapshot_id INTEGER;
CREATE INDEX idx_conversations_persona_snapshot
  ON conversations(persona_snapshot_id);
```

Migration 回填：

- 对每条 conversation，按 `conversation.persona_id` 找最新 `persona_snapshots.id`。
- 找不到 snapshot 的 conversation 保留 NULL，但 ChatService 读取时必须返回修复错误，不回退 active persona。

A1 之后：

- 新建 conversation 必须写入 `persona_snapshot_id`。
- 已有 conversation 继续使用自己的 `persona_snapshot_id`。
- 切 active persona / active snapshot 不更新已有 conversation。

## 4. Compiler

A1 compiler 是 legacy compiler，不定义最终 source package。

输入：

- `PersonaSourceDraft`，由前端 Inspector 当前 draft 发送。

输出：

- `source_text`: 可存入 `persona_snapshots.content` 的 markdown body。
- `runtime_profile`: `SoulRuntimeProfile` JSON。
- `diagnostics`: blocking errors / warnings。
- `source_hash`: `sha256(source_text)`。

编译规则：

- `identity_prompt` 来自 Structured 身份。
- `style_prompt` 由 Structured 性格 + 行为规则 + Simple tone 摘要组成。
- `examples` 从 Structured 示例对话解析；A1 允许为空。
- `initiative_config` 用默认值，后续主动陪伴阶段再扩。
- `memory_policy` 用默认值，后续 MemorySub 阶段再扩。
- `ui_metadata` 包含 name / version / source / relationshipStyle。
- `source_kind` 固定为 `legacy_soul_md`。

Validation blocking errors：

- 名字为空。
- 身份为空。
- 性格为空。
- 能力为空。
- Do / Don't 规则均为空。
- source text 包含可疑越权 section 或字段：`permissions`、`tools`、`safety_prefix`、`system_prefix`、`clipboard`、`screen_capture`。

Validation warnings：

- token 估算超过 A1 软预算。
- examples 为空。
- 反应配置无法解析时保留 source，但不进入 runtime profile。

## 5. 后端 API

新增 IPC：

```text
persona_validate_draft(draft) -> PersonaDraftValidationResult
persona_save_draft(draft) -> PersonaSaveResult
persona_save_and_activate_draft(draft) -> PersonaSaveResult
persona_activate_snapshot(snapshot_id) -> void
persona_get_snapshot_profile(snapshot_id) -> SoulRuntimeProfile
```

`PersonaSaveResult`：

```json
{
  "persona_id": "momo",
  "snapshot_id": "42",
  "version": "1.0.1",
  "activated": true,
  "diagnostics": []
}
```

错误语义：

- validate 有 blocking error 时，save 返回错误，不写 DB。
- `snapshot_id` 不存在时，activate 返回 not found。
- snapshot 所属 persona 与 active persona 不同也允许激活；激活会切 active persona。
- DB 写入必须在事务内完成：upsert persona -> insert snapshot -> insert profile -> optional activate。

事件：

- `persona:activated` 保留，payload 仍是 persona id 字符串，兼容现有前端。
- A1 新增 `persona:snapshot-activated`，payload 是 `{ personaId, snapshotId }`，供未来更细 UI 消费。

## 6. Chat 过渡设计

A1 不一次性切 PromptBuilder 到 `SoulRuntimeProfile`，但必须完成 snapshot binding。

新建会话：

```text
active persona
  -> active_snapshot_id
  -> persona_snapshots.content
  -> conversations.persona_snapshot_id
```

已有会话：

```text
conversation.persona_snapshot_id
  -> persona_snapshots.content
  -> build_messages legacy parser
```

禁止行为：

- 已有会话不得因为 active persona 变化而重新绑定。
- 已有会话缺失 `persona_snapshot_id` 时，不得静默 fallback active persona。
- PromptBuilder 不读取 source 文件路径。

允许的 A1 过渡：

- Chat prompt 仍调用 `extract_persona_sections(raw_markdown)`。
- `SoulRuntimeProfile` 先写入 profile 表，A2 再让 PromptBuilder 消费它。

## 7. 前端 UI

Inspector actions：

- `验证`：调用 `persona_validate_draft`，更新 diagnostics。
- `保存快照`：调用 `persona_save_draft`，成功后刷新卡片列表和 Inspector 状态。
- `保存并激活`：调用 `persona_save_and_activate_draft`，成功后刷新卡片 active 标记，并触发现有 active persona 联动。

按钮规则：

- 有 blocking diagnostics 时，保存按钮 disabled。
- draft 修改后显示 unsaved 状态。
- 保存成功后显示 snapshot id / version。
- 激活成功后，角色卡 active tag 更新。

卡片状态：

- active persona。
- selected persona。
- unsaved draft。
- latest saved version。

## 8. 测试口径

Rust tests：

- `persona_save_draft` validates required fields before writing.
- save creates one `persona_snapshots` row and one `persona_snapshot_profiles` row.
- repeated save bumps patch version when version conflicts.
- save-and-activate updates exactly one `personas.is_active=1`.
- activate snapshot switches `active_snapshot_id`.
- new conversation binds active snapshot id.
- existing conversation keeps old snapshot after active snapshot changes.
- existing conversation with missing `persona_snapshot_id` returns repair error.
- dangerous source fields are rejected before profile insertion.

Vue tests：

- Inspector validate button calls service and renders diagnostics.
- blocking diagnostics disable save actions.
- save success refreshes persona cards.
- save-and-activate success updates active card state.

Integration verification：

- `cargo test --lib`
- targeted persona Rust tests
- targeted Persona Workshop vitest tests
- `pnpm typecheck`
- `pnpm test`
- `pnpm build`

## 9. Implementation Slices

Slice 1: DB + backend compiler

- Add migration for active snapshot, profile table, conversation snapshot binding.
- Add `SoulRuntimeProfile` Rust DTO.
- Add compiler from A1 draft DTO to source text + runtime profile.
- Add save / validate / activate snapshot commands.

Slice 2: Chat snapshot binding

- Update conversation creation to write `persona_snapshot_id`.
- Update prepare path to load conversation snapshot for existing conversations.
- Preserve legacy raw markdown prompt construction from snapshot content.

Slice 3: Workshop actions

- Wire validate / save / save-and-activate buttons.
- Refresh card stage after save.
- Render saved / active / diagnostic states.

## 10. Risks

Version bumping must be deterministic. A1 uses patch bump against existing snapshot versions to avoid unique-index conflicts.

Existing conversations need backfill. Migration should be idempotent and should not delete or rewrite message history.

Chat currently validates conversation ownership against active persona. A1 must remove that active-persona check for existing conversations, otherwise old conversations break after switching persona.

The legacy compiler still produces markdown source. This is intentional; A1 creates the runtime-profile table so the next slice can migrate PromptBuilder without changing Workshop save semantics again.
