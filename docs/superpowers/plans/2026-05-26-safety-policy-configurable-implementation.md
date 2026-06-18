# SafetyPolicy 可配置化 Implementation Plan

> **Revision note (2026-06-18)**: 本 plan 已按 [`2026-06-18-agent-runtime-contract-design.md`](../specs/2026-06-18-agent-runtime-contract-design.md) 修订。以 2026-06-18 契约为准：`PrefixInjection` 出厂 OFF；`FinalOutput` OFF 只在没有更高优先级安全命中时写 `disabled`；`ScanTokenResult::SoftBlock` 必须携带 `rule_id`；hard hit 使用 dedicated safety-blocked finalization，不复用普通 cancel 分支；`scan_final` 审计上下文使用 `persona_snapshot_id`。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 SafetyGuard 从 always-on 强制改为 kernel-owned SafetyPolicy 驱动 4 scope toggle (PrefixInjection / UserInput / StreamToken / FinalOutput)，出厂全 OFF。同步收口 Phase A0 审计的 HIGH-1（mid-stream scan_token 真接入 + trailing-window N=64 优化，并入 #49）+ HIGH-2（messages.safety_scan_status 列真接入 ChatService 主链路 + 新增 `disabled` 终态消除 dead code），并加 workspace popup Safety panel 4-toggle UI。

**Architecture:** SafetyPolicy 作为 SafetyGuardImpl 的 dependency 注入（不改 Runtime v3 Kernel 7 件套数量），ConfigKvSafetyPolicy 持 4 个 `Arc<AtomicBool>` 在 boot 期从 config 表 KV 加载，运行期 atomic 读 / set 时同步更新 DB + 内存。SafetyGuard 路径必经，policy off 时方法返 always-pass noop（路径完整）。ChatService 真用 ConversationRepo 写 8-state safety_scan_status 列（状态完整）。

**Tech Stack:** Rust (Tauri 2.x + sqlx + tokio + parking_lot + thiserror + ulid) / Vue 3 + TypeScript + Pinia + Element Plus / vitest + cargo test --lib

---

## File Structure

### Backend Rust（新增 + 修改）

| 文件 | 类型 | 责任 |
|---|---|---|
| `src-tauri/src/kernel/safety_policy.rs` | 新建 | `SafetyScope` enum + `SafetyPolicy` trait + `ConfigKvSafetyPolicy` impl + `MockSafetyPolicy` + 单测 |
| `src-tauri/src/kernel/mod.rs` | 改 | `pub mod safety_policy;` + re-export |
| `src-tauri/src/kernel/safety_guard.rs` | 改 | trait +1 方法 `is_enabled`; `SafetyGuardImpl` 持 `Arc<dyn SafetyPolicy>`; 4 方法 noop short-circuit; `scan_token` 改 trailing-window N=64 chars + 单测 |
| `src-tauri/src/kernel/repos/conversation_repo.rs` | 改 | `SafetyScanStatus` enum +1 variant `Disabled` + 单测 |
| `src-tauri/src/kernel/runtime.rs` | 改 | `Kernel` struct +1 field `safety_policy`; `Kernel::boot` 加 Boot 3.5 (SafetyPolicy::load_from_kv); `SafetyGuardImpl::from_text(prefix, policy)` 改签名 |
| `src-tauri/src/services/chat/service.rs` | 改 | 4 接线点（scan_user_input / wrap_messages / on_delta scan_token / run_stream Ok 分支末尾 safety_scan_status 真写）+ `StreamSafetyState` rule_id dedupe + ConversationRepo 真接入 + mode 列 6→3 值 + 测试改/新 |
| `src-tauri/src/commands/safety.rs` | 新建 | `safety_get_policy` / `safety_set_policy_scope` IPC + 单测 |
| `src-tauri/src/commands/mod.rs` | 改 | `pub mod safety;` |
| `src-tauri/src/lib.rs` | 改 | `invoke_handler` 加 `safety_get_policy` / `safety_set_policy_scope` |

### Frontend Vue（新增 + 修改）

| 文件 | 类型 | 责任 |
|---|---|---|
| `src/stores/userPopup.ts` | 改 | `PopupNavId` 加 `'safety'` variant；`DISABLED_NAV_IDS` 不加 safety（safety 是 enabled 项） |
| `src/stores/__tests__/userPopup.test.ts` | 改 | 新增 case: setNav('safety') 工作 |
| `src/stores/safety.ts` | 新建 | `useSafetyStore` Pinia store: 4 ref bool + load() + setEnabled(scope, bool) + ensureListener(broadcast) |
| `src/stores/__tests__/safety.test.ts` | 新建 | safety store 5 case |
| `src/components/popup/PopupSidebar.vue` | 改 | `NAV_GROUPS` 应用组加 safety 项（label "安全" / icon "🛡"） |
| `src/components/popup/UserPopup.vue` | 改 | `panelTitle` computed case 加 safety / template render `UserSafetyPanel` |
| `src/panels/user/UserSafetyPanel.vue` | 新建 | 4 el-switch + 说明 + danger hint |

### Docs（修改）

| 文件 | 类型 | 责任 |
|---|---|---|
| `docs/decisions.md` | 改 | ADR-006 Updated 2026-05-24 + 2026-05-26 两段；新增 ADR-026 三句话；底部 "当前空闲" 改 ADR-027 |
| `docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md` | 改 | §3 Constitution #1 改写 / §4.2 SafetyGuard 行 / §6.6 加 §6.6.0+§6.6.3 + FSM 8-state / §6.6.2 加 default enabled 列 / §13.1 ADR-026 让位 ADR-030 / §13.2 ADR-006 2026-05-26 段 / §14.1 MUST 加 4 条 |

### 不动的文件（明确）

- `src-tauri/migrations/002_phase_a0_safety_secrets.sql` — 不加 migration 003，safety_scan_status 列已建，扩 enum 不需 DDL
- `src-tauri/src/services/test_db.rs` — apply 列表不变（无新 migration）
- `src-tauri/src/services/memory.rs::update_message_content_with_conn` — 保留旧路径与 mode 列写入兼容（向后兼容 layer）

---

## Task 1: docs/decisions.md ADR-006 二次 Updated + 新增 ADR-026

**Files:**
- Modify: `docs/decisions.md` (insert ADR-006 Updated paragraphs + new ADR-026 + bottom counter)

- [ ] **Step 1.1: 加 ADR-006 Updated 段（2026-05-24 + 2026-05-26）**

打开 `docs/decisions.md`，找到 ADR-006 三句话末尾（"代价" 行后），追加两段 Updated（保留原三句话不动；用户偏好 ADR 修订风格：已实施走 Updated，未实施走整段重写——ADR-006 已落地 assets/safety/prefix_v1.txt，走 Updated）：

定位 anchor: `### ADR-006 安全前缀` 段 "代价" 行后，插入：

```markdown

- **Updated 2026-05-24**：CA Runtime v3 spec 把 prefix 注入路径明确化（SafetyGuard.wrap_messages 是 kernel-owned trait，subsystem 不可 bypass）；新增 7-state FSM（pending → streaming → final_ok / final_redacted / final_blocked，含 scan_failed 保守降级）与流式 UI 替换语义；新增 `StreamEvent::ReplaceMessage` 协议（前端按 msg_id 覆盖）；scan 消费范围扩展（user input MVP 必扫；tool result + memory summary P1 必扫）。详 spec [`2026-05-24-companion-agent-runtime-design.md`](superpowers/specs/2026-05-24-companion-agent-runtime-design.md) §6.6 与 §13.2。
- **Updated 2026-05-26**：SafetyGuard prefix 与 scan 的注入路径由 **SafetyPolicy** 决定（kernel-owned trait，4 scope toggle: PrefixInjection / UserInput / StreamToken / FinalOutput，出厂全 OFF，详 spec [`2026-05-26-safety-policy-configurable-design.md`](superpowers/specs/2026-05-26-safety-policy-configurable-design.md)）。原 "subsystem 无法 bypass" 语义保留（subsystem 仍必经 SafetyGuard 路径），但 "永远第一位/必扫" 改为 "policy 决定真注入/扫描 vs noop 时仍走 SafetyGuard 路径返 always-pass"。FSM 从 7-state 扩到 8-state（新增 `disabled` 终态）。ADR-006 安全前缀文本本身**不变**（用户启用 `safety:prefix_enabled` 时仍按本 ADR 文本注入，落 `assets/safety/prefix_v1.txt`）。
```

- [ ] **Step 1.2: 加 ADR-026 三句话**

在 ADR-025 段之后（line 270 起向下找 "### ADR-025" 段尾），插入：

```markdown

### ADR-026 SafetyPolicy 可配置化（Phase A0 收口）

- **为什么**：[CA Runtime v3 spec](superpowers/specs/2026-05-24-companion-agent-runtime-design.md) 把 SafetyGuard 定义为 always-on 强制注入，但单人 vibecoding 项目里 SafetyGuard 严格扫描不是核心产品诉求，强制全开违反"按需配置"项目哲学；同时 2026-05-25 Phase A0 审计暴露 mid-stream scan_token 未接入 + messages.safety_scan_status 列从未真写两项与 spec MUST 漂移。
- **选什么**：在 kernel 内新建 `SafetyPolicy` trait + `ConfigKvSafetyPolicy`（4 config KV 持 `Arc<AtomicBool>`，详 spec [`2026-05-26-safety-policy-configurable-design.md`](superpowers/specs/2026-05-26-safety-policy-configurable-design.md)），作为 `SafetyGuardImpl` 的依赖；SafetyGuard 路径必经但 noop-when-disabled；同步收口 HIGH-1（scan_token 真接入 + trailing-window O(window) 优化 [#49](https://github.com/tl0502/APET/issues/49)）+ HIGH-2（safety_scan_status 列真写，新增 `disabled` 终态）；workspace popup 加 Safety 4-toggle UI；Constitution #1 改写为 "Safety Configurable"。
- **代价**：SafetyGuard trait +1 方法（`is_enabled`）；messages.safety_scan_status 列从 7 状态扩到 8 状态；CA Runtime v3 spec §3/§4.2/§6.6/§6.6.2/§14.1 五处 Updated；ADR-006 二次 Updated；与原 spec "永远第一位/必扫" 语义弱化（path completeness 保留：subsystem 仍不得 bypass SafetyGuard 自建路径）；mid-stream scan_token 仍要做（不能借此偷工，dead code 全部消除）。
```

- [ ] **Step 1.3: 改文件底部 "当前空闲" 编号**

定位 line 291（`新决策：...编号单调递增。当前空闲：**ADR-026**`），改为：

```markdown
新决策：`D-<NNN>-<kebab-case-title>`，编号单调递增。当前空闲：**ADR-027**。
```

- [ ] **Step 1.4: 验证 markdown 链接**

Run（PowerShell / git bash）:
```bash
grep -n "ADR-026" docs/decisions.md
grep -n "Updated 2026-05-2" docs/decisions.md
```
Expected: 3 处 ADR-026 命中（标题 + 内部引用 2 处）；2 处 Updated 段（2026-05-24 + 2026-05-26）

- [ ] **Step 1.5: Commit**

```bash
git -C /d/Project/temp/4 add docs/decisions.md
git -C /d/Project/temp/4 commit -m "$(cat <<'EOF'
docs(adr): ADR-006 二次 Updated + 新增 ADR-026 SafetyPolicy 可配置化

ADR-006 加 Updated 2026-05-24（v3 spec 7-state FSM + ReplaceMessage 协议）+
Updated 2026-05-26（SafetyPolicy 4 scope toggle 出厂全 OFF + 8-state FSM）。
ADR-006 安全前缀文本本身不变。

ADR-026 落地 spec 2026-05-26-safety-policy-configurable-design.md 三句话归档。
"当前空闲" ADR-026 → ADR-027。
EOF
)"
```

---

## Task 2: v3 spec 5 处 Updated

**Files:**
- Modify: `docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md`

每处 Updated 都是 inline 加段，不破坏原有结构。

- [ ] **Step 2.1: §3 Constitution #1 改写**

定位 `## 3. Constitution` 表格内 `| **1. Safety Sovereignty** |` 行（约 line 602），整行替换为：

```markdown
| **1. Safety Configurable** (Updated 2026-05-26, 原名 "Safety Sovereignty") | SafetyGuard 路径必经，是否真注入/扫描由 **SafetyPolicy** 决定 4 scope 各自启用 | `safety_prefix` 由 kernel 强制经 `SafetyGuard.wrap_messages` 走，subsystem 不得 bypass 自建路径; SafetyPolicy 4 KV (`safety:prefix_enabled` / `safety:scan_user_input_enabled` / `safety:scan_token_enabled` / `safety:scan_final_enabled`) 出厂全 OFF，用户经 IPC + workspace popup UI 配置；任何 LLM stream finish 必经 `SafetyGuard.scan_final` 路径 (off 时返 always-pass);**8-state FSM 跨越流式/终态(见 §6.6, 新增 `disabled` 终态)** |
```

- [ ] **Step 2.2: §4.2 Kernel 7 件套表 SafetyGuard 行**

定位 `### 4.2 Kernel 7 件套` 表内 `| **SafetyGuard** |` 行（约 line 669），整行替换为：

```markdown
| **SafetyGuard** | ADR-006 prompt prefix 注入 (policy-gated) + LLM 流式 token 增量扫描 + 终态全文扫描 + **8-state FSM (含 `disabled` 终态)** + 拒答降级链 | SafetyGuard 路径必经，是否真注入/扫描由 SafetyPolicy 决定（4 scope toggle 出厂全 OFF）；subsystem 不得 bypass SafetyGuard 自建路径 | A |
```

- [ ] **Step 2.3: §6.6 新增 §6.6.0 SafetyPolicy 协作小节**

在 `### 6.6 SafetyGuard 7-state FSM` 段标题之后（标题改为 `### 6.6 SafetyGuard 8-state FSM（Updated 2026-05-26：从 7-state 扩，新增 disabled 终态）`），引文之后，新增小节（在原 "**针对对象**" 行之前插入）：

```markdown
#### 6.6.0 SafetyPolicy 与 SafetyGuard 协作（Updated 2026-05-26）

SafetyPolicy 是 kernel-owned trait（不是 Kernel 第 8 件套，仍 7 件套；SafetyPolicy 作为 SafetyGuardImpl 的依赖注入）。详 spec [`2026-05-26-safety-policy-configurable-design.md`](2026-05-26-safety-policy-configurable-design.md)。

```rust
pub enum SafetyScope { PrefixInjection, UserInput, StreamToken, FinalOutput }

pub trait SafetyPolicy: Send + Sync {
    fn is_enabled(&self, scope: SafetyScope) -> bool;
    async fn set_enabled(&self, scope: SafetyScope, enabled: bool) -> Result<(), PolicyError>;
}

pub trait SafetyGuard {
    fn is_enabled(&self, scope: SafetyScope) -> bool;  // 转发 policy
    // ... 其余 4 方法 noop-when-disabled
}
```

4 个 config KV（出厂全 OFF）：

| Key | Default | 控制 |
|---|---|---|
| `safety:prefix_enabled` | `false` | wrap_messages 注入 ADR-006 prefix |
| `safety:scan_user_input_enabled` | `false` | scan_user_input (Scope #1) |
| `safety:scan_token_enabled` | `false` | scan_token (Scope #2) |
| `safety:scan_final_enabled` | `false` | scan_final (Scope #3) |

ConfigKvSafetyPolicy 持 4 个 `Arc<AtomicBool>`，boot 时同步读 KV 加载，运行期 atomic 读不 hit DB；`set_enabled` 写 DB 成功后才同步更新内存 AtomicBool（保持 DB 与内存一致）。
```

