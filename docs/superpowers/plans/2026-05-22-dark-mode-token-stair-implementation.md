# Dark mode token 阶梯改造 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `src/styles/tokens.css` 的 light/dark mode surface elevation 改成「背峰式 light + 保守型 dark」，修 light L2/L3 倒序+同色 / dark 阶梯差太小 / dark border-faint 不可见 三个根因问题。

**Architecture:** 单文件 patch — 仅修改 `src/styles/tokens.css`。45 个消费者通过 CSS 变量自动继承新值。零 Vue / Rust 代码改动。配套同步 STATUS.md + 新增 ADR-024 + 关闭 issue #38。

**Tech Stack:** CSS custom properties，无新依赖。回归用现有 vitest 套件（293 cases，token 改不应触发任何测试失败）+ `pnpm tauri:dev` 手动 e2e 巡检 4 大窗 × light/dark 两态。

**关联文档:**
- Spec: [`docs/superpowers/specs/2026-05-22-dark-mode-token-stair-design.md`](../specs/2026-05-22-dark-mode-token-stair-design.md)
- Issue: [#38](https://github.com/tl0502/APET/issues/38)
- 父 issue: [#37](https://github.com/tl0502/APET/issues/37)（workspace 重设计 P3 复检暴露）
- Tokens 文件: [`src/styles/tokens.css`](../../../src/styles/tokens.css)

---

## File Structure

| 文件 | 责任 | 任务 |
|---|---|---|
| `src/styles/tokens.css` | 唯一代码改动 — light/dark CSS 变量定义 | Task 1 + Task 2 |
| `docs/STATUS.md` | M2 W3 进度同步（追加 #38 完成行） | Task 5 |
| `docs/decisions.md` | 新增 ADR-024（dark/light surface elevation 阶梯设计原则） | Task 6 |

无新建文件，无新测试，无组件改动。

---

## Task 1: Light mode 背峰式 patch

**Files:**
- Modify: `src/styles/tokens.css` 行 28-42（light mode `:root` 块）

**Goal:** 把 L2/L3 改成 #ffffff（与 L0 同色，靠 shadow + border 分层），border-faint 6% → 8%。

- [ ] **Step 1: 修改 light mode 4 个 surface token + border-faint**

定位 `src/styles/tokens.css:28-42`，把以下两行替换：

**Old（行 30）:**
```css
  --aipet-color-surface: #fafafa; /* 卡片 / content area / 气泡浮起层(L2)；与 dark 主题对称四层阶梯 */
```

**New:**
```css
  --aipet-color-surface: #ffffff; /* L2 卡片 — 与 L0 同色，靠 shadow + border 浮起（Bear/Linear/MacOS Big Sur 通行）*/
```

**Old（行 42）:**
```css
  --aipet-color-border-faint: rgba(0, 0, 0, 0.06);
```

**New:**
```css
  --aipet-color-border-faint: rgba(0, 0, 0, 0.08);
```

注：`--aipet-color-surface-raised: #ffffff`（行 31）原本已是 #ffffff，无需改动；现在意义从「与 L0 同色是 bug」转为「与 L0 同色 + 更强 shadow 是设计」。

- [ ] **Step 2: 同步顶部注释**

把 `src/styles/tokens.css` 行 10-11 的旧 elevation 注释：

**Old:**
```css
 * 亮色 elevation(2 档):
 *   bg(#ffffff,主区) → surface-soft(#f5f5f5 neutral-100,sidebar) → surface/raised(#ffffff 浮起)
```

**New:**
```css
 * 亮色 elevation(背峰式 3+1):
 *   bg(#ffffff,L0 主区) / surface-soft(#f5f5f5 neutral-100,L1 sidebar)
 *     / surface(#ffffff,L2 卡片) / surface-raised(#ffffff,L3 modal)
 *   —— L0/L2/L3 同色，靠 shadow + border 分层（Bear/Linear/MacOS Big Sur 通行）
```

- [ ] **Step 3: 验证语法**

Run: `pnpm vue-tsc --noEmit` 或 `pnpm typecheck`
Expected: PASS（CSS 改动不影响 TS，但顺手确认仓库未崩）

实际：仓库无 `typecheck` script，直接走 vitest：

Run: `pnpm test -- --run`
Expected: 293/293 全绿（token 改动不应影响任何单测）

---

## Task 2: Dark mode 保守型 4 色阶 patch

**Files:**
- Modify: `src/styles/tokens.css` 行 146-196（dark mode `:root.dark` 块）

**Goal:** L1 #1c1c1c → #1f1f1f / L2 #262626 → #2a2a2a / L3 #2e2e2e → #333333，border-faint 6% → 10%，bubble-assistant 跟 L2 → #2a2a2a。

- [ ] **Step 1: 修改 dark mode 4 个 surface token**

定位 `src/styles/tokens.css:154-157`，整段替换：

**Old:**
```css
  --aipet-color-bg: #171717; /* L0:主区 / 全窗背景(neutral-900) */
  --aipet-color-surface-soft: #1c1c1c; /* L1:sidebar / 二级面板 */
  --aipet-color-surface: #262626; /* L2:卡片 / hover / 气泡浮起(neutral-800) */
  --aipet-color-surface-raised: #2e2e2e; /* L3:dropdown / modal / dialog 顶层 */
```

**New:**
```css
  --aipet-color-bg: #171717; /* L0:主区 / 全窗背景(neutral-900) */
  --aipet-color-surface-soft: #1f1f1f; /* L1:sidebar / 二级面板（旧 #1c1c1c，+3 拉开与 L0 至 +8） */
  --aipet-color-surface: #2a2a2a; /* L2:卡片 / hover / 气泡浮起（旧 #262626，+4 拉开与 L1 至 +11） */
  --aipet-color-surface-raised: #333333; /* L3:dropdown / modal / dialog 顶层（旧 #2e2e2e，+5 拉开与 L2 至 +9） */
```

- [ ] **Step 2: 修改 border-faint**

定位 `src/styles/tokens.css:165`：

**Old:**
```css
  --aipet-color-border-faint: rgba(255, 255, 255, 0.06);
```

**New:**
```css
  --aipet-color-border-faint: rgba(255, 255, 255, 0.10);
```

- [ ] **Step 3: 同步 bubble-assistant 跟 L2**

定位 `src/styles/tokens.css:177`：

**Old:**
```css
  --aipet-color-bubble-assistant: #262626; /* L2 浮层,在 bg #171717 上是"卡片"感 */
```

**New:**
```css
  --aipet-color-bubble-assistant: #2a2a2a; /* 跟 L2 surface（L2 由 #262626 → #2a2a2a） */
```

- [ ] **Step 4: 同步顶部注释**

把 `src/styles/tokens.css` 行 13-15 的旧 dark elevation 注释：

**Old:**
```css
 * 暗色 elevation(4 档,Linear 风):
 *   bg(#171717 neutral-900,主区) → surface-soft(#1c1c1c,sidebar)
 *     → surface(#262626 neutral-800,卡片/hover/气泡) → surface-raised(#2e2e2e,modal/dropdown)
```

**New:**
```css
 * 暗色 elevation(保守型 4 色阶,Linear/Bear 风):
 *   bg(#171717 L0,主区) → surface-soft(#1f1f1f L1,sidebar,+8)
 *     → surface(#2a2a2a L2,卡片/气泡,+11) → surface-raised(#333333 L3,modal,+9)
 *   —— 每档差 ≥8 单位（人眼可分辨下限 ~6），总跨 28
```

- [ ] **Step 5: 跑回归**

Run: `pnpm test -- --run`
Expected: 293/293 全绿。

---

## Task 3: pnpm tauri:dev 手动 e2e 巡检（4 窗 × 2 主题）

**Files:** 无 — 这是 dev 环境手动验证 checkpoint。

**Goal:** 起 dev 实环境，按 spec §7 巡检清单逐窗对比 light/dark 两态，确认 token 改造未引入回归且 dark 分层感清晰。

- [ ] **Step 1: 起 dev 环境**

Run: `pnpm tauri:dev`
Expected: 应用启动到 workspace 主窗 + pet 桌宠窗（chat / onboarding 窗按需弹）。

- [ ] **Step 2: workspace 窗巡检（先 light，后 dark）**

切换 workspace → 设置 → 外观 → 切换 light 和 dark：

- **L 框 vs detail 分层**：dark 模式下 brand-bar / sidebar / master（都用 surface-soft #1f1f1f）与 detail（bg #171717）应明显有「上下两档」立体感，不再「像同一色板」。
- **panel divider**：detail 内 panel__title / panel__content 之间的 border-faint 应肉眼可见但不抢戏（dark 10% 白线 / light 8% 黑线）。
- **Profile popup**：点 user-avatar 入口，overlay 浮起的 popup（panel 用 bg #ffffff/#171717，sidebar 用 surface-soft）应显著浮起于 workspace 之上。

判据通过：dark workspace 主壳分层清晰；popup 浮卡感强；light 不退化。

- [ ] **Step 3: chat 主床 + 磁吸子窗巡检**

进入 workspace 主壳 chat tab + 唤起独立 chat 窗（如有快捷键），对比两形态：

- **assistant 气泡 vs thread 背景**：dark 模式下 bubble-assistant（#2a2a2a）与 thread（bg #171717）应有清晰对比；border-faint 10% hairline 加强分层。light 模式 bubble-assistant 仍是 #fafafa，未改。
- **composer 浮卡**：composer 用 shadow-composer-soft + 1px border 浮在 thread 上方，dark 模式下应比改造前更显眼（因 thread 背景未变但 sidebar L1 拉到 #1f1f1f）。

判据通过：assistant 气泡与背景区分清晰；composer 浮卡感不弱于改造前。

- [ ] **Step 4: pet 桌宠窗 + bubble 巡检**

让 pet 显示一条 reminder / onboarding bubble：

- **PetReminderBubble / PetOnboardingBubble**：bubble 用 surface (L2 = #2a2a2a in dark)，在桌面透明背景上浮卡感保持。
- bubble border-faint 10% 在 dark 下 hairline 应可见但克制。

判据通过：pet bubble 视觉无回归。

- [ ] **Step 5: onboarding 窗巡检（如能触发）**

新用户路径或 dev 触发 OnboardingApp：

- **卡片浮起**：选项卡 / persona picker 卡片用 surface (L2)，在 bg (L0) 上浮起感保持。
- dark 模式 #2a2a2a 卡片 vs #171717 bg = +19 单位对比 → 比改造前 +15 单位略强。

判据通过：onboarding 选项卡浮起感清晰。

- [ ] **Step 6: TokensPreview dev 页（如能访问）**

如 dev 路由能进 `_dev/TokensPreview.vue`，对照 light/dark 两态 color swatch 表，确认所有 elevation 阶梯视觉差均超 6 单位下限。

判据通过：所有 surface 色块都能用肉眼区分。

- [ ] **Step 7: 报告巡检结论**

向用户报告 §7 spec 清单的 5 条判据（dark L 框分层 / dark popup 浮卡 / composer 浮卡 / light 不退化 / modal 浮起）是否全部通过。任何一条不通过就回到 Task 1/2 调数值。

---

## Task 4: Commit token 改造

**Files:**
- Stage: `src/styles/tokens.css`

- [ ] **Step 1: 检查改动**

Run: `git diff src/styles/tokens.css`
Expected: 仅 5-6 处 token value 改动 + 2 处注释更新，无别处误触。

- [ ] **Step 2: stage + commit**

```bash
git add src/styles/tokens.css
git commit -m "feat: #38 dark mode token 阶梯改造 + light 背峰式

Light mode（背峰式 3+1）：
- surface #fafafa → #ffffff（L2 与 L0 同色，靠 shadow + border 浮起）
- surface-raised #ffffff 不变（语义从'与 L0 同色是 bug'转为'同色 + 更强 shadow 是设计'）
- border-faint 6% → 8%

Dark mode（保守型 4 色阶）：
- surface-soft #1c1c1c → #1f1f1f（+3，L0→L1 至 +8）
- surface #262626 → #2a2a2a（+4，L1→L2 至 +11）
- surface-raised #2e2e2e → #333333（+5，L2→L3 至 +9）
- border-faint 6% → 10%
- bubble-assistant #262626 → #2a2a2a（跟 L2）

总跨 28，每档差 ≥8 单位（人眼可分辨下限 ~6）。
Linear/Bear 锚，与 Apple/Bear 中灰路线一致，chat 气泡无需重校。

Closes #38"
```

Expected: 1 file changed, ~10 insertions(+), ~10 deletions(-)

---

## Task 5: 同步 STATUS.md

**Files:**
- Modify: `docs/STATUS.md`

**Goal:** M2 W3 进度从 8/8 改为 9/9（或新增 #38 完成行），更新 current session 字段。

- [ ] **Step 1: 读当前 STATUS.md**

Run: 通过 Read 工具查 `docs/STATUS.md`，定位 M2 W3 段落与 current session 字段。

- [ ] **Step 2: 改 current session 行**

定位「当前 session 在做」字段（约第 23 行），改为：

```markdown
- **当前 session 在做**：[#38](https://github.com/tl0502/APET/issues/38) dark mode token 阶梯改造 — tokens.css 单文件 patch（light 背峰式 + dark 保守型 4 色阶 + border-faint 8%/10% + dark bubble-assistant 跟 L2）— 293/293 vitest pass，4 大窗 × 2 主题手动巡检全绿
```

「下一步」字段（约第 24 行）改为：

```markdown
- **下一步**：[#29](https://github.com/tl0502/APET/issues/29) Todo + #21 KV 实例化 + LivingPet hook + AI 拆解 IPC 占位 → [#23](https://github.com/tl0502/APET/issues/23) 物理交互 + 心情/精力 + 摸鱼
```

- [ ] **Step 3: 改 M2 milestone 进度行**

定位「**当前 milestone**」字段（约第 22 行），改为：

```markdown
- **当前 milestone**：M2 W3 进行中（9/9 落地 ✅；待办 + 物理交互待办）
```

- [ ] **Step 4: 在 M2 段尾追加 #38 完成行**

在 `### M2 W3-W4` 段当前 8 项 ✅ 列表后追加（[#37](https://github.com/tl0502/APET/issues/37) 行之后）：

```markdown
- ✅ [#38](https://github.com/tl0502/APET/issues/38) [设计系统] dark mode token 阶梯改造：tokens.css 单文件 patch（light 背峰式 3+1 + dark 保守型 4 色阶 总跨 28 + border-faint 6%→8%/10% + dark bubble-assistant 跟 L2）— 1 commit + spec/plan/ADR-024，4 大窗 × 2 主题手动 e2e 全绿
```

同时把原「⏳ [#38] ...」一行删除。

- [ ] **Step 5: 改 updated 字段**

frontmatter 行 2：`updated: 2026-05-22` 保持不变（同日）。

---

## Task 6: 新增 ADR-024（dark/light surface elevation 阶梯设计原则）

**Files:**
- Modify: `docs/decisions.md` — 在 ADR-023 之后、「命名约定」之前插入 ADR-024

**Goal:** 把本次 token 改造的设计原则沉淀为 ADR，方便后续 token 调整时参考锚点。

- [ ] **Step 1: 在 decisions.md ADR-023 之后插入 ADR-024**

定位 `docs/decisions.md:245`（ADR-023 结尾「关联：#37」那行之后）+ `docs/decisions.md:247`（`---` 分隔符）。在 `---` 之后、`## 命名约定` 之前插入：

```markdown
### ADR-024 dark / light surface elevation 阶梯设计原则

- **为什么**：[#37](https://github.com/tl0502/APET/issues/37) 落地 workspace L 型 chrome 框后复检发现 ([#38](https://github.com/tl0502/APET/issues/38))：light mode L1 比 L2 更暗（倒序）+ L3 与 L0 同色 / dark mode 4 色阶差只有 5 单位（低于人眼可分辨下限 ~6）/ dark border-faint 6% 几乎不可见 — 三个系统性 token 问题。
- **选了什么**：
  - **Light 背峰式 3+1**：L0(#ffffff) / L1(#f5f5f5 sidebar) / L2(#ffffff) / L3(#ffffff)，L0/L2/L3 同色靠 shadow + border 分层（Bear/Linear/MacOS Big Sur 通行）。
  - **Dark 保守型 4 色阶**：L0(#171717) / L1(#1f1f1f, +8) / L2(#2a2a2a, +11) / L3(#333333, +9)，总跨 28，每档差 ≥8 单位。
  - **border-faint**：dark 6% → 10% / light 6% → 8%。
  - **dark bubble-assistant** 跟 L2 surface（#2a2a2a）。
  - 衍生 token（code-bg / surface-blur / shadow alpha）不动 —— 差 4 单位仍可见 / frosted blur 微差肉眼难感知 / 避免大面积视觉漂移。
- **代价**：
  - light 模 4 色阶设计不再对称（L0=L2=L3=#ffffff，仅 L1 单点偏灰），mental model 不如 dark 直观；换来 light 模 desktop 软件常见做法的一致性。
  - dark 模 4 色阶总跨 28 仍略保守（vs Discord ~35-40），桌宠陪伴语境暂不进 Discord 强工具感。
  - 改 surface token 影响 45 个消费者，需要全窗口手动巡检；落地后无回归。
- **关联**：spec [`docs/superpowers/specs/2026-05-22-dark-mode-token-stair-design.md`](superpowers/specs/2026-05-22-dark-mode-token-stair-design.md)；实施 commit（Task 4 待生成 hash 替换）；父 issue [#37](https://github.com/tl0502/APET/issues/37)。
- **后续扩展锚**：未来新增 elevation 档（如 L4 toast 浮层）按相同原则补 +8~12 单位差 + 同色靠 shadow（light 路线）。

---
```

- [ ] **Step 2: 改 frontmatter updated 字段**

定位 `docs/decisions.md:3`：

**Old:**
```yaml
updated: 2026-05-20
```

**New:**
```yaml
updated: 2026-05-22
```

- [ ] **Step 3: 改文件末「命名约定」空闲号**

定位 `docs/decisions.md:251`：

**Old:**
```markdown
新决策：`D-<NNN>-<kebab-case-title>`，编号单调递增。当前空闲：**ADR-024**。
```

**New:**
```markdown
新决策：`D-<NNN>-<kebab-case-title>`，编号单调递增。当前空闲：**ADR-025**。
```

---

## Task 7: 把 Task 4 实际 commit hash 回填 ADR-024

**Files:**
- Modify: `docs/decisions.md` ADR-024 「关联」段

**Goal:** Task 4 commit 完后才有 hash，回填占位文本。

- [ ] **Step 1: 取 commit hash**

Run: `git log --oneline -5`
Expected: 看到 `feat: #38 dark mode token 阶梯改造` 那条 commit 的短 hash（如 `abc1234`）。

- [ ] **Step 2: 把占位替换**

定位 ADR-024 「关联」行：

**Old:**
```markdown
实施 commit（Task 4 待生成 hash 替换）；
```

**New（用实际 hash 替换 abc1234）:**
```markdown
实施 commit `abc1234`；
```

---

## Task 8: Commit 文档同步

**Files:**
- Stage: `docs/STATUS.md` + `docs/decisions.md`

- [ ] **Step 1: 检查改动**

Run: `git diff docs/STATUS.md docs/decisions.md`
Expected: STATUS.md 4 行改动（current session / 下一步 / milestone / #38 完成行），decisions.md 新增 ADR-024 段 + 2 处小修。

- [ ] **Step 2: stage + commit**

```bash
git add docs/STATUS.md docs/decisions.md
git commit -m "docs: #38 STATUS 同步 + ADR-024 dark/light surface elevation 阶梯"
```

Expected: 2 files changed.

---

## Task 9: 关闭 issue #38

**Files:** 无 — GitHub 操作。

- [ ] **Step 1: 写 closing comment 并关闭**

Run（用 HEREDOC 防换行错乱）:

```bash
gh issue close 38 --comment "$(cat <<'EOF'
## 落地总结

单文件 patch `src/styles/tokens.css`，3 commit（spec + token + docs）。

### 实施

- `src/styles/tokens.css` light mode（背峰式 3+1）：L2 #fafafa → #ffffff / border-faint 6% → 8% / 注释更新
- `src/styles/tokens.css` dark mode（保守型 4 色阶）：L1 #1c1c1c → #1f1f1f / L2 #262626 → #2a2a2a / L3 #2e2e2e → #333333 / border-faint 6% → 10% / bubble-assistant 跟 L2 / 注释更新

### 数值审计

| 档 | 旧 dark | 新 dark | 旧间距 | 新间距 |
|---|---|---|---|---|
| L0→L1 | #171717→#1c1c1c | #171717→#1f1f1f | +5 | +8 |
| L1→L2 | #1c1c1c→#262626 | #1f1f1f→#2a2a2a | +10 | +11 |
| L2→L3 | #262626→#2e2e2e | #2a2a2a→#333333 | +8 | +9 |
| 总跨 | | | 23 | 28 |

light：L1=#f5f5f5 不变；L2/L3 由各异色统一为 #ffffff，靠 shadow + border 分层。

### 巡检

`pnpm test`：293/293 全绿。

`pnpm tauri:dev` 4 大窗 × 2 主题手动 e2e：
- ✅ dark workspace L 框（surface-soft #1f1f1f）与 detail（bg #171717）明显分层
- ✅ dark Profile popup overlay 显著浮起于 workspace 上
- ✅ chat assistant 气泡（#2a2a2a）与 thread 背景（#171717）清晰区分
- ✅ pet bubble / onboarding 卡片浮起感保持
- ✅ light 模无回归（L1 sidebar 与主区对比 + modal 浮起感不弱于改前）

### 文档

- spec：`docs/superpowers/specs/2026-05-22-dark-mode-token-stair-design.md`
- plan：`docs/superpowers/plans/2026-05-22-dark-mode-token-stair-implementation.md`
- ADR-024：dark/light surface elevation 阶梯设计原则（decisions.md）
- STATUS.md：M2 W3 9/9 ✅

### 衍生 token 未动（设计决策）

- `--aipet-color-code-bg`（dark #262626）：与新 L2 #2a2a2a 差 4 仍可辨
- `--aipet-color-surface-blur`：frosted blur 微差肉眼难感知
- `--aipet-shadow-*` alpha：避免大面积视觉漂移
- `bubble-user` / `primary` / status / focus ring：与 elevation 无关

后续需要新增 elevation 档（如 L4 toast 浮层）按 ADR-024 原则补 +8~12 单位差。
EOF
)"
```

Expected: gh CLI 输出 `https://github.com/tl0502/APET/issues/38` 已关闭。

- [ ] **Step 2: 验证 issue 已关**

Run: `gh issue view 38 --json state -q .state`
Expected: `CLOSED`

---

## 自检清单（Plan 完成判据）

落地后逐项核对：

- [ ] `src/styles/tokens.css` light L2 = #ffffff，border-faint = rgba(0,0,0,0.08)
- [ ] `src/styles/tokens.css` dark L1/L2/L3 = #1f1f1f/#2a2a2a/#333333，border-faint = rgba(255,255,255,0.10)，bubble-assistant = #2a2a2a
- [ ] `src/styles/tokens.css` 顶部注释 light/dark elevation 段已更新
- [ ] `pnpm test` 293/293 pass
- [ ] 4 大窗 × 2 主题手动巡检 5 条判据全绿
- [ ] STATUS.md M2 W3 9/9 + #38 完成行已写
- [ ] decisions.md ADR-024 已加 + 空闲号变 ADR-025 + commit hash 已回填
- [ ] `gh issue view 38` state = CLOSED
- [ ] 共 3 个 commit：spec（4ef019d 已有）+ token feat + docs

---

## 风险与回退

- **风险 1**：手动巡检发现 dark workspace L 框与 detail 分层仍不够 → 回到 Task 2 把 L1 拉到 #202020（+3 单位再加）。
- **风险 2**：dark assistant 气泡与 thread 背景区分仍弱 → MessageBubble 局部加 border-faint 1px hairline（不改 token）。
- **风险 3**：light 模 modal 浮起感弱 → 提高 `--aipet-shadow-float` alpha 0.10 → 0.14（出 ADR-024 范围外的微调）。

回退方法：`git revert <token commit hash>` 一条命令回到改前状态。
