<script setup lang="ts">
// PetReminderOverlayApp：pet-reminder overlay 根组件。
//
// 职责：
// - 包 <PetReminderBubble>（组件实现复用 src/components/）
// - 监听 PetReminderBubble 暴露的 bubbleCount，emit 全局 Tauri 事件 pet-reminder:active /
//   pet-reminder:idle 让 Rust 端 services/pet_overlay.rs 控 overlay show/hide
// - 监听 window:visibility-changed 跟踪 pet-command overlay 开关状态 → trayOpen prop
//   （spec 2026-05-25-pet-reminder-card-stack §2 #6：tray 开时 reminder stack dim 到 40%）
// - ResizeObserver 跟踪卡片栈实际尺寸 → appWindow.setSize() 让 Rust 用 outer_size() 重算 anchor
//
// 2026-05-26（plan Task 5 重写）：
// - PetReminderBubble 新 stackEl 是普通 div ref（非 TransitionGroup ref）；
//   ResizeObserver 行为不变
// - OVERLAY_W 320 → 280：新单卡 240px + 两侧 padding 缓冲 + badge overhang 安全余量

import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'
import { LogicalSize, getCurrentWindow } from '@tauri-apps/api/window'
import AppShell from '@/components/layouts/AppShell.vue'
import PetReminderBubble from '@/components/PetReminderBubble.vue'

const bubbleRef = ref<InstanceType<typeof PetReminderBubble> | null>(null)
const trayOpen = ref(false)

/** 窗口固定宽度：240 card + ghost overhang 0 + badge overhang 7（右侧）+ 双侧安全 + box-shadow
 *  ≈ 24px 横向扩散。300px 给 outer padding-right 8 + 左右各 ~26px buffer。 */
const OVERLAY_W = 300
/** 最小高度（单卡 + box-shadow 缓冲 + ghost 底部 + badge 顶部余量）。 */
const MIN_H = 96
/** 垂直方向额外 padding（给 box-shadow 24px 向下扩散 + 视觉安全余量）。 */
const STACK_PAD = 24

/** 与 Rust window_actions.rs 同步：'pet-command' 是命令托盘 overlay 窗 label。 */
const PET_COMMAND_LABEL = 'pet-command'
/** 与 Rust window_actions.rs 同步：visibility 广播事件名。 */
const VISIBILITY_EVENT = 'window:visibility-changed'

interface VisibilityPayload {
  label: string
  visible: boolean
}

let resizeObserver: ResizeObserver | null = null
let resizePending = false
let unlistenVisibility: UnlistenFn | null = null

function applyResize(h: number) {
  if (resizePending) return
  resizePending = true
  requestAnimationFrame(async () => {
    resizePending = false
    const newH = Math.max(MIN_H, Math.ceil(h) + STACK_PAD)
    try {
      await getCurrentWindow().setSize(new LogicalSize(OVERLAY_W, newH))
    } catch (e) {
      console.warn('[pet-reminder-overlay] setSize failed:', e)
    }
  })
}

watch(
  () => bubbleRef.value?.bubbleCount ?? 0,
  (n, prev) => {
    if (n > 0 && (prev ?? 0) === 0) {
      void emit('pet-reminder:active')
    } else if (n === 0 && (prev ?? 0) > 0) {
      void emit('pet-reminder:idle')
    }
  },
)

onMounted(async () => {
  // PetReminderBubble 用 defineExpose 暴露 stackEl（普通 div ref）。
  // Vue 3 保证父组件 onMounted 时子组件已 mount，无需 nextTick。
  const el = bubbleRef.value?.stackEl as HTMLElement | null | undefined
  if (el) {
    resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0]
      if (!entry) return
      // borderBoxSize 包含 padding/border；contentRect 仅内容区，
      // 叠卡时 outer .reminder-bubble-stack 的 padding 8/8/18/0 会被 contentRect 丢掉
      // 导致窗口算少 26px、badge / ghost 被 Tauri 窗口边裁掉。改用 borderBoxSize。
      const bbs = entry.borderBoxSize?.[0]
      const h = bbs?.blockSize ?? entry.contentRect.height
      if (h > 0) applyResize(h)
    })
    resizeObserver.observe(el)
  }

  // 监听命令托盘开/关 → trayOpen 状态（spec §2 #6 dim 40%）。
  try {
    unlistenVisibility = await listen<VisibilityPayload>(VISIBILITY_EVENT, (e) => {
      if (e.payload?.label === PET_COMMAND_LABEL) {
        trayOpen.value = e.payload.visible === true
      }
    })
  } catch (e) {
    console.warn('[pet-reminder-overlay] listen visibility failed:', e)
  }
})

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  resizeObserver = null
  unlistenVisibility?.()
})
</script>

<template>
  <AppShell variant="transparent">
    <PetReminderBubble ref="bubbleRef" :tray-open="trayOpen" />
  </AppShell>
</template>

<style scoped>
/* 透明 overlay 窗 — stack 自身 pointer-events: none 让透明区不拦截下层 pet / desktop；
   .reminder-card 自己 pointer-events: auto 接 hover/click（CSS 内部已设）。 */
:deep(.reminder-bubble-stack) {
  pointer-events: none;
}
:deep(.reminder-card) {
  pointer-events: auto;
}
</style>
