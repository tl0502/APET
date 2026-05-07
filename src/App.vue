<script setup lang="ts">
// 角色窗主壳：保持透明（PRD §7.2 角色窗）。VRM 渲染由 PetCanvas 接管；物理交互 / 容器布局后续 task 接入。
// 主题（ADR-017）已在 main.ts 通过 useThemeStore().init() 启动；本组件不渲染任何控件。
// AppShell variant='transparent'：纯语义包装，由 components.css .aipet-shell--transparent 提供 100% / 透明背景。
// #11 全局快捷键：仅 pet 窗口 listen `shortcut:chat`（settings/onboarding 不监听避免重复 toast）；
// #14 ChatPanel ready 后把 toast 替换为 invoke('chat_show')。
import { onBeforeUnmount, onMounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import AppShell from '@/components/layouts/AppShell.vue'
import PetCanvas from '@/components/PetCanvas.vue'
import { useToast } from '@/composables/useToast'
import type { ShortcutChatPayload } from '@/types/shortcut'

const toast = useToast()
let unlisten: UnlistenFn | null = null

onMounted(async () => {
  unlisten = await listen<ShortcutChatPayload>('shortcut:chat', () => {
    toast.info('Ctrl+Alt+Space 已触发，chat 窗口将在 #14 上线')
  })
})

onBeforeUnmount(() => {
  unlisten?.()
})
</script>

<template>
  <AppShell variant="transparent">
    <PetCanvas />
  </AppShell>
</template>

<style scoped></style>