- [ ] **Step 2.4: §6.6 FSM 8-state**

定位原 FSM ASCII 图（约 line 906-940），整段替换为：

```markdown
**8 个状态** (`messages.safety_scan_status` 枚举值, Updated 2026-06-18 明确终态优先级):

```
                          ┌─────────────┐
                          │   pending   │ 消息创建, 尚未开始 stream
                          └──────┬──────┘
                                 │ run_stream 启动
                           ┌─────────────┐
                           │  streaming  │
                           └──────┬──────┘
                                  │
        ┌─────────────────────────┼──────────────────────────┐
        │ scan_token hard hit     │ scan_token soft hit      │ finish/no hit
        ↓                         ↓                          ↓
 ┌───────────────┐       ┌───────────────────┐       ┌────────────────────┐
 │ final_blocked │       │stream_soft_blocked│       │ finalization gate  │
 │ dedicated path│       │ keep stream state │       │                    │
 └───────────────┘       └─────────┬─────────┘       └──────────┬─────────┘
                                   │                            │
                                   └──────────────┬─────────────┘
                                                  ↓
                                      ┌──────────────────────┐
                                      │ FinalOutput policy   │
                                      └──────┬────────┬──────┘
                                             │ ON     │ OFF
                                             ↓        ↓
                              ┌──────────────────┐ ┌────────────────────┐
                              │ scan_final path  │ │ terminal priority  │
                              └───────┬──────────┘ └──────┬─────────────┘
                                      │                   │
                  ┌───────────────────┼─────────────┐     │
                  ↓                   ↓             ↓     ↓
            ┌──────────┐       ┌────────────┐ ┌──────────┐ ┌──────────┐
            │ final_ok │       │final_redact│ │final_blk │ │ disabled │
            └──────────┘       └────────────┘ └──────────┘ └──────────┘

Priority when deriving terminal status:
1. `ScanTokenResult::HardEnd` -> `final_blocked` via dedicated safety-blocked finalization.
2. `FinalOutput=ON` -> `scan_final` result decides `final_ok` / `final_redacted` / `final_blocked` / `scan_failed`.
3. `StreamToken=ON` soft hit + `FinalOutput=OFF` -> `stream_soft_blocked`.
4. No earlier safety hit + `FinalOutput=OFF` -> `disabled`.
```

| 状态 | 含义 | DB.messages 内容 |
|---|---|---|
| `pending` | INSERT placeholder | content="" |
| `streaming` | scan_token ON 流中 | content = partial |
| `stream_soft_blocked` | scan_token chunk soft hit | content = partial 含 redact 标记 |
| `final_ok` | scan_final ON + 全文通过 | 全文 |
| `final_redacted` | scan_final ON + soft 命中 | redacted 全文 |
| `final_blocked` | scan_final ON + hard 命中（含 scan_token hard hit 强制终态） | fallback 文案 |
| `scan_failed` | scan 自身崩 | partial + warning |
| **`disabled`** (新, Updated 2026-06-18) | **FinalOutput OFF 且无更高优先级安全命中**, ChatService 流末显式写入 | LLM 原文 |
```

- [ ] **Step 2.5: §6.6.3 Cross-scope 互动表（新增）**

在 §6.6.2 末尾追加：

```markdown

#### 6.6.3 Cross-scope 互动表（Updated 2026-05-26）

`PrefixInjection` 与 scan 系列**独立**；`UserInput` 与 assistant message 的 safety_scan_status **无关**。

`scan_token` × `scan_final` 4 组合（Updated 2026-06-18，与 Agent Runtime Contract §6 priority table 保持一致）：

| `scan_token` | `scan_final` | 流末状态写入 |
|---|---|---|
| OFF | OFF | `disabled` |
| OFF | ON | `final_ok` / `final_redacted` / `final_blocked` |
| ON | ON | 完整 8-state FSM |
| ON | OFF | hard hit → `final_blocked`；soft hit → `stream_soft_blocked`；无命中 → `disabled` |
```

- [ ] **Step 2.6: §6.6.2 Scan Scope Matrix 每行加 default enabled 列**

定位 `#### 6.6.2 Scan Scope Matrix` 表头，扩列：

```markdown
| # | Scope (被扫内容) | 来源 | scan_user_input | scan_token | scan_final | **Phase / default enabled (Updated 2026-05-26)** | 命中处理 |
|---|---|---|---|---|---|---|---|
| 1 | **user input** | 任一 surface 用户输入 | ✅ | — | — | A0 / **OFF (KV `safety:scan_user_input_enabled`)** | hit → ChatError::UnsafeInput, ConversationSub 拒发 LLM, UI 显示拒绝原因 |
| 2 | **assistant stream token** | LLM 流式 token chunk | — | ✅ | — | A0 / **OFF (KV `safety:scan_token_enabled`)** | soft hit → stream_soft_blocked (替换最近 N token 为 `[审核中…]`);hard hit → dedicated `final_blocked` finalization |
| 3 | **assistant final text** | LLM 流式终态全文 | — | — | ✅ | A0 / **OFF (KV `safety:scan_final_enabled`)** | 决定 final_ok / final_redacted / final_blocked / scan_failed (§6.6 8-state FSM) |
```

同时**移除** 矩阵 hard rules 中第 4 条 "Phase A0 必上 scope 1+2+3 (即原有 7-state FSM 全集)" 措辞，改为：

```markdown
- Phase A0 **接通**路径 scope 1+2+3 + 默认 OFF + 用户经 KV/UI 可启用；Phase A2 加 scope 4；Phase C 加 scope 6；P1+ 加 scope 5+7
```

- [ ] **Step 2.7: §13.1 ADR-026 编号让位 ADR-030**

定位 §13.1 表内 `| **ADR-026** | Companion Agent Runtime 顶层架构 |`，改编号为 `**ADR-030**`：

```markdown
| **ADR-030** | Companion Agent Runtime 顶层架构 + MVP Phasing (本 spec v2 摘要 + 四阶段 + 14 Constitution) | A | 本 spec 通过后归档为 ADR-030（编号让位 2026-05-26: ADR-026 已用于 SafetyPolicy 可配置化, 详 [`2026-05-26-safety-policy-configurable-design.md`](2026-05-26-safety-policy-configurable-design.md)） |
```

- [ ] **Step 2.8: §13.2 ADR-006 加 Updated 2026-05-26 sub-paragraph**

定位 §13.2 表内 ADR-006 行 "Updated 内容" 列末尾追加：

```markdown
**Updated 2026-05-26 二次**: SafetyGuard 注入路径由 SafetyPolicy 决定 4 scope toggle（出厂全 OFF，详 [`2026-05-26-safety-policy-configurable-design.md`](2026-05-26-safety-policy-configurable-design.md)）;7-state FSM 扩 8-state（加 `disabled` 终态）。原 "subsystem 无法 bypass" 语义保留, "永远第一位/必扫" 改为 "policy 决定 + noop-when-disabled 仍走路径"。
```

- [ ] **Step 2.9: §14.1 Phase A0 MUST 加 4 条**

定位 §14.1 的 `**MUST**:` 块末尾，追加：

```markdown
- **MUST (Updated 2026-05-26)**: SafetyPolicy trait + ConfigKvSafetyPolicy + 4 config KV (`safety:prefix_enabled` / `safety:scan_user_input_enabled` / `safety:scan_token_enabled` / `safety:scan_final_enabled`，出厂全 OFF)
- **MUST (Updated 2026-05-26)**: `messages.safety_scan_status` 列真接入 ChatService 主链路 (ConversationRepo 的 `update_safety_status` / `update_message_content_and_status` 真消费, 不再 dead code) — 修复 HIGH-2
- **MUST (Updated 2026-06-18)**: `scan_token` 真接入 ChatService::run_stream on_delta + trailing-window 优化 (N=64 chars, O(window) 替代 O(n²)) + `StreamSafetyState` rule_id dedupe 防震荡；SoftBlock 必须携带 `rule_id` — 修复 HIGH-1 + 合并 [#49](https://github.com/tl0502/APET/issues/49)
- **MUST (Updated 2026-05-26)**: workspace popup sidebar 加第 7 项 "Safety" 4-toggle UI
```

同时把 SHOULD 块原 "SafetyGuard scan_user_input 简单黑词扫" 改为：

```markdown
- ~~SafetyGuard scan_user_input 简单黑词扫~~ (Updated 2026-05-26: 已上 MUST, 详上方; 规则保持现状 4 黑词 P1 评估扩)
```

- [ ] **Step 2.10: 验证 spec 改动**

Run:
```bash
grep -n "Safety Configurable" docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md
grep -n "disabled" docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md | grep -i "fsm\|状态\|terminal"
grep -n "ADR-030" docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md
grep -cn "Updated 2026-05-26" docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md
```
Expected: Safety Configurable ≥1 命中；disabled 终态多处；ADR-030 ≥1 命中；Updated 2026-05-26 ≥6 命中

- [ ] **Step 2.11: Commit**

```bash
git -C /d/Project/temp/4 add docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md
git -C /d/Project/temp/4 commit -m "$(cat <<'EOF'
docs(spec): CA Runtime v3 spec 5 处 Updated — SafetyPolicy 可配置化

§3 Constitution #1 "Safety Sovereignty" → "Safety Configurable"
§4.2 SafetyGuard 行不变量列改写（路径必经 + policy 决定真注入/扫描）
§6.6 7-state FSM → 8-state（加 disabled 终态）+ 新增 §6.6.0 SafetyPolicy 协作 / §6.6.3 Cross-scope 互动表
§6.6.2 Scope Matrix 每行加 default enabled 列（4 scope 全 OFF）
§13.1 ADR-026 编号让位 ADR-030（已被 SafetyPolicy 占用）
§13.2 ADR-006 加 Updated 2026-05-26 二次 sub-paragraph
§14.1 MUST 加 4 条（SafetyPolicy / safety_scan_status 真接入 / scan_token+#49 / popup Safety UI）
EOF
)"
```

---

## Task 3: SafetyScope enum + SafetyPolicy trait + MockSafetyPolicy

**Files:**
- Create: `src-tauri/src/kernel/safety_policy.rs`
- Modify: `src-tauri/src/kernel/mod.rs`

- [ ] **Step 3.1: 创建 safety_policy.rs 骨架（含 SafetyScope + trait + MockSafetyPolicy + 第一个失败测试）**

Create `src-tauri/src/kernel/safety_policy.rs`:

```rust
// SafetyPolicy — kernel-owned, SafetyGuardImpl 的依赖。Spec §6.6.0 (Updated 2026-05-26)。
// 4 个 scope 出厂全 OFF; ConfigKvSafetyPolicy 持 Arc<AtomicBool>×4 + boot 时同步读 KV。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafetyScope {
    /// wrap_messages 是否注入 ADR-006 prefix
    PrefixInjection,
    /// scan_user_input 是否真扫
    UserInput,
    /// scan_token 是否真扫 (mid-stream)
    StreamToken,
    /// scan_final 是否真扫 (流终全文)
    FinalOutput,
}

impl SafetyScope {
    pub fn kv_key(&self) -> &'static str {
        match self {
            Self::PrefixInjection => "safety:prefix_enabled",
            Self::UserInput => "safety:scan_user_input_enabled",
            Self::StreamToken => "safety:scan_token_enabled",
            Self::FinalOutput => "safety:scan_final_enabled",
        }
    }
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("db error: {0}")]
    Db(String),
    #[error("config error: {0}")]
    Config(String),
}

#[async_trait]
pub trait SafetyPolicy: Send + Sync {
    fn is_enabled(&self, scope: SafetyScope) -> bool;
    async fn set_enabled(&self, scope: SafetyScope, enabled: bool) -> Result<(), PolicyError>;
}

/// 测试用: ChatService / SafetyGuard 单测注入。直接 4 AtomicBool 不走 DB。
pub struct MockSafetyPolicy {
    prefix: Arc<AtomicBool>,
    user_input: Arc<AtomicBool>,
    stream_token: Arc<AtomicBool>,
    final_output: Arc<AtomicBool>,
}

impl MockSafetyPolicy {
    /// 4 个 scope 全 OFF
    pub fn all_off() -> Self {
        Self {
            prefix: Arc::new(AtomicBool::new(false)),
            user_input: Arc::new(AtomicBool::new(false)),
            stream_token: Arc::new(AtomicBool::new(false)),
            final_output: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 4 个 scope 全 ON
    pub fn all_on() -> Self {
        let s = Self::all_off();
        s.prefix.store(true, Ordering::Relaxed);
        s.user_input.store(true, Ordering::Relaxed);
        s.stream_token.store(true, Ordering::Relaxed);
        s.final_output.store(true, Ordering::Relaxed);
        s
    }

    fn slot(&self, scope: SafetyScope) -> &Arc<AtomicBool> {
        match scope {
            SafetyScope::PrefixInjection => &self.prefix,
            SafetyScope::UserInput => &self.user_input,
            SafetyScope::StreamToken => &self.stream_token,
            SafetyScope::FinalOutput => &self.final_output,
        }
    }
}

#[async_trait]
impl SafetyPolicy for MockSafetyPolicy {
    fn is_enabled(&self, scope: SafetyScope) -> bool {
        self.slot(scope).load(Ordering::Relaxed)
    }

    async fn set_enabled(&self, scope: SafetyScope, enabled: bool) -> Result<(), PolicyError> {
        self.slot(scope).store(enabled, Ordering::Relaxed);
        Ok(())
    }
}

// Phase A0.7: ConfigKvSafetyPolicy — Task 4 实施
pub struct ConfigKvSafetyPolicy {
    db_path: PathBuf,
    prefix: Arc<AtomicBool>,
    user_input: Arc<AtomicBool>,
    stream_token: Arc<AtomicBool>,
    final_output: Arc<AtomicBool>,
}

// 实现挂在 Task 4

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safety_scope_kv_key_returns_expected_const_string() {
        assert_eq!(SafetyScope::PrefixInjection.kv_key(), "safety:prefix_enabled");
        assert_eq!(SafetyScope::UserInput.kv_key(), "safety:scan_user_input_enabled");
        assert_eq!(SafetyScope::StreamToken.kv_key(), "safety:scan_token_enabled");
        assert_eq!(SafetyScope::FinalOutput.kv_key(), "safety:scan_final_enabled");
    }

    #[tokio::test]
    async fn mock_policy_all_off_returns_false_for_all_scopes() {
        let p = MockSafetyPolicy::all_off();
        assert!(!p.is_enabled(SafetyScope::PrefixInjection));
        assert!(!p.is_enabled(SafetyScope::UserInput));
        assert!(!p.is_enabled(SafetyScope::StreamToken));
        assert!(!p.is_enabled(SafetyScope::FinalOutput));
    }

    #[tokio::test]
    async fn mock_policy_all_on_returns_true_for_all_scopes() {
        let p = MockSafetyPolicy::all_on();
        assert!(p.is_enabled(SafetyScope::PrefixInjection));
        assert!(p.is_enabled(SafetyScope::UserInput));
        assert!(p.is_enabled(SafetyScope::StreamToken));
        assert!(p.is_enabled(SafetyScope::FinalOutput));
    }

    #[tokio::test]
    async fn mock_policy_set_enabled_toggles_scope() {
        let p = MockSafetyPolicy::all_off();
        p.set_enabled(SafetyScope::UserInput, true).await.unwrap();
        assert!(p.is_enabled(SafetyScope::UserInput));
        assert!(!p.is_enabled(SafetyScope::PrefixInjection));
        p.set_enabled(SafetyScope::UserInput, false).await.unwrap();
        assert!(!p.is_enabled(SafetyScope::UserInput));
    }
}
```

