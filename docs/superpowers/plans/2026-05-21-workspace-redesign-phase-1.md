# Workspace 重设计 Phase 1 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 workspace 顶部 chrome 改成实色 L 型框 + 实做 in-workspace 用户 popup（含 6 panel）+ 简化 workspace IA + 立 panel 容器公约 + SettingsTheme 套用容器公约作示范。

**Architecture:** CSS Grid 三栏壳（48px 顶栏 + 60 sidebar + 240 master + flex detail），实色 L 型 chrome 框（surface-soft）+ 白 detail 主舞台两档色阶。popup 是 in-workspace overlay（880×580，backdrop + ESC/click 关闭），内置 `userPopup` Pinia store 管理 isOpen + activeNav。Panel 公约 `.panel--form` / `--list` / `--chat` + `.panel__content` 包裹层，正文 max-width 居中而标题 sticky 通栏。

**Tech Stack:** Vue 3 + TypeScript + Pinia (setup-style) + Vite + vitest + Tauri 2.x. 复用现有 `useNicknameStore` / `useAvatarsStore` / `NicknameForm` / `UserAvatarUploader` 组件。

**Spec 基线**：[docs/superpowers/specs/2026-05-21-workspace-redesign/design.md](../specs/2026-05-21-workspace-redesign/design.md)

---

## File Structure

### 新建文件

| 文件 | 责任 |
|---|---|
| `src/stores/userPopup.ts` | popup 开关 + 当前选中 nav 项 state |
| `src/stores/__tests__/userPopup.test.ts` | userPopup store 单测 |
| `src/components/popup/UserPopup.vue` | popup overlay shell（backdrop + 容器 + grid + 关闭逻辑） |
| `src/components/popup/PopupSidebar.vue` | popup 左 240 列（user card + 搜索 + nav） |
| `src/panels/user/UserProfilePanel.vue` | 实做：头像 + 昵称 + 个性资料 |
| `src/panels/user/UserHelpPanel.vue` | 实做：GitHub + 文档 + 快捷键 |
| `src/panels/user/UserAboutPanel.vue` | 实做：搬 SettingsAboutPanel 内容 |
| `src/panels/user/UserPlaceholderPanel.vue` | 通用 disabled 占位（账户/隐私/通知共用） |
| `src/services/userProfile.ts` | thin wrapper：`getUserBio` / `setUserBio` 走 memory KV |

### 修改文件

| 文件 | 改动 |
|---|---|
| `src/styles/panel.css` | 追加 `.panel__content` / `.panel--form` / `.panel--chat` |
| `src/styles/tokens.css` | 文件头注释加色区映射速查 |
| `src/views/workspace/WorkspaceApp.vue` | 改 CSS Grid + 新增 topbar 结构 + chrome 三按钮入 grid |
| `src/views/workspace/BrandBar.vue` | 删头像区 + 删 32px 让位 + 底部 help 换为用户头像 |
| `src/views/workspace/DetailColumn.vue` | 删 SettingsNickname / SettingsAbout 的 v-show 块 |
| `src/stores/workspaceLayout.ts` | BRAND_BAR_ITEMS.config.masterItems 删两项 |
| `src/stores/__tests__/workspaceLayout.test.ts` | 更新 case 2/4 引用的 panel id |
| `src/panels/settings/SettingsThemePanel.vue` | 套 `panel--form` + 包 `.panel__content`（示范） |

### 删除文件

| 文件 | 理由 |
|---|---|
| `src/panels/settings/SettingsNicknamePanel.vue` | 内容搬到 `UserProfilePanel`（NicknameForm 组件保留共用） |
| `src/panels/settings/SettingsAboutPanel.vue` | 内容搬到 `UserAboutPanel` |

---

## Task 1: 开 Phase 1 GitHub issue

**Files:**
- 远端 GitHub（`tl0502/APET` 仓库），无本地文件

- [ ] **Step 1: 用 gh 创建 issue，title 短，body 引 spec 路径**

Run:
```bash
gh issue create \
  --title "workspace chrome L 型框重做 + Profile popup（ADR-021 P3）" \
  --label "type:feat,module:ui,milestone:M2" \
  --body "$(cat <<'EOF'
spec 基线：docs/superpowers/specs/2026-05-21-workspace-redesign/design.md

实施 plan：docs/superpowers/plans/2026-05-21-workspace-redesign-phase-1.md

## Phase 1 范围

- chrome 改实色 L 型框：48px 顶栏（avatar + capsule 占位 + 三按钮）+ sidebar/master 同 surface-soft 填充，detail 白色
- sidebar 底部 help 换成用户头像，点击呼出 in-workspace popup（880×580）
- popup 内含 6 panel：UserProfile（实做，复用 NicknameForm + 新 bio 字段）/ UserHelp（实做）/ UserAbout（搬 SettingsAbout）/ UserAccount + UserPrivacy + UserNotifications（disabled 占位）
- workspace 设置类简化：删 SettingsNickname / SettingsAbout（搬走 / 删 panel SFC），master 只剩 外观 + LLM Provider
- 立 panel 容器公约 `.panel--form` / `--list` / `--chat`；SettingsTheme 套用作示范

## 验收

- pnpm typecheck && pnpm build && pnpm test && cargo check 四绿
- 手动 e2e（按 spec §7.6）
- 视觉对照 mockup 01 + 02 + 03
- 关闭 #36（被本 issue 覆盖）

## Phase 2 待办

其余 6 panel（SettingsProvider / SettingsPersona / TasksReminder / TasksPomodoro / TasksTodo / ChatThreadPane）套容器公约 → 后续单独 issue。
EOF
)"
```

Expected: 输出 issue URL + 编号（记录为 `$PHASE1_ISSUE`）。

- [ ] **Step 2: 把 issue 号写进 plan 头部备用**

Run:
```bash
gh issue list --search "workspace chrome L 型框重做" --json number --jq '.[0].number'
```

Expected: 输出数字（例 `37`）。后续 commit message 用 `feat: #37 ...` 引用。

---

## Task 2: panel.css 容器公约

**Files:**
- Modify: `src/styles/panel.css`（追加 ~30 行）

- [ ] **Step 1: 读现有 panel.css**

Run:
```bash
cat src/styles/panel.css | wc -l
```

Expected: 现状约 67 行。

- [ ] **Step 2: 在文件末尾追加 `.panel__content` + `.panel--form` + `.panel--chat`**

Use Edit tool to append after `.panel__actions` block:

```css
/* === 容器公约（2026-05-21 重设计）===
   每个 panel SFC 在根节点加 `panel--form` / `panel--list` / `panel--chat` 修饰类，
   .panel__content 包裹 body 文字（非 .panel__title），自动套对应 max-width。

   设计取舍：
   - title 通栏（保留 sticky breakout）+ 正文居中是 Linear/Notion/Vercel 范式
   - form 720：单列表单读起来稳，不被宽窗拉散
   - chat 880：长文本对话防行长过长（>100 字伤阅读）
   - list 全宽（默认）：长列表 + 多列字段，宽度有用 */

.panel__content {
  flex: 1 1 auto;
  /* 默认 list 行为：全宽 */
}

.panel--form .panel__content {
  max-width: 720px;
  margin: 0 auto;
  width: 100%;
}

.panel--chat .panel__content {
  max-width: 880px;
  margin: 0 auto;
  width: 100%;
}
```

- [ ] **Step 3: 在文件头注释追加色区映射速查（仅注释，无规则）**

Use Edit tool to add after the existing header comment block:

```css
/* === 色区 token 映射（2026-05-21 重设计）===
   - workspace L 型框（topbar + sidebar + master）→ --aipet-color-surface-soft
   - workspace detail 主舞台 → --aipet-color-bg
   - zone 之间 1px 分隔 → --aipet-color-border-faint
   - .panel__title sticky 浮玻璃 → --aipet-color-surface-blur + blur(12)
   - popup backdrop → --aipet-color-overlay
   - popup 容器 / 主区 → --aipet-color-bg（同 detail）
   - popup sidebar 240 → --aipet-color-surface-soft（同 L 框） */
```

- [ ] **Step 4: typecheck 三绿**

Run:
```bash
pnpm typecheck && pnpm lint
```

Expected: 两条都 0 错误（CSS-only 改动，typecheck 不会影响）。

- [ ] **Step 5: Commit**

Run:
```bash
git add src/styles/panel.css
git commit -m "feat: #<PHASE1_ISSUE> panel.css 容器公约 .panel--form/--chat + 色区映射速查"
```

---

## Task 3: userPopup store (TDD)

**Files:**
- Create: `src/stores/userPopup.ts`
- Create: `src/stores/__tests__/userPopup.test.ts`

- [ ] **Step 1: 写失败的测试**

Create file `src/stores/__tests__/userPopup.test.ts`:

```typescript
// userPopup store 单测 — 5 case
//
// 覆盖：open / close / setNav / 默认 activeNav / 重复 open 不抖

import { setActivePinia, createPinia } from 'pinia'
import { beforeEach, describe, expect, it } from 'vitest'

import { useUserPopupStore } from '../userPopup'

beforeEach(() => {
  setActivePinia(createPinia())
})

describe('userPopup store', () => {
  it('case 1: 默认 isOpen=false, activeNav="profile"', () => {
    const store = useUserPopupStore()
    expect(store.isOpen).toBe(false)
    expect(store.activeNav).toBe('profile')
  })

  it('case 2: open() 翻 isOpen + 默认进 profile', () => {
    const store = useUserPopupStore()
    store.setNav('about') // 模拟上次留在 about
    store.close()
    store.open()
    expect(store.isOpen).toBe(true)
    expect(store.activeNav).toBe('profile') // 每次重新进 profile（spec §4.3）
  })

  it('case 3: close() 翻 isOpen=false', () => {
    const store = useUserPopupStore()
    store.open()
    store.close()
    expect(store.isOpen).toBe(false)
  })

  it('case 4: setNav 切 nav 但不影响 isOpen', () => {
    const store = useUserPopupStore()
    store.open()
    store.setNav('help')
    expect(store.activeNav).toBe('help')
    expect(store.isOpen).toBe(true)
    store.setNav('about')
    expect(store.activeNav).toBe('about')
  })

  it('case 5: setNav 拒绝 disabled nav id（保持当前 activeNav 不变）', () => {
    const store = useUserPopupStore()
    store.open()
    store.setNav('profile')
    store.setNav('account') // disabled
    expect(store.activeNav).toBe('profile') // 不动
    store.setNav('privacy') // disabled
    expect(store.activeNav).toBe('profile')
    store.setNav('notifications') // disabled
    expect(store.activeNav).toBe('profile')
  })
})
```

- [ ] **Step 2: 运行测试，确认失败**

Run:
```bash
pnpm test src/stores/__tests__/userPopup.test.ts
```

Expected: 5 case 全 FAIL（store 文件不存在）。

- [ ] **Step 3: 创建 userPopup store**

Create file `src/stores/userPopup.ts`:

```typescript
// userPopup store（2026-05-21 重设计）：管理 in-workspace 用户 popup 的开关 + nav 选中。
//
// 设计：
// - 6 个 nav 项：profile / account / privacy / notifications / help / about
// - 其中 account / privacy / notifications 是 disabled 占位（spec §4.3）
// - 每次 open() 都从 profile 开始（不记忆 activeNav 跨次打开，spec §4.3）
// - setNav 自带 disabled 守卫，UI 层无需重复判断

import { defineStore } from 'pinia'
import { ref } from 'vue'

export type PopupNavId =
  | 'profile'
  | 'account'
  | 'privacy'
  | 'notifications'
  | 'help'
  | 'about'

const DISABLED_NAV_IDS: readonly PopupNavId[] = [
  'account',
  'privacy',
  'notifications',
] as const

export const useUserPopupStore = defineStore('userPopup', () => {
  const isOpen = ref(false)
  const activeNav = ref<PopupNavId>('profile')

  function open() {
    activeNav.value = 'profile' // 每次都重置（spec §4.3）
    isOpen.value = true
  }

  function close() {
    isOpen.value = false
  }

  function setNav(id: PopupNavId) {
    if (DISABLED_NAV_IDS.includes(id)) return
    activeNav.value = id
  }

  function isDisabled(id: PopupNavId): boolean {
    return DISABLED_NAV_IDS.includes(id)
  }

  return { isOpen, activeNav, open, close, setNav, isDisabled }
})
```

- [ ] **Step 4: 运行测试，确认通过**

Run:
```bash
pnpm test src/stores/__tests__/userPopup.test.ts
```

Expected: 5 case 全 PASS。

- [ ] **Step 5: Commit**

Run:
```bash
git add src/stores/userPopup.ts src/stores/__tests__/userPopup.test.ts
git commit -m "feat: #<PHASE1_ISSUE> userPopup store + 5 vitest case（open/close/setNav/disabled 守卫）"
```

---

## Task 4: userProfile service（bio 字段）

**Files:**
- Create: `src/services/userProfile.ts`

- [ ] **Step 1: 创建 service 文件**

Create file `src/services/userProfile.ts`:

```typescript
// userProfile service — Profile 字段（bio / 个性资料）的 KV 读写。
//
// 走通用 memory KV（services/memory.ts），不新建 Tauri command。
// avatar_path / nickname 走各自专用 service；本文件只管 bio。
//
// KV key 约定：
// - user:bio — 个性资料文本（一段话，<= 200 字符前端校验）

import { getMemory, setMemory } from './memory'

const USER_BIO_KEY = 'user:bio'

/** 读用户个性资料；不存在返 null。 */
export function getUserBio(): Promise<string | null> {
  return getMemory(USER_BIO_KEY)
}

/** 写入用户个性资料；空字符串与正常字符串都允许（用户清空意图）。 */
export function setUserBio(value: string): Promise<void> {
  return setMemory(USER_BIO_KEY, value)
}
```

- [ ] **Step 2: typecheck 通过**

Run:
```bash
pnpm typecheck
```

Expected: 0 错误。

- [ ] **Step 3: Commit**

Run:
```bash
git add src/services/userProfile.ts
git commit -m "feat: #<PHASE1_ISSUE> userProfile service（bio 字段走 memory KV）"
```

---

## Task 5: UserPopup shell（overlay + 关闭逻辑）

**Files:**
- Create: `src/components/popup/UserPopup.vue`

- [ ] **Step 1: 写 UserPopup.vue**

Create file `src/components/popup/UserPopup.vue`:

```vue
<script setup lang="ts">
// UserPopup（2026-05-21 重设计）：in-workspace 用户 popup overlay。
//
// 职责：
// - 渲染 backdrop + 容器（880×580）
// - ESC / 点 backdrop / 点 × 关闭
// - focus trap（首个 focusable = popup 内第一个可聚焦元素）
//
// 不在本组件做：
// - 内部 sidebar（PopupSidebar.vue）
// - 6 个 panel（UserProfile / UserHelp / UserAbout / UserPlaceholder）

import { computed, onMounted, onBeforeUnmount, ref, watch, nextTick } from 'vue'

import PopupSidebar from './PopupSidebar.vue'
import UserProfilePanel from '@/panels/user/UserProfilePanel.vue'
import UserHelpPanel from '@/panels/user/UserHelpPanel.vue'
import UserAboutPanel from '@/panels/user/UserAboutPanel.vue'
import UserPlaceholderPanel from '@/panels/user/UserPlaceholderPanel.vue'

import { useUserPopupStore } from '@/stores/userPopup'

const popup = useUserPopupStore()

const containerRef = ref<HTMLElement | null>(null)
const previousFocus = ref<HTMLElement | null>(null)

const panelTitle = computed(() => {
  switch (popup.activeNav) {
    case 'profile': return '个人资料'
    case 'account': return '账户'
    case 'privacy': return '数据与隐私'
    case 'notifications': return '通知'
    case 'help': return '帮助'
    case 'about': return '关于'
  }
})

function onBackdropClick(e: MouseEvent) {
  // 仅 backdrop 本体点击关闭（避免子元素点击穿透）
  if (e.target === e.currentTarget) {
    popup.close()
  }
}

function onKeydown(e: KeyboardEvent) {
  if (!popup.isOpen) return
  if (e.key === 'Escape') {
    e.preventDefault()
    popup.close()
  }
}

// focus trap：popup 打开时聚焦首个可聚焦元素，关闭时还原焦点
watch(
  () => popup.isOpen,
  async (open) => {
    if (open) {
      previousFocus.value = document.activeElement as HTMLElement | null
      await nextTick()
      const first = containerRef.value?.querySelector<HTMLElement>(
        'button, input, textarea, [tabindex]:not([tabindex="-1"])',
      )
      first?.focus()
    } else {
      previousFocus.value?.focus()
      previousFocus.value = null
    }
  },
)

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <Transition name="popup-fade">
    <div
      v-if="popup.isOpen"
      class="popup-backdrop"
      role="dialog"
      aria-modal="true"
      :aria-label="`用户设置 - ${panelTitle}`"
      @click="onBackdropClick"
    >
      <div ref="containerRef" class="popup-container">
        <PopupSidebar />

        <main class="popup-main">
          <header class="popup-main__header">
            <span class="popup-main__title">{{ panelTitle }}</span>
            <button
              class="popup-main__close"
              type="button"
              aria-label="关闭"
              @click="popup.close()"
            >✕</button>
          </header>

          <div class="popup-main__content">
            <UserProfilePanel v-show="popup.activeNav === 'profile'" />
            <UserHelpPanel v-show="popup.activeNav === 'help'" />
            <UserAboutPanel v-show="popup.activeNav === 'about'" />
            <!-- 3 个 disabled nav 选中时不渲染（store 守卫不允许 setNav 到这些；保留模板 v-show 防御性） -->
            <UserPlaceholderPanel
              v-show="popup.activeNav === 'account'"
              kind="account"
            />
            <UserPlaceholderPanel
              v-show="popup.activeNav === 'privacy'"
              kind="privacy"
            />
            <UserPlaceholderPanel
              v-show="popup.activeNav === 'notifications'"
              kind="notifications"
            />
          </div>
        </main>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.popup-backdrop {
  position: fixed;
  inset: 0;
  background: var(--aipet-color-overlay);
  z-index: var(--aipet-z-dialog);
  display: flex;
  align-items: center;
  justify-content: center;
  /* 不开 backdrop-filter，避免与 panel__title 浮玻璃叠加性能差 */
}

.popup-container {
  width: 880px;
  height: 580px;
  max-width: calc(100vw - 48px);
  max-height: calc(100vh - 48px);
  background: var(--aipet-color-bg);
  border-radius: 12px;
  box-shadow: var(--aipet-shadow-float);
  display: grid;
  grid-template-columns: 240px 1fr;
  overflow: hidden;
}

.popup-main {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.popup-main__header {
  flex: 0 0 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px 0 24px;
  border-bottom: 1px solid var(--aipet-color-border-faint);
  user-select: none;
}

.popup-main__title {
  font-size: 15px;
  font-weight: 600;
  color: var(--aipet-color-text-1);
}

.popup-main__close {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 6px;
  color: var(--aipet-color-text-2);
  cursor: pointer;
  font-size: 14px;
  transition: background-color 120ms ease, color 120ms ease;
}

.popup-main__close:hover {
  background: color-mix(in srgb, var(--aipet-color-text-1) 6%, transparent);
  color: var(--aipet-color-text-1);
}

.popup-main__close:focus-visible {
  outline: none;
  box-shadow: var(--aipet-ring-focus);
}

.popup-main__content {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: var(--aipet-space-6) var(--aipet-space-8);
  min-height: 0;
}

/* 进入/离开动效 */
.popup-fade-enter-active {
  transition: opacity 220ms ease-out;
}

.popup-fade-enter-active .popup-container {
  transition: transform 220ms var(--aipet-ease-emphasized);
}

.popup-fade-leave-active {
  transition: opacity 160ms ease-in;
}

.popup-fade-enter-from {
  opacity: 0;
}

.popup-fade-enter-from .popup-container {
  transform: scale(0.96);
}

.popup-fade-leave-to {
  opacity: 0;
}
</style>
```

