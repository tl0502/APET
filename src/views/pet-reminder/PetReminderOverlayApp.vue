<script setup lang="ts">
// PetReminderOverlayApp：pet-reminder overlay 根组件。
//
// 职责：
// - 包 <PetReminderBubble>（组件实现复用 src/components/）
// - 监听 PetReminderBubble 暴露的 bubbleCount，emit 全局 Tauri 事件 pet-reminder:active /
//   pet-reminder:idle 让 Rust 端 services/pet_overlay.rs 控 overlay show/hide
// - P6（2026-05-25 结构重构）：ResizeObserver 跟踪气泡栈实际尺寸 → appWindow.setSize()
//   Rust Resized 事件里改用 overlay.outer_size() 而非固定常量，动态重算 anchor。

import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { emit } from '@tauri-apps/api/event'
import { LogicalSize, getCurrentWindow } from '@tauri-apps/api/window'
import AppShell from '@/components/layouts/AppShell.vue'
import PetReminderBubble from '@/components/PetReminderBubble.vue'

const bubbleRef = ref<InstanceType<typeof PetReminderBubble> | null>(null)

/** 窗口固定宽度（与气泡 max-width 280 + 两侧 padding 对齐）。 */
const OVERLAY_W = 320
/** 最小高度（1 张卡片 + 顶部 padding）。 */
const MIN_H = 80
/** 垂直方向额外 padding（给 box-shadow / badge 留呼吸空间）。 */
const STACK_PAD = 16

let resizeObserver: ResizeObserver | null = null
let resizePending = false

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

onMounted(() => {
  // 用 nextTick 等 PetReminderBubble mount 完成后才能访问 $el
  const el = bubbleRef.value?.$el as HTMLElement | undefined
  if (!el) return
  resizeObserver = new ResizeObserver((entries) => {
    const h = entries[0]?.contentRect.height ?? 0
    if (h > 0) applyResize(h)
  })
  resizeObserver.observe(el)
})

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  resizeObserver = null
})
</script>

<template>
  <AppShell variant="transparent">
    <PetReminderBubble ref="bubbleRef" />
  </AppShell>
</template>

<style scoped>
/* 第三轮（2026-05-24）：透明 overlay 窗 — stack 自身 pointer-events: none 让透明区
   不拦截下层 pet / desktop；.reminder-bubble 自己 pointer-events: auto 接 hover/click。 */
:deep(.reminder-bubble-stack) {
  pointer-events: none;
}
:deep(.reminder-bubble) {
  pointer-events: auto;
}
</style>