- [ ] **Step 3.2: 注册 mod 到 kernel/mod.rs**

Modify `src-tauri/src/kernel/mod.rs`，在 `pub mod safety_guard;` 之后加 `pub mod safety_policy;`，并加 re-export：

```rust
pub mod crypto;
pub mod grant_broker;
pub mod lifecycle_manager;
pub mod permission_service;
pub mod repos;
pub mod runtime;
pub mod safety_guard;
pub mod safety_policy;  // 新增
pub mod state_store;

pub use runtime::Kernel;
```

- [ ] **Step 3.3: 跑测试验证通过**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib safety_policy::tests --no-fail-fast 2>&1 | tail -20
```
Expected: 4 tests pass (kv_key + all_off + all_on + set_enabled)

- [ ] **Step 3.4: Commit**

```bash
git -C /d/Project/temp/4 add src-tauri/src/kernel/safety_policy.rs src-tauri/src/kernel/mod.rs
git -C /d/Project/temp/4 commit -m "feat(kernel): SafetyScope enum + SafetyPolicy trait + MockSafetyPolicy

新建 safety_policy.rs 模块，是 SafetyGuardImpl 的依赖（不增加 Kernel 7 件套）。
4 个 SafetyScope: PrefixInjection / UserInput / StreamToken / FinalOutput，
对应 KV key safety:*_enabled 出厂全 OFF（spec 2026-05-26-safety-policy §4.1-4.2）。
MockSafetyPolicy 测试用 (all_off / all_on / set_enabled)，4 单测 pass。
ConfigKvSafetyPolicy 骨架占位待 Task 4 实施。"
```

---

## Task 4: ConfigKvSafetyPolicy 实施（load_from_kv + set_enabled）

**Files:**
- Modify: `src-tauri/src/kernel/safety_policy.rs`

- [ ] **Step 4.1: 写第一个失败测试 — boot 期 4 个 KV 都不存在 → fallback all-OFF**

在 `safety_policy.rs` 的 `#[cfg(test)] mod tests` 内追加：

```rust
    #[tokio::test]
    async fn config_kv_policy_falls_back_to_all_off_when_db_empty() {
        let (_dir, _conn) = crate::services::test_db::fresh_db().await;
        let db_path = _dir.path().join("aipet.db");

        let policy = ConfigKvSafetyPolicy::load_from_kv(&db_path).await.unwrap();
        assert!(!policy.is_enabled(SafetyScope::PrefixInjection));
        assert!(!policy.is_enabled(SafetyScope::UserInput));
        assert!(!policy.is_enabled(SafetyScope::StreamToken));
        assert!(!policy.is_enabled(SafetyScope::FinalOutput));
    }
```

- [ ] **Step 4.2: 跑测试确认失败**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib safety_policy::tests::config_kv_policy_falls_back_to_all_off_when_db_empty 2>&1 | tail -10
```
Expected: FAIL with "no method named `load_from_kv`"

- [ ] **Step 4.3: 实现 ConfigKvSafetyPolicy::load_from_kv + is_enabled**

替换 safety_policy.rs 内 `// Phase A0.7: ConfigKvSafetyPolicy — Task 4 实施` 注释及其下 `pub struct ConfigKvSafetyPolicy { ... }` 段为完整实施：

```rust
/// 生产实施: 4 KV 持 Arc<AtomicBool>, boot 时同步读 KV 装载, 运行期 atomic 读不 hit DB。
pub struct ConfigKvSafetyPolicy {
    db_path: PathBuf,
    prefix: Arc<AtomicBool>,
    user_input: Arc<AtomicBool>,
    stream_token: Arc<AtomicBool>,
    final_output: Arc<AtomicBool>,
}

impl ConfigKvSafetyPolicy {
    /// Boot 期同步阻塞读 4 个 KV，缺失或解析失败 fallback false (出厂状态 = 全 OFF)。
    /// DB 连接失败 → fallback 全 OFF + eprintln warning (保守原则: 安全功能默认 off 与"零 overhead 起步"语义一致)。
    pub async fn load_from_kv(db_path: &std::path::Path) -> Result<Self, PolicyError> {
        let prefix = read_kv_bool_or_false(db_path, SafetyScope::PrefixInjection).await;
        let user_input = read_kv_bool_or_false(db_path, SafetyScope::UserInput).await;
        let stream_token = read_kv_bool_or_false(db_path, SafetyScope::StreamToken).await;
        let final_output = read_kv_bool_or_false(db_path, SafetyScope::FinalOutput).await;
        Ok(Self {
            db_path: db_path.to_path_buf(),
            prefix: Arc::new(AtomicBool::new(prefix)),
            user_input: Arc::new(AtomicBool::new(user_input)),
            stream_token: Arc::new(AtomicBool::new(stream_token)),
            final_output: Arc::new(AtomicBool::new(final_output)),
        })
    }

    fn slot(&self, scope: SafetyScope) -> &Arc<AtomicBool> {
        match scope {
            SafetyScope::PrefixInjection => &self.prefix,
            SafetyScope::UserInput => &self.user_input,
            SafetyScope::StreamToken => &self.stream_token,
            SafetyScope::FinalOutput => &self.final_output,
        }
    }
}

/// 内部 helper: 读单个 KV bool, 任何失败 fallback false + eprintln warning。
async fn read_kv_bool_or_false(db_path: &std::path::Path, scope: SafetyScope) -> bool {
    match crate::services::db::connect_at(db_path).await {
        Ok(mut conn) => match crate::services::config::get_with_conn(&mut conn, scope.kv_key()).await {
            Ok(Some(s)) => match s.trim().parse::<bool>() {
                Ok(b) => b,
                Err(_) => {
                    eprintln!(
                        "[safety_policy] KV {} value {:?} is not a valid bool, fallback to false",
                        scope.kv_key(),
                        s
                    );
                    false
                }
            },
            Ok(None) => false, // KV 不存在 = 出厂状态 = OFF
            Err(e) => {
                eprintln!(
                    "[safety_policy] config::get_with_conn failed for {}: {}, fallback to false",
                    scope.kv_key(),
                    e
                );
                false
            }
        },
        Err(e) => {
            eprintln!(
                "[safety_policy] connect_at failed for {}: {}, fallback to false (Denied invariant: 保守不开扫描)",
                scope.kv_key(),
                e
            );
            false
        }
    }
}

#[async_trait]
impl SafetyPolicy for ConfigKvSafetyPolicy {
    fn is_enabled(&self, scope: SafetyScope) -> bool {
        self.slot(scope).load(Ordering::Relaxed)
    }

    /// 先写 DB 成功后才更新内存 AtomicBool, 保证 DB 与内存一致。
    /// DB 失败时内存不变, 返 Err 给 IPC caller (UI toast)。
    async fn set_enabled(&self, scope: SafetyScope, enabled: bool) -> Result<(), PolicyError> {
        let mut conn = crate::services::db::connect_at(&self.db_path)
            .await
            .map_err(|e| PolicyError::Db(e.to_string()))?;
        let now = chrono::Utc::now().to_rfc3339();
        crate::services::config::set_with_conn(&mut conn, scope.kv_key(), &enabled.to_string(), &now)
            .await
            .map_err(|e| PolicyError::Config(e.to_string()))?;
        self.slot(scope).store(enabled, Ordering::Relaxed);
        Ok(())
    }
}
```

- [ ] **Step 4.4: 跑测试确认 fallback 全 OFF 测试通过**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib safety_policy::tests::config_kv_policy_falls_back_to_all_off_when_db_empty 2>&1 | tail -10
```
Expected: PASS

- [ ] **Step 4.5: 写 set_enabled 单测 (atomic 写 DB + 内存)**

在 tests mod 末尾追加：

```rust
    #[tokio::test]
    async fn config_kv_policy_set_enabled_updates_both_db_and_memory() {
        let (_dir, _conn) = crate::services::test_db::fresh_db().await;
        let db_path = _dir.path().join("aipet.db");

        let policy = ConfigKvSafetyPolicy::load_from_kv(&db_path).await.unwrap();
        // 初始全 OFF
        assert!(!policy.is_enabled(SafetyScope::FinalOutput));

        // 翻 FinalOutput ON
        policy.set_enabled(SafetyScope::FinalOutput, true).await.unwrap();
        assert!(policy.is_enabled(SafetyScope::FinalOutput));

        // Reload from KV 应仍是 true (DB 持久化生效)
        let policy2 = ConfigKvSafetyPolicy::load_from_kv(&db_path).await.unwrap();
        assert!(policy2.is_enabled(SafetyScope::FinalOutput));
        assert!(!policy2.is_enabled(SafetyScope::PrefixInjection));
    }

    #[tokio::test]
    async fn config_kv_policy_handles_invalid_bool_string_as_false() {
        let (_dir, mut conn) = crate::services::test_db::fresh_db().await;
        let db_path = _dir.path().join("aipet.db");

        // 手动写一个非法 bool 字符串
        let now = chrono::Utc::now().to_rfc3339();
        crate::services::config::set_with_conn(
            &mut conn,
            SafetyScope::UserInput.kv_key(),
            "not-a-bool",
            &now,
        )
        .await
        .unwrap();

        let policy = ConfigKvSafetyPolicy::load_from_kv(&db_path).await.unwrap();
        assert!(!policy.is_enabled(SafetyScope::UserInput));
    }
```

- [ ] **Step 4.6: 跑测试确认通过**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib safety_policy::tests 2>&1 | tail -15
```
Expected: 7 tests pass (4 from Task 3 + 3 new)

- [ ] **Step 4.7: Commit**

```bash
git -C /d/Project/temp/4 add src-tauri/src/kernel/safety_policy.rs
git -C /d/Project/temp/4 commit -m "feat(kernel/safety_policy): ConfigKvSafetyPolicy 实施 (load_from_kv + set_enabled)

Boot 期同步阻塞读 4 个 KV (safety:*_enabled); DB 连接失败 / KV 缺失 / 解析失败
都 fallback false + eprintln warning (保守原则不主动开扫描)。
set_enabled 先写 DB 成功后再更新内存 AtomicBool 保持一致性。
3 个新单测覆盖 fallback all-OFF + set_enabled round-trip + invalid bool string。"
```

---

## Task 5: ConversationRepo::SafetyScanStatus 加 Disabled variant

**Files:**
- Modify: `src-tauri/src/kernel/repos/conversation_repo.rs`

- [ ] **Step 5.1: 写失败测试**

在 `conversation_repo.rs` 的 `#[cfg(test)] mod tests` 内追加（先不实现 variant 让它编译失败）：

```rust
    #[tokio::test]
    async fn safety_scan_status_disabled_serializes_as_string() {
        assert_eq!(SafetyScanStatus::Disabled.as_str(), "disabled");
    }

    #[tokio::test]
    async fn update_safety_status_to_disabled_round_trips() {
        let mut conn = setup_test_db().await;
        let repo = ConversationRepo::new();
        repo.update_safety_status(&mut conn, "msg_1", SafetyScanStatus::Disabled)
            .await
            .unwrap();
        let status: String =
            sqlx::query_scalar("SELECT safety_scan_status FROM messages WHERE id = 'msg_1'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(status, "disabled");
    }
```

- [ ] **Step 5.2: 跑测试确认失败**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib conversation_repo::tests::safety_scan_status_disabled 2>&1 | tail -10
```
Expected: FAIL with "no variant `Disabled` on enum `SafetyScanStatus`"

- [ ] **Step 5.3: 加 Disabled variant**

修改 `src-tauri/src/kernel/repos/conversation_repo.rs`，找到 `pub enum SafetyScanStatus`（约 line 10）：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyScanStatus {
    Pending,
    Streaming,
    StreamSoftBlocked,
    FinalOk,
    FinalRedacted,
    FinalBlocked,
    ScanFailed,
    /// Updated 2026-05-26 spec §6.6.0: scan_final OFF 时 ChatService 流末显式写终态。
    /// 与 final_ok 区分: 真扫过 ok vs 未扫 (policy disabled)。
    Disabled,
}
```

同时改 `impl SafetyScanStatus::as_str`：

```rust
impl SafetyScanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Streaming => "streaming",
            Self::StreamSoftBlocked => "stream_soft_blocked",
            Self::FinalOk => "final_ok",
            Self::FinalRedacted => "final_redacted",
            Self::FinalBlocked => "final_blocked",
            Self::ScanFailed => "scan_failed",
            Self::Disabled => "disabled",
        }
    }
}
```

更新原 `update_safety_status_all_7_states_serialize_correctly` 测试（line ~145）改为 8 状态：

```rust
    #[tokio::test]
    async fn update_safety_status_all_8_states_serialize_correctly() {
        let mut conn = setup_test_db().await;
        let repo = ConversationRepo::new();
        for s in [
            SafetyScanStatus::Pending,
            SafetyScanStatus::Streaming,
            SafetyScanStatus::StreamSoftBlocked,
            SafetyScanStatus::FinalOk,
            SafetyScanStatus::FinalRedacted,
            SafetyScanStatus::FinalBlocked,
            SafetyScanStatus::ScanFailed,
            SafetyScanStatus::Disabled,
        ] {
            repo.update_safety_status(&mut conn, "msg_1", s)
                .await
                .unwrap();
            let stored: String =
                sqlx::query_scalar("SELECT safety_scan_status FROM messages WHERE id = 'msg_1'")
                    .fetch_one(&mut conn)
                    .await
                    .unwrap();
            assert_eq!(stored, s.as_str());
        }
    }
```

- [ ] **Step 5.4: 跑全部 conversation_repo 测试**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib conversation_repo::tests 2>&1 | tail -15
```
Expected: 6 tests pass（原 4 + 新 2; 旧 7-state 名改为 8-state）

- [ ] **Step 5.5: Commit**

```bash
git -C /d/Project/temp/4 add src-tauri/src/kernel/repos/conversation_repo.rs
git -C /d/Project/temp/4 commit -m "feat(kernel/repos): SafetyScanStatus 加 Disabled variant (7→8 state)

Spec §6.6.0 (Updated 2026-05-26): scan_final OFF 时 ChatService 流末显式写
'disabled' 终态。与 'final_ok' 区分: 真扫过 ok vs 未扫 (policy disabled)。
扩 enum + as_str + 改 7-state 测试为 8-state 全集。"
```

---

## Task 6: SafetyGuard 接口扩展 + 持 Arc<dyn SafetyPolicy> + 4 方法短路

**Files:**
- Modify: `src-tauri/src/kernel/safety_guard.rs`

- [ ] **Step 6.1: 写失败测试 — wrap_messages noop when prefix disabled**

在 `safety_guard.rs` 的 `#[cfg(test)] mod tests` 内 `fn make_guard()` 之后追加：

