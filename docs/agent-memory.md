---
title: Agent 长期协作记忆
updated: 2026-06-18
related:
  - ../AGENTS.md
  - WORKFLOW.md
  - github-workflow.md
  - decisions.md
---

# Agent 长期协作记忆

> 本文件由 `C:/Users/TXL/.claude/projects/D--Project-temp-4/memory/` 迁移而来，用于补齐从 Claude 项目索引转换到 Codex 项目索引时漏掉的 memory。Codex 入场优先读 `AGENTS.md`；需要细节时读本文件。

---

## 索引

- [不加 Claude Code 署名](#不加-claude-code-署名)
- [文件操作工具与权限约定](#文件操作工具与权限约定)
- [实测优先，不机械执行字面建议](#实测优先不机械执行字面建议)
- [ADR 修订风格](#adr-修订风格)
- [UI bug 必须结构级根因诊断](#ui-bug-必须结构级根因诊断)
- [subagent DONE_WITH_CONCERNS 必须 verify](#subagent-done_with_concerns-必须-verify)
- [gh auth status timeout 不代表 gh 失效](#gh-auth-status-timeout-不代表-gh-失效)

---

## 不加 Claude Code 署名

所有 git commit 一律不要附加 `Co-Authored-By: Claude ...` 或类似 AI 署名行。

**Why**：2026-05-06 用户在 #4 commit 时明确指示“不加 claudecode 署名，不加署名这个规则为长久使用”。

**How to apply**：不论 commit 类型（feat / fix / docs / chore / WIP / amend），写 message 时都不要在末尾追加 Co-Authored-By 行。

---

## 文件操作工具与权限约定

项目内常规读写直接操作，不为项目根内文件读写二次请求审批。当前 Codex 环境按本 session 的工具权限执行；若工具失败，换实际可用路径或工具，不把失败工具固化为规则。

Claude 旧环境的实测结论是：内置 Read / Edit / Write + 相对项目根路径最稳定；mcp filesystem 在当时 sandbox 下会报 access denied。Codex 当前环境没有完全相同的工具映射，因此这条 memory 的核心不是指定某个工具，而是：用实测有效的方式读写项目文件，少制造权限噪音。

---

## 实测优先，不机械执行字面建议

用户给出工具选择、方法、约定、规则建议时，即使字面看起来合理，也要先用实测、读代码或查文档验证后再照做或写入任何持久化产物（memory / commit / plan / 决策记录 / STATUS.md）。

如果实测发现建议不可行，直接反馈：“我实测了，X 在 ___ 场景失败，实际有效是 Y。”不要用模糊措辞把未验证建议包装成结论。

适用点：

- 写 memory / 推荐方案 / commit message / STATUS / 决策前，确认声明有实测或查证依据。
- 收到“请使用实际有效的方式”“真的能用吗”“你确认过吗”类 challenge 时，先复盘实测过程再回答。
- 未来发现本文件内容与实测矛盾时，修正本文件，不教条遵守旧记忆。

---

## ADR 修订风格

ADR 若还未被代码实施就遇到大幅修订，直接整段重写，让 ADR 单段反映当前真相，不追加 Updated 段。

已实施的 ADR 才用 `Updated YYYY-MM-DD` 段追加保留演化历史，让历史变更能对照代码阅读。

操作前先判定 ADR 状态：ADR 提到的核心代码模块是否已经存在仓库内。不确定时问用户，不默认猜测。

---

## UI bug 必须结构级根因诊断

UI 错位、视觉异常类 bug 必须做结构级根因诊断，不能只改 padding / margin / color 等表层样式做“看起来对了”的补丁。

诊断流程：

1. 复述用户描述的具体现象，尤其是“叠加”“无法翻页”“错位”等关键词。
2. 找出涉及的 CSS / Tauri 机制，如 sticky、overflow、grid item min-size、scoped CSS 特异性、capability 权限。
3. 推算 layout 与 paint 阶段差异。
4. 给出根因、修法、影响范围。
5. 根因不确定时建议查官方文档或网络资料，再动手。

不接受的修法：堆 z-index、加 `!important`、加额外 padding 抵消视觉差、改色彩绕过 layout 问题。

接受的修法：删 negative margin、改 grid item min-height、加 box-sizing reset、修正全局 CSS selector 特异性、补 Tauri capability 权限。

---

## subagent DONE_WITH_CONCERNS 必须 verify

subagent / implementer 报 `DONE_WITH_CONCERNS`，或在主任务之外带“顺手修”，主控必须独立查证，不能照办合并。

Phase A0.5b 的旧坑：implementer 只读了 `migrations/001_init.sql`，没读 `002_phase_a0_safety_secrets.sql`，把 `SecretRepo::set` 从 4 列 INSERT 改回 3 列，并让测试 schema 跟着漂移；测试通过仅因为测试库没有 apply 002，生产环境会丢审计列。

执行规则：

- concerns 当未确认假设，亲自跑命令或读文件验证。
- “顺手修”必须确认是真 bug，不是 implementer 没看全 migration / spec / lessons。
- spec 跨多个文件时，主控必须交叉验证。
- 验证后在提交或说明里写清 accepted / reverted 的原因。

---

## gh auth status timeout 不代表 gh 失效

Windows 上 `gh auth status` 可能显示 `Timeout trying to log in ... (keyring)`，但 `gh issue create / list / close` 等目标命令仍然可用。

执行规则：

- gh 操作开始时不必先跑 `gh auth status`。
- 直接试目标命令；轻探活用 `gh issue list --limit 1`。
- 看到 Timeout / keyring 类报错时，仍试一次目标命令。
- 只有目标命令也失败，才按“远端未接入回退本地”处理。
