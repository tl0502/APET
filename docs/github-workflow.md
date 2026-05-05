---
title: GitHub Issues 工作流
updated: 2026-05-05
related:
  - ../CLAUDE.md
  - WORKFLOW.md
  - STATUS.md
  - roadmap/development-roadmap.md
---

# GitHub Issues 工作流

> 单人项目用 GitHub Issues 管任务，不用 GitHub Projects。本文件说明 labels / milestones / 命名约定 / 接入步骤。

---

## 1. 为什么用 Issues

| 场景 | 工具 |
|---|---|
| 当前在做什么、下一步做什么 | [STATUS.md](STATUS.md) 当前状态四行 |
| 任务清单（feat / fix / spike） | **GitHub Issues** |
| 已完成里程碑摘要 | STATUS.md「已完成」区 |
| 关键技术决策 | [decisions.md](decisions.md) |
| 详细需求 / 架构 | [requirements/](requirements/) / [architecture/](architecture/) |

**核心分工**：STATUS.md 是当前状态快照（一页）；Issues 是任务级真实源（含历史评论、状态、依赖）。

参考：[ccpm](https://github.com/automazeio/ccpm) 的精神（GitHub Issues as source of truth），但不装完整框架。

---

## 2. Labels 体系

### 2.1 type:* （必选 1 个）

| Label | 用途 |
|---|---|
| `type:feat` | 新功能（PRD 里的 A-Q 模块功能） |
| `type:fix` | 修 bug |
| `type:refactor` | 不改行为的重构 |
| `type:spike` | 调研 / 验证（如 RAWINPUT spike、组件库选型） |
| `type:chore` | 工具链、配置、依赖升级 |
| `type:docs` | 仅文档变更 |

### 2.2 module:* （必选 1 个，feat/fix 至少 1 个）

对应 PRD §7 的 A-Q 模块编号：

| Label | 模块 |
|---|---|
| `module:A-shell` | 桌宠壳层 |
| `module:B-chat` | 对话 |
| `module:C-reminder` | 提醒 |
| `module:D-pomodoro` | 番茄钟 |
| `module:E-todo` | 待办 |
| `module:F-memory` | 记忆 |
| `module:G-settings` | 设置 |
| `module:H-persona` | 人格系统 |
| `module:I-living` | 生命感 |
| `module:J-care` | 情境关心 |
| `module:K-bosskey` | 摸鱼模式 |
| `module:L-filedrop` | 文件拖入 |
| `module:M-pledge` | 灵魂宣誓 |
| `module:N-interact` | 物理交互 |
| `module:O-wardrobe` | 装扮 |
| `module:P-voice` | 声音表情 |
| `module:Q-game` | 小游戏 |
| `module:infra` | 基础设施（Migration / Crypto / Telemetry / Network / Updater） |

### 2.3 priority:* （可选）

- `priority:p0` —— 阻塞当前 milestone
- `priority:p1` —— 当前 milestone 应做但可推迟
- 不打 = 默认（normal）

### 2.4 status:* （可选状态标记）

- `status:blocked` —— 被外部因素阻塞（在 issue body 里写明阻塞原因）
- 其他状态用 issue 的 open/closed 表达即可，不打 label

---

## 3. Milestones 映射

GitHub 原生 milestones 对应路线图 M1-M5：

| Milestone | 周次 | 主交付（详见 [roadmap](roadmap/development-roadmap.md)） |
|---|---|---|
| `M1` | W1-W2 | 壳层 + 对话 |
| `M2` | W3-W4 | 任务三件套 + 物理交互 |
| `M3` | W5-W6 | 记忆 + 主动陪伴 |
| `M4` | W7-W8 | 装扮 + 声音 + 纪念日 |
| `M5` | W9-W10 | 小游戏 + 自测 |
| `P1` | M6+ | 上线后扩展（R1-R3） |

---

## 4. Issue 命名约定

格式：`<module-letter>: <subject>`

例：

- `B: ChatService MVP + 流式渲染`
- `N: hitbox 抗议规则（VecDeque 30s）`
- `H: 人格工坊三档编辑 GUI`
- `infra: Tauri 2.x + Vue 3 项目脚手架`

**反向链接**：issue body 里写一行 `Refs: docs/...md §X.Y` 指回基线文档对应章节。

---

## 5. Issue 模板

`.github/ISSUE_TEMPLATE/` 下三份模板：

- `feat.yml` —— 新功能
- `spike.yml` —— 调研验证
- `fix.yml` —— 修 bug

模板字段保持极简，关键是 **module + 反向链接 + 验收标准**。

---

## 6. 自定义 slash 命令

| 命令 | 用途 |
|---|---|
| `/resumexx` | 召回项目上下文（读 STATUS + 当前 milestone + 最近 5 个开放 issue） |
| `/new-task` | 从对话创建 issue（自动推断 type/module label，写反向链接） |
| `/sync-status` | session 末同步：关闭已完成 issue + 更新 STATUS |

详见 `.claude/commands/` 各命令说明文件。

---

## 7. 日常工作流

### 7.1 开始新 session

```
/resumex
```

Claude 会读 STATUS + 最近开放 issue + 当前 milestone 章节，回报状态。

### 7.2 接到新任务

对 Claude 说「开个 issue 跟踪 X」，或：

```
/new-task <一句话描述>
```

Claude 会推断 label 并调 `gh issue create` 创建。

### 7.3 干完一个 task

让 Claude 调 `gh issue close <number> --comment "<完成摘要>"`。

### 7.4 session 结束

```
/sync-status
```

Claude 会：

1. 列出本 session 关闭的 issue 编号
2. 更新 STATUS.md「当前状态」与「历史 session 摘要」
3. 提示是否要 commit & push

---

## 8. 接入步骤（已接入 https://github.com/tl0502/APET）

### 8.1 一次性准备

```bash
cd D:/Project/temp/4

# 1. 初始化 git
git init
git add .
git commit -m "docs: bootstrap baseline (M0 → M1) + 项目记忆 + Issues 工作流"

# 2. 接入远端
git remote add origin https://github.com/tl0502/APET.git
git branch -M main
git push -u origin main

# 3. 创建 milestones
gh api repos/:owner/:repo/milestones -f title=M1 -f description="壳层 + 对话（W1-W2）"
gh api repos/:owner/:repo/milestones -f title=M2 -f description="任务三件套 + 物理交互（W3-W4）"
gh api repos/:owner/:repo/milestones -f title=M3 -f description="记忆 + 主动陪伴（W5-W6）"
gh api repos/:owner/:repo/milestones -f title=M4 -f description="装扮 + 声音 + 纪念日（W7-W8）"
gh api repos/:owner/:repo/milestones -f title=M5 -f description="小游戏 + 自测（W9-W10）"
gh api repos/:owner/:repo/milestones -f title=P1 -f description="上线后扩展（M6+）"

# 4. 批量创建 labels
bash docs/scripts/init-labels.sh
```

### 8.2 init-labels.sh

完整脚本在 [docs/scripts/init-labels.sh](scripts/init-labels.sh)（27 个 label：6 type + 18 module + 2 priority + 1 status）。

### 8.3 网络代理（如需）

国内访问 GitHub 不稳定时，给 git / gh 配代理（按你浏览器使用的代理端口替换 `7890` —— 常见：Clash 7890 / v2rayN 10809 / 自建 1080）：

```bash
# 方法 A：环境变量（仅当前 shell）
export HTTPS_PROXY=http://127.0.0.1:7890
export HTTP_PROXY=http://127.0.0.1:7890

# 方法 B：git 全局配置（永久）
git config --global http.proxy http://127.0.0.1:7890
git config --global https.proxy http://127.0.0.1:7890

# 验证
gh auth status
git ls-remote origin
```

### 8.4 .gitignore

已在仓库根：[.gitignore](../.gitignore)。

---

## 9. 反模式（不要做）

- ✗ 不在 STATUS.md 维护任务复选框列表（重复 Issues）
- ✗ 不开 GitHub Projects 看板（Issues + milestones + labels 已够）
- ✗ 不装完整 ccpm 30+ 命令（单人 overkill）
- ✗ 不用 epic/sub-issue 父子关系（M? milestone 已是 epic）
- ✗ 不做并行 git worktree（一次干一条线）
- ✗ 不在 issue body 复制 PRD 内容（写反向链接 `Refs:`）

---

## 99. 附录

### 99.1 参考资料

- [automazeio/ccpm](https://github.com/automazeio/ccpm) —— Issue-driven 项目管理框架（启发，未全装）
- [GitHub Issue Forms 规范](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/syntax-for-issue-forms)
- [gh CLI 文档](https://cli.github.com/manual/)

### 99.2 待办

- [x] 仓库 URL 已填（https://github.com/tl0502/APET）
- [x] `docs/scripts/init-labels.sh` 已写完整版