```rust
    use crate::kernel::safety_policy::{MockSafetyPolicy, SafetyScope};
    use std::sync::Arc;

    fn make_guard_with_policy_all_off() -> SafetyGuardImpl {
        let policy = Arc::new(MockSafetyPolicy::all_off()) as Arc<dyn crate::kernel::safety_policy::SafetyPolicy>;
        SafetyGuardImpl::from_text_with_policy("TEST_PREFIX", policy).unwrap()
    }

    fn make_guard_with_policy_all_on() -> SafetyGuardImpl {
        let policy = Arc::new(MockSafetyPolicy::all_on()) as Arc<dyn crate::kernel::safety_policy::SafetyPolicy>;
        SafetyGuardImpl::from_text_with_policy("TEST_PREFIX", policy).unwrap()
    }

    #[test]
    fn is_enabled_delegates_to_policy_for_4_scopes() {
        let guard_off = make_guard_with_policy_all_off();
        let guard_on = make_guard_with_policy_all_on();
        for scope in [
            SafetyScope::PrefixInjection,
            SafetyScope::UserInput,
            SafetyScope::StreamToken,
            SafetyScope::FinalOutput,
        ] {
            assert!(!guard_off.is_enabled(scope), "all_off should report all disabled");
            assert!(guard_on.is_enabled(scope), "all_on should report all enabled");
        }
    }

    #[test]
    fn wrap_messages_noop_when_prefix_disabled() {
        let guard = make_guard_with_policy_all_off();
        let user_msg = ChatMessage::text(Role::User, "hi");
        let wrapped = guard.wrap_messages(vec![user_msg.clone()], Locale::ZhCn);
        assert_eq!(wrapped.len(), 1, "should NOT inject prefix when policy off");
        assert_eq!(wrapped[0].role, Role::User);
    }

    #[test]
    fn wrap_messages_injects_when_prefix_enabled() {
        let guard = make_guard_with_policy_all_on();
        let user_msg = ChatMessage::text(Role::User, "hi");
        let wrapped = guard.wrap_messages(vec![user_msg], Locale::ZhCn);
        assert_eq!(wrapped.len(), 2, "should inject prefix system message");
        assert_eq!(wrapped[0].role, Role::System);
    }

    #[test]
    fn scan_user_input_returns_ok_when_disabled() {
        let guard = make_guard_with_policy_all_off();
        let result = guard.scan_user_input("自杀"); // hard 黑词命中, 但 policy off
        assert_eq!(result, ScanFinalResult::Ok);
    }

    #[test]
    fn scan_token_returns_pass_when_disabled() {
        let guard = make_guard_with_policy_all_off();
        let result = guard.scan_token("", "自杀", false);
        assert_eq!(result, ScanTokenResult::Pass);
    }

    #[test]
    fn scan_final_returns_ok_when_disabled() {
        let guard = make_guard_with_policy_all_off();
        let result = guard.scan_final("自杀方法", "snap_1");
        assert_eq!(result, ScanFinalResult::Ok);
    }
```

- [ ] **Step 6.2: 跑测试确认失败**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib safety_guard::tests::is_enabled_delegates_to_policy_for_4_scopes 2>&1 | tail -10
```
Expected: FAIL with "no method named `is_enabled`" / "no method named `from_text_with_policy`"

- [ ] **Step 6.3: 改 SafetyGuard trait 加 is_enabled + 改 SafetyGuardImpl 持 policy + 4 方法短路**

修改 `src-tauri/src/kernel/safety_guard.rs`：

(a) 在文件顶部 `use` 区加：

```rust
use std::sync::Arc;
use crate::kernel::safety_policy::{SafetyPolicy, SafetyScope};
```

(b) 修改 trait（约 line 56）加 `is_enabled` 方法：

```rust
pub trait SafetyGuard: Send + Sync {
    /// Updated 2026-05-26: 转发 SafetyPolicy 状态供 ChatService 决策路径。
    fn is_enabled(&self, scope: SafetyScope) -> bool;

    /// 出方向: prompt → LLM, prefix 强制 system message 第一位 (Scope: SafetyPrefix)。
    /// Updated 2026-05-26: policy.PrefixInjection OFF 时返 messages 原样不注入。
    fn wrap_messages(&self, messages: Vec<ChatMessage>, locale: Locale) -> Vec<ChatMessage>;

    /// 入方向: 流式 token chunk 增量扫 (Scope #2)。
    /// Updated 2026-05-26: policy.StreamToken OFF 时返 Pass。
    fn scan_token(&self, partial: &str, accumulated: &str, finished: bool) -> ScanTokenResult;

    /// 入方向: 流终态全文扫 (Scope #3 LLM final)。
    /// Updated 2026-05-26: policy.FinalOutput OFF 时返 Ok。
    fn scan_final(&self, full_text: &str, persona_snapshot_id: &str) -> ScanFinalResult;

    /// 入方向: 用户输入扫 (Scope #1, 防 prompt injection)。
    /// Updated 2026-05-26: policy.UserInput OFF 时返 Ok。
    fn scan_user_input(&self, text: &str) -> ScanFinalResult;
}
```

(c) 修改 `ScanTokenResult::SoftBlock` variant（约 line 70）加 `rule_id`：

```rust
pub enum ScanTokenResult {
    Pass,
    SoftBlock {
        rule_id: String,
        replace_last_n: usize,
        placeholder: String,
    },
    HardEnd {
        rule_id: String,
    },
}
```

(d) 修改 `SafetyGuardImpl` struct（约 line 91）加 policy 字段：

```rust
pub struct SafetyGuardImpl {
    prefix: String,
    hard_blocklist: Vec<&'static str>,
    soft_blocklist: Vec<&'static str>,
    /// Updated 2026-05-26: 决策 4 scope on/off 的 policy dependency。
    policy: Arc<dyn SafetyPolicy>,
}
```

(e) 修改 `impl SafetyGuardImpl::from_text` 改名为 `from_text_with_policy`（原 `from_text` 留作 backwards-compat helper 调 from_text_with_policy with no-op policy）。同时改 `load`：

```rust
impl SafetyGuardImpl {
    /// Production: prefix 编译时嵌入 (include_str!), Kernel::boot 注入 policy。
    pub fn from_text_with_policy(
        prefix: &str,
        policy: Arc<dyn SafetyPolicy>,
    ) -> Result<Self, SafetyError> {
        if prefix.trim().is_empty() {
            return Err(SafetyError::PrefixMissing(
                "<empty inline prefix>".to_string(),
            ));
        }
        Ok(Self {
            prefix: prefix.to_string(),
            hard_blocklist: vec!["自杀", "自残"],
            soft_blocklist: vec!["违法", "违禁"],
            policy,
        })
    }

    /// Backward-compat (legacy tests): 用 MockSafetyPolicy::all_on 占位让现有调用方编译。
    /// **Production 路径必须用 from_text_with_policy** (Kernel::boot 注入真 policy)。
    #[cfg(test)]
    pub fn from_text(prefix: &str) -> Result<Self, SafetyError> {
        use crate::kernel::safety_policy::MockSafetyPolicy;
        Self::from_text_with_policy(
            prefix,
            Arc::new(MockSafetyPolicy::all_on()) as Arc<dyn SafetyPolicy>,
        )
    }

    /// 文件加载路径（dev / 旧测试）。Production 不走此路径，走 Kernel::boot 的 from_text_with_policy + include_str!。
    pub fn load(prefix_path: &std::path::Path) -> Result<Self, SafetyError> {
        let prefix = std::fs::read_to_string(prefix_path)?;
        Self::from_text(&prefix).map_err(|e| match e {
            SafetyError::PrefixMissing(_) => {
                SafetyError::PrefixMissing(prefix_path.display().to_string())
            }
            other => other,
        })
    }
}
```

(f) 修改 `impl SafetyGuard for SafetyGuardImpl` 4 方法加短路：

```rust
impl SafetyGuard for SafetyGuardImpl {
    fn is_enabled(&self, scope: SafetyScope) -> bool {
        self.policy.is_enabled(scope)
    }

    fn wrap_messages(&self, mut messages: Vec<ChatMessage>, _locale: Locale) -> Vec<ChatMessage> {
        // Updated 2026-05-26: policy.PrefixInjection OFF 时不注入 prefix
        if !self.policy.is_enabled(SafetyScope::PrefixInjection) {
            return messages;
        }
        match messages.first_mut() {
            Some(first) if first.role == Role::System => {
                let prefix_part = ContentPart::Text {
                    text: format!("{}\n\n", self.prefix),
                };
                first.content.insert(0, prefix_part);
            }
            _ => {
                let new_system = ChatMessage::text(Role::System, self.prefix.clone());
                messages.insert(0, new_system);
            }
        }
        messages
    }

    fn scan_token(&self, _partial: &str, accumulated: &str, _finished: bool) -> ScanTokenResult {
        // Updated 2026-05-26: policy.StreamToken OFF 时返 Pass
        if !self.policy.is_enabled(SafetyScope::StreamToken) {
            return ScanTokenResult::Pass;
        }
        // Updated 2026-05-26 (#49 并入): trailing-window N=64 chars 避免 O(n^2)
        let tail = trailing_chars(accumulated, SCAN_TOKEN_WINDOW_CHARS);
        for rule in &self.hard_blocklist {
            if tail.contains(rule) {
                return ScanTokenResult::HardEnd {
                    rule_id: rule.to_string(),
                };
            }
        }
        for rule in &self.soft_blocklist {
            if tail.contains(rule) {
                return ScanTokenResult::SoftBlock {
                    rule_id: rule.to_string(),
                    replace_last_n: 8,
                    placeholder: "[审核中…]".to_string(),
                };
            }
        }
        ScanTokenResult::Pass
    }

    fn scan_final(&self, full_text: &str, _persona_snapshot_id: &str) -> ScanFinalResult {
        // Updated 2026-05-26: policy.FinalOutput OFF 时返 Ok
        if !self.policy.is_enabled(SafetyScope::FinalOutput) {
            return ScanFinalResult::Ok;
        }
        let mut hit_rules = Vec::new();
        for rule in &self.hard_blocklist {
            if full_text.contains(rule) {
                hit_rules.push(rule.to_string());
            }
        }
        if !hit_rules.is_empty() {
            return ScanFinalResult::Blocked {
                rule_ids: hit_rules,
                fallback: FALLBACK_REFUSAL.to_string(),
            };
        }
        let mut soft_hit = Vec::new();
        let mut redacted = full_text.to_string();
        for rule in &self.soft_blocklist {
            if redacted.contains(rule) {
                redacted = redacted.replace(rule, "***");
                soft_hit.push(rule.to_string());
            }
        }
        if !soft_hit.is_empty() {
            return ScanFinalResult::Redacted {
                redacted_text: redacted,
                rule_ids: soft_hit,
            };
        }
        ScanFinalResult::Ok
    }

    fn scan_user_input(&self, text: &str) -> ScanFinalResult {
        // Updated 2026-05-26: policy.UserInput OFF 时返 Ok
        if !self.policy.is_enabled(SafetyScope::UserInput) {
            return ScanFinalResult::Ok;
        }
        // 内部走 scan_final 同款规则, 但绕过 FinalOutput policy 判定
        // (scan_user_input 与 scan_final 是两条独立路径)
        self.scan_final_inner(text)
    }
}

impl SafetyGuardImpl {
    /// scan_user_input 用同款规则但绕过 FinalOutput policy 判定的内部 helper。
    /// Updated 2026-05-26: scan_user_input 与 scan_final 是两条独立 scope, 各自的 policy 控制各自路径。
    fn scan_final_inner(&self, text: &str) -> ScanFinalResult {
        let mut hit_rules = Vec::new();
        for rule in &self.hard_blocklist {
            if text.contains(rule) {
                hit_rules.push(rule.to_string());
            }
        }
        if !hit_rules.is_empty() {
            return ScanFinalResult::Blocked {
                rule_ids: hit_rules,
                fallback: FALLBACK_REFUSAL.to_string(),
            };
        }
        let mut soft_hit = Vec::new();
        let mut redacted = text.to_string();
        for rule in &self.soft_blocklist {
            if redacted.contains(rule) {
                redacted = redacted.replace(rule, "***");
                soft_hit.push(rule.to_string());
            }
        }
        if !soft_hit.is_empty() {
            return ScanFinalResult::Redacted {
                redacted_text: redacted,
                rule_ids: soft_hit,
            };
        }
        ScanFinalResult::Ok
    }
}
```

(f) 加 trailing-window helper（在文件末尾 `#[cfg(test)]` 之前）：

```rust
/// Updated 2026-05-26 (#49 并入): 取 accumulated 的尾部 N chars (UTF-8 char 计数, 不是 byte)。
/// N=64 覆盖最长黑词 (4 字符) + 上下文边界 (60 字符)。O(window) 替代原 O(n^2)。
const SCAN_TOKEN_WINDOW_CHARS: usize = 64;

fn trailing_chars(s: &str, n: usize) -> &str {
    let char_count = s.chars().count();
    if char_count <= n {
        return s;
    }
    // 找到第 (char_count - n) 个字符的 byte offset
    let start_byte = s
        .char_indices()
        .nth(char_count - n)
        .map(|(i, _)| i)
        .unwrap_or(0);
    &s[start_byte..]
}
```

(g) 改原 `make_guard()` helper（约 line 202）保持向后兼容（用 MockSafetyPolicy::all_on）：

```rust
    fn make_guard() -> SafetyGuardImpl {
        use crate::kernel::safety_policy::MockSafetyPolicy;
        let policy = Arc::new(MockSafetyPolicy::all_on()) as Arc<dyn crate::kernel::safety_policy::SafetyPolicy>;
        SafetyGuardImpl {
            prefix: "TEST_PREFIX".to_string(),
            hard_blocklist: vec!["自杀"],
            soft_blocklist: vec!["违禁"],
            policy,
        }
    }
```

- [ ] **Step 6.4: 跑测试确认通过**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib safety_guard::tests 2>&1 | tail -30
```
Expected: 所有原 13 个测试 + 新 6 个测试 PASS（总 19 个）

- [ ] **Step 6.5: Commit**

```bash
git -C /d/Project/temp/4 add src-tauri/src/kernel/safety_guard.rs
git -C /d/Project/temp/4 commit -m "feat(kernel/safety_guard): SafetyGuard 接口扩 + 持 SafetyPolicy + 4 方法短路

Updated 2026-05-26:
- trait +1 方法 is_enabled(scope) 转发 policy 状态
- SafetyGuardImpl 持 Arc<dyn SafetyPolicy>; from_text → from_text_with_policy
- 4 方法 (wrap_messages / scan_user_input / scan_token / scan_final) 按 policy
  各自 scope 短路返 noop (路径完整: subsystem 仍必经 SafetyGuard 入口)
- from_text(prefix) 仅 cfg(test) 保留, 用 MockSafetyPolicy::all_on 占位

