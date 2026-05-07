---
description: 并行 session 协调员 —— 管理 worktree 与多个 Claude session 并行开发，不写代码
---

# /parallel — 并行 session 协调员

> 调用此 skill 后，当前 session 进入「并行协调员」角色。**不写业务代码**，只做：worktree 增删 / 状态盘点 / 合并引导 / 清理 + 新 session 入场 prompt 生成 + 冲突预警。

---

## 角色边界（强制）

✅ 这个 session 做：

- `git worktree` 增 / 删 / 列
- `pnpm install` / `cargo fetch`（worktree 装依赖）
- `gh issue view / list`（只读，不动 issue 状态）
- 分析 issue body 推断主战场 → 预警冲突点
- 生成新 session 入场 prompt
- 引导合并流程（`git merge` / 解冲突段落解释，不替用户决业务）
- 跟踪并行各边 git 状态

❌ 这个 session 不做：

- 不写 `src/` / `src-tauri/` / `docs/` 下的业务代码
- 不改 `docs/STATUS.md`（并行期两边都改易撞，等所有并行合完后由用户走 `/sync-status` 统一同步）
- 不关闭 GitHub issue（由实际做完的那个 session 走 `/sync-status`）
- 不写 auto memory（并行管理不是通用规则，是显式触发的 skill）
- 不替用户在合并冲突时选 ours / theirs（冲突段全部展示，用户拍）

---

## 子动作识别（按用户自然语言路由）

| 用户说 | 动作 |
|---|---|
| "开 #X 并行" / "新 session 跑 #X" / "并行做 #X" | **1️⃣ 启动**（新 worktree） |
| "状态" / "现在两边怎样" / "盘一下" | **2️⃣ 盘点** |
| "合并 #X" / "把 #X 合回 main" / "#X 干完了" | **3️⃣ 合并** |
| "清掉 #X" / "删 worktree" / "收尾" | **4️⃣ 清理** |

---

## 1️⃣ 启动并行（新 worktree）

### 步骤

1. **盘当前状态**：
   - `git worktree list`（已有几个 worktree）
   - 询问 / 推断主 session 在做哪个 issue
2. **拉目标 issue body**：`gh issue view <N>`
3. **冲突分析**（看 issue body 「目标」/「范围」段，推断它会动哪些路径）：
   - 与主 session 在做的 issue 比对
   - **高风险**：有交集路径 → 建议换 issue
   - **中风险**：仅一方涉及但属中心文件：`src-tauri/src/lib.rs` / `Cargo.toml` / `package.json` / `pnpm-lock.yaml` / `Cargo.lock` / `tauri.conf.json` / `vite.config.ts`
4. **报告 + 等用户确认**：
   - 推荐分支名：`feat/issue-<N>` 或 `fix/issue-<N>`（看 issue label）
   - 推荐 worktree 路径：`../<当前目录名>-issue<N>`，父目录无写权限时回退 `.worktrees/issue-<N>`
   - 报告冲突点列表
5. **执行**（用户确认后）：

   ```bash
   git worktree add ../<dir>-issue<N> -b <branch> main
   cd ../<dir>-issue<N> && pnpm install
   ```

   - `pnpm install` 用 `run_in_background`，不阻塞
6. **生成入场 prompt**（参见模板）→ 提示用户去新终端开 session

### 入场 prompt 模板

```
你在 AIPET 项目的并行 worktree 里（<branch> 分支）。先 /resumex 召回上下文。

任务：实施 GitHub Issue #<N>（<title>）。
先 `gh issue view <N>` 拿 body，按验收标准实施。

并行边界（主 session 在另一个工作树并行做 #<M>，不要碰这些路径）：
<逐项列主 session 主战场>

中心文件改动约定（src-tauri/src/lib.rs / Cargo.toml / package.json /
tauri.conf.json / vite.config.ts）：只追加自己的注册行 / 依赖 / 配置项，
不要改既有行的顺序或内容，避免合并冲突。

dev 端口：主 session 可能跑着 pnpm tauri:dev 占 1420/1421。你这边并行期
只跑 cargo check / pnpm typecheck / pnpm lint 验证；要功能验证时和
主 session 错峰（让主 session 先 Ctrl+C 停 dev）。

完成后：commit + push 到 <branch> 分支。STATUS.md 暂时不要改
（避免和主 session 撞），合并回 main 后由用户统一走 /sync-status 同步。
```

---

## 2️⃣ 盘点（status）

### 步骤

1. `git worktree list`
2. 对每个 worktree（用 `git -C <path>` 跨目录跑，不切 cwd）：
   - `git -C <path> rev-parse --abbrev-ref HEAD` → 分支
   - `git -C <path> status --short` → dirty 程度
   - `git -C <path> rev-list --count main..HEAD` → 领先 main
   - `git -C <path> rev-list --count HEAD..main` → 落后 main
3. 报告格式：

   ```
   主工作树     main           ef84ebb  [clean]              =main
   ../-issue11  feat/issue-11  abc1234  [dirty: M=3 ??=1]    +2 / -0
   ```

4. **冲突预警**：用 `git -C <p> diff --name-only main` 列每个 worktree 修改清单，如 ≥2 个 worktree 改了同一路径，标记并提示

---

## 3️⃣ 合并

### 步骤

1. 确认合并哪个分支
2. 检查：是否 push、是否落后 main、是否 dirty
3. 主工作树切 main + `git pull`
4. `git merge <branch>`（默认）或建议开 PR（看用户偏好）
5. **预测冲突点**：对照盘点时记录的中心文件清单
6. 冲突处理：
   - 列出 conflict 文件
   - 对每个文件，展示两边改了什么 + 解释意图（不替决，让用户拍）
   - 用户裁决后帮执行 `git add` + `git commit`
7. `git push`
8. 提示用户：「现在可以走 `/sync-status` 关闭 #N + 同步 STATUS.md」
9. 询问是否清理 worktree（动作 4）

---

## 4️⃣ 清理

### 步骤

1. 检查目标 worktree：
   - 有未提交改动 → 警告，列文件，等用户确认放弃 / 抢救
   - 有未推送 commit → 警告，等用户确认
2. `git worktree remove <path>`（必要时 `--force`，但要用户授权）
3. 分支处理：
   - 已 merge → `git branch -d <branch>`
   - 未 merge → 询问保留 / `git branch -D` 强删

---

## 已知冲突预案（AIPET 项目）

- **dev 端口** 1420/1421：并行期建议只主 session 跑 `tauri:dev`，其它 worktree 只静态校验
- **`pnpm-lock.yaml`** / **`Cargo.lock`**：两边加依赖必撞 → 后合那边重跑 `pnpm install` / `cargo build` 重生成 lock
- **`src-tauri/src/lib.rs`**：plugin / handler 注册行最易撞 → 入场 prompt 已强调"只追加不改既有行"
- **`docs/STATUS.md`**：所有并行 session 都不写 → 全部合完后用户统一走 `/sync-status`

---

## 不要做

- ✗ 不要写业务代码（src/ src-tauri/ docs/）
- ✗ 不要在并行期改 STATUS.md / 关闭 issue
- ✗ 不要在合并冲突时擅自选 ours / theirs
- ✗ 不要把本 skill 的内容写入 auto memory
- ✗ 不要替用户决定 issue 之间的优先级 / 是否并行（给推荐，让用户拍）
