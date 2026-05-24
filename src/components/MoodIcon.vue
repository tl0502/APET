<script setup lang="ts">
// MoodIcon（#41）— 桌宠 mood 状态浮层，1s polling mood_get。
//
// ## 设计
// - 显示位置：pet 窗左上角（与原 PetCanvas stub 同位）；CSS absolute
// - 6 mood emoji：neutral 不渲染，happy/focused/sleepy/cozy/annoyed 各 1 个
// - hover 显示中文 mood label（aria-label + title）
// - polling 由 store startPolling/stopPolling 引用计数管理；多组件挂载安全
// - disabledFeatures 含 'mood_icon' 时整组件 v-if 不渲染（用户偏好持久跨重启）
//
// ## 与 #40 stub 的差异
// - 原 PetCanvas 内 .pet-mood-icon 直接读 usePetInteractionFeedback.mood（只 happy/annoyed/calm）
// - 现在改读 mood store（Rust 真状态机，含 focused/sleepy/cozy 三种 base mood）
// - usePetInteractionFeedback 不再产 mood ref（保 bubble + shake）

import { onBeforeUnmount, onMounted } from 'vue'
import { useMoodStore } from '@/stores/mood'
import { MOOD_EMOJI, MOOD_LABEL } from '@/services/mood'

const store = useMoodStore()

onMounted(() => {
  store.startPolling()
  void store.loadDisabledFeatures()
})

onBeforeUnmount(() => {
  store.stopPolling()
})
</script>

<template>
  <div
    v-if="store.isMoodIconEnabled && store.mood !== 'neutral'"
    class="mood-icon"
    :aria-label="MOOD_LABEL[store.mood]"
    :title="MOOD_LABEL[store.mood]"
  >
    {{ MOOD_EMOJI[store.mood] }}
  </div>
</template>

<style scoped>
.mood-icon {
  position: absolute;
  top: 4px;
  left: 6px;
  font-size: 18px;
  line-height: 1;
  pointer-events: none;
  user-select: none;
  animation: mood-icon-fade-in 120ms var(--aipet-ease-standard);
  z-index: 4;
}

@keyframes mood-icon-fade-in {
  from {
    opacity: 0;
    transform: translateY(-2px) scale(0.85);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}
</style>
