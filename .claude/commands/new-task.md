---
description: 从对话上下文综合推断 + 创建 GitHub issue（智能版）
---

# /new-task — 智能创建 GitHub issue

把当前对话里讨论的任务写成 GitHub issue。**先综合上下文推断 → 给我看推断结果 → 我确认后再调 gh CLI**。

## 输入

- `/new-task <一句话描述>` —— 描述可省略；省略时从最近对话内容推断
- 用户可能附带提示：「这个推到 M3」「priority 0」「这个属于 N 模块」—— 这些是硬指令，覆盖你的推断

## 推断流程（5 步）

### 第 1 步：综合上下文判 type

不是只看关键词，而是综合：

| 信号 | type |
|---|---|
| 实现一段全新代码 / 新功能 / 模块的某个子功能 | `type:feat` |
| 修一个已有的 bug、错误行为、用户反馈的问题 | `type:fix` |
| 调整代码结构但不改外部行为 | `type:refactor` |
| 调研选型 / 验证假设 / spike（如"试试 X 是否可行"） | `type:spike` |
| 升级依赖 / 改 CI / 改构建脚本 / .gitignore | `type:chore` |
| 仅改 docs/ 下的内容、README、注释 | `type:docs` |

模糊时倾向更具体的（feat/fix > refactor/chore）。

### 第 2 步：综合上下文判 module

参考 [docs/github-workflow.md §2.2](../../docs/github-workflow.md) 的 18 个 module label。

判断信号（按优先级）：

1. **用户明说的模块字母**（"H 模块"、"infra"）—— 最高优先级
2. **PRD §7 关键词**（"ChatService" → B-chat；"hitbox/抗议" → N-interact；"VRM 渲染" → A-shell；"番茄钟" → D-pomodoro；"灵魂宣誓" → M-pledge）
3. **架构 §3 服务名**（"PersonaService" → H-persona；"WardrobeService" → O-wardrobe）
4. **从修改的文件路径推断**（如果对话里讨论了 `src/main/services/chat/...` → B-chat）
5. **基础设施关键词**（Migration / Crypto / Telemetry / Network / Updater / 项目脚手架 / 构建工具 / IPC 框架 → infra）

**多模块情况**：如果一个任务确实跨 2-3 个模块（如 N + A），选**主要受影响**的那个；其他模块在 issue body 里写一行 `Cross: module:X-foo, module:Y-bar`。

**新模块情况**（不在 A-Q 也不是 infra）：先告诉我，由我决定是否加新 module label，不要自作主张。

### 第 3 步：判 milestone

默认从 `docs/STATUS.md` 的「当前 milestone」字段取。

特殊情况：

- 用户明说"放 M3" / "推到 P1" → 按用户指示
- 当前 milestone 已满 / 任务跟当前 milestone 关键路径无关 → 询问用户是否推到下一个 milestone
- Spike 类任务：通常归当前 milestone（spike 是为了解锁当前 milestone 的工作）

### 第 4 步：判 priority

| 信号 | priority |
|---|---|
| 用户说"很重要"/"卡住了"/"必须先做" | `priority:p0` |
| 关键路径节点（roadmap §4 列出的）| `priority:p0` |
| 影响 ≥ 2 个其他 task 的前置依赖 | `priority:p0` |
| 重要但不阻塞 | `priority:p1` |
| 普通任务 | 不打 priority label |

**默认不打 priority** —— 不要给每个 issue 都打 p1。

### 第 5 步：找反向链接

从 docs/ 找 1-2 个最相关的章节链接：

- feat / spike：链 `docs/requirements/prd.md §模块号` 或 `docs/architecture/system-architecture.md §X.Y`
- fix：尽量链回相关功能定义所在章节
- chore / docs：可不带链接

格式：`Refs: docs/requirements/prd.md §B.3.a`（用 §+ 章节号，不用锚点）

如果找不到对应章节 → 在 issue body 里写 `Refs: 无（模块尚未在基线文档展开）`。

---

## 推断结果展示（创建前）

把推断展示成这种格式让我确认：

```
我准备创建 issue：

  Title:     B: ChatService MVP 流式渲染
  Labels:    type:feat, module:B-chat
  Milestone: M1
  Priority:  （无）

  Refs:      docs/requirements/prd.md §B.3.a, docs/architecture/system-architecture.md §3.2

  Body:
    做什么：实现 ChatService 主进程服务的最小版本，含 OpenAI Provider + 流式渲染到前端。
    验收标准：
    - [ ] /chat IPC 命令跑通
    - [ ] 流式 token 显示在 ChatPanel
    - [ ] 错误兜底（API key 无效、网络失败）
    依赖：H-persona MVP（已完成 ADR-008 灵魂宣誓后才能给 ChatService 喂安全前缀）

  确认创建？(y / 改 / 取消)
```

收到 `y` 才调 gh。收到 `改 ...` 调整后重新展示。

## 调用 gh CLI

```bash
gh issue create \
  --title "<title>" \
  --label "<labels-comma-separated>" \
  --milestone "<M?>" \
  --body "<body>"
```

如果加了 priority，加进 `--label`。

成功后回报 issue 编号 + URL，并问我「要不要 `/sync-status` 把它写入 STATUS？」（通常**不**需要写入；STATUS 只放当前 session 在做的事）。

---

## 远端未接入时

如果 `git remote -v` 没有 origin / `gh` 命令报错 / 网络拦截：

1. 不要强行报错。
2. 把这条 task 暂存到 `docs/STATUS.md` 的「即将开始」临时节，加一行：
   ```
   - [ ] [待开 issue] B: ChatService MVP 流式渲染（type:feat, milestone:M1）
   ```
3. 提示我：远端接入后用 `/new-task --batch` 一次性把暂存的 task 批量推到 GitHub。

## 防错清单（发出 gh 前过一遍）

- [ ] type label 恰好 1 个
- [ ] module label 恰好 1 个（除非 type=docs/chore 可省）
- [ ] priority label ≤ 1 个，且只在合理时打
- [ ] title 以 `<module-letter>:` 开头
- [ ] body 含 `Refs:` 一行
- [ ] body 含验收标准复选框（feat / spike 必须；fix / chore 可省）
- [ ] milestone 存在（如果 gh milestone list 不含目标 milestone，先告诉我，不要自动建）

## 不要做

- ✗ 不要不展示推断就直接调 gh
- ✗ 不要把 PRD 整段复制到 body（写 Refs 链接）
- ✗ 不要给每个 issue 都打 priority
- ✗ 不要自动创建不存在的 milestone
- ✗ 不要在我说"放到 M3"时自作主张改成 M2
