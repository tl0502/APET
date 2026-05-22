<script setup lang="ts">
// UserPopup（#37 2026-05-21 重设计）：in-workspace 用户 popup overlay。
//
// 职责：
// - 渲染 backdrop + 容器（880×580）
// - ESC / 点 backdrop / 点 × 关闭
// - focus trap（首个 focusable = popup 内第一个可聚焦元素）
//
// 不在本组件做：
// - 内部 sidebar（PopupSidebar.vue）
// - 6 个 panel（UserProfile / UserHelp / UserAbout / UserPlaceholder）

import { computed, onMounted, onBeforeUnmount, ref, watch, nextTick } from 'vue'

import PopupSidebar from './PopupSidebar.vue'
import UserProfilePanel from '@/panels/user/UserProfilePanel.vue'
import UserHelpPanel from '@/panels/user/UserHelpPanel.vue'
import UserAboutPanel from '@/panels/user/UserAboutPanel.vue'
import UserPlaceholderPanel from '@/panels/user/UserPlaceholderPanel.vue'

import { useUserPopupStore } from '@/stores/userPopup'

const popup = useUserPopupStore()

const containerRef = ref<HTMLElement | null>(null)
const previousFocus = ref<HTMLElement | null>(null)

const panelTitle = computed(() => {
  switch (popup.activeNav) {
    case 'profile':
      return '个人资料'
    case 'account':
      return '账户'
    case 'privacy':
      return '数据与隐私'
    case 'notifications':
      return '通知'
    case 'help':
      return '帮助'
    case 'about':
      return '关于'
    default:
      return ''
  }
})

function onBackdropClick(e: MouseEvent) {
  // 仅 backdrop 本体点击关闭（避免子元素点击穿透）
  if (e.target === e.currentTarget) {
    popup.close()
  }
}

function onKeydown(e: KeyboardEvent) {
  if (!popup.isOpen) return
  if (e.key === 'Escape') {
    e.preventDefault()
    e.stopPropagation()
    popup.close()
  }
}

// focus trap：popup 打开时聚焦首个可聚焦元素，关闭时还原焦点
watch(
  () => popup.isOpen,
  async (open) => {
    if (open) {
      previousFocus.value = document.activeElement as HTMLElement | null
      await nextTick()
      const first = containerRef.value?.querySelector<HTMLElement>(
        'button, input, textarea, [tabindex]:not([tabindex="-1"])',
      )
      first?.focus()
    } else {
      previousFocus.value?.focus()
      previousFocus.value = null
    }
  },
)

onMounted(() => {
  window.addEventListener('keydown', onKeydown, true)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown, true)
})
</script>

<template>
  <Transition name="popup-fade">
    <div
      v-if="popup.isOpen"
      class="popup-backdrop"
      role="dialog"
      aria-modal="true"
      :aria-label="`用户设置 - ${panelTitle}`"
      @click="onBackdropClick"
    >
      <div ref="containerRef" class="popup-container">
        <PopupSidebar />

        <main class="popup-main">
          <header class="popup-main__header">
            <span class="popup-main__title">{{ panelTitle }}</span>
            <button
              class="popup-main__close"
              type="button"
              aria-label="关闭"
              @click="popup.close()"
            >
              ✕
            </button>
          </header>

          <div class="popup-main__content">
            <UserProfilePanel v-show="popup.activeNav === 'profile'" />
            <UserHelpPanel v-show="popup.activeNav === 'help'" />
            <UserAboutPanel v-show="popup.activeNav === 'about'" />
            <!-- 3 个 disabled nav 选中时不渲染（store 守卫不允许 setNav 到这些；保留模板 v-show 防御性） -->
            <UserPlaceholderPanel v-show="popup.activeNav === 'account'" kind="account" />
            <UserPlaceholderPanel v-show="popup.activeNav === 'privacy'" kind="privacy" />
            <UserPlaceholderPanel
              v-show="popup.activeNav === 'notifications'"
              kind="notifications"
            />
          </div>
        </main>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.popup-backdrop {
  position: fixed;
  inset: 0;
  background: var(--aipet-color-overlay);
  z-index: var(--aipet-z-dialog);
  display: flex;
  align-items: center;
  justify-content: center;
}

.popup-container {
  width: 880px;
  height: 580px;
  max-width: calc(100vw - 48px);
  max-height: calc(100vh - 48px);
  background: var(--aipet-color-bg);
  border-radius: 12px;
  box-shadow: var(--aipet-shadow-float);
  display: grid;
  grid-template-columns: 240px 1fr;
  overflow: hidden;
}

.popup-main {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.popup-main__header {
  flex: 0 0 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px 0 24px;
  border-bottom: 1px solid var(--aipet-color-border-faint);
  user-select: none;
}

.popup-main__title {
  font-size: 15px;
  font-weight: 600;
  color: var(--aipet-color-text-1);
}

.popup-main__close {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 6px;
  color: var(--aipet-color-text-2);
  cursor: pointer;
  font-size: 14px;
  transition: background-color 120ms ease, color 120ms ease;
}

.popup-main__close:hover {
  background: color-mix(in srgb, var(--aipet-color-text-1) 6%, transparent);
  color: var(--aipet-color-text-1);
}

.popup-main__close:focus-visible {
  outline: none;
  box-shadow: var(--aipet-ring-focus);
}

.popup-main__content {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: var(--aipet-space-6) var(--aipet-space-8);
  min-height: 0;
}

/* 进入/离开动效 */
.popup-fade-enter-active {
  transition: opacity 220ms ease-out;
}

.popup-fade-enter-active .popup-container {
  transition: transform 220ms var(--aipet-ease-emphasized);
}

.popup-fade-leave-active {
  transition: opacity 160ms ease-in;
}

.popup-fade-enter-from {
  opacity: 0;
}

.popup-fade-enter-from .popup-container {
  transform: scale(0.96);
}

.popup-fade-leave-to {
  opacity: 0;
}
</style>
