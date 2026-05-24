<script setup lang="ts">
// PetReminderOverlayApp：pet-reminder overlay 根组件。
//
// 职责：
// - 包 <PetReminderBubble>（组件实现复用 src/components/）
// - 监听 PetReminderBubble 暴露的 bubbleCount，emit 全局 Tauri 事件 pet-reminder:active /
//   pet-reminder:idle 让 Rust 端 services/pet_overlay.rs 控 overlay show/hide
// - 不再透传 trayOpen（tray 在另一个 overlay；跨窗协作交 Rust 集中调度）
//
// 透明窗：pet-reminder.html inline style 已让 html/body 透明；PetReminderBubble 自带卡片样式。

import { ref, watch } from 'vue'
import { emit } from '@tauri-apps/api/event'
import AppShell from '@/components/layouts/AppShell.vue'
import PetReminderBubble from '@/components/PetReminderBubble.vue'

const bubbleRef = ref<InstanceType<typeof PetReminderBubble> | null>(null)

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
</script>

<template>
  <AppShell variant="transparent">
    <PetReminderBubble ref="bubbleRef" />
  </AppShell>
</template>