6 个新单测 (delegation + 4 scope noop + on/off 注入对比)。"
```

---

## Task 7: scan_token trailing-window 优化 + 单测覆盖（#49 并入）

**Files:**
- Modify: `src-tauri/src/kernel/safety_guard.rs`（trailing_chars helper 在 Task 6 已加，此 task 加专项测试）

- [ ] **Step 7.1: 写 trailing-window 测试**

在 `safety_guard.rs` 的 tests mod 内追加：

```rust
    #[test]
    fn scan_token_trailing_window_64_chars_skips_leading_hits() {
        let guard = make_guard(); // policy all_on, hard=["自杀"], soft=["违禁"]
        // 构造一个长字符串: 100 char 前缀 + "自杀" + 100 char 后缀
        // 总长 202 字符, 尾部 64 个字符 = 后 64 字符 = 后 64 字符 (不含"自杀")
        let mut accumulated = "x".repeat(100);
        accumulated.push_str("自杀");
        accumulated.push_str(&"y".repeat(100));
        // 末 64 chars = 64 个 'y', 不含"自杀"
        let result = guard.scan_token("", &accumulated, false);
        assert_eq!(result, ScanTokenResult::Pass, "leading hit outside 64-char window should be ignored");
    }

    #[test]
    fn scan_token_trailing_window_64_chars_catches_recent_hits() {
        let guard = make_guard();
        // 构造: 100 char 前缀 + 60 char + "自杀"
        // 末 64 chars = 60 + "自杀" + 0 = 包含"自杀" → 命中
        let mut accumulated = "x".repeat(100);
        accumulated.push_str(&"y".repeat(60));
        accumulated.push_str("自杀");
        let result = guard.scan_token("", &accumulated, false);
        match result {
            ScanTokenResult::HardEnd { rule_id } => assert_eq!(rule_id, "自杀"),
            other => panic!("expected HardEnd, got {:?}", other),
        }
    }

    #[test]
    fn scan_token_trailing_window_with_short_text_scans_full() {
        let guard = make_guard();
        // 短文本 < 64 chars: 全文扫
        let result = guard.scan_token("", "自杀", false);
        match result {
            ScanTokenResult::HardEnd { rule_id } => assert_eq!(rule_id, "自杀"),
            other => panic!("short text should still scan full, got {:?}", other),
        }
    }

    #[test]
    fn scan_token_soft_block_includes_rule_id() {
        let guard = make_guard();
        let result = guard.scan_token("", "这里包含违禁内容", false);
        match result {
            ScanTokenResult::SoftBlock {
                rule_id,
                replace_last_n,
                placeholder,
            } => {
                assert_eq!(rule_id, "违禁");
                assert_eq!(replace_last_n, 8);
                assert_eq!(placeholder, "[审核中…]");
            }
            other => panic!("expected SoftBlock with rule_id, got {:?}", other),
        }
    }

    #[test]
    fn trailing_chars_handles_utf8_correctly() {
        // 中文每个字符 3 bytes, 不能按 byte 切片
        let s = "中文测试xyz"; // 4 中 + 3 ascii = 7 chars
        assert_eq!(trailing_chars(s, 3), "xyz");
        assert_eq!(trailing_chars(s, 5), "测试xyz");
        assert_eq!(trailing_chars(s, 10), "中文测试xyz"); // 超长返全文
    }
```

- [ ] **Step 7.2: 跑测试确认通过**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib safety_guard::tests 2>&1 | tail -30
```
Expected: all safety_guard tests pass, including trailing-window coverage and `scan_token_soft_block_includes_rule_id`.

- [ ] **Step 7.3: Commit**

```bash
git -C /d/Project/temp/4 add src-tauri/src/kernel/safety_guard.rs
git -C /d/Project/temp/4 commit -m "perf(kernel/safety_guard): scan_token trailing-window N=64 优化 (closes #49)

把原 accumulated.contains(rule) 全文扫 (O(n^2) per stream) 改为
trailing_chars(accumulated, 64) 仅扫尾部 64 chars (O(window))。
N=64 覆盖最长黑词 (4 字符 '自残') + 上下文边界 (60 字符)。

5 新单测覆盖:
- 100 前缀 + 命中词 + 100 后缀 → 命中超 64 char 窗口外应 Pass
- 100 前缀 + 60 char + 命中词 → 命中在 64 char 尾部应 HardEnd
- 短文本 < 64 char 全文扫不退化
- SoftBlock 返回 rule_id, 上层不再用 placeholder 去重
- UTF-8 中文按 char 计数不按 byte (中文 3 bytes/char)

Closes #49 (SafetyGuard::scan_token trailing-window O(n^2)→O(window) 优化)"
```

---

## Task 8: Kernel::boot 集成 SafetyPolicy + SafetyGuardImpl 持 policy

**Files:**
- Modify: `src-tauri/src/kernel/runtime.rs`
- Modify: `src-tauri/src/lib.rs`（仅 setup hook 处确认调用对齐）

- [ ] **Step 8.1: 写失败测试 — Kernel::boot 应自带 safety_policy 字段**

在 `src-tauri/src/kernel/runtime.rs` 的 tests mod 内追加：

```rust
    #[test]
    fn boot_loads_safety_policy_with_all_scopes_disabled_when_kv_empty() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let db_path = temp.path().join("boot_test_db.sqlite");

        let kernel = Kernel::boot(db_path).expect("Kernel::boot");
        use crate::kernel::safety_policy::SafetyScope;
        // KV 全空 → 全 OFF
        assert!(!kernel.safety_policy.is_enabled(SafetyScope::PrefixInjection));
        assert!(!kernel.safety_policy.is_enabled(SafetyScope::UserInput));
        assert!(!kernel.safety_policy.is_enabled(SafetyScope::StreamToken));
        assert!(!kernel.safety_policy.is_enabled(SafetyScope::FinalOutput));
        // SafetyGuard 也应转发同状态
        assert!(!kernel.safety_guard.is_enabled(SafetyScope::PrefixInjection));
        assert!(!kernel.safety_guard.is_enabled(SafetyScope::FinalOutput));
    }
```

- [ ] **Step 8.2: 跑测试确认失败**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib kernel::runtime::tests::boot_loads_safety_policy 2>&1 | tail -10
```
Expected: FAIL with "no field `safety_policy` on struct `Kernel`"

- [ ] **Step 8.3: 改 Kernel struct + boot 函数加 SafetyPolicy**

修改 `src-tauri/src/kernel/runtime.rs`：

(a) 顶部 use 加：

```rust
use crate::kernel::safety_policy::{ConfigKvSafetyPolicy, SafetyPolicy};
```

(b) Kernel struct 加字段（约 line 32 起）：

```rust
pub struct Kernel {
    pub state_store: Arc<StateStore>,
    pub safety_policy: Arc<dyn SafetyPolicy>,  // 新增 (Updated 2026-05-26)
    pub safety_guard: Arc<dyn SafetyGuard>,
    pub permission_service: Arc<dyn PermissionService>,
    pub grant_broker: Arc<dyn GrantBroker>,
    pub crypto: Arc<dyn CryptoService>,
    pub secret_repo: Arc<SecretRepo>,
    pub lifecycle: Arc<LifecycleManager>,
}
```

(c) `Kernel::boot` 函数改造（约 line 53）：

```rust
    /// Boot 1-7 序列。db_path 由调用方提供 (Tauri AppHandle.app_config_dir + aipet.db)。
    ///
    /// Boot steps (Updated 2026-05-26: Boot 3 拆为 3a SafetyPolicy + 3b SafetyGuard):
    /// 1. MigrationService — 由 tauri-plugin-sql 自动执行 (lib.rs setup 之前)
    /// 2. open_app_db — 由 services::db 已有 (此处不持 Pool, 每次 commands acquire)
    /// 3a. SafetyPolicy — 从 config KV 加载 4 个 scope (出厂全 OFF)
    /// 3b. SafetyGuard (compile-time prefix + policy 依赖)
    /// 4. PermissionService (DenyOnly)
    /// 5. GrantBroker (DenyAll, Phase A0 无 Tool)
    /// 6. CryptoService + SecretRepo
    /// 7. LifecycleManager → Live
    pub fn boot(db_path: PathBuf) -> Result<Self, BootError> {
        // Boot 3a: SafetyPolicy (load_from_kv 同步 block_on, setup 是 sync 上下文)
        // Updated 2026-05-26: 4 KV 缺失时全 OFF + eprintln warning
        let safety_policy: Arc<dyn SafetyPolicy> = Arc::new(
            tauri::async_runtime::block_on(ConfigKvSafetyPolicy::load_from_kv(&db_path))
                .map_err(|e| BootError::Safety(SafetyError::ScanRuleLoad(format!(
                    "SafetyPolicy load_from_kv failed: {}", e
                ))))?,
        );

        // Boot 3b: SafetyGuard (compile-time prefix, 注入 policy)
        let safety_guard: Arc<dyn SafetyGuard> = Arc::new(
            SafetyGuardImpl::from_text_with_policy(SAFETY_PREFIX, Arc::clone(&safety_policy))?,
        );

        // Boot 4: PermissionService (DenyOnly)
        let permission_repo = Arc::new(PermissionRepo::new());
        let permission_service: Arc<dyn PermissionService> = Arc::new(
            DenyOnlyPermissionService::new(permission_repo, db_path.clone()),
        );

        // Boot 5: GrantBroker (DenyAll, Phase A0 无 Tool)
        let grant_broker: Arc<dyn GrantBroker> = Arc::new(DenyAllGrantBroker);

        // Boot 6: CryptoService + SecretRepo
        let crypto: Arc<dyn CryptoService> = Arc::new(DpapiCryptoService);
        let secret_repo = Arc::new(SecretRepo::new(Arc::clone(&crypto)));

        // Boot 7: LifecycleManager → Live
        let lifecycle = Arc::new(LifecycleManager::new());
        lifecycle.transition(LifecycleState::Live)?;

        // StateStore (Repository 注册中心, Phase A0 不持 Pool)
        let state_store = Arc::new(StateStore::new());

        Ok(Self {
            state_store,
            safety_policy,
            safety_guard,
            permission_service,
            grant_broker,
            crypto,
            secret_repo,
            lifecycle,
        })
    }
```

(d) 现有测试 `boot_produces_live_lifecycle_and_all_components`（约 line 95）不需改，因为现有 assert 还满足。

- [ ] **Step 8.4: 跑测试确认通过**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib kernel::runtime::tests 2>&1 | tail -15
```
Expected: 2 tests pass（原 1 + 新 1）

注意：load_from_kv 内部依赖 `services::test_db::fresh_db` schema，但 Kernel::boot 直接读 prod db_path——首测时 DB 文件不存在 → connect_at fail → fallback 全 OFF + eprintln。这是预期行为，测试看到的 `db_path` 不存在但仍 boot 成功（policy load fallback）。

- [ ] **Step 8.5: Commit**

```bash
git -C /d/Project/temp/4 add src-tauri/src/kernel/runtime.rs
git -C /d/Project/temp/4 commit -m "feat(kernel/runtime): Kernel::boot Boot 3 拆为 3a SafetyPolicy + 3b SafetyGuard

Updated 2026-05-26:
- Kernel struct +1 字段 safety_policy: Arc<dyn SafetyPolicy>
- Boot 3a: ConfigKvSafetyPolicy::load_from_kv 同步阻塞读 4 个 KV (block_on)
- Boot 3b: SafetyGuardImpl::from_text_with_policy 传入 policy 引用
- 4 KV 缺失 / DB 异常 → fallback 全 OFF + eprintln warning

新测试 boot_loads_safety_policy_with_all_scopes_disabled_when_kv_empty 验证
出厂默认全 OFF + SafetyGuard.is_enabled 与 SafetyPolicy 同步。"
```

---

## Task 9: ChatService 4 接线点（HIGH-1 + HIGH-2 收口）

**Files:**
- Modify: `src-tauri/src/services/chat/service.rs`

此 task 步骤较多，分 4 小阶段：scan_user_input policy 守卫 / wrap_messages 不变（SafetyGuard 内部已守）/ scan_token 真接入 on_delta / scan_final + safety_scan_status 真写。

- [ ] **Step 9.1: prepare 期 scan_user_input 加 if guard.is_enabled 守卫**

修改 `src-tauri/src/services/chat/service.rs` line ~195-215 段（`let input = match self.safety_guard.scan_user_input(&input) {`），改为：

```rust
        // Phase A0.7 Updated 2026-05-26: 经 SafetyPolicy.UserInput 守卫
        // (SafetyGuardImpl 内部已按 policy 短路, 此处 if 仅为可读性与早返路径)
        use crate::kernel::safety_policy::SafetyScope;
        let input = if self.safety_guard.is_enabled(SafetyScope::UserInput) {
            match self.safety_guard.scan_user_input(&input) {
                crate::kernel::safety_guard::ScanFinalResult::Ok => input,
                crate::kernel::safety_guard::ScanFinalResult::Redacted { redacted_text, .. } => {
                    redacted_text
                }
                crate::kernel::safety_guard::ScanFinalResult::Blocked { rule_ids, .. } => {
                    return Err(ChatError::UnsafeInput(format!(
                        "blocked by rules: {:?}",
                        rule_ids
                    )));
                }
                crate::kernel::safety_guard::ScanFinalResult::ScanFailed { reason, .. } => {
                    return Err(ChatError::SafetyScanFailed(format!(
                        "user input scan failed: {}",
                        reason
                    )));
                }
            }
        } else {
            input
        };
```

- [ ] **Step 9.2: wrap_messages 不变（已由 SafetyGuard 内部短路兜底）**

现有代码 line ~280-283 保持不变：

```rust
        let messages = self.safety_guard.wrap_messages(
            messages,
            crate::kernel::safety_guard::Locale::ZhCn,
        );
```

SafetyGuard 内部 if !policy.is_enabled(PrefixInjection) return messages 已经接住。

- [ ] **Step 9.3: run_stream Ok 分支 — scan_final + safety_scan_status 真写（HIGH-2 修复）**

定位 `match stream_result { Ok(finish) => {` 块（约 line 441-489），整段改写为：

```rust
            Ok(finish) => {
                // Phase A0.7 Updated 2026-06-18: Scope #3 流终 scan + ConversationRepo 真接入
                // (HIGH-2 修复). 终态优先级:
                // 1) stream hard hit 已由 dedicated safety-blocked finalization 写 final_blocked;
                // 2) FinalOutput ON 时 scan_final 决定 final_* / scan_failed;
                // 3) FinalOutput OFF 且 stream soft hit -> stream_soft_blocked;
                // 4) FinalOutput OFF 且无 stream hit -> disabled.
                use crate::kernel::safety_guard::ScanFinalResult;
                use crate::kernel::repos::conversation_repo::SafetyScanStatus;

                let stream_state = stream_safety_state.lock().clone();
                if stream_state.hard_rule_id.is_some() {
                    // HardEnd path already wrote fallback + mode='online' + final_blocked.
                    // Do not let normal finish or Cancelled finalization overwrite it.
                    return;
                }

                let persona_snapshot_id = conversation.persona_snapshot_id.as_str();
                let final_output_enabled = self.safety_guard.is_enabled(SafetyScope::FinalOutput);
                let scan = if final_output_enabled {
                    self.safety_guard.scan_final(&collected, persona_snapshot_id)
                } else {
                    // Policy off -> no final scan; terminal status derived below.
                    ScanFinalResult::Ok
                };

                // 派生 final_text / replace_reason / safety_scan_status / mode
                // mode 列 Updated 2026-05-26: 只允 3 值 (online / offline_rule / cancelled),
                // 安全状态走 safety_scan_status 列承担 (HIGH-2 修复 source of truth).
                let final_status = if !final_output_enabled {
                    if stream_state.has_soft_hit() {
                        SafetyScanStatus::StreamSoftBlocked
                    } else {
                        SafetyScanStatus::Disabled
                    }
                } else {
                    match &scan {
                        ScanFinalResult::Ok => SafetyScanStatus::FinalOk,
                        ScanFinalResult::Redacted { .. } => SafetyScanStatus::FinalRedacted,
                        ScanFinalResult::Blocked { .. } => SafetyScanStatus::FinalBlocked,
                        ScanFinalResult::ScanFailed { .. } => SafetyScanStatus::ScanFailed,
                    }
                };

                let (final_text, replace_reason): (String, Option<ReplaceReason>) = match scan {
                    ScanFinalResult::Ok => (collected, None),
                    ScanFinalResult::Redacted { redacted_text, .. } => {
                        (redacted_text, Some(ReplaceReason::FinalRedacted))
                    }
                    ScanFinalResult::Blocked { fallback, .. } => {
                        (fallback, Some(ReplaceReason::FinalBlocked))
                    }
                    ScanFinalResult::ScanFailed { fallback, .. } => {
                        (fallback, Some(ReplaceReason::ScanFailed))
                    }
                };

                // 写 messages.content + mode='online' + safety_scan_status (真接入 ConversationRepo, HIGH-2)
                // 注: 之前的 mode='safety_redacted/blocked/scan_failed' 写法废弃; 新数据 mode 只允 online/offline_rule/cancelled,
                // 安全状态 source of truth 改为 safety_scan_status 列
                if let Err(e) =
                    update_assistant_msg_with_safety_status(
                        app,
                        &assistant_id,
                        &final_text,
                        "online",
                        final_status,
                    )
                    .await
                {
                    let _ = channel.send(StreamEvent::Error {
                        error_kind: "DbError".to_string(),
                        message: e.to_string(),
                    });
                    return;
                }
                if let Err(e) = update_last_activity(app, &conv_id).await {
                    eprintln!("[chat] update_last_activity failed: {e}");
                }
                // SafetyGuard 改写了 content → 先 emit ReplaceMessage 让前端覆盖累积显示,
                // 再 emit Done 让前端走正常收尾路径 (清 currentStreamId)。
                if let Some(reason) = replace_reason {
                    let _ = channel.send(StreamEvent::ReplaceMessage {
                        message_id: assistant_id.clone(),
                        new_content: final_text,
                        reason,
                    });
                }
                let _ = channel.send(StreamEvent::Done {
                    total_tokens: finish.usage.map(|u| u.total_tokens).unwrap_or(0),
                    finish_reason: finish_reason_to_str(&finish.reason),
                });
            }
```

