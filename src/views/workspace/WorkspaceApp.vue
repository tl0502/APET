<script setup lang="ts">
// WorkspaceApp (#33 phase B-redo)：workspace 独立窗口 root — 三栏 Desktop App Shell。
//
// 三栏永久同屏（不可 split/tab/拖拽）：
//   ┌────┐┌────────┐┌─────────────────────┐
//   │🐱  ││# 对话  ││  消息 / 详情          │
//   │──  ││  conv 1││                      │
//   │ 💬 ││  conv 2││                      │
//   │ 📋 ││  conv 3││                      │
//   │ 🎨 ││  …    ││                      │
//   │ ⚙ ││        ││                      │
//   │──  ││        ││                      │
//   │ ❓ ││        ││                      │
//   └────┘└────────┘└─────────────────────┘
//    60     240 (sash)    flex:1
//
// 生命周期：
// 1) workspaceLayout.loadFromKv → 还原 category / item / masterWidth
// 2) ESC 监听 → 关 workspace = hideWorkspace（进托盘不退）
// 3) emit_visibility_changed listener → hide 时 saveSnapshot（避免 webview 销毁丢失最近 KV）
// 4) onBeforeUnmount 再 save 一次（极少数 quit 路径）

import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import AppShell from '@/components/layouts/AppShell.vue'
import BrandBar from './BrandBar.vue'
import MasterColumn from './MasterColumn.vue'
import DetailColumn from './DetailColumn.vue'
import SashHandle from './SashHandle.vue'

import { useWorkspaceLayoutStore } from '@/stores/workspaceLayout'
import { hideWorkspace } from '@/services/window'

const layout = useWorkspaceLayoutStore()
const ready = ref(false)
const unlistenFns: UnlistenFn[] = []

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
  // ESC 弹窗 / input 聚焦时不响应
  if (document.querySelector('.el-message-box, .el-dialog__wrapper, .el-overlay')) return
  const active = document.activeElement
  if (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement) return
  void onClose()
}

onMounted(async () => {
  await layout.loadFromKv()
  ready.value = true

  window.addEventListener('keydown', onGlobalKeydown)

  // emit_visibility_changed：lib.rs 在 show/hide 时 emit `window:visibility-changed`
  // payload = { label, visible }。hide 之前 store 内的 debounce 已自动落盘最近改动。
  // 本 listener 用于额外触发一次显式 flush（debounce 期间 hide 时兜底）
  try {
    const un = await listen<{ label: string; visible: boolean }>(
      'window:visibility-changed',
      async (event) => {
        if (event.payload.label === 'workspace' && event.payload.visible === false) {
          // workspaceLayout 的 saveXxxToKv 都已在 setter 内自动触发；这里仅 console.debug
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
  <AppShell variant="standalone">
    <template #header>
      <span class="workspace-shell-title" data-tauri-drag-region>工作台</span>
      <span class="aipet-shell__header-spacer" data-tauri-drag-region />
      <button
        class="aipet-shell__close"
        title="关闭（进托盘）"
        aria-label="关闭"
        data-tauri-drag-region="false"
        @click="onClose"
      >✕</button>
    </template>

    <div v-if="ready" class="workspace-body">
      <BrandBar />
      <MasterColumn />
      <SashHandle
        :width="layout.masterWidth"
        :min="layout._MASTER_WIDTH_MIN"
        :max="layout._MASTER_WIDTH_MAX"
        @update:width="onSashChange"
      />
      <DetailColumn />
    </div>
  </AppShell>
</template>

<style scoped>
.workspace-shell-title {
  font-size: var(--aipet-font-size-base);
  font-weight: 500;
  color: var(--aipet-color-text-1);
  padding: 0 var(--aipet-space-3);
}

.workspace-body {
  flex: 1 1 auto;
  width: 100%;
  display: flex;
  min-height: 0;
  background: var(--aipet-color-bg);
}
</style>
