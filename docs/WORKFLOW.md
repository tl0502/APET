---
title: 我的工作约定（WORKFLOW）
updated: 2026-05-05
related:
  - README.md
  - decisions.md
---

# 我的工作约定（WORKFLOW.md）

单人 vibecoding 项目，本文件是写给未来自己的备忘 — 哪些事情做不做、怎么做、用什么文件记。

> **核心原则**：能用 git history / 自言自语 / 一段话搞定的事，不要走流程。

---

## 1. 文档边界

`docs/` 是项目的**单一权威文档源**。`docs/` 之外的所有描述（聊天记录、Notion 速记、灵感本）都是临时的。

### 什么进 docs/

| 类别 | 位置 | 例 |
|---|---|---|
| 基线文档 | `requirements/` `architecture/` `persona/` `roadmap/` | `prd.md` `system-architecture.md` |
| 决策记录 | `decisions.md`（单文件） | ADR-001 ... ADR-015 |
| 研究档案 | `research/` | `competitor-research.md` |
| 进度状态 | `STATUS.md`（单文件） | 当前进度 / 阶段摘要 |
| 任务级清单 | **GitHub Issues** | 详见 [github-workflow.md](github-workflow.md) |

### 什么不进 docs/

- 自我对话、灵感速记
- 实施期周报、看板截图
- 第三方资料原件（用 URL 引用即可）
- 临时调试笔记（写在代码注释或 issue 里）

---

## 2. 文档头规范（YAML frontmatter）

每份基线文档头部 3 个字段：

```yaml
---
title: AI 桌宠 需求文档（PRD）
updated: 2026-05-05
related:
  - ../architecture/system-architecture.md
---
```

新文档从 [templates/doc-template.md](templates/doc-template.md) 复制再改写。

> 不放 version / status / created / supersedes — 这些 git 知道。

---

## 3. 版本号约定（极简）

| 变更 | 处理 |
|---|---|
| typo / 字段补充 / 新事件 | 直接改，更新 `updated` |
| 章节级新增 / 修改 | 直接改，更新 `updated`，必要时在文档顶部留一段"## 最近变化"小节简述 |
| 重大架构调整 / 新模块 | 写一条决策到 `decisions.md`，再改文档 |

> 正文里的 "v1.1" 字样不强求改成 "v1.2"；版本号已经不再具有契约意义。

---

## 4. 决策记录约定

### 何时记

- 影响 ≥ 2 个模块的技术选型
- 影响实现工期 ≥ 3 天的方案
- 影响安全/隐私/数据存储格式的设计
- 把已记录的决策推翻

### 怎么记

打开 [decisions.md](decisions.md)，找下一个空闲编号（如 ADR-016），加一段三句话：

```markdown
### ADR-016 标题

- **为什么**：（背景与现状）
- **选了什么**：（最终选 X，理由一句话）
- **代价**：（明知会牺牲什么）
```

被推翻的决策不删除；在原条目末尾追加 `**Supersedes**：ADR-XXX（理由）`。

---

## 5. Commit 风格

`<type>: <subject>` — 例 `feat: add ChatService MVP` / `docs: ADR-016 cloud sync` / `fix: hitbox edge case`

type 自由：`feat / fix / refactor / docs / chore`，不强制。

---

## 6. 命名风格

| 元素 | 风格 | 例 |
|---|---|---|
| 文件夹 | 英文小写 + 连字符 | `architecture/` |
| 文档名 | 英文小写 + 连字符 | `system-architecture.md` |
| 决策编号 | `ADR-NNN-kebab-case` | `ADR-016` |
| 中英混排 | 中英文之间空一格 | `Vue 3 项目` |
| 中文标点 | 全角 `（）` | `（建议）` |

---

## 7. 工具配置提示

- **Obsidian**：`Settings → Files and links → Excluded files`，添加 `_archive/`。
- **VS Code**：在 `.vscode/settings.json` 加 `"search.exclude": { "**/_archive": true }`。
- **Git**：`.gitignore` 把 `.DS_Store` 等加上，文档目录不忽略任何 `.md`。

---

## 8. Session 进度管理（STATUS.md）

### 8.1 为什么有这个文件

AI 对话不持久。新 session 是空白页。[STATUS.md](STATUS.md) 是写给下次 Claude 看的便条 —— "上次到哪、下一步、阻塞"。

### 8.2 什么时候更新

- **每个 session 末**：完成的事勾选；新发现的下一步加进列表；阻塞写进"阻塞"字段。
- **session 中遇到关键决策**：直接调 [decisions.md](decisions.md) 加 ADR；STATUS.md 里只留"决策待办"复选框。

### 8.3 怎么更新

session 结束前调用 `/sync-status`。Claude 会按**三层分离**结构操作：

1. **关闭已完成 issue**：close-comment 必须**自包含**（commit hash + 做了什么 + 关键偏离/取舍 + 实测 + follow-up），不能写"详见 STATUS.md"指针
2. **更新 STATUS.md**：只改"当前状态"四行 + "Milestone 进度"一行（如 13/18 → 14/18）；**不**写本 session 详细流水（详情已在 issue closing comment）
3. **历史 deep dive**：按月归档到 [`_archive/sessions/YYYY-MM.md`](_archive/sessions/)，STATUS.md 主体不再装"历史 session 摘要"
4. **经验教训**：踩过且容易再踩的坑写到 [lessons.md](lessons.md)，不写进 STATUS.md

不用手写；让 `/sync-status` 改。

### 8.4 新 session 怎么入场

打开新 session 后输入 `/resumex`，Claude 会读 STATUS + README + 当前 milestone 章节，回报状态。

或直接对 Claude 说：「读 docs/STATUS.md 然后告诉我现在该做什么」。

### 8.5 STATUS.md 写得太厚怎么办

STATUS.md 当前结构（三层分离后）只装"当前状态快照"+"Milestone 进度索引"，理论上**永远不会膨胀**——本 session 详情留在 issue closing comment，月度 deep dive 按月归档到 `_archive/sessions/`。

如果发现 STATUS.md 超过 60 行，通常说明：

- close-comment 没自包含，详情被错误塞回 STATUS（按 §8.3 规则修正下次 sync）
- 当前 milestone 索引过细（压成"M1 W1-W2 进行中（X/Y）+ 剩余 issue 列表"即可）

每月切归档文件（`_archive/sessions/2026-05.md` → `2026-06.md`），归档文件信息无损保留旧 session deep dive。

---

## 9. GitHub Issues 工作流

任务级清单不写在 STATUS.md 里 —— 用 GitHub Issues 管。详见 [github-workflow.md](github-workflow.md)。

### 9.1 三件最常用的事

| 场景 | 命令 |
|---|---|
| 开始 session | `/resumex`（读 STATUS + 最近 5 个开放 issue） |
| 接到新任务 | `/new-task <一句话描述>` |
| Session 结束 | `/sync-status`（关闭完成的 issue + 更新 STATUS） |

### 9.2 远端未接入时

`git remote -v` 没有 origin / `gh` 命令报错 / 网络拦截访问不到时：

- `/new-task` 会回退为把任务写到 STATUS.md 的「即将开始」临时区
- `/sync-status` 跳过 `gh issue close` 步骤，只更新 STATUS.md
- commit 步骤只 `git commit`，不 `git push`

接入步骤见 [github-workflow.md §8](github-workflow.md)。
