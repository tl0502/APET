---
title: workspace chrome 层视觉适配设计（issue #36）
updated: 2026-05-21
related:
  - ../../README.md
  - ../STATUS.md
  - ../design/desktop-ui-principles.md
  - https://github.com/tl0502/APET/issues/36
---

# workspace chrome 层视觉适配设计

> issue [#36](https://github.com/tl0502/APET/issues/36) — workspace chrome 层精修。
>
> 关联 issue [#33](https://github.com/tl0502/APET/issues/33)（ADR-021 P2 三栏 Desktop App Shell）已完成功能迁移，本 spec 收尾 chrome 层视觉/交互适配。

---

## 1. 背景与共同根因

issue #33 ADR-021 P2 三栏 Desktop App Shell 完成 5+3 panel 迁入 + chat 主床/磁吸双形态。chrome 层 4 个手测可见缺陷：

1. **brand-bar 没占据左侧整列** — AppShell 横向 chrome header（"工作台"标题 + ✕ 关闭）把 brand-bar 拦腰截掉
2. **workspace 面板底部死区** — detail/master 列内容区没贴到窗口底
3. **content-header（panel 内 h2）不固定** — settings/tasks panel 滚动时 `.panel__title` 跟随滚走
4. **sash 不可拖** — chrome padding 偏位让 sash 实际不在用户可点击位置

**共同根因**：`.aipet-shell__body { padding: var(--aipet-space-6) }` 是给 settings/onboarding 等**独立工具型窗**设计的。workspace 三栏复用 AppShell 时这 24px padding 把整个 workspace-body 向内推 → brand-bar 没贴左、底部死区、sash 偏位。chrome header 横切又让 brand-bar 不能占整列。

**业界研究 finding**（详见 §7 references）：

- Discord/Slack/VSCode 都用自绘 frameless titlebar + min/max/close 三按钮 + 整窗顶部 invisible drag region
- VSCode Activity Bar 48px / Discord server bar 72px，项目 brand-bar 60px 居中
- Slack/Linear sidebar typography：14-16px font / 36-48px item / 16px horizontal padding / 1.4 line-height
- macOS Big Sur 起 sticky header 标配 `backdrop-filter: blur` + 半透明背景（vibrancy 材质）
- Linear empty state：subtle illustration + 单 CTA + clean copy

---

## 2. 关键决策

| # | 决策 | 选择 | 理由 |
|---|---|---|---|
| 1 | scope 范围 | A（4 bug）+ B（polish 顺手做） | 一次交齐避免再起多个 small issue |
| 2 | chrome 范式 | 自绘 chrome + 关闭按钮浮右上 + brand-bar 占整列 | Discord/Slack/Linear 桌面 app 标准范式；与 desktop-ui-principles §3/§4 §4 一致 |
| 3 | 架构方式 | A 方案：WorkspaceApp 替换 AppShell（不再用 AppShell wrapper） | workspace 是"长居型 frameless 工作窗"，与 AppShell 服务的"工具型一次性窗"（onboarding）不同形态；解耦让两边都清楚 |
| 4 | sticky header 实现 | P1：抽 panel.css 全局 + `.panel__title` 加 sticky + backdrop blur | 最低改造一处生效；与 chat content-header 同语言（blur + saturate） |
| 5 | chrome 按钮范式 | min + max + close 三按钮（Win11 标准） | Win11 用户 hover max 触发 snap layouts；workspace 是"长居型工作窗"非 modal/settings |
| 6 | sash 宽度 | 保持 3px（不扩到 5px） | chrome padding 修复后 sash 自然回到用户期望位置；3px 视觉更细腻；仅做 3 状态视觉强化 |
| 7 | detail__panel 底部 padding | 保留 24px 作为 "panel 内部呼吸" | 不是 chrome 死区，是 panel 自然边距 |
| 8 | chat 空状态放置点 | C：workspace `MasterColumn` 内 v-if 包一层 | 最低侵入用户工作树（不改 ConversationSidebar 源）|
| 9 | max 状态崩溃恢复 | **不**实现，记 follow-up | 当前 workspaceLayout 不存 max；后续小工时单开 issue |
| 10 | drag region 范围 | 顶部 32px invisible + brand-bar 上下空白挂 drag-region | Tauri 协议要求 drag-region 不能整窗；扩 hit 区到 brand-bar 边缘 |

---

## 3. 总体架构

### 3.1 WorkspaceApp.vue 重写为不用 AppShell 的版本

```html
<div class="workspace-root">
  <!-- 顶部 32px invisible drag bar -->
  <div class="workspace-root__drag-bar" data-tauri-drag-region />

  <!-- chrome 三按钮：右上角绝对定位 -->
  <div class="workspace-root__chrome">
    <button class="aipet-chrome-btn aipet-chrome-btn--min" @click="onMinimize">─</button>
    <button class="aipet-chrome-btn aipet-chrome-btn--max" @click="onMaximize">□</button>
    <button class="aipet-chrome-btn aipet-chrome-btn--close" @click="onClose">✕</button>
  </div>

  <!-- 三栏 body -->
  <BrandBar />
  <MasterColumn />
  <SashHandle />
  <DetailColumn />
</div>
```

### 3.2 z-index 协议

| 元素 | z-index |
|---|---|
| chrome buttons (右上角) | 10 |
| brand-bar buttons 内部 | 6 |
| drag-bar | 5 |
| sash | 3 |
| brand-bar 容器 | 2 |
| master/detail body | auto |

### 3.3 chrome 按钮 Tauri 调用

```ts
import { getCurrentWindow } from '@tauri-apps/api/window'
const win = getCurrentWindow()
function onMinimize() { void win.minimize() }
function onMaximize() { void win.toggleMaximize() }
function onClose() { void hideWorkspace() }  // 关 = hide 进托盘
```

**重要约束**：min/max 走 Tauri window API，不进 hideWorkspace（不进托盘，只是窗口最小化/最大化）。仅 ✕ 走 hideWorkspace。

---

## 4. 文件清单

### 新增

| 文件 | 行数 | 说明 |
|---|---|---|
| `src/styles/buttons.css` | ~50 | 抽出 `.aipet-chrome-btn` + `.aipet-chrome-btn--min/--max/--close` 全局类（Win11 风格 / Segoe Fluent Icons font） |
| `src/styles/panel.css` | ~70 | 抽 panel 通用类（`.panel / .panel__title / .panel__subtitle / .panel__hint / .panel__section / .panel__error / .panel__actions`），含 sticky + backdrop blur |

### 改写

| 文件 | 改动 | 说明 |
|---|---|---|
| `src/views/workspace/WorkspaceApp.vue` | ~90 → ~110 | 删 AppShell；自拼 chrome；root flex column 转 flex row；新增 min/max/close 三按钮 Tauri 调用 |
| `src/views/workspace/BrandBar.vue` | +10 -5 | padding-top 加 32px 让 drag-bar 覆盖头部；`__spacer` 挂 `data-tauri-drag-region` |
| `src/views/workspace/SashHandle.vue` | css 局部改 | 保持 3px width；hover/drag 3 状态视觉强化（::after 线变深 + bg primary） |
| `src/views/workspace/MasterColumn.vue` | +30 -5 | (a) header css 改：高度 44→48 / padding 12→16 / font base→15 / 加 sticky backdrop blur；(b) chat 类别分支内加 v-if 空状态：`conv.length === 0 ? <ConvEmpty/> : <ConversationListPane/>`（决策 #8 — 不动 ConversationListPane / ConversationSidebar） |
| `src/views/workspace/MasterList.vue` | css 局部改 + 模板 icon size | font sm→14；line-height 1.4；min-height 36；gap 8→10；icon 16→18 |
| `src/views/workspace/DetailColumn.vue` | css 局部改 | `.detail-col__panel` padding 保持，添加 panel sticky title 容器约束 |
| `src/main.ts` | +2 import | import panel.css + buttons.css |
| `src/styles/components.css` | -25 | 删 `.aipet-shell__close` scoped 重复（替换为 `.aipet-chrome-btn--close`），保留 `.aipet-shell__header-spacer`（onboarding 可能用） |
| `src/components/chat/ChatThreadPane.vue` | -30 | 内部 `.content-header__close` 复用 `.aipet-chrome-btn--close` 类，去 css 重复 |
| 5 个 settings panel SFC + 3 个 tasks panel SFC | 每 -30 +0 | 删 scoped css 中通用 `.panel / .panel__title / .panel__subtitle / .panel__hint / .panel__section / .panel__error / .panel__actions`；保留 panel-specific 类（如 `.panel__dev / .panel__provider-row`） |

### 不动（明示）

- `src/components/chat/ConversationSidebar.vue / ChatInput.vue / MessageBubble.vue / MessageList.vue`（用户独立工作领地）
- `src/styles/tokens.css`（用户独立工作领地）
- `src/views/workspace/main.ts`（无需改动）
- `src-tauri/tauri.conf.json`（decorations:false 已配置；min/maxWidth 已配置）
- onboarding 窗（仍用 AppShell standalone variant）

---

## 5. 实现细节分段

### 5.1 chrome 协议

- 顶部 32px invisible drag-bar 绝对定位 `top:0 left:0 right:0 height:32px z-index:5`
- 右上角 chrome 按钮组 `top:0 right:0 z-index:10` flex row，三个 46×32 按钮
- BrandBar `.brand-bar__spacer` 挂 `data-tauri-drag-region` 扩 hit 区
- BrandBar 顶部 padding 从 `var(--aipet-space-2)` 改 `calc(32px + var(--aipet-space-2))` 让 drag-bar 完全覆盖头部
- 按钮 z-index 6 高于 drag-bar 5，不被吞

### 5.2 brand-bar 占整列

- 删 AppShell 后 BrandBar 是 `.workspace-root` 的 flex row 第一个 child
- `flex: 0 0 60px; height: 100%`
- 60px 保持（VSCode 48px / Discord 72px / 项目居中，40px avatar + 上下 padding）

### 5.3 sash 视觉强化（保 3px）

```css
.sash { flex: 0 0 3px; width: 3px; ... }

/* 3 状态分离 */
.sash::after { width: 1px; background: var(--aipet-color-border-faint); }  /* 常态：弱灰线 */
.sash:hover { background: color-mix(in srgb, var(--aipet-color-primary) 30%, transparent); }
.sash:hover::after { background: var(--aipet-color-border-strong); }       /* hover：深灰线 + 浅 primary bg */
.sash--dragging { background: color-mix(in srgb, var(--aipet-color-primary) 50%, transparent); }
.sash--dragging::after { background: var(--aipet-color-primary); }         /* drag：primary 线 */
```

### 5.4 panel sticky title（核心）

**`src/styles/panel.css`**：

```css
.panel { display: flex; flex-direction: column; gap: var(--aipet-space-4); }

.panel__title {
  position: sticky;
  top: 0;
  z-index: 1;
  margin: calc(-1 * var(--aipet-space-5)) calc(-1 * var(--aipet-space-6)) 0;
  padding: var(--aipet-space-3) var(--aipet-space-6);
  background: var(--aipet-color-surface-blur);
  backdrop-filter: blur(12px) saturate(180%);
  -webkit-backdrop-filter: blur(12px) saturate(180%);
  border-bottom: 1px solid var(--aipet-color-border-faint);
  font-size: var(--aipet-font-size-lg);
  font-weight: 600;
  color: var(--aipet-color-text-1);
  line-height: 1.4;
  user-select: none;
}

.panel__subtitle { ... }     /* 按现有 settings panel scoped css 数值抽取保持原值（margin/font/color/weight） */
.panel__hint { ... }
.panel__section { ... }
.panel__error { ... }
.panel__actions { ... }
```

**关键技巧**：`margin: -16px -24px 0` 把 sticky title 拉出 `.detail-col__panel { padding: 16px 24px 24px }` 的内边距，bg 覆盖到 panel 边缘。

### 5.5 master list typography 节奏

```css
.master-list__btn {
  font-size: 14px;        /* sm 13 → 14 */
  line-height: 1.4;
  min-height: 36px;       /* 业界 36-48 区间下限 */
  gap: 10px;              /* icon-text 间距 */
  padding: var(--aipet-space-2) var(--aipet-space-3);  /* 保持 */
}
```

**MasterList.vue 模板**：`<ElIcon :size="16">` → `<ElIcon :size="18">`

**MasterColumn.vue header**：高度 44→48 / 横向 padding 12→16 / 字号 base→15 / 加 sticky backdrop blur

### 5.6 chat 空状态

放置点：**workspace `MasterColumn.vue` 的 chat 类别分支内** v-if（决策 #8）。**不动** `ConversationListPane` / `ConversationSidebar` 源（磁吸窗仍走 ConversationListPane 原有空白行为）。

```html
<!-- MasterColumn.vue chat 类别分支 -->
<template v-if="layout.currentCategory === 'chat'">
  <div v-if="store.conversations.length === 0" class="conv-empty">
    <img src="/avatar/momo-avatar.svg" class="conv-empty__illustration" alt="" />
    <p class="conv-empty__title">还没开始对话</p>
    <p class="conv-empty__hint">说一句"你好"就行</p>
    <ElButton type="primary" @click="store.create()">
      <ElIcon><Plus /></ElIcon> 新对话
    </ElButton>
  </div>
  <ConversationListPane v-else :collapsed="false" />
</template>
```

### 5.7 brand-bar 微动效

- 切类别 → 左竖条 200ms scaleY 进入动效
- 当前 SettingsPersona → avatar 周围 pulse 光环 2s 循环

---

## 6. 验收清单

### 6.1 自动化

```bash
pnpm tsc --noEmit
pnpm test --run
pnpm build
cd src-tauri && cargo check && cargo clippy
```

期待：全绿。无新增 vitest case（纯 css/视觉改动）。

### 6.2 grep 残留

```bash
grep -rn "\.aipet-shell__close" src
grep -rn "AppShell" src/views/workspace
```

期待：第一条全空（除了 components.css 内的 `.aipet-shell__header-spacer`），第二条全空（WorkspaceApp 不再用 AppShell）。

### 6.3 手测

1. **brand-bar 占整列**：截图显示 brand-bar 从窗口顶 (0,0) 占到底部，没被 chrome header 截
2. **chrome 三按钮**：右上角 ─ □ ✕ 三按钮；hover ─ → 浅灰 / hover □ → 浅灰 + Win11 snap layouts 出现 / hover ✕ → 红
3. **drag 行为**：拖顶部 32px invisible 条 → 窗口移动；拖 brand-bar 顶/底空白 → 移动；点击 brand-bar 类别按钮 → 不移动
4. **min/max/close 行为**：min → 任务栏（不进托盘）；max → 全屏切换；close → 进托盘
5. **sash 可拖**：master/detail 之间 3px 区域 hover 显高亮 → 拖动改宽 180-380 区间 → 持久化恢复
6. **panel sticky title**：切到 SettingsTheme，滚动 panel 内容时 "外观" 标题 sticky 列顶（backdrop blur）
7. **底部死区**：detail/master 列贴到窗口底
8. **chat 空状态**：删光所有对话 → 空状态显示 + "+ 新对话" 按钮可点
9. **master list typography**：item 36px 高 + 14px font + 18px icon + 1.4 line-height（截图证明）
10. **brand-bar 微动效**：切类别 → 左竖条 200ms 缩放进入；当前 SettingsPersona → avatar 周围有 pulse 光环
11. **chat 主床 + 磁吸窗回归测试**：磁吸窗 snap/AOT/ESC 全无回归；两窗共享 conversations
12. **崩溃恢复**：kill 进程 → 重启 → workspace 当前类别 + 项 + master 宽全还原（max 状态本轮不存，记 follow-up）
13. **settings/onboarding 独立窗视觉零回归**：onboarding 仍是 chrome header + body padding 24px 不被影响

---

## 7. 风险与依赖

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| 1 | 抽 panel.css 后 panel SFC 残留 scoped 重复定义 → 视觉错位 | 中 | settings/tasks 视觉降级 | grep 残留 + 手测每个 panel |
| 2 | sticky panel__title backdrop blur 在 Windows 老 WebView2 渲染异常 | 低 | 视觉降级（无 blur，纯 bg） | -webkit-backdrop-filter 同步设置；fallback `background: var(--aipet-color-surface)` |
| 3 | drag-bar 32px 覆盖 brand-bar 头像点击区 | 低 | 头像不能点 | brand-bar z-index 高于 drag-bar；padding-top 加 32 让头像下移 |
| 4 | chrome 按钮在 detail/master 上方导致 panel 内容被覆盖 | 低 | 右上角 46×96 区遮挡 | DetailColumn `.panel__title` 已 sticky + chrome 按钮组背景需要相同 surface-blur 一致 |
| 5 | Tauri Win11 max 按钮 hover 不触发 snap layouts | 中 | UX 期望差异 | data-tauri-drag-region 不在 max 按钮上；max 按钮挂正常点击事件即可 |
| 6 | 用户工作树 chat 子组件视觉收敛（已 commit b88641e）被覆盖 | 中 | 视觉降级 | 严格 not 改 ConversationSidebar/ChatInput/MessageBubble/MessageList/tokens.css |
| 7 | onboarding 窗 panel SFC 引用 panel.css 后视觉变化 | 低 | onboarding 视觉变化 | onboarding 当前是 AppShell standalone + 独立 chrome；panel SFC 引入 panel.css 后通用类生效，需要手测 |
| 8 | macOS / Linux 跨平台 Tauri 协议差异 | N/A | 不适用 | 项目仅 Win11 目标 |

---

## 8. 工时估算

| 阶段 | 工时 | 说明 |
|---|---|---|
| 8.1 chrome 架构（WorkspaceApp 重写 + buttons.css + Tauri min/max 调用 + BrandBar drag-region） | 0.25d | A 方案核心 |
| 8.2 panel.css 抽取 + 8 panel SFC scoped css 重构 | 0.25-0.5d | 散落改 8 文件 |
| 8.3 master list typography + MasterColumn sticky header | 0.1d | 数值微调 |
| 8.4 sash 3 状态视觉强化 + 验证可拖 | 0.05d | css 局部改 |
| 8.5 chat 空状态 + 微动效 | 0.15d | ConversationListPane 加 v-if + brand-bar 动画 |
| 8.6 手测 13 项 + 截图 + grep 残留 | 0.2d | 验收 |
| **合计** | **1d** | issue #36 估算 0.5-1d 上限 |

---

## 9. 后续 follow-up

- max 状态崩溃恢复（workspaceLayout store 增加 isMaximized 字段 + KV 持久化）
- About 模态弹窗实装（brand-bar ❓ 按钮，M3+）
- task panels idle state polish（提醒列表空 / 番茄未启动），M3+
- onboarding 引导覆盖 brand-bar 三按钮区域（如有 onboarding 触达 workspace 的 step）

---

## 参考

业界范式来源（web research 2026-05-21）：

- [Tauri v2 Window Customization](https://v2.tauri.app/learn/window-customization/) — `data-tauri-drag-region` 协议 / decorations:false 范式
- [VS Code Activity Bar UX Guidelines](https://code.visualstudio.com/api/ux-guidelines/activity-bar) — Activity Bar 48px 默认 / 24px icon
- [Linear redesign](https://linear.app/now/how-we-redesigned-the-linear-ui) — sidebar tabs/headers/panels 节奏 / Apple 标准对齐
- [Slack design system](https://slack.engineering/the-gradual-design-system-how-we-built-slack-kit/) — typography mixins / typeset 单源
- [Sidebar Design for Web Apps (2026 Guide)](https://www.alfdesigngroup.com/post/improve-your-sidebar-design-for-web-apps) — 14-16px font / 36-48px item / 16px padding / 20-24px icon
- [Pixel Envy: Sidebar Translucency](https://pxlnv.com/blog/sidebar-translucency/) — macOS Vibrancy 材质
- [NN/G Empty State Guidelines](https://www.nngroup.com/articles/empty-state-interface-design/) — 三要素 context + guidance + visual
- [Microsoft Titlebar Design](https://learn.microsoft.com/en-us/windows/apps/design/basics/titlebar-design) — Win11 standard chrome
- [desktop-ui-principles.md](../design/desktop-ui-principles.md) — 项目桌面 UI 范式（多窗 / 表面分层 / 自绘圆角 / 反例自检）