- [ ] **Step 2: typecheck 通过（PopupSidebar / 4 panel 还没创建，预期会失败）**

Run:
```bash
pnpm typecheck
```

Expected: typecheck 报 "Cannot find module" for PopupSidebar + 4 user panels（下几个 task 解决）。

- [ ] **Step 3: 暂不 commit（依赖 Task 6-10 才能跑通）**

继续到 Task 6。

---

## Task 6: PopupSidebar 组件

**Files:**
- Create: `src/components/popup/PopupSidebar.vue`

- [ ] **Step 1: 写 PopupSidebar.vue**

Create file `src/components/popup/PopupSidebar.vue`:

```vue
<script setup lang="ts">
// PopupSidebar（2026-05-21 重设计）：popup 左 240 列。
//
// 三段（spec §4.2）：
// 1) User identity card（固定，整块点击 → 切到 profile）
// 2) 搜索框（固定，Phase 1 客户端 nav 项过滤）
// 3) Nav 列表（滚动，3 分组扁平结构）
//
// 复用 useNicknameStore / useAvatarsStore（与磁吸 chat 窗同源）。

import { computed, onMounted, ref } from 'vue'

import { useUserPopupStore, type PopupNavId } from '@/stores/userPopup'
import { useNicknameStore } from '@/stores/nickname'
import { useAvatarsStore } from '@/stores/avatars'

const popup = useUserPopupStore()
const nickname = useNicknameStore()
const avatars = useAvatarsStore()

const searchQuery = ref('')

interface NavItemDef {
  id: PopupNavId
  label: string
  icon: string
  badge?: string
  disabled?: boolean
}

interface NavGroupDef {
  title: string
  items: NavItemDef[]
}

const NAV_GROUPS: NavGroupDef[] = [
  {
    title: '个人',
    items: [
      { id: 'profile', label: '个人资料', icon: '👤' },
      { id: 'account', label: '账户', icon: '🔑', badge: '登录后', disabled: true },
    ],
  },
  {
    title: '应用',
    items: [
      { id: 'privacy', label: '数据与隐私', icon: '🔒', badge: 'M3+', disabled: true },
      { id: 'notifications', label: '通知', icon: '🔔', badge: 'M3+', disabled: true },
    ],
  },
  {
    title: '支持',
    items: [
      { id: 'help', label: '帮助', icon: '❓' },
      { id: 'about', label: '关于', icon: 'ⓘ' },
    ],
  },
]

// 客户端 nav 项过滤：模糊匹配 label
const filteredGroups = computed<NavGroupDef[]>(() => {
  const q = searchQuery.value.trim().toLowerCase()
  if (!q) return NAV_GROUPS
  return NAV_GROUPS.map((g) => ({
    ...g,
    items: g.items.filter((i) => i.label.toLowerCase().includes(q)),
  })).filter((g) => g.items.length > 0)
})

function onIdentityClick() {
  popup.setNav('profile')
}

function onNavClick(item: NavItemDef) {
  if (item.disabled) return
  popup.setNav(item.id)
}

onMounted(async () => {
  // 拉用户昵称 + avatar（popup 复用 store；store 自带 loaded 守卫，多次调安全）
  await Promise.all([nickname.load(), avatars.load()])
  await Promise.all([nickname.ensureListener(), avatars.ensureListener()])
})
</script>

<template>
  <aside class="popup-sidebar" aria-label="用户设置导航">
    <!-- 1) User identity card -->
    <button
      class="popup-sidebar__identity"
      type="button"
      aria-label="编辑个人资料"
      @click="onIdentityClick"
    >
      <div class="popup-sidebar__identity-avatar">
        <img
          v-if="avatars.userAvatarUrl"
          :src="avatars.userAvatarUrl"
          alt=""
          class="popup-sidebar__identity-img"
        />
        <span v-else>{{ nickname.user?.[0] ?? '你' }}</span>
      </div>
      <div class="popup-sidebar__identity-info">
        <div class="popup-sidebar__identity-name">
          {{ nickname.user ?? '未设置昵称' }}
        </div>
        <div class="popup-sidebar__identity-edit">编辑资料</div>
      </div>
    </button>

    <!-- 2) 搜索 -->
    <div class="popup-sidebar__search">
      <span class="popup-sidebar__search-icon" aria-hidden="true">🔍</span>
      <input
        v-model="searchQuery"
        type="text"
        class="popup-sidebar__search-input"
        placeholder="搜索设置..."
        aria-label="搜索设置"
      />
    </div>

    <!-- 3) Nav 分组列表 -->
    <nav class="popup-sidebar__nav" aria-label="设置分组">
      <div
        v-for="group in filteredGroups"
        :key="group.title"
        class="popup-sidebar__nav-group"
      >
        <div class="popup-sidebar__nav-group-title">{{ group.title }}</div>
        <button
          v-for="item in group.items"
          :key="item.id"
          type="button"
          class="popup-sidebar__nav-item"
          :class="{
            'popup-sidebar__nav-item--active':
              !item.disabled && popup.activeNav === item.id,
            'popup-sidebar__nav-item--disabled': item.disabled,
          }"
          :disabled="item.disabled"
          :aria-current="popup.activeNav === item.id ? 'page' : undefined"
          @click="onNavClick(item)"
        >
          <span class="popup-sidebar__nav-item-icon">{{ item.icon }}</span>
          <span class="popup-sidebar__nav-item-label">{{ item.label }}</span>
          <span v-if="item.badge" class="popup-sidebar__nav-item-badge">
            {{ item.badge }}
          </span>
        </button>
      </div>
    </nav>
  </aside>
</template>

<style scoped>
.popup-sidebar {
  background: var(--aipet-color-surface-soft);
  border-right: 1px solid var(--aipet-color-border-faint);
  display: flex;
  flex-direction: column;
  min-height: 0;
}

/* 1) identity card */
.popup-sidebar__identity {
  flex: 0 0 auto;
  margin: var(--aipet-space-4) var(--aipet-space-3) var(--aipet-space-3);
  padding: var(--aipet-space-3);
  display: flex;
  align-items: center;
  gap: var(--aipet-space-3);
  background: var(--aipet-color-bg);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: 12px;
  cursor: pointer;
  text-align: left;
  transition:
    border-color 120ms ease,
    box-shadow 120ms ease;
}

.popup-sidebar__identity:hover {
  border-color: var(--aipet-color-border);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
}

.popup-sidebar__identity:focus-visible {
  outline: none;
  border-color: var(--aipet-color-primary);
  box-shadow: var(--aipet-ring-focus);
}

.popup-sidebar__identity-avatar {
  width: 44px;
  height: 44px;
  flex: 0 0 auto;
  border-radius: 50%;
  overflow: hidden;
  background: var(--aipet-color-surface);
  color: var(--aipet-color-text-2);
  font-size: 16px;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--aipet-color-border);
}

.popup-sidebar__identity-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.popup-sidebar__identity-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.popup-sidebar__identity-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--aipet-color-text-1);
  line-height: 1.3;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.popup-sidebar__identity-edit {
  font-size: 12px;
  color: var(--aipet-color-text-3);
  line-height: 1.3;
  transition: color 120ms ease;
}

.popup-sidebar__identity:hover .popup-sidebar__identity-edit {
  color: var(--aipet-color-primary);
}

/* 2) 搜索 */
.popup-sidebar__search {
  flex: 0 0 auto;
  position: relative;
  margin: 0 var(--aipet-space-3) var(--aipet-space-3);
}

.popup-sidebar__search-icon {
  position: absolute;
  left: 12px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 12px;
  color: var(--aipet-color-text-3);
  pointer-events: none;
}

.popup-sidebar__search-input {
  width: 100%;
  height: 32px;
  padding: 0 12px 0 32px;
  background: var(--aipet-color-bg);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: 16px;
  font-size: 13px;
  color: var(--aipet-color-text-1);
  outline: none;
  box-sizing: border-box;
  font-family: inherit;
  transition: border-color 120ms ease, box-shadow 120ms ease;
}

.popup-sidebar__search-input:focus {
  border-color: var(--aipet-color-primary);
  box-shadow: var(--aipet-ring-focus);
}

/* 3) Nav */
.popup-sidebar__nav {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: var(--aipet-space-1) var(--aipet-space-2) var(--aipet-space-4);
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
  min-height: 0;
}

.popup-sidebar__nav-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.popup-sidebar__nav-group-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--aipet-color-text-3);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  padding: var(--aipet-space-1) var(--aipet-space-3) var(--aipet-space-2);
  user-select: none;
}

.popup-sidebar__nav-item {
  position: relative;
  width: 100%;
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
  padding: 7px var(--aipet-space-3);
  border-radius: 6px;
  font-size: 14px;
  color: var(--aipet-color-text-2);
  background: transparent;
  border: none;
  cursor: pointer;
  min-height: 32px;
  text-align: left;
  font-family: inherit;
  transition: background-color 100ms ease, color 100ms ease;
}

.popup-sidebar__nav-item:hover {
  background: color-mix(in srgb, var(--aipet-color-text-1) 5%, transparent);
  color: var(--aipet-color-text-1);
}

.popup-sidebar__nav-item:focus-visible {
  outline: none;
  box-shadow: var(--aipet-ring-focus);
}

.popup-sidebar__nav-item--active {
  background: color-mix(in srgb, var(--aipet-color-primary) 12%, transparent);
  color: var(--aipet-color-primary);
  font-weight: 500;
}

.popup-sidebar__nav-item--active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 8px;
  bottom: 8px;
  width: 2px;
  border-radius: 2px;
  background: var(--aipet-color-primary);
}

.popup-sidebar__nav-item--disabled {
  color: var(--aipet-color-text-3);
  cursor: not-allowed;
  opacity: 0.5;
}

.popup-sidebar__nav-item--disabled:hover {
  background: transparent;
  color: var(--aipet-color-text-3);
}

.popup-sidebar__nav-item-icon {
  font-size: 15px;
  width: 18px;
  text-align: center;
  flex: 0 0 auto;
}

.popup-sidebar__nav-item-label {
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.popup-sidebar__nav-item-badge {
  font-size: 10px;
  color: var(--aipet-color-text-3);
  background: color-mix(in srgb, var(--aipet-color-text-1) 5%, transparent);
  padding: 1px 6px;
  border-radius: 4px;
  flex: 0 0 auto;
}
</style>
```

