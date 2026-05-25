# workspace chrome 层视觉适配 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 workspace 三栏 Desktop App Shell 视觉/交互收口到现代桌面 app 标准（Discord/Slack/VSCode 范式），修 issue #36 列的 4 个 chrome 缺陷 + 顺手做 B 组 polish。

**Architecture:** WorkspaceApp 替换 AppShell（删 standalone wrapper，自绘 chrome 三按钮 + 顶部 invisible drag-bar + brand-bar 占整列）；抽 `panel.css` / `buttons.css` 两个全局样式表统一 panel / chrome button 范式；8 个 panel SFC scoped css 重构（删通用 .panel* 类）；master 节奏与 sash 3 状态视觉强化对齐业界。

**Tech Stack:** Vue 3 SFC + scoped CSS + Pinia + Tauri 2.x window API（getCurrentWindow / minimize / toggleMaximize）+ Element Plus 图标

**Spec:** [2026-05-21-workspace-chrome-adapt-design.md](./2026-05-21-workspace-chrome-adapt-design.md)

**Issue:** [#36](https://github.com/tl0502/APET/issues/36)

---

## 任务总览

| # | 任务 | 关键文件 | commit message |
|---|---|---|---|
| 1 | 全局 css 抽取（panel.css + buttons.css）+ main.ts 引入 | 2 新 css + 2 main.ts | `feat: #36 phase 1 全局 panel.css + buttons.css 抽取` |
| 2 | WorkspaceApp 重写（删 AppShell + 自绘 chrome 三按钮 + drag-bar）+ BrandBar drag-region 扩张 + components.css 清理 | WorkspaceApp.vue / BrandBar.vue / components.css | `feat: #36 phase 2 WorkspaceApp 自绘 chrome 三按钮 + brand-bar 占整列` |
| 3 | ChatThreadPane close 按钮复用 .aipet-chrome-btn--close | ChatThreadPane.vue | `refactor: #36 phase 3 ChatThreadPane close 复用全局类` |
| 4 | 8 panel SFC scoped css 抽取（settings 5 + tasks 3 删通用类） | 8 panel SFC | `refactor: #36 phase 4 panel SFC 删 scoped 通用类（由 panel.css 接管）` |
| 5 | MasterColumn header sticky backdrop + MasterList typography 节奏 | MasterColumn.vue / MasterList.vue | `feat: #36 phase 5 master 节奏对齐业界（typography + sticky header）` |
| 6 | SashHandle 3 状态视觉强化 | SashHandle.vue | `feat: #36 phase 6 sash 3 状态视觉强化（常态/hover/drag）` |
| 7 | chat 空状态（MasterColumn chat 分支 v-if） | MasterColumn.vue | `feat: #36 phase 7 chat 空状态 + 新对话 CTA` |
| 8 | brand-bar 微动效（active 竖条 + persona pulse） | BrandBar.vue | `feat: #36 phase 8 brand-bar 微动效收口` |
| 9 | 全套验收 + grep 残留 + STATUS + issue close | docs/STATUS.md | `docs: #36 chrome 适配落地 STATUS 同步` |

---

## Task 1: 全局 css 抽取（panel.css + buttons.css）+ main.ts 引入

**Files:**
- Create: `src/styles/panel.css`
- Create: `src/styles/buttons.css`
- Modify: `src/views/workspace/main.ts:20`（引入 2 css）
- Modify: `src/views/chat/main.ts`（引入 buttons.css）
- Modify: `src/main.ts:9`（引入 buttons.css，pet 窗未来可能用）

### - [ ] Step 1.1: 创建 `src/styles/panel.css`

```css
/* AIPET 项目 panel 通用样式（#36 chrome 适配 抽取）。
   消费方：src/panels/settings/*.vue + src/panels/tasks/*.vue（共 8 个 panel）
   引入方：src/views/workspace/main.ts（panel 仅在 workspace 内渲染）

   各 panel SFC scoped css 删 .panel / .panel__title / .panel__subtitle /
   .panel__hint / .panel__section / .panel__error / .panel__actions；
   保留 panel-specific 类（如 .panel__dev、.panel__provider-row）。

   .panel__title 用 sticky + backdrop blur 实现"列顶部固定标题"，与
   chat content-header 同语言（blur(12) saturate(180)）。
   margin 负值把 sticky title 拉出 detail-col__panel 16px 24px 内边距。
*/

.panel {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
}

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

.panel__subtitle {
  margin: 0;
  font-size: var(--aipet-font-size-base);
  font-weight: 600;
  color: var(--aipet-color-text-2);
}

.panel__hint {
  margin: 0;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
  line-height: var(--aipet-line-height-base);
}

.panel__section {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
}

.panel__error {
  color: var(--aipet-color-danger);
  font-size: var(--aipet-font-size-sm);
}

.panel__actions {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-3);
  margin-top: var(--aipet-space-2);
}
```

### - [ ] Step 1.2: 创建 `src/styles/buttons.css`

```css
/* AIPET 项目 chrome 按钮通用样式（#36 chrome 适配 抽取）。
   消费方：WorkspaceApp.vue 三按钮 + ChatThreadPane content-header__close
   引入方：workspace/main.ts + chat/main.ts + 未来需要自绘 chrome 的窗

   .aipet-chrome-btn  - Win11 风格按钮基础（46×32 / Segoe Fluent Icons font）
   .aipet-chrome-btn--min / --max - 普通 hover 浅灰
   .aipet-chrome-btn--close      - hover 红（#c42b1c / Win11 标准）
*/

.aipet-chrome-btn {
  width: 46px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--aipet-color-text-2);
  font-size: 13px;
  font-family: 'Segoe Fluent Icons', 'Segoe MDL2 Assets', system-ui, sans-serif;
  cursor: pointer;
  padding: 0;
  user-select: none;
  transition: background-color 100ms ease, color 100ms ease;
}

.aipet-chrome-btn:hover {
  background: color-mix(in srgb, var(--aipet-color-text-1) 8%, transparent);
  color: var(--aipet-color-text-1);
}

.aipet-chrome-btn:active {
  background: color-mix(in srgb, var(--aipet-color-text-1) 14%, transparent);
}

.aipet-chrome-btn--close:hover {
  background: #c42b1c;
  color: #ffffff;
}

.aipet-chrome-btn--close:active {
  background: #a01e15;
  color: #ffffff;
}
```

### - [ ] Step 1.3: 修改 `src/views/workspace/main.ts` 引入两个新 css

把第 19 行（`import '@/styles/components.css'`）之后插入：

```ts
import '@/styles/components.css'
import '@/styles/panel.css'
import '@/styles/buttons.css'
```

### - [ ] Step 1.4: 修改 `src/views/chat/main.ts` 引入 buttons.css

读现状（仅引入 buttons.css，因为 chat 磁吸窗 content-header 用 close 按钮）：

```bash
grep -n "styles/components.css" src/views/chat/main.ts
```

在 `components.css` 之后插入：

```ts
import '@/styles/components.css'
import '@/styles/buttons.css'
```

### - [ ] Step 1.5: 修改 `src/main.ts` 引入 buttons.css（pet 窗）

虽然 pet 窗当前不消费 chrome 按钮，但为未来一致性（pet App.vue 可能引用 chat 子组件），第 8 行 components.css 后插入 buttons.css。如果不需要，跳过；当前 plan 范围内**只在 workspace + chat main.ts 引入** buttons.css，pet/onboarding/pomodoro main.ts 不动。

**决定**：Step 1.5 跳过，pet/onboarding/pomodoro main.ts 不动；仅 workspace + chat 两 main.ts 引入。

### - [ ] Step 1.6: typecheck + build

```bash
pnpm tsc --noEmit
pnpm build
```

期待：全绿（仅引入 css 不会报 ts 错；build 必须把 panel.css/buttons.css 打入 workspace.html + chat.html chunk）。

### - [ ] Step 1.7: Commit

```bash
git add src/styles/panel.css src/styles/buttons.css src/views/workspace/main.ts src/views/chat/main.ts
git commit -m "feat: #36 phase 1 全局 panel.css + buttons.css 抽取"
```

---

## Task 2: WorkspaceApp 重写（chrome shell） + BrandBar drag-region + components.css 清理

**Files:**
- Modify: `src/views/workspace/WorkspaceApp.vue`（重写 ~90 → ~110 行）
- Modify: `src/views/workspace/BrandBar.vue:111-121` + `:179-181`（padding-top 加 32 / spacer drag-region）
- Modify: `src/styles/components.css:64-96`（删 .aipet-shell__close + .aipet-shell__header-spacer）

### - [ ] Step 2.1: 重写 WorkspaceApp.vue

完整新文件内容（替换现有 1-137 行）：

```vue
<script setup lang="ts">
// WorkspaceApp (#36 chrome 适配重写)：workspace 独立窗口 root — 三栏 Desktop App Shell。
//
// 与 #33 phase B-redo 版本差异：
// - 删 AppShell wrapper（workspace 是 frameless 长居型工作窗，与 AppShell 服务的
//   工具型一次性窗 onboarding 不同形态）
// - 自绘 chrome：顶部 32px invisible drag-bar + 右上角 min/max/close 三按钮
// - brand-bar 从 (0,0) 占整列到底
//
// chrome 协议（z-index）：
//   chrome 按钮(10) > brand-bar 按钮(6) > drag-bar(5) > sash(3) > brand-bar 容器(2)
//
// 三按钮行为差异：
// - min/max 走 Tauri window API（不进托盘）
// - close 走 hideWorkspace IPC（关 = hide 进托盘）

import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

import BrandBar from './BrandBar.vue'
import MasterColumn from './MasterColumn.vue'
import DetailColumn from './DetailColumn.vue'
import SashHandle from './SashHandle.vue'

import { useWorkspaceLayoutStore } from '@/stores/workspaceLayout'
import { hideWorkspace } from '@/services/window'

const layout = useWorkspaceLayoutStore()
const ready = ref(false)
const unlistenFns: UnlistenFn[] = []
const win = getCurrentWindow()

async function onMinimize() {
  try {
    await win.minimize()
  } catch (e) {
    console.warn('[WorkspaceApp] minimize failed:', e)
  }
}

async function onMaximize() {
  try {
    await win.toggleMaximize()
  } catch (e) {
    console.warn('[WorkspaceApp] toggleMaximize failed:', e)
  }
}

async function onClose() {
  // workspace ✕ → hide（lib.rs CloseRequested 联判 + 联走 IPC）
  try {
    await hideWorkspace()
  } catch (e) {
    console.warn('[WorkspaceApp] hideWorkspace failed:', e)
  }
}

function onSashChange(width: number) {
  layout.setMasterWidth(width)
}

function onGlobalKeydown(e: KeyboardEvent) {
  if (e.key !== 'Escape') return
  if (document.querySelector('.el-message-box, .el-dialog__wrapper, .el-overlay')) return
  const active = document.activeElement
  if (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement) return
  void onClose()
}

onMounted(async () => {
  await layout.loadFromKv()
  ready.value = true

  window.addEventListener('keydown', onGlobalKeydown)

  try {
    const un = await listen<{ label: string; visible: boolean }>(
      'window:visibility-changed',
      async (event) => {
        if (event.payload.label === 'workspace' && event.payload.visible === false) {
          console.debug('[WorkspaceApp] hide event received, KV already persisted')
        }
      },
    )
    unlistenFns.push(un)
  } catch (e) {
    console.warn('[WorkspaceApp] listen visibility-changed failed:', e)
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onGlobalKeydown)
  unlistenFns.forEach((u) => u())
})
</script>

<template>
  <div class="workspace-root">
    <div class="workspace-root__drag-bar" data-tauri-drag-region />

    <div class="workspace-root__chrome">
      <button
        class="aipet-chrome-btn"
        title="最小化"
        aria-label="最小化"
        @click="onMinimize"
      >─</button>
      <button
        class="aipet-chrome-btn"
        title="最大化"
        aria-label="最大化"
        @click="onMaximize"
      >□</button>
      <button
        class="aipet-chrome-btn aipet-chrome-btn--close"
        title="关闭（进托盘）"
        aria-label="关闭"
        @click="onClose"
      >✕</button>
    </div>

    <template v-if="ready">
      <BrandBar />
      <MasterColumn />
      <SashHandle
        :width="layout.masterWidth"
        :min="layout._MASTER_WIDTH_MIN"
        :max="layout._MASTER_WIDTH_MAX"
        @update:width="onSashChange"
      />
      <DetailColumn />
    </template>
  </div>
</template>

<style scoped>
.workspace-root {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: row;
  background: var(--aipet-color-bg);
  position: relative;
  overflow: hidden;
}

/* 顶部 32px invisible drag-bar：覆盖整窗顶用作拖动 hit 区 */
.workspace-root__drag-bar {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 32px;
  z-index: 5;
  /* 视觉透明，仅作为拖动 hit 区 */
  background: transparent;
  pointer-events: auto;
}

/* 右上角 chrome 按钮组 */
.workspace-root__chrome {
  position: absolute;
  top: 0;
  right: 0;
  z-index: 10;
  display: flex;
  flex-direction: row;
  user-select: none;
}
</style>
```

### - [ ] Step 2.2: 修改 BrandBar.vue padding-top + spacer drag-region

读现状：BrandBar 第 91 行 `<div class="brand-bar__spacer" />` 改为挂 `data-tauri-drag-region`。

模板部分：找到 `<div class="brand-bar__spacer" />` 改为：

```html
<div class="brand-bar__spacer" data-tauri-drag-region />
```

scoped css 部分：找到 `.brand-bar { ... padding: var(--aipet-space-2) 0 var(--aipet-space-3); }`（约第 120 行）改为：

```css
.brand-bar {
  flex: 0 0 60px;
  width: 60px;
  height: 100%;
  background: var(--aipet-color-surface);
  border-right: 1px solid var(--aipet-color-border-faint);
  display: flex;
  flex-direction: column;
  align-items: stretch;
  /* 顶部 32px 让位给 invisible drag-bar，避免与 pet 头像点击冲突 */
  padding: calc(32px + var(--aipet-space-2)) 0 var(--aipet-space-3);
  position: relative;
  z-index: 2;
}
```

### - [ ] Step 2.3: 删 components.css `.aipet-shell__close` + `.aipet-shell__header-spacer`

读 `src/styles/components.css:59-96` 整块（包括注释和定义）：

```css
/* === AppShell.standalone 自绘 header 拖动+关闭组件 ===
   配合 settings / tasks 删除 OS 原生标题栏（decorations:false + transparent:false，Win11 DWM 仍给圆角）
   后，在 #header slot 内用这两个组件提供拖动区 + ✕ 关闭。
   - data-tauri-drag-region 不被子元素继承，所以 .aipet-shell__title 和 __header-spacer 各自挂；
     按钮挂 ="false" 反向取消。
   - __close 风格与 chat content-header__close 同源（Win11 标准 close-button red）。 */
.aipet-shell__header-spacer {
  flex: 1 1 auto;
  align-self: stretch;
}

.aipet-shell__close {
  ...全部 ~25 行
}

.aipet-shell__close:hover {
  background: #c42b1c;
  color: #ffffff;
}

.aipet-shell__close:active {
  background: #a01e15;
  color: #ffffff;
}
```

全删（这两个类 grep 仅 WorkspaceApp 用，删 AppShell 后无消费方；onboarding 没用）。

### - [ ] Step 2.4: typecheck + build

```bash
pnpm tsc --noEmit
pnpm build
```

期待：全绿。

### - [ ] Step 2.5: 启动 dev 跑 workspace 手测

```bash
pnpm tauri dev
```

托盘双击或 Ctrl+Alt+W 打开 workspace。验证：

- [ ] brand-bar 从窗口顶 (0,0) 占满左侧 60px 到底
- [ ] 右上角三个按钮 ─ □ ✕（不被 brand-bar 覆盖）
- [ ] 拖顶部 32px 区域 → 窗口移动；拖 brand-bar 中间 spacer 空白 → 窗口移动
- [ ] 点 brand-bar 类别图标 → 切换类别（不被 drag-bar 吞）
- [ ] 点击 min → 窗口最小化到任务栏（不进托盘）
- [ ] 点击 max → 全屏切换 / 还原；Win11 hover max 出 snap layouts
- [ ] 点击 ✕ → 进托盘（窗口隐藏不退出）

### - [ ] Step 2.6: Commit

```bash
git add src/views/workspace/WorkspaceApp.vue src/views/workspace/BrandBar.vue src/styles/components.css
git commit -m "feat: #36 phase 2 WorkspaceApp 自绘 chrome 三按钮 + brand-bar 占整列"
```

---

## Task 3: ChatThreadPane close 按钮复用 .aipet-chrome-btn--close

**Files:**
- Modify: `src/components/chat/ChatThreadPane.vue:223`（template class 改）+ `:375-401`（scoped css 删）

### - [ ] Step 3.1: 模板 class 改

ChatThreadPane.vue 第 223 行：

```html
<button
  v-if="showCloseButton"
  class="content-header__close"
  title="关闭（进托盘）"
  ...
>
```

改为：

```html
<button
  v-if="showCloseButton"
  class="aipet-chrome-btn aipet-chrome-btn--close content-header__close-sized"
  title="关闭（进托盘）"
  ...
>
```

`content-header__close-sized` 是保留 chat content-header 内 close 按钮的特殊尺寸（高度 56 而非 32，配合 56px content-header 高度）。

### - [ ] Step 3.2: scoped css 改 .content-header__close → .content-header__close-sized

读 `src/components/chat/ChatThreadPane.vue:375-401` 这段：

```css
.content-header__close {
  flex: 0 0 auto;
  width: 46px;
  height: 56px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--aipet-color-text-2);
  font-size: 13px;
  font-family: 'Segoe Fluent Icons', 'Segoe MDL2 Assets', system-ui, sans-serif;
  cursor: pointer;
  padding: 0;
  margin: 0;
  transition: background-color 100ms ease, color 100ms ease;
}

.content-header__close:hover {
  background: #c42b1c;
  color: #ffffff;
}

.content-header__close:active {
  background: #a01e15;
  color: #ffffff;
}
```

改为（仅保留 chat 特殊 56px 高度 + 重置 margin；其它由 `.aipet-chrome-btn--close` 全局类接管）：

```css
/* chat content-header 内 close 按钮特殊尺寸（与 56px header 同高，覆盖全局 32px） */
.content-header__close-sized {
  height: 56px;
}
```

### - [ ] Step 3.3: typecheck + 手测 chat 磁吸窗 close

```bash
pnpm tsc --noEmit
pnpm tauri dev
```

手测：
- [ ] 打开 chat 磁吸窗，content-header 右侧 ✕ 按钮显示正常（46×56 红 hover）
- [ ] 点 ✕ → 磁吸窗 hide 进托盘

### - [ ] Step 3.4: Commit

```bash
git add src/components/chat/ChatThreadPane.vue
git commit -m "refactor: #36 phase 3 ChatThreadPane close 复用全局类"
```

---

## Task 4: 8 panel SFC scoped css 抽取（删通用类）

**Files:**
- Modify: `src/panels/settings/SettingsThemePanel.vue`
- Modify: `src/panels/settings/SettingsProviderPanel.vue`
- Modify: `src/panels/settings/SettingsPersonaPanel.vue`
- Modify: `src/panels/settings/SettingsNicknamePanel.vue`
- Modify: `src/panels/settings/SettingsAboutPanel.vue`
- Modify: `src/panels/tasks/TasksReminderPanel.vue`
- Modify: `src/panels/tasks/TasksPomodoroPanel.vue`
- Modify: `src/panels/tasks/TasksTodoPanel.vue`

### 通用规则

每个 panel SFC 内 `<style scoped>` 段：

**删**：以下通用类的定义（如有）
- `.panel { ... }`
- `.panel__title { ... }`
- `.panel__subtitle { ... }`
- `.panel__hint { ... }`
- `.panel__section { ... }`
- `.panel__error { ... }`
- `.panel__actions { ... }`

**保留**：panel-specific 类（如 `.panel__dev / .panel__provider-row / .panel__nickname-form / .about-grid / code` 等）

### - [ ] Step 4.1: SettingsThemePanel.vue

读 `src/panels/settings/SettingsThemePanel.vue:99-145` scoped css 段。

删除：
- `.panel { display: flex; flex-direction: column; gap: var(--aipet-space-4); }`
- `.panel__title { margin: 0; font-size: var(--aipet-font-size-lg); font-weight: 600; color: var(--aipet-color-text-1); }`
- `.panel__section { display: flex; flex-direction: column; gap: var(--aipet-space-3); }`
- `.panel__subtitle { margin: 0; font-size: var(--aipet-font-size-base); font-weight: 600; color: var(--aipet-color-text-2); }`
- `.panel__hint { margin: 0; color: var(--aipet-color-text-3); font-size: var(--aipet-font-size-sm); line-height: var(--aipet-line-height-base); }`

保留：
- `.panel__dev { ... }`（panel-specific dev mode 提示框）
- `code { ... }`（panel 内 inline code 样式）

### - [ ] Step 4.2: SettingsProviderPanel.vue

定位：

```bash
grep -n "^\.panel" src/panels/settings/SettingsProviderPanel.vue
```

读现状 → 删通用 `.panel / .panel__title / .panel__hint / .panel__section / .panel__error / .panel__actions / .panel__subtitle`（如出现）→ 保留 provider-specific 类（如 `.provider-row` / form 包装 / `.panel__provider-*`）。

### - [ ] Step 4.3: SettingsPersonaPanel.vue

定位：

```bash
grep -n "^\.panel" src/panels/settings/SettingsPersonaPanel.vue
```

读现状 → 删通用类 → 保留 persona-specific 类（actions 区 / descriptions 包装）。

### - [ ] Step 4.4: SettingsNicknamePanel.vue

定位：

```bash
grep -n "^\.panel" src/panels/settings/SettingsNicknamePanel.vue
```

读现状 → 删通用类 → 保留 nickname form 专属类。

### - [ ] Step 4.5: SettingsAboutPanel.vue

读 `src/panels/settings/SettingsAboutPanel.vue:51-72` scoped css 段：

```css
.panel {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
}
.panel__title {
  margin: 0;
  font-size: var(--aipet-font-size-lg);
  font-weight: 600;
  color: var(--aipet-color-text-1);
}
.panel__hint {
  margin: 0;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
  line-height: var(--aipet-line-height-base);
}
.panel__error {
  color: var(--aipet-color-danger);
  font-size: var(--aipet-font-size-sm);
}
```

全删（4 个全是通用类）；保留下方 `.about-grid / .about-grid dt / .about-grid dd / .about-grid a / .about-grid a:hover / code` 这些 panel-specific 类。

### - [ ] Step 4.6: TasksReminderPanel.vue / TasksPomodoroPanel.vue / TasksTodoPanel.vue

定位（三个一起 grep）：

```bash
grep -n "^\.panel" src/panels/tasks/*.vue
```

3 个 tasks panel 同上处理：删 `.panel*` 通用 → 保留 reminder-row / pomodoro-circle / todo-empty 等 panel-specific。

### - [ ] Step 4.7: typecheck + build + 手测每个 panel

```bash
pnpm tsc --noEmit
pnpm build
pnpm tauri dev
```

手测每个 panel 切换：

- [ ] SettingsTheme：标题 "外观" sticky 在 detail 列顶（backdrop blur）；滚动副标题不漏穿背景
- [ ] SettingsProvider：标题 "LLM Provider" sticky；下方 form 显示正常
- [ ] SettingsPersona：标题 "人格" sticky；VRM 头像导出区域不被覆盖
- [ ] SettingsNickname：标题 "昵称" sticky
- [ ] SettingsAbout：标题 "关于" sticky；about-grid 显示正常
- [ ] TasksReminder：标题 sticky；reminder 列表显示正常
- [ ] TasksPomodoro：标题 sticky；圆环不被覆盖
- [ ] TasksTodo：标题 sticky；占位说明正常

### - [ ] Step 4.8: Commit

```bash
git add src/panels/settings src/panels/tasks
git commit -m "refactor: #36 phase 4 panel SFC 删 scoped 通用类（由 panel.css 接管）"
```

---

## Task 5: MasterColumn header sticky backdrop + MasterList typography 节奏

**Files:**
- Modify: `src/views/workspace/MasterColumn.vue:55-93`（scoped css）
- Modify: `src/views/workspace/MasterList.vue:35`（icon size）+ `:44-108`（scoped css）

### - [ ] Step 5.1: MasterColumn.vue scoped css

读 `src/views/workspace/MasterColumn.vue:55-93` scoped css。

替换为：

```css
.master-col {
  flex: 0 0 auto;
  height: 100%;
  background: var(--aipet-color-surface);
  border-right: 1px solid var(--aipet-color-border-faint);
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.master-col__header {
  flex: 0 0 48px;
  height: 48px;
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
  padding: 0 var(--aipet-space-4);
  border-bottom: 1px solid var(--aipet-color-border-faint);
  user-select: none;
  /* 与 panel__title 同语言：sticky 浮玻璃 */
  position: sticky;
  top: 0;
  z-index: 2;
  background: var(--aipet-color-surface-blur);
  backdrop-filter: blur(12px) saturate(180%);
  -webkit-backdrop-filter: blur(12px) saturate(180%);
}

.master-col__header-icon {
  flex: 0 0 auto;
  color: var(--aipet-color-text-2);
}

.master-col__header-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--aipet-color-text-1);
  letter-spacing: 0.01em;
}

.master-col__body {
  flex: 1 1 auto;
  overflow-y: auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
```

### - [ ] Step 5.2: MasterList.vue 模板 icon size 改

读 `src/views/workspace/MasterList.vue:35`：

```html
<ElIcon :size="16" class="master-list__icon">
```

改为：

```html
<ElIcon :size="18" class="master-list__icon">
```

### - [ ] Step 5.3: MasterList.vue scoped css 改

读 `src/views/workspace/MasterList.vue:54-108` scoped css。

替换 `.master-list__btn` 段：

```css
.master-list__btn {
  position: relative;
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: var(--aipet-space-2) var(--aipet-space-3);
  background: transparent;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  color: var(--aipet-color-text-2);
  font-size: 14px;
  line-height: 1.4;
  min-height: 36px;
  text-align: left;
  transition: background-color 100ms ease, color 100ms ease;
}
```

其它定义保持。

### - [ ] Step 5.4: typecheck + 手测 master list

```bash
pnpm tsc --noEmit
pnpm tauri dev
```

手测：
- [ ] master 列 header "对话/任务/创作/设置" 显示 15px 字 + sticky 锁顶
- [ ] master 列 list item：18px icon + 14px 文字 + 36px 最小高 + 10px icon-text 间距
- [ ] hover 浅灰 / active 左侧 3px primary 竖条 + bg

### - [ ] Step 5.5: Commit

```bash
git add src/views/workspace/MasterColumn.vue src/views/workspace/MasterList.vue
git commit -m "feat: #36 phase 5 master 节奏对齐业界（typography + sticky header）"
```

---

## Task 6: SashHandle 3 状态视觉强化

**Files:**
- Modify: `src/views/workspace/SashHandle.vue:87-119`（scoped css）

### - [ ] Step 6.1: SashHandle.vue scoped css

读 `src/views/workspace/SashHandle.vue:87-119` 整段 scoped css。

替换为：

```css
.sash {
  flex: 0 0 3px;
  width: 3px;
  height: 100%;
  cursor: col-resize;
  position: relative;
  background: transparent;
  transition: background 100ms ease;
  z-index: 3;
}

.sash:hover {
  background: color-mix(in srgb, var(--aipet-color-primary) 30%, transparent);
}

.sash--dragging {
  background: color-mix(in srgb, var(--aipet-color-primary) 50%, transparent);
}

/* ::after 渲染 1px 视觉线（常态可见，hover / drag 时变深） */
.sash::after {
  content: '';
  position: absolute;
  left: 1px;
  top: 0;
  bottom: 0;
  width: 1px;
  background: var(--aipet-color-border-faint);
  pointer-events: none;
  transition: background 100ms ease;
}

.sash:hover::after {
  background: var(--aipet-color-border-strong);
}

.sash--dragging::after {
  background: var(--aipet-color-primary);
}
```

### - [ ] Step 6.2: 手测 sash 三状态

```bash
pnpm tauri dev
```

- [ ] 常态：master/detail 之间能看见 1px 弱灰线
- [ ] hover：1px 深灰线 + 3px 浅 primary 背景（视觉信号"可拖"）
- [ ] drag：1px primary 线 + 3px 深 primary 背景；松手宽度落到 180-380 区间 + 持久化恢复

### - [ ] Step 6.3: Commit

```bash
git add src/views/workspace/SashHandle.vue
git commit -m "feat: #36 phase 6 sash 3 状态视觉强化（常态/hover/drag）"
```

---

## Task 7: chat 空状态（MasterColumn chat 分支 v-if）

**Files:**
- Modify: `src/views/workspace/MasterColumn.vue`（template chat 分支 + scoped css 加 conv-empty）

### - [ ] Step 7.1: MasterColumn.vue 添加 ConversationStore + Plus icon import

在 script 段（约 12-25 行）：

```ts
import { computed } from 'vue'
import { ElIcon, ElButton } from 'element-plus'
import { Plus } from '@element-plus/icons-vue'

import MasterList from './MasterList.vue'
import ConversationListPane from '@/components/chat/ConversationListPane.vue'
import { useWorkspaceLayoutStore } from '@/stores/workspaceLayout'
import { useConversationStore } from '@/stores/conversation'

const layout = useWorkspaceLayoutStore()
const convStore = useConversationStore()
```

### - [ ] Step 7.2: 模板 chat 分支 v-if 加空状态

读现状 `src/views/workspace/MasterColumn.vue:37-49` 模板：

```html
<div class="master-col__body">
  <!-- chat 类别：用 ConversationListPane（共享 store）；其余用 MasterList -->
  <ConversationListPane
    v-if="layout.currentCategory === 'chat'"
    :collapsed="false"
  />
  <MasterList
    v-else
    :items="layout.currentMasterItems"
    :active-item-id="layout.currentItem"
    @select="onSelect"
  />
</div>
```

改为：

```html
<div class="master-col__body">
  <!-- chat 类别：空状态 vs ConversationListPane（共享 store）；其余用 MasterList -->
  <template v-if="layout.currentCategory === 'chat'">
    <div v-if="convStore.conversations.length === 0" class="conv-empty">
      <img src="/avatar/momo-avatar.svg" alt="" class="conv-empty__illustration" />
      <p class="conv-empty__title">还没开始对话</p>
      <p class="conv-empty__hint">说一句"你好"就行</p>
      <ElButton type="primary" @click="convStore.create()">
        <ElIcon><Plus /></ElIcon>
        <span style="margin-left: 6px;">新对话</span>
      </ElButton>
    </div>
    <ConversationListPane v-else :collapsed="false" />
  </template>
  <MasterList
    v-else
    :items="layout.currentMasterItems"
    :active-item-id="layout.currentItem"
    @select="onSelect"
  />
</div>
```

### - [ ] Step 7.3: MasterColumn.vue 加 conv-empty scoped css

在 scoped css 段末尾追加：

```css
.conv-empty {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--aipet-space-6);
  gap: var(--aipet-space-3);
  text-align: center;
}

.conv-empty__illustration {
  width: 64px;
  height: 64px;
  opacity: 0.6;
  filter: grayscale(20%);
}

.conv-empty__title {
  font-size: 15px;
  font-weight: 500;
  color: var(--aipet-color-text-1);
  margin: 0;
}

.conv-empty__hint {
  font-size: 13px;
  color: var(--aipet-color-text-3);
  margin: 0;
}
```

### - [ ] Step 7.4: typecheck + 手测 chat 空状态

```bash
pnpm tsc --noEmit
pnpm tauri dev
```

手测：
- [ ] 切到 chat 类别 + 数据库 conversations 为空 → master 列显空状态（默默头像 + "还没开始对话" + "+ 新对话" 按钮）
- [ ] 点 "+ 新对话" → 创建一条；空状态消失，显 ConversationListPane
- [ ] 删光所有对话再回到空 → 空状态再次出现
- [ ] 磁吸窗 chat 表现零回归（磁吸窗仍走 ConversationListPane 没空状态，因为 ConversationListPane 内部不动）

### - [ ] Step 7.5: Commit

```bash
git add src/views/workspace/MasterColumn.vue
git commit -m "feat: #36 phase 7 chat 空状态 + 新对话 CTA"
```

---

## Task 8: brand-bar 微动效（active 竖条 + persona pulse）

**Files:**
- Modify: `src/views/workspace/BrandBar.vue`（scoped css）

### - [ ] Step 8.1: BrandBar.vue scoped css 追加

在 scoped css 段末尾（`.brand-bar__btn--ghost` 之后）追加：

```css
/* active 左竖条进入动画（切类别时给反馈） */
.brand-bar__btn--active::before {
  animation: brand-bar-active-in 200ms var(--aipet-ease-emphasized);
}

@keyframes brand-bar-active-in {
  from {
    transform: scaleY(0);
    opacity: 0;
  }
  to {
    transform: scaleY(1);
    opacity: 1;
  }
}

/* avatar pulse 光环（仅当 currentCategory=creation 且 currentItem=SettingsPersona 时）*/
.brand-bar__avatar--active::after {
  content: '';
  position: absolute;
  inset: -3px;
  border-radius: 50%;
  border: 1px solid color-mix(in srgb, var(--aipet-color-primary) 50%, transparent);
  animation: brand-bar-avatar-pulse 2s ease-in-out infinite;
  pointer-events: none;
}

@keyframes brand-bar-avatar-pulse {
  0%,
  100% {
    opacity: 0;
    transform: scale(1);
  }
  50% {
    opacity: 1;
    transform: scale(1.08);
  }
}
```

### - [ ] Step 8.2: 手测 brand-bar 微动效

```bash
pnpm tauri dev
```

- [ ] 点 chat → task → creation → config 切类别 → 左侧 primary 竖条 200ms 缩放进入
- [ ] 切到 creation 类别 + currentItem=SettingsPersona → avatar 周围 2s 周期 pulse 光环
- [ ] 切走 → pulse 消失

### - [ ] Step 8.3: Commit

```bash
git add src/views/workspace/BrandBar.vue
git commit -m "feat: #36 phase 8 brand-bar 微动效收口"
```

---

## Task 9: 全套验收 + grep 残留 + STATUS + issue close

**Files:**
- Modify: `docs/STATUS.md`

### - [ ] Step 9.1: 自动化验收

```bash
pnpm tsc --noEmit
pnpm test --run
pnpm build
cd src-tauri && cargo check && cargo clippy
```

期待：全绿。

### - [ ] Step 9.2: grep 残留

```bash
# .aipet-shell__close / .aipet-shell__header-spacer 全空
grep -rn "\.aipet-shell__close\|aipet-shell__header-spacer" src
# WorkspaceApp 不再用 AppShell
grep -rn "AppShell" src/views/workspace
# .content-header__close 仅剩 .content-header__close-sized
grep -rn "\.content-header__close" src/components/chat
```

期待：第一条全空；第二条全空；第三条仅 `.content-header__close-sized` 一处。

### - [ ] Step 9.3: 手测 13 项验收

按 spec §6.3 走一遍：

1. - [ ] **brand-bar 占整列**：截图显示 brand-bar 从 (0,0) 占到底
2. - [ ] **chrome 三按钮**：右上角 ─ □ ✕；hover ─/□ 浅灰 / hover ✕ 红 / hover max 出 Win11 snap layouts
3. - [ ] **drag 行为**：顶部 32px / brand-bar spacer 可拖；brand-bar 按钮不响应 drag
4. - [ ] **min/max/close**：min 任务栏（不进托盘）；max 全屏切；close 进托盘
5. - [ ] **sash 可拖**：3px hover 显高亮 → 拖宽 180-380 → 重启恢复
6. - [ ] **panel sticky title**：切 SettingsTheme 滚动 → "外观" sticky 列顶 backdrop blur
7. - [ ] **底部死区**：detail/master 贴底
8. - [ ] **chat 空状态**：删光对话 → 空状态显 + CTA 可点
9. - [ ] **master typography**：36px item + 14px font + 18px icon + 1.4 line-height
10. - [ ] **brand-bar 微动效**：切类别左竖条 200ms 缩放；SettingsPersona avatar pulse
11. - [ ] **chat 磁吸窗回归**：snap/AOT/ESC/Win11 透明 零回归；两窗同源 conversations
12. - [ ] **崩溃恢复**：kill → 重启 → 类别 + 项 + master 宽 还原（max 状态不存 — follow-up）
13. - [ ] **onboarding 零回归**：onboarding 仍走 AppShell standalone + body padding 24px

### - [ ] Step 9.4: 更新 STATUS.md

读 `docs/STATUS.md`。

修改"当前 milestone"行（约 22 行）：

```diff
- **当前 milestone**：M2 W3 进行中（7/7 落地；待办 + 物理交互待办）
+ **当前 milestone**：M2 W3 进行中（7/7 落地 + chrome 适配；待办 + 物理交互待办）
```

修改"当前 session 在做"行（约 23 行）：

```diff
- **当前 session 在做**：[#33](https://github.com/tl0502/APET/issues/33) ADR-021 P2 ...
+ **当前 session 在做**：[#36](https://github.com/tl0502/APET/issues/36) workspace chrome 层适配（自绘 chrome 三按钮 + brand-bar 占整列 + panel sticky + sash 3 状态 + chat 空状态 + 微动效）— 9 commit
```

修改"下一步"行（约 24 行）：

```diff
- **下一步**：#33 关闭 → workspace chrome 层精修单开 issue（brand-bar padding / sash 可见性 / header 固定 / 底部死区，0.5-1d）→ [#29](...) Todo + #21 KV 实例化 + LivingPet hook + AI 拆解 IPC 占位
+ **下一步**：#36 关闭 → [#29](https://github.com/tl0502/APET/issues/29) Todo + #21 KV 实例化 + LivingPet hook + AI 拆解 IPC 占位
```

修改 M2 W3-W4 段（约 39 行）：

```diff
- ### M2 W3-W4（任务三件套 + 物理交互 + 磁吸 + 人格工坊 + workspace 壳）— 进行中（6/7 完成）
+ ### M2 W3-W4（任务三件套 + 物理交互 + 磁吸 + 人格工坊 + workspace 壳）— 进行中（7/7 完成 + chrome 适配 ✅）
```

在 #33 段之后插入 #36 段：

```markdown
- ✅ [#36](https://github.com/tl0502/APET/issues/36) workspace chrome 层视觉适配：WorkspaceApp 自绘 chrome 三按钮（min/max/close）+ brand-bar 占整列 + panel.css/buttons.css 全局抽取 + 8 panel SFC scoped css 重构 + master 节奏对齐业界 + sash 3 状态 + chat 空状态 + brand-bar 微动效（9 commit `<hash1>→<hash9>`）
```

实际 commit hash 在 Step 9.5 之前用 `git log --oneline -10` 取。

### - [ ] Step 9.5: Commit STATUS

```bash
git log --oneline -10  # 记录 9 个 phase 的 commit hash
# 把 hash 填入 STATUS.md
git add docs/STATUS.md
git commit -m "docs: #36 chrome 适配落地 STATUS 同步"
```

### - [ ] Step 9.6: GitHub issue close

```bash
gh issue close 36 --comment "$(cat <<'EOF'
落地完成。9 commit `<hash1>→<hash9>`。

## 关键决策回放（详见 [spec](../blob/main/docs/specs/2026-05-21-workspace-chrome-adapt-design.md)）

- chrome 范式 = 自绘 + 关闭浮右上 + brand-bar 占整列（Discord/Slack/VSCode 范式 / desktop-ui-principles §3 §4）
- 架构 = WorkspaceApp 替 AppShell（workspace 长居型工作窗 vs onboarding 工具型一次性窗 解耦）
- chrome 按钮 = min/max/close 三按钮（Win11 snap layouts hover 支持）
- panel sticky title = 全局 panel.css + backdrop blur（与 chat content-header 同语言）
- master typography = 14px font / 36px item / 18px icon / 1.4 line-height（业界中位数）
- sash 保 3px width（chrome padding 修后偏位问题自解，热区不扩）+ 3 状态视觉强化
- chat 空状态放置 = MasterColumn chat 分支 v-if（不动 ConversationListPane / ConversationSidebar）

## 关键偏离

- max 状态崩溃恢复**未实现**（workspaceLayout store 不存 isMaximized）— follow-up 单开
- About 模态弹窗 ❓ 按钮仍 placeholder（M3+ 实装）

## 实测

- pnpm tsc --noEmit / pnpm test --run / pnpm build / cargo check / cargo clippy 全绿
- 13 项手测全过（spec §6.3）
- grep 残留全空：.aipet-shell__close 无 / AppShell 无 / dockview-vue 无

## Follow-up

- workspace max 状态崩溃恢复（小工时，单开 issue）
- About 模态实装（M3+）
EOF
)"
```

---

## Verification

### 自动化（在 Task 9 跑）
- `pnpm tsc --noEmit`
- `pnpm test --run`
- `pnpm build`
- `cd src-tauri && cargo check && cargo clippy`

### grep 残留（Task 9 跑）
- `grep -rn "\.aipet-shell__close\|aipet-shell__header-spacer" src` → 空
- `grep -rn "AppShell" src/views/workspace` → 空
- `grep -rn "\.content-header__close" src/components/chat` → 仅 `.content-header__close-sized`

### 手测（Task 9 跑 spec §6.3 13 项）

---

## 风险与回滚

| 风险 | 缓解 |
|---|---|
| panel.css 抽取后 onboarding 视觉异常 | onboarding/main.ts 不引入 panel.css，零影响；Task 9 Step 9.3 #13 验证 |
| chrome 按钮 hover max 不出 snap layouts | max 按钮挂 click 不挂 drag-region，Win11 自动出 |
| ChatThreadPane close 复用类视觉降级 | content-header__close-sized 保留 height:56 override；Task 3.3 手测 |
| 用户工作树 chat 子组件被覆盖 | 严格不改 ConversationSidebar/ChatInput/MessageBubble/MessageList/tokens.css；Task 7 仅在 MasterColumn 内动 |

回滚：每 phase 一 commit，`git revert <hash>` 即可单独回滚某 phase。

---

## 工时

| Task | 估时 | 累计 |
|---|---|---|
| 1 全局 css 抽取 | 0.1d | 0.1d |
| 2 WorkspaceApp 重写 + BrandBar drag + components.css 清理 | 0.25d | 0.35d |
| 3 ChatThreadPane close 复用 | 0.05d | 0.4d |
| 4 8 panel SFC 重构 | 0.25d | 0.65d |
| 5 master 节奏调整 | 0.1d | 0.75d |
| 6 sash 视觉强化 | 0.05d | 0.8d |
| 7 chat 空状态 | 0.1d | 0.9d |
| 8 brand-bar 微动效 | 0.05d | 0.95d |
| 9 验收 + STATUS + issue close | 0.15d | 1.1d |

**实际工时上限 1.1d**，与 spec §8 估算 1d 上限基本一致。
