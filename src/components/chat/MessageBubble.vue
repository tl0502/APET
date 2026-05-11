<script setup lang="ts">
// MessageBubble：单条消息气泡（issue #14）。
// - 三种 role 视觉差异：user 靠右紫底白字 / assistant 靠左浅底深字 / system 居中灰条（M1 几乎不出现）
// - streaming=true：在 content 末尾追加 ▌ 闪烁光标（CSS animation）
// - mode='offline_rule'：底部加灰色小标"（离线模板）"（#13 离线降级路径标识）
// - mode='cancelled'：底部加灰色小标"（已取消）"（#13 取消 partial 路径标识；与 offline_rule 风格一致，互斥不并存）
// - 时间戳（mm:ss 显示，hover 看完整 ISO）—— M1 简化：仅 hh:mm
import { computed } from 'vue'
import type { Message } from '@/types/chat'

interface Props {
  message: Message
  streaming?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  streaming: false,
})

const roleClass = computed(() => `msg msg--${props.message.role}`)
const isOffline = computed(() => props.message.mode === 'offline_rule')
const isCancelled = computed(() => props.message.mode === 'cancelled')

// hh:mm 简化展示；hover title 显示原始 ISO（dev 排查用）。
const timeLabel = computed(() => {
  const dt = new Date(props.message.created_at)
  if (Number.isNaN(dt.getTime())) return ''
  const hh = String(dt.getHours()).padStart(2, '0')
  const mm = String(dt.getMinutes()).padStart(2, '0')
  return `${hh}:${mm}`
})
</script>

<template>
  <li :class="roleClass">
    <div class="msg__bubble">
      <span class="msg__text">{{ message.content }}<span v-if="streaming" class="msg__cursor">▌</span></span>
      <div v-if="isOffline" class="msg__status-tag">（离线模板）</div>
      <div v-else-if="isCancelled" class="msg__status-tag">（已取消）</div>
    </div>
    <time class="msg__time" :title="message.created_at">{{ timeLabel }}</time>
  </li>
</template>

<style scoped>
.msg {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-1);
  max-width: 85%;
  list-style: none;
}

.msg--user {
  align-self: flex-end;
  align-items: flex-end;
}

.msg--assistant {
  align-self: flex-start;
  align-items: flex-start;
}

.msg--system {
  align-self: center;
  align-items: center;
  max-width: 95%;
}

.msg__bubble {
  padding: var(--aipet-space-2) var(--aipet-space-3);
  border-radius: var(--aipet-radius-lg);
  font-size: var(--aipet-font-size-base);
  line-height: var(--aipet-line-height-base);
  word-break: break-word;
  white-space: pre-wrap;
}

.msg--user .msg__bubble {
  background: var(--aipet-color-primary);
  color: #fff;
  border-bottom-right-radius: var(--aipet-radius-base);
}

.msg--assistant .msg__bubble {
  background: var(--aipet-color-surface-raised);
  color: var(--aipet-color-text-1);
  border: 1px solid var(--aipet-color-border);
  border-bottom-left-radius: var(--aipet-radius-base);
}

.msg--system .msg__bubble {
  background: transparent;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
  font-style: italic;
}

.msg__text {
  display: inline;
}

.msg__cursor {
  display: inline-block;
  margin-left: 2px;
  color: currentColor;
  animation: aipet-blink 1s steps(2, start) infinite;
}

@keyframes aipet-blink {
  to {
    visibility: hidden;
  }
}

.msg__status-tag {
  margin-top: var(--aipet-space-1);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}

.msg__time {
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
  user-select: none;
}
</style>
