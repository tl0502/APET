<script setup lang="ts">
// PetCommandOverlayApp：pet-command overlay 根组件。
//
// 2026-05-25 结构重构：
// - 删除 fakePetSize / x / y / petSize 传参（anchor 算法已从 PetCommandTray 删除）
// - 删除 :deep(.command-tray) !important override（双重定位已清除）
// - PetCommandTray 用 position: absolute; inset: 4px 在 overlay 窗内自然填满
// - 新增 onFocusChanged listener：overlay 获焦后再失焦（点桌面/其他窗口）→ closeAll()
//   配合 App.vue 的 pet 窗 blur 监听，覆盖所有 click-outside 场景（结构性修复 P7）
//
// 2026-05-26 Bug 1 修：onFocusChanged 同时向 pet 窗 emit tray-focused / tray-blurred
// 让 pet 窗的 blur-close 兜底能识别"用户正在 tray 内操作"避免误关二级展开。

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import AppShell from '@/components/layouts/AppShell.vue'
import PetCommandTray from '@/components/PetCommandTray.vue'

const open = ref(false)
const currentView = ref<'root' | 'settings'>('root')

let unlistenOpen: UnlistenFn | null = null
let unlistenClose: UnlistenFn | null = null
let unlistenFocus: (() => void) | null = null

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

/** Esc 监听：二级先返一级，一级关闭。 */
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

  // P7: overlay 失焦（用户在 overlay 内点击按钮后、又点了别处）→ 关闭。
  // 2026-05-26 Bug 1 修：额外向 pet 窗广播 tray-focused / tray-blurred 信号，
  // 让 pet 窗的 blur-close 兜底能识别"用户正在 tray 内操作"避免误关。
  // 配合 App.vue 的 trayIsFocused 门控 → 二级展开不被误关 + 桌面点击仍关闭。
  const appWin = getCurrentWindow()
  try {
    unlistenFocus = await appWin.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        void emit('pet:contextmenu:tray-focused')
      } else {
        void emit('pet:contextmenu:tray-blurred')
        if (open.value) closeAll()
      }
    })
  } catch (e) {
    console.warn('[pet-command-overlay] onFocusChanged listen failed:', e)
  }

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
  unlistenFocus?.()
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
      :view="currentView"
      @update:view="(v) => (currentView = v)"
      @close="closeAll"
      @pointerdown.stop
    />
  </AppShell>
</template>

<style scoped>
/* backdrop：透明全屏点击层。overlay 关闭时 v-if=false 整层消失，pet 窗 / 桌面操作不受影响。 */
.pet-command-overlay__backdrop {
  position: fixed;
  inset: 0;
  background: transparent;
  pointer-events: auto;
  z-index: 0;
}
/* 2026-05-25 结构重构：移除 :deep(.command-tray) !important override。
   PetCommandTray 现在用 position: absolute; inset: 4px，无双重定位。 */
</style>
