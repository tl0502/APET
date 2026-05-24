<script setup lang="ts">
// PetCommandOverlayApp：pet-command overlay 根组件（2026-05-24 pet UI 重构第二轮）。
//
// 职责：
// - listen 全局 Tauri 事件 `pet:contextmenu:request-open`（pet 主窗 emit）→ v-if 显示 tray
// - tray @close → emit `pet:contextmenu:request-close` 让 Rust services/pet_overlay.rs hide 自身
// - PetCommandTray 内部 anchor 算法基于 petSize 计算 fixed 浮层 top/left；overlay 是独占
//   160×220 容器，tray 应填满 —— 用 fakePetSize 占位 + `:deep(.command-tray)` 强制覆盖偏移。
//   下一轮重构 PetCommandTray 接受 layoutMode prop 时可取消此 hack（见 plan follow-up）。

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'
import AppShell from '@/components/layouts/AppShell.vue'
import PetCommandTray from '@/components/PetCommandTray.vue'

const open = ref(false)
const fakePetSize = { width: 160, height: 0 }

let unlistenOpen: UnlistenFn | null = null

async function onClose() {
  open.value = false
  void emit('pet:contextmenu:request-close')
}

onMounted(async () => {
  try {
    unlistenOpen = await listen('pet:contextmenu:request-open', () => {
      open.value = true
    })
  } catch (e) {
    console.warn('[pet-command-overlay] listen failed:', e)
  }
})

onBeforeUnmount(() => {
  unlistenOpen?.()
})
</script>

<template>
  <AppShell variant="transparent">
    <PetCommandTray
      v-if="open"
      :x="0"
      :y="0"
      :pet-size="fakePetSize"
      @close="onClose"
    />
  </AppShell>
</template>

<style scoped>
/* 强制 tray 填满 overlay 窗（覆盖 PetCommandTray 内 fixed 浮层 top/left 算法的偏差） */
:deep(.command-tray) {
  top: 4px !important;
  left: 4px !important;
  width: calc(100% - 8px) !important;
}
</style>
