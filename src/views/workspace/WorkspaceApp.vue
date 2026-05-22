<script setup lang="ts">
// WorkspaceApp (#37 2026-05-21 重设计 P3)：workspace 三栏 + 顶栏 L 型框 chrome shell。
//
// Grid：
//   grid-template-rows: 48px 1fr
//   grid-template-columns: 60px 240px auto 1fr
//   grid-areas:
//     "topbar  topbar  topbar  topbar"
//     "sidebar master  sash    detail"
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
              layout.currentCategory === 'creation' && layout.currentItem === 'SettingsPersona',
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
}

.workspace-topbar__capsule {
  height: 28px;
  width: 320px;
  max-width: min(320px, calc(100% - 24px));
  justify-self: center;
  border: 1px solid var(--aipet-color-border);
  border-radius: 16px;
  background: var(--aipet-color-bg);
  position: relative;
  z-index: 6;
}

.workspace-topbar__chrome {
  display: flex;
  align-items: center;
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