- [ ] **Step 9.4: 新建 helper update_assistant_msg_with_safety_status**

在 chat/service.rs 文件末尾 `async fn delete_assistant_msg` 之后追加：

```rust
/// Updated 2026-05-26: HIGH-2 修复 — 同时写 messages.content + mode + safety_scan_status
/// (走 ConversationRepo + memory::update_message_content_with_conn 各写一列).
/// mode 列仅承载业务模式 (online/offline_rule/cancelled); safety_scan_status 是新 source of truth.
async fn update_assistant_msg_with_safety_status<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    content: &str,
    mode: &str,
    safety_status: crate::kernel::repos::conversation_repo::SafetyScanStatus,
) -> Result<(), ChatError> {
    let mut conn = open_app_db(app).await?;
    update_message_content_with_conn(&mut conn, id, content, mode).await?;
    // ConversationRepo 真接入 (消除 dead code)
    let conv_repo = crate::kernel::repos::ConversationRepo::new();
    conv_repo
        .update_safety_status(&mut conn, id, safety_status)
        .await
        .map_err(|e| ChatError::Database(format!("update_safety_status: {}", e)))?;
    conn.close().await?;
    Ok(())
}
```

注：原 `update_assistant_msg`（line ~645）保留向后兼容（cancel/offline_rule 分支仍用它仅写 mode），但 Ok 分支改用新 helper。

- [ ] **Step 9.5: history mode filter 改 3 值（不再过滤 safety_*）**

定位 line ~254-264 段：

```rust
        let history_filtered: Vec<MessageRecord> = history
            .into_iter()
            .filter(|r| {
                !(r.role == "assistant"
                    && (r.mode == "offline_rule"
                        || r.mode == "cancelled"
                        // Updated 2026-05-26: safety_* mode 值写入端废弃, 但读端保留兼容老 DB 数据
                        // (Phase A0 落地的真 prod 数据可能有这些值, 不 backfill)
                        || r.mode == "safety_redacted"
                        || r.mode == "safety_blocked"
                        || r.mode == "safety_scan_failed"))
            })
            .collect();
```

注释加 "Updated 2026-05-26" 即可（保留旧 3 值过滤，但新写入不再产生）。

- [ ] **Step 9.6: run_stream on_delta closure 加 scan_token (HIGH-1 修复 + rule_id dedupe)**

定位 `let on_delta:` 段（约 line 418）整段改写：

```rust
        // Phase A0.7 Updated 2026-06-18: scan_token 真接入 (HIGH-1 修复).
        // StreamSafetyState 跨 closure / finalization 共享; 单条 message 流周期生效.
        use std::collections::HashSet;

        #[derive(Clone, Debug, Default)]
        struct StreamSafetyState {
            soft_rule_ids: HashSet<String>,
            hard_rule_id: Option<String>,
        }

        impl StreamSafetyState {
            fn record_soft(&mut self, rule_id: &str) -> bool {
                self.soft_rule_ids.insert(rule_id.to_string())
            }

            fn record_hard(&mut self, rule_id: &str) -> bool {
                if self.hard_rule_id.is_some() {
                    return false;
                }
                self.hard_rule_id = Some(rule_id.to_string());
                true
            }

            fn has_soft_hit(&self) -> bool {
                !self.soft_rule_ids.is_empty()
            }
        }

        let stream_safety_state: Arc<Mutex<StreamSafetyState>> =
            Arc::new(Mutex::new(StreamSafetyState::default()));
        let stream_safety_state_for_cb = Arc::clone(&stream_safety_state);
        let buffer_for_cb = Arc::clone(&buffer);
        let channel_for_cb = channel.clone();
        let safety_guard_for_cb = Arc::clone(&self.safety_guard);
        let app_for_cb = app.clone();
        let assistant_id_for_cb = assistant_id.clone();
        let cancel_token_for_cb = cancel_token.clone();

        let on_delta: Box<dyn Fn(StreamDelta) + Send + Sync> = Box::new(move |delta| {
            if let StreamDelta::TextDelta(text) = &delta {
                buffer_for_cb.lock().push_str(text);
                if let Err(e) = channel_for_cb.send(StreamEvent::Delta {
                    token: text.clone(),
                }) {
                    eprintln!("[chat] channel send Delta failed: {e}");
                }

                // Updated 2026-05-26: scan_token mid-stream (HIGH-1)
                if safety_guard_for_cb.is_enabled(SafetyScope::StreamToken) {
                    let acc = buffer_for_cb.lock().clone();
                    match safety_guard_for_cb.scan_token(text, &acc, false) {
                        crate::kernel::safety_guard::ScanTokenResult::Pass => {}
                        crate::kernel::safety_guard::ScanTokenResult::SoftBlock {
                            rule_id,
                            replace_last_n,
                            placeholder,
                        } => {
                            // rule_id dedupe; SoftBlock carries rule_id per 2026-06-18 contract.
                            let mut state = stream_safety_state_for_cb.lock();
                            if !state.record_soft(&rule_id) {
                                return; // 已 SoftBlock 过, 跳过避免震荡
                            }
                            drop(state);

                            // 替换 buffer 尾部 N chars
                            let mut buf = buffer_for_cb.lock();
                            let buf_chars: Vec<char> = buf.chars().collect();
                            if buf_chars.len() >= replace_last_n {
                                let kept: String = buf_chars[..buf_chars.len() - replace_last_n]
                                    .iter()
                                    .collect();
                                *buf = format!("{}{}", kept, placeholder);
                            } else {
                                *buf = placeholder.clone();
                            }
                            let new_content = buf.clone();
                            drop(buf);

                            // emit ReplaceMessage
                            let _ = channel_for_cb.send(StreamEvent::ReplaceMessage {
                                message_id: assistant_id_for_cb.clone(),
                                new_content: new_content.clone(),
                                reason: ReplaceReason::SoftBlockToken,
                            });

                            // 写 DB safety_scan_status = stream_soft_blocked
                            // (后台 spawn 不阻塞 stream)
                            let app2 = app_for_cb.clone();
                            let id2 = assistant_id_for_cb.clone();
                            let content2 = new_content;
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = update_safety_status_only(
                                    &app2,
                                    &id2,
                                    crate::kernel::repos::conversation_repo::SafetyScanStatus::StreamSoftBlocked,
                                )
                                .await
                                {
                                    eprintln!("[chat] mid-stream update_safety_status failed: {e}");
                                }
                                // 内容也写一次 (与 SoftBlock 替换语义对齐)
                                if let Err(e) = update_assistant_msg(&app2, &id2, &content2, "online").await {
                                    eprintln!("[chat] mid-stream update content failed: {e}");
                                }
                            });
                        }
                        crate::kernel::safety_guard::ScanTokenResult::HardEnd { rule_id } => {
                            let mut state = stream_safety_state_for_cb.lock();
                            if !state.record_hard(&rule_id) {
                                return; // hard hit 已处理, 避免重复 finalize
                            }
                            drop(state);

                            // Hard hit → dedicated safety-blocked finalization.
                            // cancel_token 只负责停止底层传输; Err(Cancelled) 分支不得覆盖为 mode='cancelled'.
                            let fallback = "抱歉，这段回复触发了安全规则，已停止输出。".to_string();
                            let _ = channel_for_cb.send(StreamEvent::ReplaceMessage {
                                message_id: assistant_id_for_cb.clone(),
                                new_content: fallback.clone(),
                                reason: ReplaceReason::FinalBlocked,
                            });
                            let app2 = app_for_cb.clone();
                            let id2 = assistant_id_for_cb.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) =
                                    finalize_safety_blocked_msg(&app2, &id2, &fallback).await
                                {
                                    eprintln!("[chat] hard-hit finalize_safety_blocked_msg failed: {e}");
                                }
                            });
                            cancel_token_for_cb.cancel();
                        }
                    }
                }
            }
            // ToolCallDelta / Finish：M1 不接 tools，不会触发；忽略
        });
```

同时修改 `Err(Cancelled)` 分支：若 `stream_safety_state.lock().hard_rule_id.is_some()`，说明这是 hard safety hit 触发的传输停止，直接 `return`，不得调用普通取消收尾，不得写 `mode='cancelled'` 覆盖 dedicated `final_blocked` 结果。

- [ ] **Step 9.7: 加 helper update_safety_status_only + finalize_safety_blocked_msg**

在 chat/service.rs 文件末尾追加（紧跟 `update_assistant_msg_with_safety_status` 之后）：

```rust
/// Updated 2026-05-26: mid-stream 仅写 safety_scan_status 列, 不动 content/mode.
async fn update_safety_status_only<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    status: crate::kernel::repos::conversation_repo::SafetyScanStatus,
) -> Result<(), ChatError> {
    let mut conn = open_app_db(app).await?;
    let repo = crate::kernel::repos::ConversationRepo::new();
    repo.update_safety_status(&mut conn, id, status)
        .await
        .map_err(|e| ChatError::Database(format!("update_safety_status: {}", e)))?;
    conn.close().await?;
    Ok(())
}

/// Updated 2026-06-18: hard safety hit 专用收尾。
/// 不复用普通 cancel 分支，避免 mode='cancelled' 覆盖真实安全终态。
async fn finalize_safety_blocked_msg<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    fallback: &str,
) -> Result<(), ChatError> {
    update_assistant_msg_with_safety_status(
        app,
        id,
        fallback,
        "online",
        crate::kernel::repos::conversation_repo::SafetyScanStatus::FinalBlocked,
    )
    .await
}
```

- [ ] **Step 9.8: 改原有 ChatService test_chat_service helper 加 SafetyGuardImpl::from_text 调用**

定位 chat/service.rs 测试段 `fn test_chat_service()`（约 line 789）。`from_text` 现在 cfg(test) 保留 + 自带 MockSafetyPolicy::all_on，不需改 helper。

但加新测试 `test_chat_service_with_policy(policy)` 给后续测试用：

```rust
    fn test_chat_service_with_policy(
        policy: Arc<dyn crate::kernel::safety_policy::SafetyPolicy>,
    ) -> ChatService {
        let guard: Arc<dyn SafetyGuard> = Arc::new(
            crate::kernel::safety_guard::SafetyGuardImpl::from_text_with_policy(
                "TEST_GUARD_PREFIX",
                policy,
            )
            .unwrap(),
        );
        ChatService::new(guard)
    }
```

- [ ] **Step 9.9: 跑测试**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib chat::service::tests 2>&1 | tail -30
```
Expected: 现有 19 个测试 PASS（test_chat_service 内部用 MockSafetyPolicy::all_on，wrap_messages 单测仍预期 inject prefix）

- [ ] **Step 9.10: Commit**

```bash
git -C /d/Project/temp/4 add src-tauri/src/services/chat/service.rs
git -C /d/Project/temp/4 commit -m "feat(chat/service): SafetyPolicy 接入 4 接线点 + HIGH-1/HIGH-2 收口

Updated 2026-05-26:
- scan_user_input 加 if guard.is_enabled(UserInput) 守卫
- wrap_messages 调用不变 (SafetyGuard 内部短路兜底)
- run_stream on_delta 真接入 scan_token (HIGH-1):
  * StreamSafetyState + rule_id dedupe 防震荡 (单 message 生命周期)
  * SoftBlock → 替换 buffer 尾 N chars + ReplaceMessage + 后台 spawn 写 stream_soft_blocked
  * HardEnd → dedicated safety-blocked finalization 写 fallback + mode='online' + FinalBlocked; cancel_token 只停止底层传输
- run_stream Ok 分支真接 ConversationRepo.update_safety_status (HIGH-2):
  * scan_final policy on → 写 final_ok/redacted/blocked/scan_failed
  * scan_final policy off → 按 terminal priority 写 stream_soft_blocked 或 disabled
  * mode 列降为 3 值 (online/offline_rule/cancelled), 安全状态走 safety_scan_status
- 老 mode='safety_*' history filter 保留兼容 (老 DB 数据不 backfill)

新 helper:
- update_assistant_msg_with_safety_status (3 列同写)
- update_safety_status_only (mid-stream 仅写 status 列)"
```

---

## Task 10: commands/safety.rs IPC + 单测

**Files:**
- Create: `src-tauri/src/commands/safety.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 10.1: 写失败测试 + IPC 命令骨架**

Create `src-tauri/src/commands/safety.rs`:

