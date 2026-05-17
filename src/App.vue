<script setup lang="ts">
// 角色窗主壳：保持透明（PRD §7.2 角色窗）。VRM 渲染由 PetCanvas 接管；物理交互 / 容器布局后续 task 接入。
// 主题（ADR-017）已在 main.ts 通过 useThemeStore().init() 启动；本组件不渲染任何控件。
// AppShell variant='transparent'：纯语义包装，由 components.css .aipet-shell--transparent 提供 100% / 透明背景。
// #11 全局快捷键：仅 pet 窗口 listen `shortcut:chat` 主路径（settings/chat 不监听避免重复触发）；
// #14 ChatPanel：listener 内 invoke('chat_toggle')（独立 chat 窗口可见性切换）。
// #21 收尾 #2：mount 时查 getChatRegisterStatus 兜底"启动期快捷键注册失败"场景。emit 单走会
// race（setup 内 emit 早于本 listener 挂），用 IPC 查 last_chat_error 留痕兜底。
// #24：pet 主窗是 view_preset 的唯一前端真相源 —— onMounted 拉 KV 初始化 view ref（后端 setup
// 已 setSize，前端 ref 还要拉一次让 size computed 同步真值）；同时 listen `pet:view-changed`
// 接 settings 改 preset 的反向通知。onboarding 窗用同款 PetCanvas 但不参与此体系（默认 half）。
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import AppShell from '@/components/layouts/AppShell.vue'
import PetCanvas from '@/components/PetCanvas.vue'
import PetReminderBubble from '@/components/PetReminderBubble.vue'
import { useToast } from '@/composables/useToast'
import { getChatRegisterStatus } from '@/services/shortcut'
import {
  PET_VIEW_CHANGED_EVENT,
  PET_VIEW_SIZES,
  getPetViewPreset,
  showSettings,
  toggleChat,
} from '@/services/window'
import type { PetViewPreset } from '@/services/window'
import type { ShortcutChatPayload } from '@/types/shortcut'
import type { AvatarView } from '@/services/vrm'

const toast = useToast()
let unlistenChat: UnlistenFn | null = null
let unlistenViewChanged: UnlistenFn | null = null

const view = ref<AvatarView>('half')
const size = computed(() => PET_VIEW_SIZES[view.value])

function asPreset(payload: unknown): PetViewPreset {
  return payload === 'full' ? 'full' : 'half'
}

onMounted(async () => {
  try {
    view.value = await getPetViewPreset()
  } catch (e) {
    console.warn('[App] getPetViewPreset failed, fallback half:', e)
  }

  try {
    unlistenViewChanged = await listen<string>(PET_VIEW_CHANGED_EVENT, (e) => {
      view.value = asPreset(e.payload)
    })
  } catch (e) {
    console.warn('[App] listen pet:view-changed failed:', e)
  }

  unlistenChat = await listen<ShortcutChatPayload>('shortcut:chat', async () => {
    try {
      await toggleChat()
    } catch (e) {
      // chat_toggle 不应失败（IPC 永远 Ok）；保留兜底诊断
      console.error('[App] chat_toggle failed:', e)
      toast.error('对话窗口唤起失败，请检查日志')
    }
  })

  // #21 收尾 #2：检查启动期 chat 快捷键注册是否失败。失败时给一个 10s warn toast
  // + "去设置改键" 行动按钮（用户点 → 打开 settings 面板；未来 #14 设置面板上线时
  // 自动跳到"快捷键"tab，M1 阶段先打开 settings 窗，让用户手动定位即可）。
  try {
    const failed = await getChatRegisterStatus()
    if (failed) {
      toast.warn(
        `快捷键 ${failed.shortcut} 注册失败（可能被其他应用占用）。可在设置里换一组组合。`,
        {
          duration: 10000,
          action: {
            text: '打开设置',
            handler: () => {
              void showSettings()
            },
          },
        },
      )
    }
  } catch (e) {
    console.warn('[App] getChatRegisterStatus failed:', e)
  }
})

onBeforeUnmount(() => {
  unlistenChat?.()
  unlistenViewChanged?.()
})
</script>

<template>
  <AppShell variant="transparent">
    <PetCanvas :view="view" :size="size" />
    <PetReminderBubble />
  </AppShell>
</template>

<style scoped></style>
