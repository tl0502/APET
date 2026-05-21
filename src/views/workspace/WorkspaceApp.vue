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