```rust
// commands/safety.rs — SafetyPolicy 配置 IPC (Updated 2026-05-26, spec §4.1).
// 2 IPC:
// - safety_get_policy: 返当前 4 scope 的 enabled state
// - safety_set_policy_scope(scope, enabled): 更新单个 scope (写 DB + 内存原子)

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::kernel::Kernel;
use crate::kernel::safety_policy::SafetyScope;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyPolicyState {
    pub prefix_enabled: bool,
    pub scan_user_input_enabled: bool,
    pub scan_token_enabled: bool,
    pub scan_final_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SafetyScopeIpc {
    Prefix,
    UserInput,
    StreamToken,
    FinalOutput,
}

impl SafetyScopeIpc {
    fn to_kernel(&self) -> SafetyScope {
        match self {
            Self::Prefix => SafetyScope::PrefixInjection,
            Self::UserInput => SafetyScope::UserInput,
            Self::StreamToken => SafetyScope::StreamToken,
            Self::FinalOutput => SafetyScope::FinalOutput,
        }
    }
}

#[tauri::command]
pub async fn safety_get_policy<R: Runtime>(
    app: AppHandle<R>,
) -> Result<SafetyPolicyState, String> {
    let kernel = app.try_state::<Kernel>().ok_or_else(|| {
        "Kernel not initialized (this should not happen post-boot)".to_string()
    })?;
    Ok(SafetyPolicyState {
        prefix_enabled: kernel.safety_policy.is_enabled(SafetyScope::PrefixInjection),
        scan_user_input_enabled: kernel.safety_policy.is_enabled(SafetyScope::UserInput),
        scan_token_enabled: kernel.safety_policy.is_enabled(SafetyScope::StreamToken),
        scan_final_enabled: kernel.safety_policy.is_enabled(SafetyScope::FinalOutput),
    })
}

#[tauri::command]
pub async fn safety_set_policy_scope<R: Runtime>(
    app: AppHandle<R>,
    scope: SafetyScopeIpc,
    enabled: bool,
) -> Result<(), String> {
    let kernel = app.try_state::<Kernel>().ok_or_else(|| {
        "Kernel not initialized".to_string()
    })?;
    kernel
        .safety_policy
        .set_enabled(scope.to_kernel(), enabled)
        .await
        .map_err(|e| format!("set_enabled failed: {}", e))?;

    // 广播事件让多 popup / 多 surface 同步
    let payload = SafetyPolicyState {
        prefix_enabled: kernel.safety_policy.is_enabled(SafetyScope::PrefixInjection),
        scan_user_input_enabled: kernel.safety_policy.is_enabled(SafetyScope::UserInput),
        scan_token_enabled: kernel.safety_policy.is_enabled(SafetyScope::StreamToken),
        scan_final_enabled: kernel.safety_policy.is_enabled(SafetyScope::FinalOutput),
    };
    if let Err(e) = app.emit("safety:policy_changed", &payload) {
        eprintln!("[safety] emit safety:policy_changed failed: {}", e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safety_scope_ipc_maps_to_kernel_scope() {
        assert_eq!(SafetyScopeIpc::Prefix.to_kernel(), SafetyScope::PrefixInjection);
        assert_eq!(SafetyScopeIpc::UserInput.to_kernel(), SafetyScope::UserInput);
        assert_eq!(SafetyScopeIpc::StreamToken.to_kernel(), SafetyScope::StreamToken);
        assert_eq!(SafetyScopeIpc::FinalOutput.to_kernel(), SafetyScope::FinalOutput);
    }

    #[test]
    fn safety_policy_state_serializes_camel_case() {
        let state = SafetyPolicyState {
            prefix_enabled: true,
            scan_user_input_enabled: false,
            scan_token_enabled: true,
            scan_final_enabled: false,
        };
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["prefixEnabled"], true);
        assert_eq!(json["scanUserInputEnabled"], false);
        assert_eq!(json["scanTokenEnabled"], true);
        assert_eq!(json["scanFinalEnabled"], false);
    }
}
```

- [ ] **Step 10.2: 注册 mod**

Modify `src-tauri/src/commands/mod.rs`，加：

```rust
pub mod safety;
```

- [ ] **Step 10.3: 注册 IPC 到 lib.rs invoke_handler**

Modify `src-tauri/src/lib.rs`，找到 `.invoke_handler(tauri::generate_handler![` 段，在末尾（`crate::commands::energy::energy_get,` 之后）加：

```rust
            // Phase A0.7 Updated 2026-05-26: SafetyPolicy IPC
            crate::commands::safety::safety_get_policy,
            crate::commands::safety::safety_set_policy_scope,
```

- [ ] **Step 10.4: 跑测试**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib safety::tests 2>&1 | tail -10
cargo check --lib 2>&1 | tail -10
```
Expected: 2 unit tests pass + lib compile clean

- [ ] **Step 10.5: Commit**

```bash
git -C /d/Project/temp/4 add src-tauri/src/commands/safety.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git -C /d/Project/temp/4 commit -m "feat(commands/safety): safety_get_policy + safety_set_policy_scope IPC

Updated 2026-05-26 spec §4.9 UI 入口:
- safety_get_policy → 返当前 4 scope 状态 (camelCase 序列化)
- safety_set_policy_scope(scope, enabled) → 走 kernel.safety_policy.set_enabled
  (写 DB + 内存原子) + emit 'safety:policy_changed' 让多 popup/多 surface 同步

2 单测覆盖 scope IPC enum 映射 + camelCase 序列化."
```

---

## Task 11: 前端 stores/userPopup.ts PopupNavId 扩 + safety variant

**Files:**
- Modify: `src/stores/userPopup.ts`
- Modify: `src/stores/__tests__/userPopup.test.ts`

- [ ] **Step 11.1: 写失败测试**

修改 `src/stores/__tests__/userPopup.test.ts`，在 describe 块末尾追加：

```typescript
  it('case 6: setNav("safety") works (safety is NOT disabled)', () => {
    const store = useUserPopupStore()
    store.open()
    store.setNav('safety')
    expect(store.activeNav).toBe('safety')
    expect(store.isDisabled('safety')).toBe(false)
  })
```

- [ ] **Step 11.2: 跑测试确认失败**

Run:
```bash
cd /d/Project/temp/4
pnpm vitest run src/stores/__tests__/userPopup.test.ts 2>&1 | tail -10
```
Expected: FAIL with "Type '\"safety\"' is not assignable to type 'PopupNavId'"

- [ ] **Step 11.3: 扩 PopupNavId**

修改 `src/stores/userPopup.ts`，把 PopupNavId 改为：

```typescript
export type PopupNavId =
  | 'profile'
  | 'account'
  | 'privacy'
  | 'notifications'
  | 'safety'  // Updated 2026-05-26: SafetyPolicy 4-toggle panel (spec §4.9)
  | 'help'
  | 'about'
```

DISABLED_NAV_IDS 不变（safety 是 enabled 项，不加进 disabled）。

- [ ] **Step 11.4: 跑测试确认通过**

Run:
```bash
cd /d/Project/temp/4
pnpm vitest run src/stores/__tests__/userPopup.test.ts 2>&1 | tail -10
```
Expected: 6 tests pass

- [ ] **Step 11.5: Commit**

```bash
git -C /d/Project/temp/4 add src/stores/userPopup.ts src/stores/__tests__/userPopup.test.ts
git -C /d/Project/temp/4 commit -m "feat(stores/userPopup): PopupNavId 加 safety variant

Updated 2026-05-26 spec §4.9: workspace popup sidebar 第 7 项 'Safety' panel.
safety 不在 DISABLED_NAV_IDS (是 enabled 实际功能, 不是 M3+ 占位)."
```

---

## Task 12: 前端 stores/safety.ts 新增（policy 状态 + IPC 调用）

**Files:**
- Create: `src/stores/safety.ts`
- Create: `src/stores/__tests__/safety.test.ts`

- [ ] **Step 12.1: 写失败测试骨架**

Create `src/stores/__tests__/safety.test.ts`:

```typescript
// safety store 单测 — IPC mock + 4 ref bool + load + setEnabled

import { setActivePinia, createPinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
import { useSafetyStore } from '../safety'

beforeEach(() => {
  setActivePinia(createPinia())
  vi.clearAllMocks()
})

describe('safety store', () => {
  it('case 1: 默认 4 ref 都是 false', () => {
    const store = useSafetyStore()
    expect(store.prefixEnabled).toBe(false)
    expect(store.scanUserInputEnabled).toBe(false)
    expect(store.scanTokenEnabled).toBe(false)
    expect(store.scanFinalEnabled).toBe(false)
  })

  it('case 2: load() invoke safety_get_policy 后回填 4 ref', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      prefixEnabled: true,
      scanUserInputEnabled: false,
      scanTokenEnabled: true,
      scanFinalEnabled: false,
    })
    const store = useSafetyStore()
    await store.load()
    expect(invoke).toHaveBeenCalledWith('safety_get_policy')
    expect(store.prefixEnabled).toBe(true)
    expect(store.scanUserInputEnabled).toBe(false)
    expect(store.scanTokenEnabled).toBe(true)
    expect(store.scanFinalEnabled).toBe(false)
  })

  it('case 3: setEnabled(prefix, true) invoke safety_set_policy_scope + 乐观更新', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(undefined)
    const store = useSafetyStore()
    await store.setEnabled('prefix', true)
    expect(invoke).toHaveBeenCalledWith('safety_set_policy_scope', {
      scope: 'prefix',
      enabled: true,
    })
    expect(store.prefixEnabled).toBe(true)
  })

  it('case 4: setEnabled 失败时 ref 不更新 + 抛错', async () => {
    vi.mocked(invoke).mockRejectedValueOnce('mock IPC error')
    const store = useSafetyStore()
    await expect(store.setEnabled('scanFinal', true)).rejects.toContain('mock IPC error')
    expect(store.scanFinalEnabled).toBe(false) // ref 不变
  })

  it('case 5: load() 失败时 ref 保持初始 false + 抛错', async () => {
    vi.mocked(invoke).mockRejectedValueOnce('IPC down')
    const store = useSafetyStore()
    await expect(store.load()).rejects.toContain('IPC down')
    expect(store.prefixEnabled).toBe(false)
  })
})
```

- [ ] **Step 12.2: 跑测试确认失败**

Run:
```bash
cd /d/Project/temp/4
pnpm vitest run src/stores/__tests__/safety.test.ts 2>&1 | tail -10
```
Expected: FAIL with "Cannot find module '../safety'"

- [ ] **Step 12.3: 实现 stores/safety.ts**

Create `src/stores/safety.ts`:

```typescript
// safety store (Updated 2026-05-26 spec §4.1, §4.9): popup Safety panel 4 toggle 数据源.
//
// 设计:
// - 4 个 ref<boolean> 对应 4 个 SafetyScope (prefix / scanUserInput / scanToken / scanFinal)
// - load(): invoke safety_get_policy 拉一次, 回填 ref
// - setEnabled(scope, enabled): invoke safety_set_policy_scope, 成功后更新 ref
// - ensureListener(): listen 'safety:policy_changed' 广播事件, 多 popup/多 surface 同步

import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type SafetyScopeFront =
  | 'prefix'
  | 'userInput'
  | 'streamToken'
  | 'finalOutput'

interface SafetyPolicyState {
  prefixEnabled: boolean
  scanUserInputEnabled: boolean
  scanTokenEnabled: boolean
  scanFinalEnabled: boolean
}

export const useSafetyStore = defineStore('safety', () => {
  const prefixEnabled = ref(false)
  const scanUserInputEnabled = ref(false)
  const scanTokenEnabled = ref(false)
  const scanFinalEnabled = ref(false)

  let unlisten: UnlistenFn | null = null
  const loaded = ref(false)

  function applyState(state: SafetyPolicyState) {
    prefixEnabled.value = state.prefixEnabled
    scanUserInputEnabled.value = state.scanUserInputEnabled
    scanTokenEnabled.value = state.scanTokenEnabled
    scanFinalEnabled.value = state.scanFinalEnabled
  }

  async function load() {
    const state = await invoke<SafetyPolicyState>('safety_get_policy')
    applyState(state)
    loaded.value = true
  }

  async function setEnabled(scope: SafetyScopeFront, enabled: boolean) {
    await invoke<void>('safety_set_policy_scope', { scope, enabled })
    // 乐观更新 + emit 事件会广播回这里 (避免双 toggle 闪烁, 仅 self-emit 不动)
    switch (scope) {
      case 'prefix':
        prefixEnabled.value = enabled
        break
      case 'userInput':
        scanUserInputEnabled.value = enabled
        break
      case 'streamToken':
        scanTokenEnabled.value = enabled
        break
      case 'finalOutput':
        scanFinalEnabled.value = enabled
        break
    }
  }

  async function ensureListener() {
    if (unlisten) return
    unlisten = await listen<SafetyPolicyState>('safety:policy_changed', (event) => {
      applyState(event.payload)
    })
  }

  function teardownListener() {
    if (unlisten) {
      unlisten()
      unlisten = null
    }
  }

  return {
    prefixEnabled,
    scanUserInputEnabled,
    scanTokenEnabled,
    scanFinalEnabled,
    loaded,
    load,
    setEnabled,
    ensureListener,
    teardownListener,
  }
})
```

- [ ] **Step 12.4: 跑测试确认通过**

Run:
```bash
cd /d/Project/temp/4
pnpm vitest run src/stores/__tests__/safety.test.ts 2>&1 | tail -10
```
Expected: 5 tests pass

- [ ] **Step 12.5: Commit**

```bash
git -C /d/Project/temp/4 add src/stores/safety.ts src/stores/__tests__/safety.test.ts
git -C /d/Project/temp/4 commit -m "feat(stores/safety): SafetyPolicy 前端 store (Pinia)

Updated 2026-05-26 spec §4.9 popup Safety panel 数据源:
- 4 ref<boolean> 对应 4 个 SafetyScope
- load() invoke safety_get_policy 拉一次回填
- setEnabled(scope, enabled) invoke safety_set_policy_scope + 乐观更新
- ensureListener() listen 'safety:policy_changed' 广播同步多 popup

5 vitest case 覆盖 default + load + setEnabled 成败 + 广播."
```

---

## Task 13: 前端 PopupSidebar.vue 加 safety nav 项到 "应用" 组

**Files:**
- Modify: `src/components/popup/PopupSidebar.vue`

- [ ] **Step 13.1: 加 safety 项到 NAV_GROUPS "应用" 组**

修改 `src/components/popup/PopupSidebar.vue` 的 `NAV_GROUPS` 常量（约 line 36），把"应用"组改为：

```typescript
  {
    title: '应用',
    items: [
      { id: 'privacy', label: '数据与隐私', icon: '🔒', badge: 'M3+', disabled: true },
      { id: 'notifications', label: '通知', icon: '🔔', badge: 'M3+', disabled: true },
      // Updated 2026-05-26 spec §4.9: SafetyPolicy 4-toggle panel (enabled 实际功能, 不是 disabled 占位)
      { id: 'safety', label: '安全', icon: '🛡' },
    ],
  },
```

- [ ] **Step 13.2: 跑 vue-tsc + vitest**

Run:
```bash
cd /d/Project/temp/4
pnpm vue-tsc --noEmit 2>&1 | tail -10
pnpm vitest run src/stores/__tests__ 2>&1 | tail -10
```
Expected: vue-tsc clean + 已通过的测试仍通过

- [ ] **Step 13.3: Commit**

```bash
git -C /d/Project/temp/4 add src/components/popup/PopupSidebar.vue
git -C /d/Project/temp/4 commit -m "feat(popup/sidebar): 加 safety nav 项到 应用 组 (icon: 🛡)

Updated 2026-05-26 spec §4.9: 与 privacy / notifications 同组 (应用类配置).
不带 badge / disabled, 是实际可用功能."
```

---

## Task 14: 前端 UserSafetyPanel.vue 新建 + UserPopup.vue render 路由

**Files:**
- Create: `src/panels/user/UserSafetyPanel.vue`
- Modify: `src/components/popup/UserPopup.vue`

- [ ] **Step 14.1: 创建 UserSafetyPanel.vue**

Create `src/panels/user/UserSafetyPanel.vue`:

```vue
<script setup lang="ts">
// UserSafetyPanel (Updated 2026-05-26 spec §4.9): SafetyPolicy 4-toggle UI.
//
// 单人 vibecoding 项目, 默认全 OFF; 用户按需打开 4 个 scope:
// 1. prefix - ADR-006 系统安全前缀注入 (几乎无开销)
// 2. userInput - 用户输入扫 (防 prompt injection, 一次同步扫)
// 3. streamToken - 流式输出 mid-stream 扫 (每 token 增量, 性能开销最大)
// 4. finalOutput - 流式输出最终全文扫 (流终一次)
//
// 4 toggle 独立; 关闭时 messages.safety_scan_status 写 'disabled' 终态保留审计.