- [ ] **Step 2: 暂不 typecheck（其他 panel 还没建）**

---

## Task 7: UserProfilePanel（实做：avatar + nickname + bio）

**Files:**
- Create: `src/panels/user/UserProfilePanel.vue`

- [ ] **Step 1: 写 UserProfilePanel.vue**

Create file `src/panels/user/UserProfilePanel.vue`:

```vue
<script setup lang="ts">
// UserProfilePanel — 用户个人资料面板（复用 NicknameForm + 新 bio 字段）。
//
// 设计：
// - 复用 NicknameForm（已含头像 cropper + 昵称编辑 + 校验 + 转场注入开关）
// - 追加 bio textarea（个性资料，<= 200 字符前端校验，走 userProfile service KV）
// - bio 与 nickname 各自独立保存（避免一起 commit 半成品）

import { onMounted, ref } from 'vue'
import { ElButton, ElInput } from 'element-plus'

import NicknameForm from '@/components/settings/NicknameForm.vue'
import { useToast } from '@/composables/useToast'
import { getUserBio, setUserBio } from '@/services/userProfile'

const toast = useToast()

const BIO_MAX = 200

const bioDraft = ref('')
const bioOriginal = ref('')
const bioLoading = ref(false)
const bioSaving = ref(false)

const bioChanged = () => bioDraft.value !== bioOriginal.value
const bioOverLimit = () => bioDraft.value.length > BIO_MAX

onMounted(async () => {
  bioLoading.value = true
  try {
    const v = await getUserBio()
    bioDraft.value = v ?? ''
    bioOriginal.value = v ?? ''
  } catch (e) {
    console.warn('[UserProfilePanel] getUserBio failed:', e)
  } finally {
    bioLoading.value = false
  }
})

async function onSaveBio() {
  if (bioOverLimit()) {
    toast.error(`个性资料不能超过 ${BIO_MAX} 字符`)
    return
  }
  bioSaving.value = true
  try {
    await setUserBio(bioDraft.value)
    bioOriginal.value = bioDraft.value
    toast.success('个性资料已保存')
  } catch (e) {
    toast.error(`保存失败：${e instanceof Error ? e.message : String(e)}`)
  } finally {
    bioSaving.value = false
  }
}
</script>

<template>
  <section class="panel panel--form">
    <h2 class="panel__title">个人资料</h2>
    <div class="panel__content">
      <p class="panel__hint">
        头像和昵称用于桃宝对你的称呼与显示；个性资料是可选的、对桃宝介绍你的几句话。
      </p>

      <!-- 复用 NicknameForm：头像上传 + 昵称编辑（含校验 + 转场开关） -->
      <NicknameForm />

      <!-- 个性资料：独立 section + 独立保存按钮 -->
      <div class="panel__section">
        <h3 class="panel__subtitle">个性资料</h3>
        <ElInput
          v-model="bioDraft"
          type="textarea"
          :rows="4"
          :disabled="bioLoading"
          :maxlength="BIO_MAX + 50"
          placeholder="简单几句话告诉桃宝你是谁、喜欢什么..."
          resize="vertical"
        />
        <p class="panel__hint">
          {{ bioDraft.length }} / {{ BIO_MAX }} 字符
          <span v-if="bioOverLimit()" class="panel__error">（已超出）</span>
        </p>
        <div class="panel__actions">
          <ElButton
            type="primary"
            :loading="bioSaving"
            :disabled="!bioChanged() || bioOverLimit() || bioLoading"
            @click="onSaveBio"
          >
            保存个性资料
          </ElButton>
        </div>
      </div>
    </div>
  </section>
</template>
```

- [ ] **Step 2: 暂不 typecheck**

---

## Task 8: UserAboutPanel（搬 SettingsAbout）

**Files:**
- Create: `src/panels/user/UserAboutPanel.vue`

- [ ] **Step 1: 写 UserAboutPanel.vue（内容来自 SettingsAboutPanel + 套 panel--form）**

Create file `src/panels/user/UserAboutPanel.vue`:

```vue
<script setup lang="ts">
// UserAboutPanel — 关于（搬自 SettingsAboutPanel，套 panel--form 规范）。
//
// 与原版差异：
// - 套 panel--form 修饰类 + 包 .panel__content
// - 内容不变（应用名 + 版本 + 仓库 + 数据策略）

import { onMounted, ref } from 'vue'
import { getVersion } from '@tauri-apps/api/app'

const APP_NAME = 'AI 桌宠'
const REPO_URL = 'https://github.com/tl0502/APET'
const DATA_POLICY_HINT = 'assets/legal/data_policy_v1.md（将在 #16 灵魂宣誓页随首次入库）'

const version = ref<string>('—')
const versionError = ref<string | null>(null)

onMounted(async () => {
  try {
    version.value = await getVersion()
  } catch (e) {
    versionError.value = e instanceof Error ? e.message : String(e)
  }
})
</script>

<template>
  <section class="panel panel--form">
    <h2 class="panel__title">关于</h2>
    <div class="panel__content">
      <dl class="about-grid">
        <dt>应用</dt>
        <dd>{{ APP_NAME }}</dd>

        <dt>版本</dt>
        <dd>
          <code v-if="!versionError">{{ version }}</code>
          <span v-else class="panel__error">{{ versionError }}</span>
        </dd>

        <dt>仓库</dt>
        <dd>
          <a :href="REPO_URL" target="_blank" rel="noopener">{{ REPO_URL }}</a>
        </dd>

        <dt>数据策略</dt>
        <dd class="panel__hint">{{ DATA_POLICY_HINT }}</dd>
      </dl>
    </div>
  </section>
</template>

<style scoped>
.about-grid {
  display: grid;
  grid-template-columns: 96px 1fr;
  gap: var(--aipet-space-2) var(--aipet-space-4);
  margin: 0;
}
.about-grid dt {
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
}
.about-grid dd {
  margin: 0;
  color: var(--aipet-color-text-1);
  font-size: var(--aipet-font-size-base);
}
.about-grid a {
  color: var(--aipet-color-primary);
  text-decoration: none;
}
.about-grid a:hover {
  text-decoration: underline;
}
code {
  padding: 0 var(--aipet-space-1);
  border-radius: var(--aipet-radius-sm);
  background: var(--aipet-color-surface-raised);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-2);
}
</style>
```

---

## Task 9: UserHelpPanel（新静态 panel）

**Files:**
- Create: `src/panels/user/UserHelpPanel.vue`

- [ ] **Step 1: 写 UserHelpPanel.vue**

Create file `src/panels/user/UserHelpPanel.vue`:

