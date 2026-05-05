---
description: session 末同步：关闭已完成 issue + 更新 STATUS.md
---

# /sync-status — Session 末同步

session 结束前调用。把本 session 的成果同步到 GitHub Issues 和 `docs/STATUS.md`。

## 步骤

1. **盘点本 session 完成的事**：
   - 我会告诉你哪些干完了；或你从对话上下文里推断
   - 列出涉及的 issue 编号（如果之前用 `/new-task` 创建过）

2. **关闭已完成的 issue**：

   ```bash
   gh issue close <number> --comment "<一段话完成摘要>"
   ```

   摘要要点：做了什么 + 关键决策 / 取舍（如有）+ 后续 follow-up issue 编号（如有）。

3. **更新 STATUS.md**：
   - 「当前状态」四行：
     - `当前 session 在做` → 改为下一个目标，或写「—」
     - `下一步` → 推进到下一项
     - `阻塞` → 如有新阻塞写上，没有写「无」
   - 「已完成」区：里程碑级成就才加（不是每个 issue 都写）
   - 「历史 session 摘要」按日期倒序加一段：
     - 一段话总结本 session 干了什么
     - 列出关闭的 issue 编号链接（如有）

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
- ✗ 不要把每个小步骤都写进 STATUS.md（颗粒粗一些，里程碑级即可）
- ✗ 不要替我决定是否要写新 ADR；问我
- ✗ 不要在 STATUS.md 里维护任务清单（任务在 Issues 里）
