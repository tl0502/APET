<script setup lang="ts">
// PetOnboardingBubble：桌宠头顶 onboarding 引导气泡（#35 ADR-021 P1 Phase E）
//
// 用途：
// - 用户完成 onboarding 后（commands::onboarding_complete 末尾 emit `onboarding:workspace-intro`）
//   弹一次提示气泡，告诉用户 "Ctrl+Alt+W / 托盘双击 / 托盘菜单 三入口可以打开工作台"
// - KV `onboarding:workspace_intro_seen` 防重 — 同账号只显示一次
// - 用户首次成功打开 workspace 时（监听 window:visibility-changed visible:true label:workspace）
//   自动 dismiss + 写 KV（与"6s 自动 dismiss"竞争）
//
// 实现：拷 PetReminderBubble.vue CSS + TransitionGroup 模式 + 简化（单条不堆叠）。
// 显示位置：pet 窗 320×320 内 absolute top:6px 居中浮于 VRM 头顶。

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { getConfig, setConfig } from '@/services/config'
import { showWorkspace } from '@/services/window'

const KV_SEEN = 'onboarding:workspace_intro_seen'
const AUTO_DISMISS_MS = 6000

const visible = ref(false)
const hover = ref(false)
let dismissTimer: number | null = null
const unlistenFns: UnlistenFn[] = []

function startAutoDismiss() {
  if (dismissTimer !== null) window.clearTimeout(dismissTimer)
  dismissTimer = window.setTimeout(() => {
    if (!hover.value) dismiss()
  }, AUTO_DISMISS_MS) as unknown as number
}

async function dismiss() {
  visible.value = false
  if (dismissTimer !== null) {
    window.clearTimeout(dismissTimer)
    dismissTimer = null
  }
  try {
    await setConfig(KV_SEEN, '1')
  } catch (e) {
    console.warn('[onboarding-bubble] persist seen failed (non-fatal):', e)
  }
}

async function openWorkspace() {
  try {
    await showWorkspace()
  } catch (e) {
    console.error('[onboarding-bubble] showWorkspace failed:', e)
  }
  await dismiss()
}

function onMouseEnter() {
  hover.value = true
}

function onMouseLeave() {
  hover.value = false
  startAutoDismiss()
}

async function maybeShow() {
  // KV 已写 = 看过了，不再弹
  try {
    const seen = await getConfig(KV_SEEN)
    if (seen) return
  } catch (e) {
    console.warn('[onboarding-bubble] read seen failed, conservative skip:', e)
    return
  }
  visible.value = true
  startAutoDismiss()
}

onMounted(async () => {
  // listen 来自 commands::onboarding_complete 的引导事件
  try {
    const un = await listen('onboarding:workspace-intro', () => {
      void maybeShow()
    })
    unlistenFns.push(un)
  } catch (e) {
    console.warn('[onboarding-bubble] listen workspace-intro failed:', e)
  }

  // listen workspace 首次被打开 → 用户自己发现入口了，立刻 dismiss + 写 KV
  // 避免 6s 内用户已开 workspace 后 bubble 还浮着碍事
  try {
    const un = await listen<{ label: string; visible: boolean }>(
      'window:visibility-changed',
      (event) => {
        if (
          event.payload.label === 'workspace' &&
          event.payload.visible === true &&
          visible.value
        ) {
          void dismiss()
        }
      },
    )
    unlistenFns.push(un)
  } catch (e) {
    console.warn('[onboarding-bubble] listen visibility-changed failed:', e)
  }
})

onBeforeUnmount(() => {
  unlistenFns.forEach((u) => u())
  if (dismissTimer !== null) window.clearTimeout(dismissTimer)
})
</script>

<template>
  <Transition name="bubble">
    <div
      v-if="visible"
      class="onboarding-bubble"
      @mouseenter="onMouseEnter"
      @mouseleave="onMouseLeave"
    >
      <div class="onboarding-bubble__body">
        <span class="onboarding-bubble__icon">✨</span>
        <div class="onboarding-bubble__text">
          <span class="onboarding-bubble__title">工作台已就绪</span>
          <span class="onboarding-bubble__sub">Ctrl+Alt+W 打开，或托盘双击</span>
        </div>
      </div>
      <div class="onboarding-bubble__actions" data-no-drag>
        <button
          type="button"
          class="onboarding-bubble__btn onboarding-bubble__btn--primary"
          @click="openWorkspace"
        >
          打开看看
        </button>
        <button
          type="button"
          class="onboarding-bubble__btn onboarding-bubble__btn--ghost"
          aria-label="关闭"
          @click="dismiss"
        >
          ✕
        </button>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.onboarding-bubble {
  position: fixed;
  top: 6px;
  left: 50%;
  transform: translateX(-50%);
  pointer-events: auto;
  min-width: 220px;
  max-width: 296px;
  padding: 8px 10px;
  background: var(--aipet-color-surface-raised, var(--aipet-color-surface));
  border: 1px solid color-mix(in srgb, var(--aipet-color-primary) 35%, var(--aipet-color-border));
  border-radius: 14px;
  box-shadow:
    0 8px 24px -8px color-mix(in srgb, var(--aipet-color-primary) 25%, transparent),
    0 2px 6px -2px rgba(0, 0, 0, 0.08);
  backdrop-filter: blur(8px);
  display: flex;
  flex-direction: column;
  gap: 6px;
  z-index: 5;
}

.onboarding-bubble__body {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.onboarding-bubble__icon {
  flex: 0 0 auto;
  font-size: 18px;
  line-height: 1.2;
}

.onboarding-bubble__text {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.onboarding-bubble__title {
  font-size: 13px;
  font-weight: 600;
  color: var(--aipet-color-text-1);
  line-height: 1.35;
  word-break: break-word;
}

.onboarding-bubble__sub {
  font-size: 11px;
  color: var(--aipet-color-text-3);
}

.onboarding-bubble__actions {
  display: flex;
  align-items: center;
  gap: 4px;
  justify-content: flex-end;
  flex-wrap: wrap;
}

.onboarding-bubble__btn {
  appearance: none;
  -webkit-appearance: none;
  border: 1px solid var(--aipet-color-border);
  background: var(--aipet-color-bg);
  color: var(--aipet-color-text-2);
  font: inherit;
  font-size: 11px;
  font-weight: 500;
  padding: 3px 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background-color var(--aipet-duration-fast) var(--aipet-ease-standard),
    color var(--aipet-duration-fast) var(--aipet-ease-standard),
    border-color var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.onboarding-bubble__btn:hover {
  border-color: var(--aipet-color-border-strong, var(--aipet-color-border));
  color: var(--aipet-color-text-1);
  background: var(--aipet-color-surface);
}

.onboarding-bubble__btn--primary {
  background: var(--aipet-color-primary);
  border-color: var(--aipet-color-primary);
  color: #fff;
}

.onboarding-bubble__btn--primary:hover {
  background: color-mix(in srgb, var(--aipet-color-primary) 88%, #000);
  border-color: color-mix(in srgb, var(--aipet-color-primary) 88%, #000);
  color: #fff;
}

.onboarding-bubble__btn--ghost {
  border-color: transparent;
  background: transparent;
  padding: 3px 6px;
  font-size: 12px;
  line-height: 1;
}

.onboarding-bubble__btn--ghost:hover {
  background: var(--aipet-color-surface);
  border-color: var(--aipet-color-border);
}

.bubble-enter-active,
.bubble-leave-active {
  transition: opacity 220ms var(--aipet-ease-standard),
    transform 220ms var(--aipet-ease-standard);
}

.bubble-enter-from,
.bubble-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(-8px);
}
</style>