```vue
<script setup lang="ts">
// UserHelpPanel — 帮助（链接 + 快捷键速查）。
//
// 静态内容：
// - GitHub 仓库链接
// - 项目文档链接（README + STATUS）
// - 全局快捷键速查表

const REPO_URL = 'https://github.com/tl0502/APET'
const DOCS_URL = 'https://github.com/tl0502/APET/blob/main/docs/README.md'

interface ShortcutDef {
  keys: string
  desc: string
}

const SHORTCUTS: ShortcutDef[] = [
  { keys: 'Ctrl + Alt + W', desc: '打开 / 切换工作区' },
  { keys: 'Esc', desc: '关闭工作区 / 关闭用户 popup' },
  { keys: 'Enter', desc: '对话发送（chat 输入框内）' },
  { keys: 'Shift + Enter', desc: '对话换行（chat 输入框内）' },
]
</script>

<template>
  <section class="panel panel--form">
    <h2 class="panel__title">帮助</h2>
    <div class="panel__content">
      <div class="panel__section">
        <h3 class="panel__subtitle">链接</h3>
        <ul class="help-links">
          <li>
            <a :href="REPO_URL" target="_blank" rel="noopener">GitHub 仓库</a>
            <span class="panel__hint">提 issue、查 release</span>
          </li>
          <li>
            <a :href="DOCS_URL" target="_blank" rel="noopener">项目文档</a>
            <span class="panel__hint">架构、决策、roadmap</span>
          </li>
        </ul>
      </div>

      <div class="panel__section">
        <h3 class="panel__subtitle">快捷键</h3>
        <dl class="shortcut-grid">
          <template v-for="s in SHORTCUTS" :key="s.keys">
            <dt><kbd>{{ s.keys }}</kbd></dt>
            <dd>{{ s.desc }}</dd>
          </template>
        </dl>
      </div>
    </div>
  </section>
</template>

<style scoped>
.help-links {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}
.help-links li {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.help-links a {
  color: var(--aipet-color-primary);
  text-decoration: none;
  font-size: var(--aipet-font-size-base);
}
.help-links a:hover {
  text-decoration: underline;
}

.shortcut-grid {
  display: grid;
  grid-template-columns: 140px 1fr;
  gap: var(--aipet-space-2) var(--aipet-space-4);
  margin: 0;
}
.shortcut-grid dt {
  margin: 0;
}
.shortcut-grid dd {
  margin: 0;
  color: var(--aipet-color-text-2);
  font-size: var(--aipet-font-size-base);
}
kbd {
  padding: 2px var(--aipet-space-2);
  border-radius: var(--aipet-radius-sm);
  background: var(--aipet-color-surface);
  border: 1px solid var(--aipet-color-border);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-1);
}
</style>
```

---

## Task 10: UserPlaceholderPanel（disabled 通用占位）

**Files:**
- Create: `src/panels/user/UserPlaceholderPanel.vue`

- [ ] **Step 1: 写 UserPlaceholderPanel.vue**

Create file `src/panels/user/UserPlaceholderPanel.vue`:

```vue
<script setup lang="ts">
// UserPlaceholderPanel — 3 个 disabled nav 项共用的占位 panel（账户 / 数据隐私 / 通知）。
//
// 通过 kind prop 区分文案。store 守卫已阻止 setNav 切到这些，理论永不会被渲染；
// 但模板上 v-show 保留作为防御性兜底，避免未来误改 store 时静默坏掉。

import { computed } from 'vue'

type PlaceholderKind = 'account' | 'privacy' | 'notifications'

interface PlaceholderCopy {
  title: string
  hint: string
  status: string
}

const COPY: Record<PlaceholderKind, PlaceholderCopy> = {
  account: {
    title: '账户',
    hint: '账号信息 / 登录方式管理 / 密码 / 安全中心 / 邮箱手机绑定 / 二步验证 / 设备管理。',
    status: '账户系统将随登录系统一同上线（M3+）。',
  },
  privacy: {
    title: '数据与隐私',
    hint: '数据导出 / 清除 / 同步、隐私权限、模型访问范围。',
    status: 'M3+ 开发中。',
  },
  notifications: {
    title: '通知',
    hint: '全局通知开关、桌面通知样式、声音偏好。',
    status: 'M3+ 开发中。',
  },
}

const props = defineProps<{ kind: PlaceholderKind }>()

const copy = computed(() => COPY[props.kind])
</script>

<template>
  <section class="panel panel--form">
    <h2 class="panel__title">{{ copy.title }}</h2>
    <div class="panel__content">
      <div class="placeholder">
        <p class="panel__hint">{{ copy.hint }}</p>
        <p class="placeholder__status">{{ copy.status }}</p>
      </div>
    </div>
  </section>
</template>

<style scoped>
.placeholder {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
  padding: var(--aipet-space-6);
  background: var(--aipet-color-surface);
  border: 1px dashed var(--aipet-color-border);
  border-radius: var(--aipet-radius-card);
  align-items: center;
  text-align: center;
}
.placeholder__status {
  margin: 0;
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
}
</style>
```

---

## Task 11: typecheck 全套 popup 组件 + commit

**Files:**
- 上 6 个组件（Task 5-10）

- [ ] **Step 1: typecheck**

Run:
```bash
pnpm typecheck
```

Expected: 0 错误（所有 popup 组件 + 4 panel 互引完整）。

- [ ] **Step 2: lint**

Run:
```bash
pnpm lint
```

Expected: 0 warning（max-warnings=0）。

- [ ] **Step 3: 跑全测**

Run:
```bash
pnpm test
```

Expected: 全过（userPopup 5 case + workspaceLayout 现有 9 case）。

- [ ] **Step 4: Commit popup 全套**

Run:
```bash
git add src/components/popup/ src/panels/user/
git commit -m "feat: #<PHASE1_ISSUE> 用户 popup 全套（shell + sidebar + 4 panel + 占位）"
```

---

## Task 12: workspaceLayout store IA 简化 + 测试同步

**Files:**
- Modify: `src/stores/workspaceLayout.ts`
- Modify: `src/stores/__tests__/workspaceLayout.test.ts`

- [ ] **Step 1: 改 BRAND_BAR_ITEMS.config.masterItems，删 SettingsNickname + SettingsAbout**

Use Edit on `src/stores/workspaceLayout.ts` at the `BRAND_BAR_ITEMS` definition:

Replace:
```typescript
  {
    id: 'config',
    title: '设置',
    icon: Brush,
    masterItems: [
      { id: 'SettingsTheme', title: '外观', icon: Brush },
      { id: 'SettingsProvider', title: 'LLM Provider', icon: Connection },
      { id: 'SettingsNickname', title: '昵称', icon: EditPen },
      { id: 'SettingsAbout', title: '关于', icon: InfoFilled },
    ],
    defaultItemId: 'SettingsTheme',
  },
```

With:
```typescript
  {
    id: 'config',
    title: '设置',
    icon: Brush,
    masterItems: [
      { id: 'SettingsTheme', title: '外观', icon: Brush },
      { id: 'SettingsProvider', title: 'LLM Provider', icon: Connection },
    ],
    defaultItemId: 'SettingsTheme',
  },
```

- [ ] **Step 2: 删除未用 import（EditPen / InfoFilled）**

Use Edit on the imports block at the top to remove `EditPen, InfoFilled,` from the icon import line.

- [ ] **Step 3: 更新 workspaceLayout.test.ts case 2 / case 4 / case 6**

Open `src/stores/__tests__/workspaceLayout.test.ts`，把任何引用 `SettingsAbout` / `SettingsNickname` 的断言改成 `SettingsProvider`（仍在 config 类的合法 id）：

Run a grep first to find references:
```bash
grep -n "SettingsAbout\|SettingsNickname" src/stores/__tests__/workspaceLayout.test.ts
```

预期看到 case 2 第 49 / 53 行、case 4 第 67 行、case 6（可能在 80-100 行附近）。

Edit each:
- case 2 内 `store.setItem('SettingsAbout')` → `store.setItem('SettingsProvider')`，对应断言里 `SettingsAbout` → `SettingsProvider`
- case 4 同上
- case 6（item-per-category 损坏自愈测试）若用了 SettingsAbout / SettingsNickname → 改成 SettingsProvider 或保留作"未知 id"测试场景（实际上 SettingsNickname / SettingsAbout 现在变成 unknown id，可以利用这点测 fallback）

如果 case 6 测的是"老用户 KV 残留 unknown id → 回 default fallback"，可以**保留** `SettingsAbout` 作为输入（它现在就是 unknown id），断言改为 `currentItem === 'SettingsTheme'`（回到 config 的 default）。这正好测了 spec §8 风险表里的"老用户 KV 残留旧 panel id"场景。

- [ ] **Step 4: 跑 store 测试**

Run:
```bash
pnpm test src/stores/__tests__/workspaceLayout.test.ts
```

Expected: 9 case 全过。

- [ ] **Step 5: typecheck**

Run:
```bash
pnpm typecheck
```

Expected: 0 错误。

- [ ] **Step 6: Commit**

Run:
```bash
git add src/stores/workspaceLayout.ts src/stores/__tests__/workspaceLayout.test.ts
git commit -m "feat: #<PHASE1_ISSUE> workspaceLayout 简化 config masterItems（删昵称/关于）+ test 同步"
```

---

## Task 13: DetailColumn 删两 panel v-show + 删旧 panel SFC

**Files:**
- Modify: `src/views/workspace/DetailColumn.vue`
- Delete: `src/panels/settings/SettingsNicknamePanel.vue`
- Delete: `src/panels/settings/SettingsAboutPanel.vue`

- [ ] **Step 1: 删 DetailColumn.vue 的 SettingsNicknamePanel / SettingsAboutPanel import + v-show 块**

Edit `src/views/workspace/DetailColumn.vue`:

Remove from imports:
```typescript
import SettingsNicknamePanel from '@/panels/settings/SettingsNicknamePanel.vue'
import SettingsAboutPanel from '@/panels/settings/SettingsAboutPanel.vue'
```

Remove from template (the two `<SettingsNicknamePanel>` and `<SettingsAboutPanel>` v-show blocks).

- [ ] **Step 2: 删除两 panel SFC 文件**

Run:
```bash
rm src/panels/settings/SettingsNicknamePanel.vue
rm src/panels/settings/SettingsAboutPanel.vue
```

- [ ] **Step 3: typecheck**

Run:
```bash
pnpm typecheck
```

Expected: 0 错误（NicknameForm 仍在 `src/components/settings/`，被 UserProfilePanel 复用，未误删）。

- [ ] **Step 4: 全量 grep 找残留引用**

Run:
```bash
grep -rn "SettingsNicknamePanel\|SettingsAboutPanel" src/ docs/
```

