<script setup lang="ts">
// AppShell：M2-M5 多面板共用根布局（issue #8）。
// - variant='standalone': settings/hub/onboarding 等独立窗口；header(可拖动) + body + 可选 footer
// - variant='transparent': pet 角色窗 / 漫画气泡；纯语义包装，无 chrome 无 padding
//
// 主题：不调 useThemeStore().init() —— 已在 main.ts 全局调过，<html class="dark"> 全局生效。
//
// 拖动：standalone 的 header 用 Tauri 2 推荐的 [data-tauri-drag-region] 而非 -webkit-app-region: drag
// （后者仅 macOS 原生支持，Windows WebView2 不识别）。按钮/输入区如需取消拖动，加 data-tauri-drag-region="false"。
import { computed, useSlots } from 'vue'

interface Props {
  variant?: 'standalone' | 'transparent'
  title?: string
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'standalone',
  title: '',
})

const slots = useSlots()

const rootClass = computed(() => ['aipet-shell', `aipet-shell--${props.variant}`])
const isStandalone = computed(() => props.variant === 'standalone')
const hasFooter = computed(() => isStandalone.value && Boolean(slots.footer))
</script>

<template>
  <div :class="rootClass">
    <header v-if="isStandalone" class="aipet-shell__header" data-tauri-drag-region>
      <slot name="header">
        <span class="aipet-shell__title">{{ title }}</span>
      </slot>
    </header>
    <main class="aipet-shell__body">
      <slot />
    </main>
    <footer v-if="hasFooter" class="aipet-shell__footer">
      <slot name="footer" />
    </footer>
  </div>
</template>
