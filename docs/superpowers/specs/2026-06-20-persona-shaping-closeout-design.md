---
title: Persona Shaping Closeout 设计
updated: 2026-06-20
related:
  - 2026-06-18-persona-workshop-design.md
  - 2026-06-20-persona-runtime-profile-promptbuilder-design.md
  - 2026-06-20-persona-example-dialogue-ui-design.md
  - ../../persona/persona-design.md
---

# Persona Shaping Closeout 设计

## 0. 结论

本 spec 命名为 A2-C0（Persona Shaping Closeout），做人格工坊“塑形感”收口，不扩功能面。这里的 A2-C0 是 A2-A/A2-B 之后的窄口径补丁，不占用既有文档里 A2-C “示例预览 / LLM 辅助生成评估”或“memory / live state PromptBuilder 扩展”的候选含义。

当前 A2-A/A2-B 已经把正式聊天 hot path 切到 `SoulRuntimeProfile`，并让示例对话进入 prompt。剩余问题不是“没接线”，而是接线材料语义太弱：滑杆以裸数字进入 `style_prompt`，`tagline` / `relationshipStyle` / `dislikes` / `initiative` 采集后主要停在 metadata，前端校验也比 Rust 编译器更宽。

本切片只做三件事：

1. 将工坊塑形字段编译为自然语言 prompt 约束。
2. 对齐前后端 draft validation，防止未来新建人格时出现“前端通过、后端 blocking”的漂移。
3. 给三个人格内置 `# 例对话`，让默认人格也演示 A2-B 的一等输入。

## 1. 目标

- 让 `warmth` / `playfulness` / `formality` / `proactivity` / `brevity` / `speech_length` 从裸数值变成可读、可执行的语气说明。
- 让 Rust 编译层对 0-5 滑杆做 prompt-safe clamp，避免异常 payload 生成 `255/5` 这类坏 prompt。
- 让 `tagline`、`relationshipStyle`、`dislikes`、`initiative` 进入 `SoulRuntimeProfile.style_prompt`，避免可编辑字段只影响 UI metadata。
- 保持 `SoulRuntimeProfile` 运行时契约不变，不新增 profile 字段。
- 让前端 `validatePersonaDraft` 对 Rust 当前必定拒绝的草稿给出 error；不要求 diagnostic code 与 Rust 一一对应。
- 为 momo / joker / coach 各补 2-3 条完整 user/persona pair。
- 用定点 Rust / Vitest 测试锁住编译结果和校验一致性。

## 2. 非目标

- 不实现新建人格、复制人格、草稿状态。
- 不实现试聊沙盒。
- 不实现快照历史、恢复、对比 UI。
- 不扩展结构编辑器字段；能力、离线模板、反应配置的编辑体验后续单独做。
- 不改 interaction 的 `# 反应配置` 消费路径。
- 不定义 `.soul/`、`.soulpack` 或导入导出最终格式。
- 不改 ChatService 的 snapshot binding / fail-closed 行为。

## 3. 当前根因

### 3.1 滑杆影响弱

`compile_persona_draft` 当前把滑杆拼为：

```text
warmth=3 playfulness=2 formality=2 proactivity=3 brevity=4 speech_length=short
```

LLM 不知道量纲、方向和行为含义。实际问题是语义不足，不是 `PromptBuilder` 没消费 profile。

### 3.2 采集字段未进入 prompt

`tagline`、`relationshipStyle`、`dislikes` 被写入 `ui_metadata`。`initiative` 写入 `initiative_config`，但 A2-A prompt path 尚不消费主动性配置。对用户来说，这些字段像能塑形，实际对聊天影响弱。

### 3.3 前后端校验漂移

Rust `compile_persona_draft` 把空 name 和空 capabilities 视为 error。前端 `validatePersonaDraft` 当前不查这两项。现阶段内置人格总能带出 name/capabilities，所以不容易爆；后续一旦加入从零新建，这会变成真实保存失败。

## 4. 设计

### 4.1 编译边界

人格塑形仍只在 `src-tauri/src/services/persona.rs::compile_persona_draft` 内完成。新增私有 helper 负责把 draft 字段转成 prompt 段落，例如：

```text
# 一句话定位
安静但靠谱的桌面伙伴。

# 关系与互动方式
- 关系风格：陪伴型搭档，优先站在用户身边一起想办法。
- 主动性：偶尔主动推进话题，但不连续催促。
- 回避偏好：除非用户主动要求，否则避开空洞鼓励。

# 语气参数
- 温暖度 4/5：语气偏温暖，会简短承接用户情绪。
- 俏皮度 2/5：可以轻微调侃，但不频繁玩梗。
- 回复长度 short：默认短句，必要时再展开。
```

`SoulRuntimeProfile` 的字段保持不变：

- `identity_prompt` 继续来自 `structured.identity`。
- `style_prompt` 包含 personality、rules、塑形字段自然语言。
- `initiative_config` 继续保留机器可读 `{ "mode": "sometimes" }`，供后续主动陪伴模块消费。
- `ui_metadata` 继续保留 UI 所需字段，供 inspector 展示。

