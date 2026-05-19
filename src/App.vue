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
import { getCurrentWindow } from '@tauri-apps/api/window'
import AppShell from '@/components/layouts/AppShell.vue'
import PetCanvas from '@/components/PetCanvas.vue'
import PetReminderBubble from '@/components/PetReminderBubble.vue'
import { useToast } from '@/composables/useToast'
import { useSnapWindow } from '@/composables/useSnapWindow'
import { getConfig } from '@/services/config'
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
let unlistenAot: UnlistenFn | null = null

/** T10 (#31 follow-up B)：AOT KV key + 默认值，与 [src-tauri/src/services/window_state.rs] 同源 */
const AOT_KV_KEY = 'window:always_on_top'
const AOT_CHANGED_EVT = 'window:always-on-top:changed'
const AOT_DEFAULT = true

// #30 磁吸窗口系统：pet 是 anchor 之一（也是负责启动期 load persistence 的窗）。
// composable 在 onMounted 注册 listener，在 onBeforeUnmount 清理。
// T2a (#31)：拿 isPreviewAnchor 给 .pet-stage 套 .snap-preview class 显示吸附预览。
// T7 (#31 follow-up B)：拿 previewEdgeFor + previewIntensityFor 渲染沿对接边的矩形 glow。
// Phase A (#31 follow-up C)：isFieldAnchor + fieldIntensityFor 渲染 field halo
//   （chat 进入 pet 影响域 120px → pet 周围渐显主色光晕，反馈"磁场存在"）
const {
  isPreviewAnchor,
  previewEdgeFor: petPreviewEdge,
  previewIntensityFor: petPreviewIntensity,
  isFieldAnchor: petIsFieldAnchor,
  fieldIntensityFor: petFieldIntensity,
} = useSnapWindow('pet')

const snapPreviewClass = computed(() => {
  const cls: Record<string, boolean> = {
    'snap-preview': isPreviewAnchor.value,
    'snap-field-anchor': petIsFieldAnchor.value,
  }
  if (isPreviewAnchor.value && petPreviewEdge.value) {
    cls[`snap-preview--edge-${petPreviewEdge.value}`] = true
  }
  return cls
})
const snapPreviewStyle = computed(() => ({
  '--snap-preview-intensity': String(petPreviewIntensity.value),
  '--snap-field-intensity': String(petFieldIntensity.value),
}))

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

  // T10 (#31 follow-up B)：AOT 前端兜底 — 启动期主动读 KV 应用一次 + listen 后端 emit
  // 同步切换。后端 [window_state.rs apply_initial_always_on_top] setup 阶段已做一次，
  // 此处是双保险（chat 是 lazy webview，setup 期注册时序不绝对可靠）。
  try {
    const raw = await getConfig(AOT_KV_KEY)
    const v = raw === null ? AOT_DEFAULT : raw === 'true'
    await getCurrentWindow().setAlwaysOnTop(v)
  } catch (e) {
    console.warn('[App] initial setAlwaysOnTop failed:', e)
  }
  try {
    unlistenAot = await listen<boolean>(AOT_CHANGED_EVT, async (ev) => {
      try {
        await getCurrentWindow().setAlwaysOnTop(ev.payload)
      } catch (e) {
        console.warn('[App] AOT changed listen apply failed:', e)
      }
    })
  } catch (e) {
    console.warn('[App] listen AOT changed failed:', e)
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
  unlistenAot?.()
})
</script>

<template>
  <AppShell variant="transparent" :class="snapPreviewClass" :style="snapPreviewStyle">
    <PetCanvas :view="view" :size="size" />
    <PetReminderBubble />
  </AppShell>
</template>

<style scoped>
/* Phase A (#31 follow-up C)：Field halo —— chat 进入 pet 120px 影响域时，
   pet 周围出现渐进 radial-gradient 光晕。距离越近 intensity 越高（fieldIntensityFromDistance 60-120 线性）。
   .snap-field-anchor 类总开关；--snap-field-intensity ∈ [0,1] 控制 opacity；非拖动期为 0 → 完全不可见。
   单独一层 ::before（与 .snap-preview ::after 互不干扰）。 */
.snap-field-anchor :deep(.pet-stage) {
  position: relative;
}
.snap-field-anchor :deep(.pet-stage)::before {
  content: '';
  position: absolute;
  inset: -40px;
  border-radius: 50%;
  pointer-events: none;
  background: radial-gradient(
    circle at center,
    color-mix(
      in srgb,
      var(--aipet-color-primary) calc(var(--snap-field-intensity, 0) * 18%),
      transparent
    )
      0%,
    color-mix(
      in srgb,
      var(--aipet-color-primary) calc(var(--snap-field-intensity, 0) * 6%),
      transparent
    )
      55%,
    transparent 75%
  );
  opacity: var(--snap-field-intensity, 0);
  transition: opacity 80ms linear;
  z-index: -1;
}

/* T2a + T7 (#31 follow-up B)：磁吸预览（已进入 ATTACH_ZONE 60px 触发 candidate）。
   pet 是矩形窗（PRD 透明背景但 .pet-stage 是矩形宽高），矩形 outline + 沿对接边 box-shadow inset glow。
   - 整圈 outline 提示"我可以被吸"
   - 单边 box-shadow inset 强化"你正吸向我这一边"
   - --snap-preview-intensity ∈ [0.25, 1]：距离越近越亮
   .pet-stage 无圆角 → outline border-radius 也 0；动画移除（intensity 已传达靠近度）。 */
.snap-preview :deep(.pet-stage) {
  position: relative;
}
.snap-preview :deep(.pet-stage)::after {
  content: '';
  position: absolute;
  inset: -4px;
  border-radius: 0;
  pointer-events: none;
  /* outline 用 box-shadow 模拟（透明窗 outline 易被裁切） */
  box-shadow:
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 70%),
        transparent
      ),
    0 0 16px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 30%),
        transparent
      );
  transition: box-shadow 80ms ease-out;
}
/* 沿对接边的内向 glow（距离越近 inset spread 越强） */
.snap-preview--edge-right :deep(.pet-stage)::after {
  box-shadow:
    inset -3px 0 18px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 65%),
        transparent
      ),
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 70%),
        transparent
      ),
    0 0 16px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 30%),
        transparent
      );
}
.snap-preview--edge-left :deep(.pet-stage)::after {
  box-shadow:
    inset 3px 0 18px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 65%),
        transparent
      ),
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 70%),
        transparent
      ),
    0 0 16px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 30%),
        transparent
      );
}
.snap-preview--edge-top :deep(.pet-stage)::after {
  box-shadow:
    inset 0 3px 18px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 65%),
        transparent
      ),
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 70%),
        transparent
      ),
    0 0 16px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 30%),
        transparent
      );
}
.snap-preview--edge-bottom :deep(.pet-stage)::after {
  box-shadow:
    inset 0 -3px 18px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 65%),
        transparent
      ),
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 70%),
        transparent
      ),
    0 0 16px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 30%),
        transparent
      );
}
</style>
