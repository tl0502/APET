---
title: Persona Workshop v1 设计
updated: 2026-06-18
related:
  - ../../persona/persona-design.md
  - ./2026-06-18-agent-runtime-contract-design.md
  - ./2026-05-24-companion-agent-runtime-design.md
---

# Persona Workshop v1 设计

## 0. 结论

Persona Workshop v1 采用三层编辑模型：

1. **塑形模式 Simple**: 面向大多数用户，用表单和滑块塑造桌宠是谁、怎么说话、主动到什么程度。
2. **结构模式 Structured**: 面向愿意细调的用户，按人格区段编辑身份、性格、能力、行为规则、离线模板、反应配置和示例对话。
3. **源码模式 Source**: 面向 power user，显示并编辑源文本，但保存必须经过 validate / compile。

底层不把 `.soul.md`、`.soul/` 或 `.soulpack` 作为 UI 的硬前提。最新 runtime contract 已将 source format 降级为未决，运行时只依赖 `SoulRuntimeProfile` 和 `PersonaSnapshot`。因此 Workshop 的核心边界是：

```text
PersonaSourceDraft
  -> validate
  -> compile
  -> SoulRuntimeProfile
  -> PersonaSnapshot
```

UI 可以继续支持 `.soul.md` 作为 legacy / simple 输入，但 Chat、Interaction、Initiative 等运行时模块不得直接读取源文件。

## 1. 外部调研摘录

### 1.1 SillyTavern Character Design

SillyTavern 的角色设计把 `Character Name` 设为唯一必填字段，其余字段可逐步填写。它明确区分常驻角色定义和非永久字段，并强调角色定义 token 会挤压聊天历史。高级定义默认隐藏，需要用户主动展开。

对 AIPET 的启发：

- 首屏不要暴露完整 prompt 或 schema。
- 人格编辑器必须显示 token / budget 影响。
- 高级字段可以存在，但默认不要打扰普通用户。

参考: https://docs.sillytavern.app/usage/core-concepts/characterdesign/

### 1.2 Character Card v2

Character Card v2 把字段分成基础角色字段、creator metadata、system prompt、post-history instructions、alternate greetings、character book 和 extensions。它要求编辑器不能破坏未知扩展字段，且 metadata 不一定进入 prompt。

对 AIPET 的启发：

- 源格式需要保留未知字段，避免导入再导出时损坏社区人格。
- UI metadata 和 runtime prompt material 必须分离。
- 扩展字段必须命名空间化，不能直接获得权限、工具或安全前缀控制。

参考: https://raw.githubusercontent.com/malfoyslastname/character-card-spec-v2/main/spec_v2.md

### 1.3 SillyTavern World Info / Lorebook

World Info 使用关键词、优先级、插入顺序和 token budget 动态注入 prompt，而不是把所有世界观永久塞进角色正文。

对 AIPET 的启发：

- 人格定义、记忆、情境关心、反应配置需要分层消费。
- Workshop 可以编辑“会进入人格快照的内容”，但用户记忆和运行时状态不属于人格源文件。
- 未来的 initiative / memory policy 应通过 `SoulRuntimeProfile` 的结构字段给对应子系统消费，而不是混进身份 prompt。

参考: https://docs.sillytavern.app/usage/core-concepts/worldinfo/

## 2. 产品目标

Persona Workshop v1 解决三个问题：

1. 用户能亲手塑造桌宠人格，而不是只能选内置三张卡。
2. 用户能理解“保存的人格”和“当前聊天使用的人格快照”之间的关系。
3. 后续 source format 演进不会推翻 UI 和运行时边界。

非目标：

- 不在 v1 实现人格市场。
- 不在 v1 实现自动学习后改写人格。
- 不在 v1 承诺 `.soul/` 是最终源格式。
- 不让人格源文件声明工具权限、系统安全前缀、屏幕读取、剪贴板读取或直接记忆写入。

## 3. 信息架构

Persona Workshop 位于 workspace 的创作区，替代当前 `SettingsPersonaPanel` 的占位态。

Workshop v1 不采用“列表 / 编辑 / 预览”三栏后台表单。人格是用户亲手塑造的角色对象，首屏应先呈现为可识别、可选择的角色卡舞台；编辑器是被某张角色卡触发的上下文工具面板。