### 4.2 映射原则

映射不追求复杂心理画像，只给 LLM 明确行为方向：

- 每个 0-5 滑杆都有低、中、高三档语言。
- Rust helper 在生成 prompt 前把可反序列化的 `u8` 滑杆值 clamp 到 `0..=5`；如果原值超出范围，编译结果追加 warning diagnostic，prompt 只使用 clamp 后的数值。
- 输出保留 clamp 后的数值，便于用户理解和测试断言。原始异常值只通过 diagnostic 暴露，不进入 prompt。
- `brevity` 与 `speech_length` 合并表达，避免相互冲突时出现双重指令；冲突时以 `speech_length` 为显式长度，`brevity` 表达信息密度。
- `initiative` 只描述聊天中的主动程度，不声明可以主动发消息；真正主动陪伴仍由后续模块调度。
- `dislikes` 表达为“除非用户主动要求，否则避开这些表达/话题/方式”。

### 4.3 前端校验

`src/features/persona-workshop/draft.ts::validatePersonaDraft` 补齐：

- `name.empty` error：名字不能为空。
- `capabilities.empty` error：能力不能为空。

对齐定义：只保证“Rust 会 blocking 的草稿，前端也至少有一个 error”。不要求 code / severity 完全一致。

规则校验继续保持前端更具体的 Do / Don't 诊断；Rust 当前将 Do 和 Don't 同空视为 blocking，并把单边缺失视为 warning。前端当前对 Do 缺失直接 error，严格于 Rust；这可以接受，因为 Do/Don't 同空时前端也必有 error，且符合当前编辑体验。

### 4.4 内置人格示例

修改：

- `src-tauri/personas/_builtin/momo.soul.md`
- `src-tauri/personas/_builtin/joker.soul.md`
- `src-tauri/personas/_builtin/coach.soul.md`

每个文件新增 `# 例对话`，采用 A2-B 已支持的 pair 格式：

```markdown
# 例对话
- 用户：我今天特别累。
  默默：那先别硬撑。放一下，我陪你缓一会。
```

示例要覆盖该人格的差异化语气，而不是泛用客服问答。

## 5. 数据流

保存路径：

```text
PersonaWorkshopPanel
  -> PersonaSourceDraft
  -> validatePersonaDraft（前端即时诊断）
  -> persona_validate_draft / persona_save_draft
  -> compile_persona_draft
  -> SoulRuntimeProfile.style_prompt（自然语言塑形）
  -> persona_snapshot_profiles.runtime_profile_json
```

聊天路径保持不变：

```text
conversation.persona_snapshot_id
  -> persona_snapshot_profiles.runtime_profile_json
  -> build_messages_from_profile
  -> SafetyGuard.wrap_messages
  -> LLMProvider
```

## 6. 错误处理

- 空 name / identity / personality / capabilities 仍为 blocking error。
- 滑杆字段如果超过 0-5，Rust helper clamp 到 0-5 后再写入 prompt，并追加 warning diagnostic。
- `speech_length` / `relationshipStyle` / `initiative` 如果出现未知枚举值，Rust helper 使用保守默认文案，并追加 warning diagnostic，避免保存崩溃；前端类型约束负责常规路径。
- 空 `dislikes` 不报错，只跳过回避偏好段。
- 空 `tagline` 不报错，只跳过一句话定位段。
- 内置人格 `# 例对话` 不改变 parser 必填规则；用户人格仍可没有 examples，只给 warning。

## 7. 测试

### Rust

- `compile_persona_draft` 生成的 `style_prompt` 包含自然语言滑杆说明，不再只依赖裸数字。
- `tagline`、`relationshipStyle`、`dislikes`、`initiative` 出现在 `style_prompt`。
- out-of-range 滑杆被 clamp 到 0-5，prompt 不出现异常值，并返回 warning diagnostic。
- 未知 `speech_length` / `relationshipStyle` / `initiative` 使用保守默认文案，并返回 warning diagnostic。
- `compile_persona_draft` 对空 capabilities 仍返回 blocking diagnostic。
- 三个内置人格解析后 `SoulRuntimeProfile.examples` 非空。

### TypeScript

- `validatePersonaDraft` 对空 name 返回 `name.empty` error。
- `validatePersonaDraft` 对空 capabilities 返回 `capabilities.empty` error。
- 现有 example pair 行为保持不变。

## 8. 验收

- 修改滑杆后，保存出的 snapshot profile 能从 `style_prompt` 直接读出语气变化。
- tagline / relationship / dislikes / initiative 不再只是 metadata。
- 前端不会把 Rust 必定拒绝保存的 name/capabilities 空草稿标成可通过。
- 三个默认人格不再触发 `examples.empty` warning。
- 不新增 IPC、schema、UI tab 或运行时 profile 字段。
