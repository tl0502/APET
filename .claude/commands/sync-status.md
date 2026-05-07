---
description: session 末同步：关闭已完成 issue + 更新 STATUS.md
---

# /sync-status — Session 末同步

session 结束前调用。把本 session 的成果同步到 GitHub Issues 和 `docs/STATUS.md`。

## 步骤

1. **盘点本 session 完成的事**：
   - 我会告诉你哪些干完了；或你从对话上下文里推断
   - 列出涉及的 issue 编号（如果之前用 `/new-task` 创建过）

2. **关闭已完成的 issue（close-comment 必须自包含）**：

   ```bash
   gh issue close <number> --comment "<自包含的落地报告>"
   ```

   close-comment 必须包含 5 项（缺一不可）：

   - **commit hash**（主体 commit + 关键 fix commit；多个用逗号分隔）
   - **做了什么**（实做文件清单 / 关键代码改动；不重复 issue body 字面规划）
   - **关键偏离 / 取舍**（实施时偏离 issue body 的所有项；这部分**最重要**，是未来 session 召回时最容易踩的坑）
   - **实测**（typecheck / lint / cargo test / 视觉验证 等具体结果）
   - **Follow-up**（后续解锁的 issue 编号 + 一句话提示，方便链路追溯）

   **❌ 禁止做法**：close-comment 只写"详见 STATUS.md 第 X 行"或"详见 commit message"——STATUS.md 会被定期归档/瘦身，commit message 也不易跨 issue 检索；这种指针式 comment 在未来 session 召回时会**断链**。issue body+comment 必须是**自包含的事实库**（[CLAUDE.md 信息流](../../CLAUDE.md) 的硬性要求）。

   参考样板：[#5](https://github.com/tl0502/APET/issues/5#issuecomment-4400413857) / [#7](https://github.com/tl0502/APET/issues/7#issuecomment-4395995504) / [#13](https://github.com/tl0502/APET/issues/13#issuecomment-4400114957)（结构清晰、5 项齐全）。

3. **更新 STATUS.md**（结构已是三层分离，**保持精简**）：
   - 「当前状态」四行：
     - `当前 session 在做` → 改为下一个目标，或写「—」
     - `下一步` → 推进到下一项（含下一个 issue 编号）
     - `阻塞` → 如有新阻塞写上，没有写「无」
   - 「Milestone 进度」：仅在 milestone 完成度变化时改一行（如 `13/18 → 14/18`，剩余 issue 列表）
   - **不要**在 STATUS.md 里写本 session 的逐项细节——那些信息**已经在 issue closing comment 里了**（自包含原则）；STATUS.md 永远只装"当前状态快照"+"指针"
   - **历史 session 摘要**：归档到 `docs/_archive/sessions/YYYY-MM.md`（按月切文件）；STATUS.md 主体里**没有**这一节

4. **如果有新决策**：
   - 提示我是否要写到 `docs/decisions.md`（ADR-016+）
   - 不要自作主张写 ADR；问我

5. **提示是否 commit & push**：
   - 列出本 session 修改 / 新建的文件清单
   - 给出 commit message 草稿（`<type>: <subject>` 风格）
   - 等我确认后再执行 `git add` + `git commit` + `git push`

## 远端未接入时

如果远端还没接入：

- 跳过 `gh issue close` 步骤
- 只更新 STATUS.md
- commit & push 步骤改为：只 `git add` + `git commit`，不 push

## 不要做

- ✗ 不要批量关闭未确认完成的 issue
- ✗ 不要写**指针式** close-comment（"详见 STATUS"/"详见 commit"），必须**自包含**含 5 项要素
- ✗ 不要在 STATUS.md 里维护任务清单（任务在 Issues 里）
- ✗ 不要在 STATUS.md 里写本 session 详细流水（信息已在 issue closing comment）
- ✗ 不要把"以后小心什么"写进 STATUS——经验教训走 [docs/lessons.md](../../docs/lessons.md)
- ✗ 不要替我决定是否要写新 ADR；问我
