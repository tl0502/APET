---
title: Dark mode token 阶梯改造设计
updated: 2026-05-22
related:
  - ../../../decisions.md
  - ../../../STATUS.md
  - ../../../../src/styles/tokens.css
---

# Dark mode token 阶梯改造 设计文档

> 对应 issue [#38](https://github.com/tl0502/APET/issues/38)。Phase 2 单独 session，承接 #37 workspace 重设计 P3 落地后复检发现的系统性 token 问题。

## 1. 背景

#37 落地 workspace L 型 chrome 框（topbar + sidebar + master 用 `surface-soft`，detail 用 `bg`）之后复检视觉发现 dark mode 下 L 框与 detail 主舞台「像同一色板」。深入诊断三个系统性问题：

1. **Light mode L2 / L3 阶梯倒序 + 同色**
   - L1 surface-soft `#f5f5f5` 比 L2 surface `#fafafa` 更暗 → 阶梯反向
   - L3 surface-raised `#ffffff` 与 L0 bg 完全同色 → modal 没有色差
2. **Dark mode 4 色阶差太小**
   - L0→L1 仅 +5 单位（`#171717` → `#1c1c1c`），低于人眼可分辨下限（~6）
   - 直接根因：workspace L 框「像同色板」
3. **Dark mode border-faint 不可见**
   - `rgba(255,255,255,0.06)` 6% 白色在暗底基本看不见 → divider / panel 边界几乎不存在

这是 token 级问题，影响所有 4 个窗口（workspace / pet / chat / onboarding）。

## 2. 设计目标

- 修 light mode 倒序 + 同色 → 改背峰式（L0=L2=L3=#ffffff，靠 shadow + border 分层）
- 拉大 dark mode 4 层阶梯差至 ≥8 单位 → 锚 Linear/Bear 保守型，总跨 28
- 提升 dark mode border-faint 可见度 → 6% → 10%
- 跟随调整 dark `bubble-assistant` 保持「比 thread 背景高一档」的浮起表达

## 3. 不在范围内

- 不改 `--aipet-color-border` / `--aipet-color-border-strong`（实色 border 已够强）
- 不改 `--aipet-color-code-bg`（旧 #262626 与新 L2 #2a2a2a 差 4 仍可辨）
- 不改 `--aipet-color-surface-blur`（frosted blur 背景，1-2 单位差肉眼难感知）
- 不改 `--aipet-shadow-*` alpha（避免大面积视觉漂移；新 elevation 在现有 shadow 下表现仍 OK）
- 不改 bubble-user / primary / status / focus ring 系列（与 elevation 无关）
- 不写自动化截图 diff 测试 —— 单人项目，4 大窗 × 2 主题 = 8 截图手动对照即可

## 4. 架构

**单文件 patch**：仅修改 `src/styles/tokens.css`。45 个消费者通过 CSS 变量自动继承新值。

无 Vue 组件代码改动。无 Rust 代码改动。无新增依赖。

## 5. Token 具体数值

### 5.1 Light mode（背峰式 3+1）

```css
:root {
  --aipet-color-bg: #ffffff;             /* L0 主区 / 全窗背景 */
  --aipet-color-surface-soft: #f5f5f5;   /* L1 sidebar / 二级面板 (neutral-100) */
  --aipet-color-surface: #ffffff;        /* L2 卡片 — 同色 + shadow + border 浮起 */
  --aipet-color-surface-raised: #ffffff; /* L3 modal — 同色 + 更强 shadow */
  --aipet-color-border-faint: rgba(0, 0, 0, 0.08); /* 6% → 8% */
}
```

**核心理念**：light 模不靠色阶分层，靠 shadow + border 分层。Bear / Linear / MacOS Big Sur 通行做法。L2/L3 都是纯白，elevation 差异由 `--aipet-shadow-base` < `--aipet-shadow-lg` < `--aipet-shadow-float` 表达。

### 5.2 Dark mode（保守型 4 色阶）

```css
:root.dark {
  --aipet-color-bg: #171717;             /* L0 主区 (neutral-900, 不变) */
  --aipet-color-surface-soft: #1f1f1f;   /* L1 sidebar (旧 #1c1c1c, +3 → +8 vs L0) */
  --aipet-color-surface: #2a2a2a;        /* L2 卡片/气泡 (旧 #262626, +4 → +11 vs L1) */
  --aipet-color-surface-raised: #333333; /* L3 modal (旧 #2e2e2e, +5 → +9 vs L2) */
  --aipet-color-border-faint: rgba(255, 255, 255, 0.10); /* 6% → 10% */
  --aipet-color-bubble-assistant: #2a2a2a; /* 跟 L2 surface */
}
```

**阶梯审计**：L0→L1 +8 / L1→L2 +11 / L2→L3 +9，总跨 28。每档差超人眼可辨下限（~6 单位 grayscale）。

### 5.3 锚选型理由

| 锚 | 总跨 | 风格 | 选 / 不选 |
|---|---|---|---|
| Notion 风 | ~15 | 沉浸阅读 | 否 — 与 #38 想解决的「像同色板」根因相反 |
| **Linear / Bear** | ~25-28 | 沉稳工具感 | **是** — 与现有 Apple/Bear 中灰路线最契合，不需要重校 chat 气泡 |
| Discord / VSCode 风 | ~35-40 | 工具感强 | 否 — 桌宠陪伴语境偏暖，工具感过重；chat 气泡 #2c2c2c 需重校 |

## 6. 衰连 token 调整

仅 `bubble-assistant` 跟动：

- **dark `--aipet-color-bubble-assistant`**：`#262626` → `#2a2a2a`（跟 L2）
- **light `--aipet-color-bubble-assistant`**：`#fafafa` 保持不变（在 #ffffff thread 上仍是「比 bg 略暗的卡片」感）

`code-bg` 不跟（dark 旧 #262626 与新 L2 #2a2a2a 差 4，仍能区分代码区）；`surface-blur` 不跟（frosted blur 微差肉眼难感知）。

## 7. 回归清单（手动 e2e）

token 是底层 patch，覆盖全部 45 文件。落地后 dev 环境逐窗截图对照 light/dark 两态：

| 窗 | 重点表面 | 观察点 |
|---|---|---|
| **workspace** | brand-bar(L1) / sidebar(L1) / master(L1) / detail(L0) / popup(L0/L2) | L 框与 detail 应明显分层（dark 下尤其）；popup overlay 浮卡感 |
| **chat** | sidebar(L1) / thread(L0) / user 气泡(L2 primary) / assistant 气泡(L2) / composer(L0+shadow) | assistant 气泡与 thread 背景区分；composer 浮卡感 |
| **pet** | 透明窗 + bubble(L2) | dark 模式 pet bubble 浮起感 |
| **onboarding** | bg(L0) / 卡片(L2) | 选项卡浮起 |

巡检通过判据：

- ✅ dark workspace 下 L 框（master/sidebar）与 detail 视觉明显「上下两档」
- ✅ dark popup overlay 显著浮起于 workspace 之上
- ✅ dark composer / panel divider 边界可见但不抢戏
- ✅ light 下 sidebar 与主区仍有清晰对比（无回归）
- ✅ light modal/dialog 浮起感不弱于改造前（同 #ffffff + 更强 shadow）

不做截图 diff 自动化 —— 单人项目，肉眼对照即可。

## 8. 风险

- **chat assistant 气泡跟 L2**：与 thread 背景区分依赖 border-faint 10%，应足够；若巡检发现仍糊，可在 MessageBubble 局部加 `--aipet-color-border-faint` 1px hairline 强化（fallback 路径）
- **composer 阴影**：composer 用 `shadow-composer-soft`，token 不变；新 L1 #1f1f1f 让 composer 落在更显眼的对比下，应增强浮卡感而非减弱
- **TokensPreview 页**（[src/views/_dev/TokensPreview.vue](src/views/_dev/TokensPreview.vue:1)）：开发预览页 12 处引用，会自动展示新阶梯 → 也作为回归参考
- **不可控因素**：用户的显示器/操作系统 gamma 校准差异 → 这是所有 dark mode 应用的通病，#38 不解决

## 9. 测试

无单元测试 —— token 是设计 token，无逻辑可单测。

vitest 全套 293/293 维持绿。`pnpm test` 巡 run 一次确认无回归（理论上不会受影响，但保险）。

`pnpm tauri:dev` 起 dev 环境，按 §7 表格手动巡检 4 窗 × 2 主题。

## 10. 文档同步

落地后同步：

- `docs/decisions.md`：新增 ADR-024「dark/light surface elevation 阶梯设计原则」（背峰式 light + 保守型 dark + border-faint 8%/10% 决策记录）
- `docs/STATUS.md`：M2 进度从 8/8 → 9/9（或独立 token 改造一行）
- 不更新 `docs/lessons.md` —— token 改造是设计校准而非「踩坑」类经验

## 11. Phase 划分

本 spec = 单一 phase，~1 session 体量。无需进一步 decompose。

实施工作量预估：

| 任务 | 时长 |
|---|---|
| tokens.css patch | 10 min |
| `pnpm test` 回归 | 5 min |
| `pnpm tauri:dev` 4 窗手动巡检 + 截图对照 | 30-60 min（含 light/dark 切换 + 边角案例） |
| commit + 关闭 issue + 文档同步 | 15 min |

总计 1-2 小时内完成。

## 12. 关联

- 父 issue [#37](https://github.com/tl0502/APET/issues/37)（workspace 重设计 P3）— 本次复检暴露的问题
- 父 spec：[2026-05-21-workspace-redesign/design.md](../2026-05-21-workspace-redesign/design.md)（Phase 1 已完）
- ADR：[decisions.md ADR-021](../../../decisions.md)（chrome 三按钮 surface 语言；本 spec 不修订该 ADR）