Expected: 0 命中（仅 docs/superpowers/specs/2026-05-21-workspace-redesign/ 里规划性引用可保留）。

- [ ] **Step 5: Commit**

Run:
```bash
git add -A src/panels/settings/ src/views/workspace/DetailColumn.vue
git commit -m "refactor: #<PHASE1_ISSUE> 删 SettingsNickname/SettingsAbout panel SFC（搬到 popup）"
```

---

## Task 14: WorkspaceApp.vue 改 CSS Grid + 新增 topbar

**Files:**
- Modify: `src/views/workspace/WorkspaceApp.vue`（整个 `<template>` + `<style>` 重写）

- [ ] **Step 1: 完整重写 WorkspaceApp.vue**

Replace the entire file content with:

```vue
<script setup lang="ts">
// WorkspaceApp (2026-05-21 重设计 P3)：workspace 三栏 + 顶栏 L 型框 chrome shell。
//
// Grid：
//   grid-template-rows: 48px 1fr
//   grid-template-columns: 60px 240px 1fr
//   grid-areas:
//     "topbar  topbar  topbar"
//     "sidebar master  detail"
//
// 色阶（spec §3.2 / §6.1）：
// - topbar + sidebar + master = surface-soft（L 型 chrome 框）
// - detail = bg（白色主舞台）
//
// chrome 按钮：从右上角 absolute 改为 grid cell（topbar 末端）。
//
// in-workspace popup：UserPopup 挂在 root 末端，z-index: var(--aipet-z-dialog)。

import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

import BrandBar from './BrandBar.vue'
import MasterColumn from './MasterColumn.vue'
import DetailColumn from './DetailColumn.vue'
import SashHandle from './SashHandle.vue'
import UserPopup from '@/components/popup/UserPopup.vue'

import { useWorkspaceLayoutStore } from '@/stores/workspaceLayout'
import { useUserPopupStore } from '@/stores/userPopup'
import { useAvatarsStore } from '@/stores/avatars'
import { hideWorkspace } from '@/services/window'

const layout = useWorkspaceLayoutStore()
const popup = useUserPopupStore()
const avatars = useAvatarsStore()

const ready = ref(false)
const unlistenFns: UnlistenFn[] = []
const win = getCurrentWindow()

const avatarFailed = ref(false)

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
  try {
    await hideWorkspace()
  } catch (e) {
    console.warn('[WorkspaceApp] hideWorkspace failed:', e)
  }
}

function onSashChange(width: number) {
  layout.setMasterWidth(width)
}

function onTopbarAvatarClick() {
  // 顶栏左上 avatar = 桃宝身份入口（spec §3.3）
  layout.setCategoryAndItem('creation', 'SettingsPersona')
}

function onGlobalKeydown(e: KeyboardEvent) {
  if (e.key !== 'Escape') return
  if (popup.isOpen) return // popup 自己接管 ESC
  if (document.querySelector('.el-message-box, .el-dialog__wrapper, .el-overlay')) return
  const active = document.activeElement
  if (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement) return
  void onClose()
}

onMounted(async () => {
  await layout.loadFromKv()
  await avatars.load()
  await avatars.ensureListener()
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
    <!-- TOPBAR：左 桃宝 avatar / 中 capsule 占位 / 右 chrome 三按钮 -->
    <header class="workspace-topbar">
      <div class="workspace-topbar__avatar-wrap">
        <button
          class="workspace-topbar__avatar"
          :class="{
            'workspace-topbar__avatar--active':
              layout.currentCategory === 'creation' &&
              layout.currentItem === 'SettingsPersona',
          }"
          type="button"
          aria-label="桃宝（点击进入人格）"
          title="桃宝（点击进入人格）"
          @click="onTopbarAvatarClick"
        >
          <img
            v-if="avatars.personaAvatarUrl && !avatarFailed"
            :src="avatars.personaAvatarUrl"
            alt=""
            class="workspace-topbar__avatar-img"
            @error="avatarFailed = true"
          />
          <img v-else src="/avatar/momo-avatar.svg" alt="" class="workspace-topbar__avatar-img" />
        </button>
      </div>

      <div class="workspace-topbar__drag-left" data-tauri-drag-region />

      <div class="workspace-topbar__capsule" aria-hidden="true">
        <!-- Phase 1 留空（spec §3.3） -->
      </div>

      <div class="workspace-topbar__drag-right" data-tauri-drag-region />

      <div class="workspace-topbar__chrome">
        <button
          class="aipet-chrome-btn"
          type="button"
          title="最小化"
          aria-label="最小化"
          @click="onMinimize"
        >─</button>
        <button
          class="aipet-chrome-btn"
          type="button"
          title="最大化"
          aria-label="最大化"
          @click="onMaximize"
        >□</button>
        <button
          class="aipet-chrome-btn aipet-chrome-btn--close"
          type="button"
          title="关闭（进托盘）"
          aria-label="关闭"
          @click="onClose"
        >✕</button>
      </div>
    </header>

    <!-- 三列：sidebar / master / detail -->
    <template v-if="ready">
      <BrandBar class="workspace-root__sidebar" />
      <MasterColumn class="workspace-root__master" />
      <SashHandle
        class="workspace-root__sash"
        :width="layout.masterWidth"
        :min="layout._MASTER_WIDTH_MIN"
        :max="layout._MASTER_WIDTH_MAX"
        @update:width="onSashChange"
      />
      <DetailColumn class="workspace-root__detail" />
    </template>

    <!-- 用户 popup（in-workspace overlay；isOpen 控制） -->
    <UserPopup />
  </div>
</template>

<style scoped>
.workspace-root {
  width: 100%;
  height: 100%;
  display: grid;
  grid-template-rows: 48px 1fr;
  grid-template-columns: 60px 240px auto 1fr;
  grid-template-areas:
    'topbar  topbar  topbar  topbar'
    'sidebar master  sash    detail';
  background: var(--aipet-color-bg);
  overflow: hidden;
}

/* topbar：grid 顶行整跨 */
.workspace-topbar {
  grid-area: topbar;
  background: var(--aipet-color-surface-soft);
  border-bottom: 1px solid var(--aipet-color-border-faint);
  display: grid;
  grid-template-columns: auto auto 1fr auto auto;
  align-items: center;
  user-select: none;
  z-index: 5;
}

.workspace-topbar__avatar-wrap {
  padding: 0 0 0 12px;
  display: flex;
  align-items: center;
  /* avatar 自身要在 drag-region 之上 */
  position: relative;
  z-index: 6;
}

.workspace-topbar__avatar {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  overflow: hidden;
  background: var(--aipet-color-bg);
  border: 1px solid var(--aipet-color-border);
  padding: 0;
  cursor: pointer;
  transition:
    transform 600ms var(--aipet-ease-emphasized),
    border-color 120ms ease,
    box-shadow 120ms ease;
}

.workspace-topbar__avatar:hover {
  transform: rotate(4deg) scale(1.04);
  border-color: var(--aipet-color-primary);
}

.workspace-topbar__avatar--active {
  border-color: var(--aipet-color-primary);
  animation: topbar-avatar-pulse 2s ease-in-out infinite;
}

.workspace-topbar__avatar:focus-visible {
  outline: none;
  box-shadow: var(--aipet-ring-focus);
}

.workspace-topbar__avatar-img {
  width: 100%;
  height: 100%;
  display: block;
}

.workspace-topbar__drag-left,
.workspace-topbar__drag-right {
  height: 48px;
  background: transparent;
  /* 让出顶端，让 element 自己接管点击；drag-region 兜底拖动 */
}

.workspace-topbar__capsule {
  height: 28px;
  width: 320px;
  max-width: min(320px, calc(100% - 24px));
  justify-self: center;
  border: 1px solid var(--aipet-color-border);
  border-radius: 16px;
  background: var(--aipet-color-bg);
  /* Phase 1 留空，无内容；与 drag-region 视觉对比表明这里是"非拖动"的占位元素 */
  position: relative;
  z-index: 6;
}

.workspace-topbar__chrome {
  display: flex;
  height: 48px;
  position: relative;
  z-index: 6;
}

/* 三列区 */
.workspace-root__sidebar {
  grid-area: sidebar;
}
.workspace-root__master {
  grid-area: master;
}
.workspace-root__sash {
  grid-area: sash;
}
.workspace-root__detail {
  grid-area: detail;
}

/* avatar pulse 光环（搬自原 BrandBar） */
@keyframes topbar-avatar-pulse {
  0%,
  100% {
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--aipet-color-primary) 25%, transparent);
  }
  50% {
    box-shadow: 0 0 0 5px color-mix(in srgb, var(--aipet-color-primary) 50%, transparent);
  }
}
</style>
```

- [ ] **Step 2: 同步调整 .aipet-chrome-btn height 适配 48px topbar**

`buttons.css` 当前 `.aipet-chrome-btn { height: 32px; }`。topbar 是 48px。chrome 三按钮在 grid cell 内 align-items: center 即可（按钮本体 32px，topbar 48px，上下各 8px 空白）。**不需要改 buttons.css**。

但本次重设计后 `.aipet-chrome-btn` 不再 absolute，而是在 flex 容器内排开。原先 `height: 32px` 是设计意图，保留；align 由父容器 align-items: center 控制。

跳过本步，不动 buttons.css。

- [ ] **Step 3: typecheck**

Run:
```bash
pnpm typecheck
```

Expected: 0 错误。

- [ ] **Step 4: 暂不 commit（与 Task 15 一起 commit）**

---

## Task 15: BrandBar.vue 去头像 + 底部用户头像