建议布局：

```text
Workspace / 创作 / 人格

┌────────────────────────────────────────────────────────────────────────────┐
│ Toolbar: 刷新 / 新建 / 复制内置 / 导入 / 导出                              │
├───────────────────────────────────────────────┬────────────────────────────┤
│ Character Card Stage                           │ Inspector Drawer           │
│                                               │                            │
│ ┌──────────┐ ┌──────────┐ ┌──────────┐         │ 当前选中人格               │
│ │ 角色卡   │ │ 角色卡   │ │ 角色卡   │         │ [塑形] [结构] [源码]       │
│ │ name     │ │ name     │ │ name     │         │ mode-specific editor       │
│ │ status   │ │ status   │ │ status   │         │                            │
│ └──────────┘ └──────────┘ └──────────┘         │ 诊断 / token / 操作         │
│                                               │ [验证] [试聊] [保存快照]    │
└───────────────────────────────────────────────┴────────────────────────────┘
```

角色卡舞台职责：

- 展示内置人格和用户人格，不使用窄列表。
- 每张卡显示名字、来源、版本、激活状态、保存状态和一句话定位。
- 当前激活人格高亮，但允许选中非激活人格进行编辑。
- 点击卡片后切换选中人格，并在右侧 Inspector Drawer 中载入草稿。
- 支持复制内置人格作为用户草稿。
- 空态或加载态必须保留舞台结构，不能塌回普通表单。

顶部工具条职责：

- 提供刷新、新建、复制内置、导入、导出等全局动作。
- 不承载当前人格的细粒度编辑动作，避免和右侧抽屉重复。

右侧 Inspector Drawer 职责：

- 承载三层编辑模式。
- 保持 draft 未保存状态。
- 切换模式时做 lossless projection，不能无提示丢字段。
- 显示 validate / compile 结果。
- 显示 prompt material 预览，但标明这是编译后视图，不是安全前缀。
- 显示 token budget。
- 提供 3 到 5 轮试聊沙盒。
- 提供验证、试聊、保存快照、激活等与当前选中人格绑定的动作。

抽屉交互：

- 首次进入 Workshop 时，默认选中当前激活人格，但不打开抽屉。
- 点击角色卡时，抽屉从右侧呼出并载入对应草稿；点击另一张卡时，抽屉保持打开并切换内容。
- 关闭抽屉只隐藏编辑器，不清空当前选中人格，不丢弃未保存草稿。
- 桌面主尺寸下，抽屉不使用 `position: fixed`，而是进入 Workshop 内部布局流，从右侧滑出并压缩角色卡舞台宽度。
- 小窗口下，抽屉在 Persona Workshop 容器内覆盖角色卡舞台，从右侧滑入；它不脱离工坊容器，不覆盖 workspace chrome 或其它 panel。

## 4. 三层编辑模型

### 4.1 塑形模式 Simple

Simple 是默认入口。它不出现 prompt、system、schema、YAML 等术语。

字段：

- 名字
- 一句话定位
- 相处风格: 陪伴 / 损友 / 督学 / 自定义
- 五维 tone slider: 温暖、俏皮、正式、主动、简洁
- 说话长度: 很短 / 正常 / 详细
- 主动关心强度: 安静 / 偶尔 / 经常
- 不喜欢的说法: 禁用称呼、禁用语气、不要做的事
- 3 条示例说话

保存行为：

- Simple 生成 `PersonaSourceDraft.simple_profile`。
- 编译时扩展为 identity / style / rules / examples。
- 不覆盖用户在 Structured / Source 中已有但 Simple 不认识的字段，除非用户确认“用简易设置重写高级内容”。

### 4.2 结构模式 Structured

Structured 面向愿意认真调人格的用户，按逻辑区段组织。

区段：

- 身份: “它是谁”
- 性格: 稳定人格特质
- 能力: 它可以帮什么
- 行为规则: Do / Don't
- 离线模板: 共情、问候、拒答、调侃、庆祝
- 反应配置: click / drag / idle 等本地物理反应覆盖
- 示例对话: 进入 PromptBuilder 的 examples

设计约束：