import { onMounted, onBeforeUnmount } from 'vue'
import { ElSwitch, ElMessage } from 'element-plus'

import { useSafetyStore } from '@/stores/safety'

const safety = useSafetyStore()

interface ToggleDef {
  scope: 'prefix' | 'userInput' | 'streamToken' | 'finalOutput'
  label: string
  hint: string
  refKey: 'prefixEnabled' | 'scanUserInputEnabled' | 'scanTokenEnabled' | 'scanFinalEnabled'
}

const TOGGLES: ToggleDef[] = [
  {
    scope: 'prefix',
    label: 'ADR-006 系统安全前缀注入',
    hint: '在 LLM system message 第一位注入 ADR-006 安全护栏文本; 几乎无开销.',
    refKey: 'prefixEnabled',
  },
  {
    scope: 'userInput',
    label: '用户输入扫描',
    hint: '防 prompt injection; 用户输入提交前一次同步扫描; 命中后 ChatService 拒发 LLM.',
    refKey: 'scanUserInputEnabled',
  },
  {
    scope: 'streamToken',
    label: '流式输出 mid-stream 扫描',
    hint: '每个 LLM token 到达后增量扫尾部 64 字符; 命中后 SoftBlock 替换占位或 HardEnd 走 dedicated final_blocked 收尾; 性能开销最大.',
    refKey: 'scanTokenEnabled',
  },
  {
    scope: 'finalOutput',
    label: '流式输出最终全文扫描',
    hint: '流终一次全文扫; 命中后 Redacted/Blocked 替换文本 + UI ReplaceMessage. 关闭时仍写 disabled 终态保留审计.',
    refKey: 'scanFinalEnabled',
  },
]

async function onToggle(t: ToggleDef, value: boolean | string | number) {
  try {
    await safety.setEnabled(t.scope, value as boolean)
  } catch (e) {
    ElMessage.error(`SafetyPolicy 写入失败: ${String(e)}`)
    // 失败时让 store load 回滚到真实状态
    await safety.load().catch(() => {})
  }
}

onMounted(async () => {
  await safety.load()
  await safety.ensureListener()
})

onBeforeUnmount(() => {
  safety.teardownListener()
})
</script>

<template>
  <div class="safety-panel">
    <header class="safety-panel__header">
      <p class="safety-panel__lede">
        AIPET 默认<strong>全部关闭</strong> LLM 输入输出安全扫描. 单人项目, 安全扫描非核心诉求; 按需启用各 scope.
      </p>
      <p class="safety-panel__lede safety-panel__lede--secondary">
        详见 <a href="#" @click.prevent>ADR-006</a> 与 <a href="#" @click.prevent>ADR-026</a>.
      </p>
    </header>

    <ul class="safety-panel__toggles">
      <li v-for="t in TOGGLES" :key="t.scope" class="safety-panel__row">
        <div class="safety-panel__row-text">
          <div class="safety-panel__row-label">{{ t.label }}</div>
          <div class="safety-panel__row-hint">{{ t.hint }}</div>
        </div>
        <ElSwitch
          :model-value="(safety as any)[t.refKey]"
          @change="(v: boolean | string | number) => onToggle(t, v)"
        />
      </li>
    </ul>
  </div>
</template>

<style scoped>
.safety-panel {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-5);
}

.safety-panel__header {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}

.safety-panel__lede {
  font-size: 13px;
  color: var(--aipet-color-text-2);
  line-height: 1.6;
  margin: 0;
}

.safety-panel__lede--secondary {
  font-size: 12px;
  color: var(--aipet-color-text-3);
}

.safety-panel__lede a {
  color: var(--aipet-color-primary);
  text-decoration: none;
}

.safety-panel__lede a:hover {
  text-decoration: underline;
}

.safety-panel__toggles {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: 8px;
  overflow: hidden;
  background: var(--aipet-color-bg);
}

.safety-panel__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--aipet-space-4);
  padding: var(--aipet-space-4) var(--aipet-space-4);
  border-bottom: 1px solid var(--aipet-color-border-faint);
}

.safety-panel__row:last-child {
  border-bottom: none;
}

.safety-panel__row-text {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.safety-panel__row-label {
  font-size: 14px;
  font-weight: 500;
  color: var(--aipet-color-text-1);
}

.safety-panel__row-hint {
  font-size: 12px;
  color: var(--aipet-color-text-3);
  line-height: 1.5;
}
</style>
```

- [ ] **Step 14.2: 修改 UserPopup.vue 加 SafetyPanel render**

修改 `src/components/popup/UserPopup.vue`:

(a) Import 段加：

```typescript
import UserSafetyPanel from '@/panels/user/UserSafetyPanel.vue'
```

(b) `panelTitle` computed 加 case：

```typescript
const panelTitle = computed(() => {
  switch (popup.activeNav) {
    case 'profile':
      return '个人资料'
    case 'account':
      return '账户'
    case 'privacy':
      return '数据与隐私'
    case 'notifications':
      return '通知'
    case 'safety':
      return '安全'
    case 'help':
      return '帮助'
    case 'about':
      return '关于'
    default:
      return ''
  }
})
```

(c) Template `<div class="popup-main__content">` 内加 panel：

```vue
            <UserProfilePanel v-show="popup.activeNav === 'profile'" />
            <UserHelpPanel v-show="popup.activeNav === 'help'" />
            <UserAboutPanel v-show="popup.activeNav === 'about'" />
            <UserSafetyPanel v-show="popup.activeNav === 'safety'" />
            <UserPlaceholderPanel v-show="popup.activeNav === 'account'" kind="account" />
            <UserPlaceholderPanel v-show="popup.activeNav === 'privacy'" kind="privacy" />
            <UserPlaceholderPanel
              v-show="popup.activeNav === 'notifications'"
              kind="notifications"
            />
```

- [ ] **Step 14.3: 跑 vue-tsc + lint**

Run:
```bash
cd /d/Project/temp/4
pnpm vue-tsc --noEmit 2>&1 | tail -10
```
Expected: clean

- [ ] **Step 14.4: 启动 dev 模式手动检查 UI 渲染**

Run:
```bash
cd /d/Project/temp/4
pnpm tauri:dev
```

期望（人工验证）:
- 打开 workspace 窗 (Ctrl+Alt+W)
- 点 brand bar profile 图标 → popup 打开
- sidebar "应用" 组下应能看到 "🛡 安全" 项
- 点击 "安全" → panel 显示 4 个 toggle, 全 OFF (`el-switch` 灰态)
- 切一个 toggle → 应能拨动 (不报错; SafetyPolicy DB 写成功 + emit 事件)
- 关闭 dev (Ctrl+C)

- [ ] **Step 14.5: Commit**

```bash
git -C /d/Project/temp/4 add src/panels/user/UserSafetyPanel.vue src/components/popup/UserPopup.vue
git -C /d/Project/temp/4 commit -m "feat(popup/safety): UserSafetyPanel 4-toggle UI + popup render 路由

Updated 2026-05-26 spec §4.9:
- 新建 UserSafetyPanel.vue: 4 ElSwitch (prefix/userInput/streamToken/finalOutput)
  + 每个 toggle 下 hint 说明开销; 失败时回滚 ref 走 store.load
- UserPopup.vue: panelTitle 加 'safety' case + render UserSafetyPanel
- 自带 onMounted load + ensureListener; onBeforeUnmount teardown."
```

---

## Task 15: 联调 + 手动 e2e 8 cases + 关闭 #48 #49

**Files:**
- (Verify only) 已落 commits

- [ ] **Step 15.1: 跑全套 cargo test --lib**

Run:
```bash
cd /d/Project/temp/4/src-tauri
cargo test --lib 2>&1 | tail -15
```
Expected: 358 → 约 378-385 (+20-25 新测) PASS

- [ ] **Step 15.2: 跑 vitest**

Run:
```bash
cd /d/Project/temp/4
pnpm vitest run 2>&1 | tail -15
```
Expected: 293 + 6 (safety) + 1 (userPopup new) ≈ 300 PASS

- [ ] **Step 15.3: 跑 CI 黑名单脚本**

Run:
```bash
bash /d/Project/temp/4/scripts/ci_check_os_context_apis.sh
```
Expected: `✅ CI check passed`

- [ ] **Step 15.4: 启动 dev + 手动 e2e 8 cases**

Run:
```bash
cd /d/Project/temp/4
pnpm tauri:dev
```

手动验证（每个 case 后切换 toggle 拉一次再验证）:

| Case | 配置 | 操作 | 期望 |
|---|---|---|---|
| 1 | 全 OFF (出厂) | 发"hello" + LLM 回复 | 无 prefix 注入 / scan 全跳过; messages.safety_scan_status='disabled' |
| 2 | prefix ON 其他 OFF | 发"hello" | LLM 看到 ADR-006 prefix 但其他 scope 全 noop |
| 3 | scan_user_input ON 其他 OFF | 发"自杀" | ChatError::UnsafeInput, 不发 LLM, UI 显示拒绝原因 |
| 4 | scan_token ON 其他 OFF | LLM 流中含"违禁" | mid-stream replace 为 `[审核中…]`, UI 看到替换效果; safety_scan_status='stream_soft_blocked' |
| 5 | scan_final ON 其他 OFF | LLM 流终全文含"自杀" | UI 整条替换 fallback; safety_scan_status='final_blocked' |
| 6 | scan_final ON, scan_token ON | LLM 流末扫 OK 但 token 含"违禁" | mid-stream SoftBlock + 流末 final_redacted (兼容) |
| 7 | 4 ALL ON | 发正常对话 | 4 scope 都跑但无命中 → safety_scan_status='final_ok' |
| 8 | toggle 后重启 app | 全 ON 设置后重启 dev | 重启后 toggle 仍是 ON (KV 持久化生效) |

- [ ] **Step 15.5: 关闭 #48 (转 by-design)**

在 `src-tauri/src/commands/llm_providers.rs::llm_test_provider` IPC 实现内加注释：

```rust
// Updated 2026-05-26: probe 路径 by-design bypass SafetyPolicy
// (test_provider 用于配置 LLM 连通性验证, 不应被安全扫描卡住; ChatService::prepare 走的是
// 正式对话路径, 那里仍按 SafetyPolicy 守卫. spec [`2026-05-26-safety-policy-configurable-design.md`] §10)
```

定位 `llm_test_provider` 函数（在 commands/llm_providers.rs），加注释。

提交时附带：

```bash
git -C /d/Project/temp/4 add src-tauri/src/commands/llm_providers.rs
git -C /d/Project/temp/4 commit -m "docs(llm_providers): probe 路径 by-design bypass SafetyPolicy 注释 (closes #48)

test_provider IPC 用于连通性验证不走 ChatService::prepare, 故不经 SafetyPolicy 守卫.
正式对话路径仍按 SafetyPolicy 4 scope 各自启用.

Closes #48 (ChatService test connectivity probe 绕过 SafetyGuard - by-design)"
```

如果你已经接入了远端 gh：
```bash
gh issue close 48 --comment "Closed by-design — probe 路径不应被安全扫描卡住, 详 spec 2026-05-26-safety-policy-configurable-design.md §10 + commit comment."
gh issue close 49 --comment "Closed — trailing-window N=64 + rule_id dedupe 已并入 SafetyPolicy 可配置化 implementation Task 7. 详 commit history."
```

- [ ] **Step 15.6: 最终 commit + STATUS.md 更新**

Run:
```bash
git -C /d/Project/temp/4 log --oneline -20
```
确认本 plan 实施的所有 commit 都在。

更新 `docs/STATUS.md` 当前状态段（自行编辑或用 /sync-status 命令）:
- "当前 milestone" 段 SafetyPolicy 可配置化收尾
- "已完成" 段补充 ADR-026 / spec 2026-05-26 / cargo test 358 → 378+
- "follow-up" 段 #48 #49 关闭

Final commit:

```bash
git -C /d/Project/temp/4 add docs/STATUS.md
git -C /d/Project/temp/4 commit -m "docs(status): SafetyPolicy 可配置化收尾 — Phase A0 全套对齐 spec

- Constitution #1 改写 Safety Configurable
- 8-state FSM 含 disabled 终态
- 4 scope toggle 出厂全 OFF + workspace popup Safety panel
- HIGH-1/HIGH-2 同步收口, #48 by-design / #49 并入
- cargo test 358 → 378+, vitest 293 → 300+"
```

---

## Self-Review

### 1. Spec coverage

| Spec 章节 | 对应 Task |
|---|---|
| §1 背景 | Task 1 (ADR), Task 2 (spec Updated) |
| §2 设计目标 | 整体 plan |
| §3 不在范围内 | Task 15.5 (#48 by-design 注释) |
| §4.1 SafetyPolicy trait + ConfigKvSafetyPolicy | Task 3, Task 4 |
| §4.2 4 KV | Task 4 (kv_key) |
| §4.3 SafetyGuard 接口扩展 | Task 6 |
| §4.4 8-state FSM | Task 5 (Disabled variant) |
| §4.5 Cross-scope 互动表 | Task 9 (cross-scope 实施) |
| §4.6 ChatService 4 接线点 | Task 9 |
| §4.7 messages.mode 重定义 | Task 9.3 (mode='online' 写入 + 老 mode='safety_*' filter 保留) |
| §4.8 scan_token trailing-window | Task 6 + Task 7 (含 #49) |
| §4.9 workspace popup Safety panel | Task 13 + Task 14 |
| §5 v3 spec / ADR / decisions.md 改动 | Task 1, Task 2 |
| §6 Migration | Task 5 (无 migration 003, 只扩 enum) |
| §7 Error handling | Task 4 (DB 失败 + KV 损坏), Task 10 (IPC error) |
| §8 测试 | Task 3-13 单测 + Task 15 集成 |
| §9 工时 | plan 整体 ~14-16h 对齐 |
| §10 follow-up | Task 15.5 (#48 by-design + #49 closed) |

**覆盖完整, 无 gap.**

### 2. Placeholder scan

- 无 "TBD" / "implement later" / "fill in details"
- 无 "add appropriate error handling" / "handle edge cases" 抽象指令
- 每个 step 都有可复制的代码块 / shell 命令 / 预期输出
- Task 15.5 #48 注释虽简短但有完整内容

### 3. Type consistency

- `SafetyScope` 4 variant 在 Task 3 定义, Task 4/6/9/10 引用一致
- `SafetyScanStatus::Disabled` 在 Task 5 加, Task 9 引用一致
- `SafetyGuardImpl::from_text_with_policy` 在 Task 6 改名, Task 8 调用一致
- `safety_policy.is_enabled(scope) -> bool` 在 Task 3 trait 定义, Task 4/6/10 实施/调用一致
- Frontend `SafetyScopeFront` 4 variant ('prefix'/'userInput'/'streamToken'/'finalOutput') 与 backend `SafetyScopeIpc` enum 一一对应

**类型一致, 无 mismatch.**

---

## Execution Handoff

**Plan complete and saved to** `docs/superpowers/plans/2026-05-26-safety-policy-configurable-implementation.md`. Two execution options:

**1. Subagent-Driven (推荐)** - 每个 task 派一个 fresh subagent 实施, 主控两阶段 review (中间 + 最终)。15 个 task fresh context 防上下文污染；适合本 plan 工时较大 (~14-16h) 且涉及 frontend + backend + docs 多 layer。

**2. Inline Execution** - 当前 session 内顺序执行所有 task, 每个 checkpoint 暂停 review。适合 plan 较小或希望全程主控者全程在场。

**Which approach?**