**Files:**
- Modify: `src/views/workspace/BrandBar.vue`（整重写）

- [ ] **Step 1: 完整重写 BrandBar.vue**

Replace the entire file content with:

```vue
<script setup lang="ts">
// BrandBar (2026-05-21 重设计 P3)：左 60px 列。
//
// 与上一版差异：
// - 删 brand-bar__top（桃宝头像搬到 topbar）
// - 删 brand-bar__divider（无 avatar 不需要分隔）
// - 删顶部 32px 让位 padding（topbar 接管 drag）
// - 底部 help 按钮替换为用户头像 32×32（点击呼出 userPopup）

import { onMounted } from 'vue'
import { ElIcon, ElTooltip } from 'element-plus'

import { useWorkspaceLayoutStore, type CategoryId } from '@/stores/workspaceLayout'
import { useUserPopupStore } from '@/stores/userPopup'
import { useNicknameStore } from '@/stores/nickname'
import { useAvatarsStore } from '@/stores/avatars'

const layout = useWorkspaceLayoutStore()
const popup = useUserPopupStore()
const nickname = useNicknameStore()
const avatars = useAvatarsStore()

function handleCategoryClick(id: CategoryId) {
  layout.setCategory(id)
}

function handleUserClick() {
  popup.open()
}

onMounted(async () => {
  await Promise.all([nickname.load(), avatars.load()])
  await Promise.all([nickname.ensureListener(), avatars.ensureListener()])
})
</script>

<template>
  <nav class="brand-bar" aria-label="工作台导航">
    <!-- 4 类别 -->
    <ul class="brand-bar__list">
      <li
        v-for="cat in layout.brandBarItems"
        :key="cat.id"
        class="brand-bar__item"
      >
        <ElTooltip :content="String(cat.title)" placement="right" :show-after="500">
          <button
            class="brand-bar__btn"
            :class="{ 'brand-bar__btn--active': layout.currentCategory === cat.id }"
            :aria-pressed="layout.currentCategory === cat.id"
            :aria-label="String(cat.title)"
            type="button"
            @click="handleCategoryClick(cat.id)"
          >
            <ElIcon :size="20">
              <component :is="cat.icon" />
            </ElIcon>
          </button>
        </ElTooltip>
      </li>
    </ul>

    <div class="brand-bar__spacer" />

    <!-- 底部：用户头像（替代原 help 按钮） -->
    <div class="brand-bar__user">
      <ElTooltip
        :content="`用户：${nickname.user ?? '未设置昵称'}`"
        placement="right"
        :show-after="500"
      >
        <button
          class="brand-bar__user-btn"
          type="button"
          aria-label="打开用户设置"
          @click="handleUserClick"
        >
          <img
            v-if="avatars.userAvatarUrl"
            :src="avatars.userAvatarUrl"
            alt=""
            class="brand-bar__user-img"
          />
          <span v-else class="brand-bar__user-fallback">
            {{ nickname.user?.[0] ?? '你' }}
          </span>
        </button>
      </ElTooltip>
    </div>
  </nav>
</template>

<style scoped>
.brand-bar {
  width: 60px;
  height: 100%;
  background: var(--aipet-color-surface-soft);
  border-right: 1px solid var(--aipet-color-border-faint);
  display: flex;
  flex-direction: column;
  align-items: stretch;
  padding: var(--aipet-space-2) 0 var(--aipet-space-3);
  position: relative;
  z-index: 2;
}

.brand-bar__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.brand-bar__spacer {
  flex: 1 1 auto;
}

.brand-bar__item {
  display: flex;
  justify-content: center;
}

.brand-bar__btn {
  position: relative;
  z-index: 3;
  width: 44px;
  height: 44px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--aipet-color-text-3);
  cursor: pointer;
  border-radius: 8px;
  transition: color 120ms ease, background-color 120ms ease;
  padding: 0;
}

.brand-bar__btn:hover {
  color: var(--aipet-color-text-1);
  background: color-mix(in srgb, var(--aipet-color-text-1) 6%, transparent);
}

.brand-bar__btn:focus-visible {
  outline: none;
  box-shadow: var(--aipet-ring-focus);
}

.brand-bar__btn:active {
  background: color-mix(in srgb, var(--aipet-color-text-1) 12%, transparent);
}

.brand-bar__btn--active {
  color: var(--aipet-color-primary);
  background: color-mix(in srgb, var(--aipet-color-primary) 8%, transparent);
}

.brand-bar__btn--active::before {
  content: '';
  position: absolute;
  left: -8px;
  top: 10px;
  bottom: 10px;
  width: 2px;
  border-radius: 2px;
  background: var(--aipet-color-primary);
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

/* 底部用户头像 */
.brand-bar__user {
  display: flex;
  justify-content: center;
  padding-top: var(--aipet-space-2);
}

.brand-bar__user-btn {
  width: 32px;
  height: 32px;
  padding: 0;
  border: 2px solid var(--aipet-color-primary);
  border-radius: 50%;
  background: var(--aipet-color-bg);
  cursor: pointer;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 120ms ease, box-shadow 120ms ease;
}

.brand-bar__user-btn:hover {
  transform: scale(1.06);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--aipet-color-primary) 20%, transparent);
}

.brand-bar__user-btn:focus-visible {
  outline: none;
  box-shadow: var(--aipet-ring-focus);
}

.brand-bar__user-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.brand-bar__user-fallback {
  color: var(--aipet-color-text-2);
  font-size: 12px;
  font-weight: 600;
}
</style>
```

- [ ] **Step 2: typecheck**

Run:
```bash
pnpm typecheck
```

Expected: 0 错误。

- [ ] **Step 3: build sanity**

Run:
```bash
pnpm build
```

Expected: 编译成功。

- [ ] **Step 4: Commit chrome 重做**

Run:
```bash
git add src/views/workspace/WorkspaceApp.vue src/views/workspace/BrandBar.vue
git commit -m "feat: #<PHASE1_ISSUE> chrome L 型框：48px 顶栏 + 桃宝 avatar 搬迁 + sidebar 底部用户头像"
```

---

## Task 16: SettingsTheme 示范套容器公约

**Files:**
- Modify: `src/panels/settings/SettingsThemePanel.vue`

- [ ] **Step 1: 改 SettingsThemePanel.vue 套 panel--form**

Use Edit on `src/panels/settings/SettingsThemePanel.vue`:

Replace:
```vue
<template>
  <section class="panel">
    <h2 class="panel__title">外观</h2>
    <p class="panel__hint">
      切换会同步到桌宠窗口（与未来的 onboarding / hub 窗口）；选择持久化到本地。
    </p>

    <div class="panel__section">
```

With:
```vue
<template>
  <section class="panel panel--form">
    <h2 class="panel__title">外观</h2>
    <div class="panel__content">
      <p class="panel__hint">
        切换会同步到桌宠窗口（与未来的 onboarding / hub 窗口）；选择持久化到本地。
      </p>

      <div class="panel__section">
```

Then replace the final closing `</section>` with `</div></section>` so the `.panel__content` div is closed before `.panel` ends. Be sure the `<div v-if="isDev" class="panel__dev">` block is inside `.panel__content`.

具体定位：找到 `</section>` 文件末标签，把它替换为：
```vue
    </div>
  </section>
```

并把 `<h2 class="panel__title">外观</h2>` 后所有子元素整体包进新加的 `<div class="panel__content">…</div>`。

- [ ] **Step 2: typecheck**

Run:
```bash
pnpm typecheck
```

Expected: 0 错误。

- [ ] **Step 3: Commit**

Run:
```bash
git add src/panels/settings/SettingsThemePanel.vue
git commit -m "feat: #<PHASE1_ISSUE> SettingsTheme 套 panel--form 容器公约（Phase 1 示范）"
```

---

## Task 17: 三绿全检 + 手动 e2e

**Files:**
- 无文件改动

- [ ] **Step 1: 三绿命令链**

Run:
```bash
pnpm typecheck && pnpm lint && pnpm test && pnpm build
```

Expected: 全部通过，0 错误 / 0 warning。

- [ ] **Step 2: cargo check（Tauri 后端）**

Run:
```bash
cd src-tauri && cargo check && cd ..
```

Expected: 0 错误（本次未改 Rust，但应保持基线）。

- [ ] **Step 3: 启动 dev 跑一遍手动 e2e**

Run:
```bash
pnpm tauri:dev
```

启动后按下列清单逐项点：

| # | 操作 | 期望 |
|---|---|---|
| 1 | Ctrl+Alt+W 打开 workspace | 三栏 + 48px topbar 出现，L 型 chrome 框可见（surface-soft 色），detail 是白 |
| 2 | 切换暗色 | 三栏 + topbar 全部跟着暗，色阶对得上（#1c1c1c / #171717） |
| 3 | 点 topbar 左上桃宝 avatar | 切到 创作 / SettingsPersona，avatar 出现 pulse |
| 4 | 拖 master/detail 分隔 sash | 可拖、3 状态视觉、KV 保存 |
| 5 | 点击 sidebar 4 个类别 icon | 切类别正常，左 2px primary 竖条 + 浅底 |
| 6 | 点 sidebar 底部用户头像 | popup 弹出（scale + fade），自动聚焦第一个可聚焦元素 |
| 7 | popup 内点击各 nav 项 | profile / help / about 三项可切；account / privacy / notifications 灰显不响应 |
| 8 | popup 内 user identity card 点击 | 切到 profile nav |
| 9 | popup 搜索框输入"帮" | filter 出"帮助"项，其他分组消失 |
| 10 | popup ESC 关闭 | 关闭 + 焦点还原到 sidebar 用户头像 |
| 11 | popup backdrop 点击关闭 | 关闭 |
| 12 | popup × 按钮点击关闭 | 关闭 |
| 13 | UserProfile 改昵称保存 | 走 NicknameForm，保存成功 toast |
| 14 | UserProfile 改个性资料保存 | toast "个性资料已保存"，重开 popup 字段仍在 |
| 15 | UserProfile 个性资料超 200 字 | 字符计数变红 + 保存按钮 disabled |
| 16 | 设置类 master 列表 | 只剩 外观 + LLM Provider 两项 |
| 17 | 进入 外观 panel | 内容 max-width 720 居中（detail 宽窗下可见居中效果），title 通栏 |
| 18 | 关 workspace（× / Esc） | hide 进托盘 |
| 19 | 重开 workspace | 状态恢复 |