- 每个区段显示是否进入 LLM prompt、是否只给本地子系统消费。
- `# 离线模板` 和 `# 反应配置` 默认不进入 Chat prompt。
- 反应配置用表格或键值编辑器，不要求用户手写 YAML。
- 每个区段有最小验收，例如身份不能为空、Do / Don't 至少各一条。

### 4.3 源码模式 Source

Source 展示当前 source format 的文本视图。v1 可以先支持 `.soul.md`，但 UI 文案使用“源文件”而不是绑定某个扩展名。

行为：

- 打开源码模式时从 `PersonaSourceDraft` 投影成文本。
- 保存源码时先 parse，再 validate，再 compile。
- parse 成功但 compile 有警告时允许试聊，不允许激活。
- 未知字段必须保留，不能因为切换模式而消失。
- 包含权限、工具、安全前缀、系统越权字段时 reject 或 ignore，并给出诊断。

## 5. Runtime 边界

Workshop 不直接写运行时 hot path。它只产出草稿和快照。

```text
PersonaSourceDraft
  source_kind: simple | structured | source
  display_metadata
  source_payload
  preserved_unknown_fields

SoulRuntimeProfile
  identity_prompt
  style_prompt
  initiative_config
  memory_policy
  examples
  ui_metadata
  source_hash

PersonaSnapshot
  id
  persona_id
  runtime_profile
  created_at
```

消费关系：

- Chat / PromptBuilder 读 `SoulRuntimeProfile.identity_prompt`、`style_prompt`、`examples`。
- Interaction 读编译后的 reaction overrides，不再临时解析 raw markdown。
- Initiative 读 `initiative_config`。
- MemorySub 读 `memory_policy`。
- SafetyPolicy / SafetyPrefix 不由人格源文件控制。
- 用户昵称、mood / energy、当前装扮、任务状态不冻结进 PersonaSnapshot。

## 6. v1 范围

P0:

- 用户可复制内置人格生成个人草稿。
- Simple 模式可编辑并保存。
- Structured 模式可编辑身份、性格、能力、规则、离线模板、反应配置、示例对话。
- Source 模式可查看和编辑 `.soul.md` legacy 文本。
- validate / compile 生成 `SoulRuntimeProfile`。
- 保存生成新的 `PersonaSnapshot`。
- 试聊沙盒使用 draft / snapshot，不写正式记忆，不污染正式会话。
- 激活新 snapshot 后，新会话使用它，旧会话不被静默重绑。

P1:

- 导入 / 导出人格源文件。
- `.soulpack` 或其他 package 格式，需另开 source-format spec 决定。
- 快照历史、恢复上一版本。
- 更细的 token budget explain。
- 反应配置动作预览。

P2:

- 社区分享 / 人格市场。
- AI 辅助生成草稿。
- 用户授权的人格成长建议。

## 7. 验收口径

产品验收：

- 首次进入 Workshop，用户能在 3 分钟内复制一个内置人格并改出明显不同的说话风格。
- 不懂 Markdown 的用户能完成编辑、试聊、保存、激活。
- power user 能看到源码并理解保存失败原因。
- 用户能区分“草稿未保存”“已保存快照”“已激活快照”。

技术验收：

- Chat hot path 不读取 `.soul.md` 或其他源文件。
- 修改人格后生成新 snapshot，旧会话 snapshot 不变。
- Source 模式未知字段导入再保存不丢失。
- 人格源文件不能声明工具权限、安全前缀、屏幕读取、剪贴板读取或直接记忆写入。
- 编译诊断覆盖缺失必填、token 超预算、非法反应 key、非法字段、parse error。

## 8. 待决问题

1. v1 的 source payload 是继续以 `.soul.md` 为主，还是先定义内部 JSON draft，再导出 `.soul.md`。
2. `SoulRuntimeProfile` 是否应把 reaction overrides 放入同一个 JSON，还是拆给 InteractionSub 单独 profile。
3. 试聊沙盒保存后是归档为 snapshot sample，还是直接删除。
4. 内置三人格是否在 v1 同步迁移到结构化 source，还是继续 legacy `.soul.md` 经 compiler 进入 runtime profile。
5. 导入导出格式是否仍使用 `.soulpack`，需要另开 source-format spec 决定。
