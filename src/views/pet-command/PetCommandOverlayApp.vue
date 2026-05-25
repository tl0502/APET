<script setup lang="ts">
// PetCommandOverlayApp：pet-command overlay 根组件。
//
// 第三轮 UX 修正（2026-05-24）：
// - 加 backdrop click-outside（透明全屏 div 在 tray 下层，命中 backdrop emit close）
// - Esc 监听放到 OverlayApp（owner of currentView）：二级先返一级，一级关闭
// - PetCommandTray 受控 view（v-model:view）
// - listen 全局 `pet:contextmenu:request-close`（pet 窗 outside click / 再次右键 / Esc 触发）
//   关闭自身 + emit `pet:contextmenu:closed-ack` 让 pet 窗 commandTrayOpen ref 复位
// - `request-open` listener 内 emit `pet:contextmenu:opened-ack` 让 pet 窗 ref 设 true
// - 所有 close 路径（backdrop / Esc / tray @close emit / global request-close）→ 单点 closeAll()

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'
import AppShell from '@/components/layouts/AppShell.vue'
import PetCommandTray from '@/components/PetCommandTray.vue'

const open = ref(false)
const currentView = ref<'root' | 'settings'>('root')
const fakePetSize = { width: 160, height: 0 }

let unlistenOpen: UnlistenFn | null = null
let unlistenClose: UnlistenFn | null = null

/** 单一关闭收敛点：所有路径都走这里。idempotent —— 已关时不重复 emit ack。 */
function closeAll() {
  if (!open.value) return
  open.value = false
  currentView.value = 'root'
  void emit('pet:contextmenu:closed-ack')
}

/** backdrop pointerdown：命中 backdrop 自身（@click.self）= tray 外部点击 */
function onBackdropDown(e: PointerEvent) {
  if (e.button !== 0) return
  closeAll()
}

/** Esc 监听：二级先返一级，一级关闭。pointerdown 不在此处理（backdrop 已接）。 */
function onKeyDown(e: KeyboardEvent) {
  if (e.key !== 'Escape') return
  if (!open.value) return
  if (currentView.value === 'settings') {
    currentView.value = 'root'
    return
  }
  closeAll()
}

onMounted(async () => {
  document.addEventListener('keydown', onKeyDown, true)
  try {
    unlistenOpen = await listen('pet:contextmenu:request-open', () => {
      open.value = true
      currentView.value = 'root'
      void emit('pet:contextmenu:opened-ack')
    })
  } catch (e) {
    console.warn('[pet-command-overlay] listen request-open failed:', e)
  }
  try {
    unlistenClose = await listen('pet:contextmenu:request-close', () => {
      closeAll()
    })
  } catch (e) {
    console.warn('[pet-command-overlay] listen request-close failed:', e)
  }
})

onBeforeUnmount(() => {
  document.removeEventListener('keydown', onKeyDown, true)
  unlistenOpen?.()
  unlistenClose?.()
})
</script>

<template>
  <AppShell variant="transparent">
    <!-- backdrop：tray 外部点击区域。pointer-events: auto 让 overlay 整窗可点击；
         tray 自身阻止冒泡（@pointerdown.stop）让点 tray 不触发关闭。 -->
    <div v-if="open" class="pet-command-overlay__backdrop" @pointerdown="onBackdropDown" />
    <PetCommandTray
      v-if="open"
      :x="0"
      :y="0"
      :pet-size="fakePetSize"
      :view="currentView"
      @update:view="(v) => (currentView = v)"
      @close="closeAll"
      @pointerdown.stop
    />
  </AppShell>
</template>

<style scoped>
/* backdrop：透明全屏点击层，hit-test 透传到下方 desktop 仅靠 pointer-events: auto 在 overlay
   范围内 = overlay 自己消化。overlay 关闭时 v-if=false 整层消失，pet 窗 / 桌面操作不受影响。 */
.pet-command-overlay__backdrop {
  position: fixed;
  inset: 0;
  background: transparent;
  pointer-events: auto;
  z-index: 0;
}

/* 强制 tray 填满 overlay 窗（覆盖 PetCommandTray 内 fixed 浮层 top/left 算法的偏差）。
   z-index 1 在 backdrop 之上让点 tray 落到 tray 自身 + @pointerdown.stop 拦截不冒泡到 backdrop。 */
:deep(.command-tray) {
  top: 4px !important;
  left: 4px !important;
  width: calc(100% - 8px) !important;
  z-index: 1;
}
</style>