如有任意项不符，回到对应 task 修复。

- [ ] **Step 4: 视觉对照 mockup**

打开浏览器：
- `file:///D:/Project/temp/4/docs/superpowers/specs/2026-05-21-workspace-redesign/scratch/01-skeleton.html`
- `file:///D:/Project/temp/4/docs/superpowers/specs/2026-05-21-workspace-redesign/scratch/02-popup.html`

对照真窗：
- 48px topbar 比例 OK
- L 型 chrome 框色块对得上
- popup 880×580 + 240 sidebar 比例 OK
- nav active 左 2px primary 竖条
- disabled 项灰显 + badge

---

## Task 18: 关闭 #36 + 同步 STATUS.md

**Files:**
- Modify: `docs/STATUS.md`
- GitHub: 关 #36 + 加 closing comment

- [ ] **Step 1: 关 #36 with closing comment**

Run:
```bash
gh issue close 36 --comment "$(cat <<'EOF'
本 issue 的 chrome 三按钮自绘 / brand-bar 占整列 / panel.css 全局抽取等成果都还在；但顶栏整体表达从「32px 不可见 drag-bar + 飘三按钮」改为实色 L 型框 + 48px 可见顶栏，在 #<PHASE1_ISSUE>(workspace 重设计 P3) 中覆盖。

关键差异（与 #36 落地状态对比）：
- 顶栏：不可见 32px drag-bar → 实色 48px topbar（surface-soft 填充）
- chrome 三按钮：absolute 右上角 → topbar grid 末端 cell
- BrandBar 头像：sidebar 列顶 → topbar 左上（36×36 + pulse）
- BrandBar 帮助按钮：替换为用户头像（32×32 + 2px primary border + 点击呼 popup）

panel.css / buttons.css 全局抽取（#36 phase 1-3）继续生效，被本次重设计扩展（容器公约 .panel--form/--chat 追加）。

后续 Phase 2 issue 单独追踪其余 6 panel 套容器公约。
EOF
)"
```

- [ ] **Step 2: 同步 STATUS.md**

Use Edit on `docs/STATUS.md`：
- "当前 session 在做" 行 → 改为 "#<PHASE1_ISSUE> workspace 重设计 P3 chrome L 型框 + Profile popup 落地"
- "下一步" 行 → "Phase 2（6 panel 套容器公约） / #29 Todo / #21 KV 实例化 / #23 物理交互"
- M2 W3 区块加新条目记录 #<PHASE1_ISSUE> 落地

- [ ] **Step 3: Commit STATUS + sync**

Run:
```bash
git add docs/STATUS.md
git commit -m "docs: #<PHASE1_ISSUE> STATUS 同步（chrome L 型框 + Profile popup 落地，#36 收口）"
```

---

## Task 19: ADR-021 Updated + 新增 sidebar nav 规范 ADR

**Files:**
- Modify: `docs/decisions.md`

- [ ] **Step 1: 找 ADR-021 现有位置**

Run:
```bash
grep -n "ADR-021" docs/decisions.md | head -5
```

预期看到 ADR-021 标题及之前的 Updated 段。

- [ ] **Step 2: ADR-021 追加 Updated 段（P3）**

Use Edit on `docs/decisions.md`，在 ADR-021 现有 Updated 段后追加：

```markdown
### Updated 2026-05-21（P3 落地 #<PHASE1_ISSUE>）

workspace 顶栏从「不可见 drag-bar + 飘三按钮」改为实色 48px L 型框 chrome：topbar + sidebar + master 共享 `--aipet-color-surface-soft`，detail 保持 `--aipet-color-bg` 主舞台。同步引入 in-workspace 用户 popup（880×580 overlay），把 SettingsNickname / SettingsAbout 搬入 popup 内（UserProfile / UserAbout），workspace `设置` 类别简化为 外观 + LLM Provider 两项。新增 panel 容器公约 `.panel--form` (max-width 720) / `.panel--chat` (880) / `.panel--list` (fluid)，由 SettingsThemePanel 在 P3 套用作示范。spec：[docs/superpowers/specs/2026-05-21-workspace-redesign/design.md](superpowers/specs/2026-05-21-workspace-redesign/design.md)。
```

- [ ] **Step 3: 新增 sidebar nav 规范 ADR（下一个未用编号）**

Run to find next ADR number:
```bash
grep -E "^## ADR-[0-9]+" docs/decisions.md | tail -3
```

假设最大是 ADR-021，则新 ADR 为 ADR-022。Use Edit on `docs/decisions.md` to append at end of file:

```markdown

## ADR-022: Sidebar Nav 通用规范（扁平 vs Accordion + 三态）

立两种 sidebar nav 结构 + 三种状态机，跨 workspace sidebar / popup sidebar / 未来其他 nav 共用：

- **扁平**：一级 = 一页，hover/active 双态，active = 浅 primary 底 + 左 2px primary 竖条 + 文字 primary 色 + 字重 500。
- **Accordion**：一级 = 展开/收起 trigger，▶ 旋转 90° → 180ms；二级用纵向 1px 层级线连接；**二级 active 时层级线整体变 primary**（与 active 项同步高亮）。
- **三态**：Active / Normal hover / Disabled（text-3 + opacity 0.5 + cursor not-allowed + 可选 badge）。

落地范围：P3 实做扁平类型（popup nav 6 项）；Accordion 仅写规范，等账户系统起来或其他需要时实做。规范全文：[docs/superpowers/specs/2026-05-21-workspace-redesign/design.md §5](superpowers/specs/2026-05-21-workspace-redesign/design.md)。
```

- [ ] **Step 4: Commit**

Run:
```bash
git add docs/decisions.md
git commit -m "docs: #<PHASE1_ISSUE> ADR-021 Updated P3 + 新增 ADR-022 sidebar nav 规范"
```

---

## Self-Review

**1. Spec coverage**（spec §7 Phase 1 改动清单逐项对照）：

| Spec §7 改动 | 对应 Task |
|---|---|
| Chrome 重做 - WorkspaceApp grid + topbar | Task 14 |
| Chrome 重做 - chrome 三按钮入 grid | Task 14 |
| BrandBar 去头像 + 32px 让位删 | Task 15 |
| BrandBar 底部 help → 用户头像 | Task 15 |
| Topbar drag region 用空 div | Task 14 |
| panel.css 追加 `.panel__content` / `--form` / `--chat` | Task 2 |
| tokens.css 文件头加色区映射注释 | Task 2 |
| UserPopup.vue shell | Task 5 |
| UserPopup ESC/backdrop/× 关 + focus trap | Task 5 |
| UserProfilePanel | Task 7 |
| UserHelpPanel | Task 9 |
| UserAboutPanel（搬 SettingsAbout） | Task 8 |
| UserAccount/Privacy/Notifications 占位 | Task 10 |
| userPopup store | Task 3 |
| Trigger：sidebar 用户头像点击 | Task 15 |
| workspaceLayout.config.masterItems 删两项 | Task 12 |
| 删 SettingsNicknamePanel.vue / SettingsAboutPanel.vue | Task 13 |
| DetailColumn 删 v-show | Task 13 |
| SettingsThemePanel 套 panel--form | Task 16 |
| 三绿验证 | Task 17 |
| 关 #36 + STATUS 同步 | Task 18 |
| ADR-021 Updated + 新 ADR | Task 19 |

✅ 全覆盖。

**2. Placeholder scan**: 没有 TBD / TODO / "appropriate error handling" / "similar to Task N" / 缺代码。`<PHASE1_ISSUE>` 是变量占位（Task 1 输出后填回 commit message），是显式的、有 Task 1 定义。

**3. Type consistency**:
- `useUserPopupStore` 的 `PopupNavId` 在 store（Task 3）+ PopupSidebar（Task 6）+ UserPopup（Task 5）一致使用
- `getUserBio` / `setUserBio` 签名在 service（Task 4）+ UserProfilePanel（Task 7）一致
- `useNicknameStore` / `useAvatarsStore` 用法在 PopupSidebar / WorkspaceApp / BrandBar 一致（`.load()` + `.ensureListener()`）

✅ 一致。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-21-workspace-redesign-phase-1.md`. Two execution options:

**1. Subagent-Driven (recommended for Phase 2 之后批量 panel)** - 每个 task 派独立 subagent，task 之间 review checkpoint，加速 + 主上下文不被污染

**2. Inline Execution (recommended for Phase 1)** - 在当前 session 顺序跑 task，每个 task 完成后用户可中断 / 提问。Phase 1 task 之间有强依赖（chrome / popup / store / panel 相互引用），inline 检查更顺手

Phase 1 推荐 **Inline Execution**（task 间依赖较强）。Phase 2 之后的 6 panel 重排适合 subagent 并行。

请选择执行方式。
