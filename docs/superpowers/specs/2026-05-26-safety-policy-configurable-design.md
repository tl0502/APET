---
title: SafetyPolicy 可配置化设计（Phase A0 收口）
updated: 2026-05-26
related:
  - 2026-05-24-companion-agent-runtime-design.md
  - ../../decisions.md
  - ../../STATUS.md
---

# SafetyPolicy 可配置化 设计文档

> Phase A0 审计收口产物。把 SafetyGuard 从 "always-on 强制" 改为 "kernel-owned policy 可配置 4 scope"，同步收口 HIGH-1 (mid-stream scan_token 真接入) 与 HIGH-2 (messages.safety_scan_status 列真接入 ChatService)，并把 follow-up [#49](https://github.com/tl0502/APET/issues/49) trailing-window 优化并入实施。

## 1. 背景

[2026-05-25 Phase A0 架构审计](../../STATUS.md) 发现 2 个 HIGH 级 spec-vs-实施漂移：

1. **HIGH-1** `scan_token` (Scope #2 mid-stream) 完全未接入 ChatService::run_stream，与 [v3 spec](2026-05-24-companion-agent-runtime-design.md) §14.1 + §6.6.2 "Phase A0 必上 scope 1+2+3" 直接冲突；衍生 `ReplaceReason::SoftBlockToken` / `SafetyScanStatus::Streaming`+`StreamSoftBlocked` 三处 dead code。
2. **HIGH-2** `messages.safety_scan_status` 列建了 + ConversationRepo 提供了 `update_safety_status` / `update_message_content_and_status` 接口，但 ChatService 从未调过这两个方法，所有消息行的 safety_scan_status 永远停在 default `'pending'`。spec §6.6 关键转移规则 5 "后端必须以 safety_scan_status 为 source of truth" 不成立。

直接驱动是 [v3 spec](2026-05-24-companion-agent-runtime-design.md) 中 Constitution #1 "Safety Sovereignty" 把 SafetyGuard 定义为 always-on 强制注入 + 不可被 subsystem bypass。这条契约与本项目的实际产品定位不匹配：

- AIPET 是单人 vibecoding 桌宠项目，SafetyGuard 严格审查不是核心产品诉求
- 强制 always-on 把"便宜的 prefix 注入"和"昂贵+易误报的 mid-stream scan"绑成一个 bool，损失 toggle 灵活度
- 默认全开的扫描会持续触发误报（4 黑词中 "违法" / "违禁" 命中率高），干扰正常聊天体验

用户决策（2026-05-25）4 条原则：

1. 不接受 spec 与 implementation 继续漂移
2. 单人项目，SafetyGuard 严格扫描不是核心产品诉求
3. 不要把安全扫描做成不可关闭的强制 always-on
4. 做成可配置开关：路径完整、状态完整、默认策略可配置

## 2. 设计目标

- **可配置**：4 个 scope (PrefixInjection / UserInput / StreamToken / FinalOutput) 各自独立 toggle；出厂全 OFF
- **路径完整**：关闭时仍走 SafetyGuard 路径，返 always-pass；不写 `if disabled return early` 短路 (subsystem 不得 bypass SafetyGuard 自建路径)
- **状态完整**：messages.safety_scan_status 列真接入 ChatService 写入主链路，所有终态显式落库
- **不漂移**：Constitution #1 改写为 "Safety Configurable"；v3 spec §3 / §4.2 / §6.6 / §6.6.2 / §14.1 五处 Updated；ADR-006 二次 Updated；decisions.md 新增 ADR-026 形式归档本决策
- **同步收口**：HIGH-1 (scan_token 真接入 + [#49](https://github.com/tl0502/APET/issues/49) trailing-window 优化) + HIGH-2 (safety_scan_status 列真写) 一并落地，消除 dead code

## 3. 不在范围内

- **不动 Constitution #9 "Privacy by Default"**：CI 黑名单 OS context API（GetForegroundWindow / getUserMedia 等）保持原状。Privacy 是 OS 上下文权限收口，与 LLM 输出 Safety 是独立体系
- **不动 [#48](https://github.com/tl0502/APET/issues/48)**：`llm_test_provider` IPC probe 路径仍 bypass SafetyPolicy（连通性测试不应被扫描卡住），降级为 "by design"
- **不动 scan_user_input 黑词表内容**：保持现状 4 词（自杀 / 自残 / 违法 / 违禁），P1 再评估扩规则
- **不做 log_only 三态 policy**：选 Approach A bool toggle 而非 Approach C 三态枚举（单人项目复杂度溢价不值）
- **不做历史数据 backfill**：现有 messages 行 `safety_scan_status='pending'` 老行 + `mode='safety_*'` 老行不改写；新写入按新 schema
- **不做 SoulPolicy / PersonaPolicy 同款扩展**：本次仅 SafetyPolicy，其他 kernel 件套 (PermissionService / GrantBroker) 的可配置化不在本 spec 范围

## 4. 设计

### 4.1 SafetyPolicy trait + ConfigKvSafetyPolicy

新建 `src-tauri/src/kernel/safety_policy.rs`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafetyScope {
    PrefixInjection, // wrap_messages 是否注入 ADR-006 prefix
    UserInput,       // scan_user_input 是否真扫
    StreamToken,     // scan_token 是否真扫
    FinalOutput,     // scan_final 是否真扫
}

pub trait SafetyPolicy: Send + Sync {
    fn is_enabled(&self, scope: SafetyScope) -> bool;
    async fn set_enabled(&self, scope: SafetyScope, enabled: bool) -> Result<(), PolicyError>;
}

pub struct ConfigKvSafetyPolicy {
    db_path: PathBuf,
    prefix: Arc<AtomicBool>,
    user_input: Arc<AtomicBool>,
    stream_token: Arc<AtomicBool>,
    final_output: Arc<AtomicBool>,
}
```

**ConfigKvSafetyPolicy 行为**：

- `load_from_kv(db_path)` 同步阻塞读 4 个 KV（boot 期 sync 上下文 OK）；KV 不存在 → fallback `false`；DB 连接失败 → fallback 全 OFF + `eprintln!("[safety_policy] load_from_kv failed, fallback to all-OFF: {e}")`
- `is_enabled(scope)` → `AtomicBool::load(Ordering::Relaxed)`，每个 token 都可调，无 DB hit
- `set_enabled(scope, enabled)` → 先写 DB（`config::set_with_conn`），DB 成功后再 `AtomicBool::store`；DB 失败时**内存不更新**保持一致性，返 Err 给 IPC caller
- KV 值不是合法 bool 字符串 → `parse::<bool>()` 失败 → fallback false + warning

**测试用 MockSafetyPolicy**：直接持 4 个 `AtomicBool`，`set_enabled` 同步写内存不走 DB，便于单测注入。

### 4.2 4 个 config 表 KV

| Key | Default | 控制 |
|---|---|---|
| `safety:prefix_enabled` | `"false"` | wrap_messages（注入 ADR-006 安全前缀） |
| `safety:scan_user_input_enabled` | `"false"` | scan_user_input（Scope #1 用户输入扫） |
| `safety:scan_token_enabled` | `"false"` | scan_token（Scope #2 流式 mid-stream 扫） |
| `safety:scan_final_enabled` | `"false"` | scan_final（Scope #3 流终全文扫） |

零迁移（[lessons §2](../../lessons.md) 27 表零迁移原则），沿用现有 config 表 KV 路径，无 schema 改动。

### 4.3 SafetyGuard 接口扩展

`SafetyGuard` trait 新增 1 方法 `is_enabled`：

```rust
pub trait SafetyGuard: Send + Sync {
    fn is_enabled(&self, scope: SafetyScope) -> bool;           // 新增
    fn wrap_messages(&self, ...) -> Vec<ChatMessage>;
    fn scan_user_input(&self, text: &str) -> ScanFinalResult;
    fn scan_token(&self, ...) -> ScanTokenResult;
    fn scan_final(&self, ...) -> ScanFinalResult;
}
```

`SafetyGuardImpl` 内部持 `Arc<dyn SafetyPolicy>`，构造签名从 `from_text(prefix)` 改为 `from_text(prefix, policy)`：

- `is_enabled(scope)` 转发 `self.policy.is_enabled(scope)`
- `wrap_messages` / `scan_*` 4 方法内部**也按 policy 短路返 noop**（防御性双保险）：
  - `wrap_messages` 关时返 messages 原样不注入 prefix
  - `scan_user_input` / `scan_final` 关时返 `ScanFinalResult::Ok`
  - `scan_token` 关时返 `ScanTokenResult::Pass`

这是路径完整原则的工程落地：ChatService 必须经过 SafetyGuard 入口，即使 ChatService 忘了 `if guard.is_enabled` 也安全（SafetyGuard 内部短路兜底）。

### 4.4 8-state FSM（在 v3 spec 7-state 基础上加 `disabled`）

`ConversationRepo::SafetyScanStatus` 扩 enum：

| 状态 | 触发条件 | DB.content | UI |
|---|---|---|---|
| `pending` | INSERT placeholder 时默认 | "" | typing placeholder |
| `streaming` | `scan_token on` + 首个 token 到达 | 累积 partial | typing 动画 |
| `stream_soft_blocked` | `scan_token on` + chunk 命中 soft 规则 | partial 含 redact 标记 | 流式继续 + 标记 |
| `final_ok` | `scan_final on` + 全文通过 | 全文 | 流式自然结束 |
| `final_redacted` | `scan_final on` + 命中 soft 规则 | redacted 全文 | ReplaceMessage 替换 |
| `final_blocked` | `scan_final on` + 命中 hard 规则（含 scan_token hard hit 强制终态） | fallback 文案 | ReplaceMessage 整条换 |
| `scan_failed` | scan 自身崩 | 累积 partial | warning banner + fallback |
| **`disabled`** (新) | **`scan_final off`** + ChatService 流末显式写入 | LLM 原文 | 流式自然结束 |

转移图加分支：`pending → {streaming \| disabled}`，分流条件 = `policy.is_enabled(FinalOutput)`。

### 4.5 Cross-scope 互动表

| `scan_token` | `scan_final` | 流末状态写入 |
|---|---|---|
| OFF | OFF | `disabled` |
| OFF | ON | `final_ok` / `final_redacted` / `final_blocked`（走 scan_final 正常路径） |
| ON | ON | 完整 8-state FSM |
| ON | OFF | mid-stream scan 命中走 `stream_soft_blocked` / `final_blocked`（hard hit 强制终态）；**无命中**时流末写 `disabled` |

`PrefixInjection` 与 scan 系列**独立**（可"不注入 prefix 但仍扫输出"或反之）。
`UserInput` 与 assistant message 的 safety_scan_status **无关**（user input scan 影响输入路径，不写 message 行）。

### 4.6 ChatService 4 接线点

**接线点 1 — prepare 期 scan_user_input**（已存在，重写）：

```rust
if guard.is_enabled(SafetyScope::UserInput) {
    match guard.scan_user_input(&input) {
        Ok => continue,
        Redacted { redacted_text, .. } => input = redacted_text,
        Blocked { rule_ids, .. } => return Err(ChatError::UnsafeInput(...)),
        ScanFailed { reason, .. } => return Err(ChatError::SafetyScanFailed(...)),
    }
}
```

**接线点 2 — prepare 期 wrap_messages**（已存在，重写）：

```rust
let messages = guard.wrap_messages(messages, Locale::ZhCn);
// SafetyGuard 内部按 policy 决定真注入还是返原 messages
```

**接线点 3 — run_stream 流式 on_delta（HIGH-1 修复）**：

```rust
let on_delta = move |delta| {
    if let TextDelta(text) = &delta {
        buffer.lock().push_str(text);
        channel.send(Delta { token });

        if guard.is_enabled(SafetyScope::StreamToken) {
            let acc = buffer.lock().clone();
            match guard.scan_token(text, &acc, false) {
                Pass => first_chunk_marks_streaming_status_once(),
                SoftBlock { replace_last_n, placeholder } => {
                    // 1. 替换 buffer 尾部 N chars 为 placeholder
                    // 2. ConversationRepo::update_message_content_and_status 写 stream_soft_blocked
                    // 3. emit ReplaceMessage 让前端覆盖 UI
                    // 4. rule_id 加入 dedupe set 避免震荡
                }
                HardEnd { rule_id } => {
                    // 1. cancel_token.cancel() 强制结束流
                    // 2. 写 safety_scan_status = 'final_blocked'
                    //    (hard hit 跳过 scan_final 重判, 强制终态)
                }
            }
        }
    }
};
```

**接线点 4 — run_stream Ok 分支末尾（HIGH-2 修复）**：

```rust
let final_status = if guard.is_enabled(SafetyScope::FinalOutput) {
    match guard.scan_final(&collected, &persona.id) {
        Ok => SafetyScanStatus::FinalOk,
        Redacted => SafetyScanStatus::FinalRedacted,  // emit ReplaceMessage
        Blocked => SafetyScanStatus::FinalBlocked,    // emit ReplaceMessage
        ScanFailed => SafetyScanStatus::ScanFailed,   // emit ReplaceMessage
    }
} else {
    SafetyScanStatus::Disabled
};

// 真接入 ConversationRepo (HIGH-2 修复, 消除 dead code)
let conv_repo = kernel.state_store.conversation_repo();
match final_status {
    FinalOk | Disabled =>
        conv_repo.update_safety_status(&mut conn, &msg_id, final_status).await?,
    _ =>
        conv_repo.update_message_content_and_status(&mut conn, &msg_id, &final_text, final_status).await?,
}
```

ChatService 构造签名保持 `new(safety_guard)`——policy 嵌在 SafetyGuard 内部，ChatService 通过 `safety_guard.is_enabled(scope)` 查询。

### 4.7 messages.mode 列含义重定义

| 旧含义 | 新含义 |
|---|---|
| 'online' / 'offline_rule' / 'cancelled' / 'safety_redacted' / 'safety_blocked' / 'safety_scan_failed' (6 值) | 'online' / 'offline_rule' / 'cancelled' (3 值) |

3 个 safety_* mode 值的**写入端废弃**（ChatService 改写到 safety_scan_status 列承担）。`messages.mode` 仅承载业务模式。

**向后兼容**：

- 现有 history filter (services/chat/service.rs:254-264) 保留对 safety_redacted / safety_blocked / safety_scan_failed 老行的 exclusion
- 新写入只用 3 值，旧值停止生成
- 未来 (Phase B+) 如需清理再做 backfill，本次不动

### 4.8 scan_token trailing-window 实现（[#49](https://github.com/tl0502/APET/issues/49) 并入）

当前 `safety_guard.rs:144` 实现是 `accumulated.contains(rule)`，每个 token 都全文 scan，O(n²) per stream。本次改 trailing-window：

```rust
const SCAN_TOKEN_WINDOW_CHARS: usize = 64;

fn scan_token(&self, partial: &str, accumulated: &str, _finished: bool) -> ScanTokenResult {
    let tail = trailing_chars(accumulated, SCAN_TOKEN_WINDOW_CHARS);
    // 仅扫尾部 64 chars (UTF-8 char 计数, 不是 byte)
    // 覆盖最长黑词 (4 字符 "自残" / "违法") + 上下文边界 (60 字符)
    for rule in &self.hard_blocklist {
        if tail.contains(rule) { return HardEnd { rule_id: rule.into() }; }
    }
    for rule in &self.soft_blocklist {
        if tail.contains(rule) { return SoftBlock { ... }; }
    }
    Pass
}
```

**rule_id dedupe**（safety_guard.rs:65-73 doc 警告的 contract gap 兜底）：ChatService 的 on_delta closure 维护 `HashSet<String>` rule_id 已命中集合，同 rule_id 第二次命中**不再触发 FSM 转换**（防震荡）。dedupe 状态生命周期 = 单条 assistant message 的 stream 周期，message 结束随 closure drop 释放。

### 4.9 workspace popup Safety 面板（UI）

[ADR-022](../../decisions.md#adr-022) sidebar nav 加第 7 项 "Safety"（在 About 之前 / Help 之后，保持"信息类→设置类→帮助类"语义）：

```
sidebar nav 7 项:
  profile / account / privacy / notifications / safety [新] / help / about
```

Panel 内容：
- 标题："安全护栏 / Safety"
- 说明文字："AIPET 默认关闭 LLM 输入输出安全扫描。如需启用，按 scope 单独打开。详见 [ADR-006](../../decisions.md#adr-006) 与 [ADR-026](../../decisions.md#adr-026)。"
- 4 个 toggle（Element Plus `el-switch`）：
  - "ADR-006 系统安全前缀注入" → `safety:prefix_enabled`
  - "用户输入扫描（防 prompt injection）" → `safety:scan_user_input_enabled`
  - "流式输出 mid-stream 扫描" → `safety:scan_token_enabled`
  - "流式输出最终全文扫描" → `safety:scan_final_enabled`
- 每个 toggle 下加灰色 hint：开销说明（"几乎无开销" / "用户输入一次同步扫" / "每个 token 增量扫" / "流终一次全文扫"）

Vue 组件 `SettingsSafety.vue` 走 `commands::safety::safety_get_policy` / `safety_set_policy_scope` IPC；监听 `safety:policy_changed` 全局事件多 popup 同步。

## 5. v3 spec / ADR / decisions.md 改动清单

### 5.1 [v3 spec §3 Constitution #1 改写](2026-05-24-companion-agent-runtime-design.md)

> **#1. Safety Configurable**（v2 名 "Safety Sovereignty"，2026-05-26 改写）
>
> SafetyGuard 路径必经，是否真注入/扫描由 **SafetyPolicy** 决定 4 个 scope (PrefixInjection / UserInput / StreamToken / FinalOutput) 各自启用；SafetyPolicy 是 kernel-owned，subsystem **不得 bypass SafetyGuard 直接处理 LLM 输入输出**；用户可经 IPC + workspace popup UI 配置 4 个 KV (`safety:*_enabled`)；出厂全 OFF。`scan_user_input` Blocked 时仍走 ChatError 抛回路径不变；`scan_final` 关时显式写 `safety_scan_status='disabled'` 终态。

### 5.2 v3 spec §4.2 Kernel 7 件套表

SafetyGuard 行的"不变量"列改：

> 旧：「Prefix 永远位于 system message 第一位;subsystem 不能跳过」
>
> 新：「SafetyGuard 路径必经，是否真注入/扫描由 SafetyPolicy 决定（4 scope toggle 出厂全 OFF）；subsystem 不得 bypass SafetyGuard 自建路径」

### 5.3 v3 spec §6.6 7-state FSM → 8-state FSM

- 7 状态 → 8 状态（加 `disabled` 终态）
- 转移图加分支 `pending → {streaming \| disabled}`，分流条件 = `policy.is_enabled(FinalOutput)`
- 新增 §6.6.0 "SafetyPolicy 与 SafetyGuard 协作"小节：定义 SafetyPolicy trait + 4 KV + Atomic 模式 + boot 顺序
- 新增 §6.6.3 "Cross-scope 互动表"（本文档 §4.5）

### 5.4 v3 spec §6.6.2 Scan Scope Matrix

- 每行加列「default enabled」，4 个 scope（SafetyPrefix + Scope #1/#2/#3）全 `OFF`
- 移除"Phase A0 必上 scope 1+2+3"措辞，改为"Phase A0 必接通路径 + 默认 OFF + 用户可配置"

### 5.5 v3 spec §14.1 Phase A0 MUST 扩列

新增 4 条 MUST：

- (a) `SafetyPolicy` trait + `ConfigKvSafetyPolicy` + 4 config KV
- (b) `messages.safety_scan_status` 列真接入 ChatService 主链路（修复 HIGH-2）
- (c) `scan_token` 真接入 ChatService::run_stream on_delta（修复 HIGH-1）+ trailing-window 64-char + rule_id dedupe（合并 [#49](https://github.com/tl0502/APET/issues/49)）
- (d) workspace popup Safety 4-toggle UI

SHOULD 改：原 "scan_user_input 简单黑词扫" 改为 "已是 A0 MUST，规则保持现状（4 黑词），P1 评估扩规则"。

MUST NOT 不变：OS context API 仍禁（Constitution #9 不在本 spec 范围）。

### 5.6 [ADR-006 二次 Updated](../../decisions.md#adr-006)

> **Updated 2026-05-26**：SafetyGuard prefix 与 scan 的注入路径由 **SafetyPolicy** 决定 (kernel-owned trait，4 scope toggle，出厂全 OFF，详 spec [`2026-05-26-safety-policy-configurable-design.md`](../superpowers/specs/2026-05-26-safety-policy-configurable-design.md))。原 "subsystem 无法 bypass" 语义保留 (subsystem 仍必经 SafetyGuard 路径)，但 "永远第一位/必扫" 改为 "policy 决定真注入/扫描 vs noop 时仍走 SafetyGuard 路径返 always-pass"。ADR-006 安全前缀文本本身**不变**（用户启用 PrefixInjection 时仍按本 ADR 文本注入，落 `assets/safety/prefix_v1.txt`）。

### 5.7 decisions.md 新增 ADR-026

> ### ADR-026 SafetyPolicy 可配置化（Phase A0 收口）
>
> - **为什么**：[CA Runtime v3 spec](../superpowers/specs/2026-05-24-companion-agent-runtime-design.md) 把 SafetyGuard 定义为 always-on 强制注入，但单人 vibecoding 项目里 SafetyGuard 严格扫描不是核心产品诉求，强制全开违反"按需配置"项目哲学；同时 [2026-05-25 Phase A0 审计](../STATUS.md) 暴露 mid-stream scan_token 未接入 + messages.safety_scan_status 列从未真写两项与 spec MUST 漂移。
> - **选什么**：在 kernel 内新建 `SafetyPolicy` trait + `ConfigKvSafetyPolicy`（4 config KV 持 `Arc<AtomicBool>`，详 spec [`2026-05-26-safety-policy-configurable-design.md`](../superpowers/specs/2026-05-26-safety-policy-configurable-design.md)），作为 `SafetyGuardImpl` 的依赖；SafetyGuard 路径必经但 noop-when-disabled；同步收口 HIGH-1（scan_token 真接入 + trailing-window O(window) 优化 [#49](https://github.com/tl0502/APET/issues/49)）+ HIGH-2（safety_scan_status 列真写，新增 `disabled` 终态）；workspace popup 加 Safety 4-toggle UI；Constitution #1 改写为 "Safety Configurable"。
> - **代价**：SafetyGuard trait +1 方法 (`is_enabled`); messages.safety_scan_status 列从 7 状态扩到 8 状态；CA Runtime v3 spec §3/§4.2/§6.6/§6.6.2/§14.1 五处 Updated；ADR-006 二次 Updated；与原 spec "永远第一位/必扫" 语义弱化（path completeness 保留：subsystem 仍不得 bypass SafetyGuard 自建路径）；mid-stream scan_token 仍要做（不能借此偷工，dead code 全部消除）。

## 6. Migration 与兼容性

- **migration 003 不新增**：safety_scan_status 列已在 002 加，扩 enum string 值不需要 DDL；`disabled` 是新允许值
- **现有 messages 行的 safety_scan_status**：全是 default 'pending'（HIGH-2 暴露），保持不动；UI 渲染时按 mode 列兜底
- **现有 messages 行的 mode='safety_redacted'/'safety_blocked'/'safety_scan_failed'**：保留，history filter 继续 exclusion
- **新写入路径**：mode 只允 3 值（online/offline_rule/cancelled）；safety_scan_status 8 状态
- **config KV 新键**：boot 时 KV 不存在 → fallback false，不自动 INSERT 默认行；用户首次 toggle 才 INSERT/UPDATE 真实值
- **test_db.rs::fresh_db**：apply 001 + 002，与 prod 一致；本次不加 003（无文件）；[lessons §17](../../lessons.md) 守护机制不退化
- **每个 repo 单测 inline schema**：本次先**不重构**为共享 helper（[审计 MED-4](../../STATUS.md) 单独 follow-up），但 conversation_repo / secret_repo 单测 inline schema 仍要确保 8 状态 enum / 4 列 secrets 与 prod 一致

## 7. Error handling

| 路径 | 失败模式 | 处理 |
|---|---|---|
| Boot 期 `SafetyPolicy::load_from_kv` DB 失败 | DB 文件损坏 / migration 未跑 | fallback 全 OFF + eprintln warning |
| `SafetyPolicy::set_enabled` 写 DB 失败 | 磁盘满 / WAL 锁 | 返 Err 给 IPC caller (UI toast)；内存态不更新 |
| `SafetyPolicy::set_enabled` 内存更新失败 | 不会发生 (`AtomicBool::store` infallible) | — |
| ChatService::run_stream `update_safety_status` DB 失败 | WAL 锁 / 磁盘满 | 与现有 `update_assistant_msg` 同款：channel send `Error { errorKind: "DbError" }` + 返；前端清流 |
| `scan_token` panic | 黑词表崩 / regex panic (P1 之后才有) | 当前简单 substring 不会 panic；防御性 `catch_unwind` 推 P1 |
| KV 值不是合法 bool 字符串 | 用户/外部工具改 DB | `parse::<bool>()` 失败 → fallback false + warning |
| `is_enabled` 跨线程读 | concurrent stream | `Arc<AtomicBool>` Relaxed ordering 已够（policy 变化对 in-flight stream 不强一致，下次 token 看到即可） |

## 8. 测试覆盖

### 8.1 新单测（kernel/safety_policy.rs）

- `boot_reads_4_kv_or_fallback_false`
- `set_enabled_atomic_updates_memory_and_db`
- `set_enabled_db_failure_rolls_back_memory`
- `kv_invalid_bool_string_falls_back_false`
- `arc_atomic_bool_visible_across_threads`

### 8.2 新单测（kernel/safety_guard.rs）

- `is_enabled_delegates_to_policy_for_4_scopes`
- `wrap_messages_noop_when_prefix_disabled`
- `scan_user_input_returns_ok_when_disabled`
- `scan_token_returns_pass_when_disabled`
- `scan_token_trailing_window_64_chars_only`
- `scan_token_rule_id_dedupe_prevents_thrash`
- `scan_final_returns_ok_when_disabled`

### 8.3 新单测（kernel/repos/conversation_repo.rs）

- `safety_scan_status_disabled_serializes_as_string`
- `update_safety_status_disabled_round_trip`

### 8.4 改单测（services/chat/service.rs）

- `test_chat_service()` helper 保留 `ChatService::new(safety_guard)` 签名（policy 已嵌在 guard 内）
- 新增 `chat_service_writes_safety_status_disabled_when_final_off`
- 新增 `chat_service_writes_safety_status_final_ok_when_final_on_and_clean`
- 新增 `chat_service_writes_safety_status_final_blocked_when_hit`
- 新增 `chat_service_writes_safety_status_stream_soft_blocked_when_mid_stream_hit`
- 新增 `chat_service_hard_hit_cancels_stream_and_writes_final_blocked`
- 新增 `chat_service_skipped_path_writes_disabled_no_repo_call_for_content`

### 8.5 新 IPC 测试（commands/safety.rs）

- `safety_get_policy_returns_4_bool`
- `safety_set_policy_scope_updates_db_and_memory_atomically`
- `safety_set_policy_scope_emits_policy_changed_event`

### 8.6 预期 cargo test 数

358 → 约 378-385（+20-25 新测）。

## 9. 工时估算

| 阶段 | 工时 |
|---|---|
| Spec 文档撰写 + v3 spec Updated 5 处 + ADR-006 二次 Updated + decisions.md ADR-026 | 2-3h |
| `kernel/safety_policy.rs` 实施 + 单测 | 2h |
| `kernel/safety_guard.rs` 重构（持 policy + is_enabled + 短路）+ trailing-window scan_token + 单测 | 3h |
| ChatService 4 接线点（含 ConversationRepo 真接入 + 8 状态写入）+ 单测改 | 3h |
| `commands/safety.rs` IPC + 测试 | 1h |
| workspace popup Safety panel UI（Vue 组件 + sidebar 加项 + 4 toggle + 广播 listener）| 2-3h |
| 联调 + 手动 e2e（4 toggle × 2 状态 = 8 cases）| 1h |
| **合计** | **14-16h（约 2 工作日）** |

## 10. follow-up issues 处置

- **[#48](https://github.com/tl0502/APET/issues/48)** （test connectivity probe 绕过 SafetyGuard）：本次不修，但在 commands/llm_providers.rs `llm_test_provider` IPC 内显式加注释 "by design: probe path bypasses SafetyPolicy"，issue 关闭转 by-design
- **[#49](https://github.com/tl0502/APET/issues/49)** （scan_token trailing-window 优化）：**并入本次 plan**（§4.8 N=64 chars + rule_id dedupe 一并实施），issue 在实施 commit 时关闭

## 99. 附录

### 99.1 参考资料

- [CA Runtime v3 spec §3 / §6.6 / §6.6.2 / §14.1](2026-05-24-companion-agent-runtime-design.md)
- [2026-05-25 Phase A0 架构审计报告（STATUS.md 内）](../../STATUS.md)
- [ADR-006 安全前缀](../../decisions.md#adr-006)
- [ADR-022 in-workspace popup sidebar nav 规范](../../decisions.md#adr-022)
- [lessons §2 27 表零迁移原则](../../lessons.md)
- [lessons §17 新增 migration 必须同步 test_db.rs](../../lessons.md)

### 99.2 待办

- [ ] Phase A1 启动前评估：UI 是否扩"安全审计查看"（list_recent context_access_log + safety_scan_status 历史）
- [ ] P1 评估：scan_token / scan_final 黑词表是否引入 regex / classifier，替换 4 词简单 substring
- [ ] P1 评估：scan_user_input log_only 三态 policy（如果调试期需要观察误报率）
- [ ] Phase B+ 评估：messages.safety_scan_status='pending' 老行是否 backfill；mode='safety_*' 老行是否清理
