<script setup lang="ts">
// MessageBubble：单条消息气泡（Phase D 精简版）。
//
// Phase D 之后只负责"纯气泡 + 流式光标 + 状态条"。avatar / 时间戳 / 自定义头像三层降级
// 已整段上移到 MessageList 的 group 容器（连续同 role + 5min 内合并为一组，单 avatar + 多 bubble）。
// CSS 尾巴一并删除（桌面 IM 风），role 区分仍靠 group 层 alignment + 气泡颜色。
//
// 保留：
// - streaming=true: 在 content 末尾追加圆点呼吸光标
// - mode='offline_rule': 底部加灰色小标"（离线模板）"
// - mode='cancelled':   底部加灰色小标"（已取消）"
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
</script>

<template>
  <li :class="roleClass">
    <div class="msg__bubble">
      <span class="msg__text"
        >{{ message.content
        }}<span v-if="streaming" class="msg__cursor" aria-hidden="true" /></span>
      <div v-if="isOffline" class="msg__status-tag">（离线模板）</div>
      <div v-else-if="isCancelled" class="msg__status-tag">（已取消）</div>
    </div>
  </li>
</template>

<style scoped>
.msg {
  list-style: none;
  display: flex;
}

.msg--user {
  justify-content: flex-end;
}

.msg--assistant {
  justify-content: flex-start;
}

.msg--system {
  justify-content: center;
}

.msg__bubble {
  position: relative;
  padding: 10px var(--aipet-space-3);
  border-radius: var(--aipet-radius-bubble);
  font-size: 15px;
  line-height: 1.5;
  word-break: break-word;
  white-space: pre-wrap;
  max-width: 100%;
}

.msg--user .msg__bubble {
  background: var(--aipet-color-bubble-user);
  color: #fff;
  border: 1px solid var(--aipet-color-bubble-user);
}

.msg--assistant .msg__bubble {
  background: var(--aipet-color-bubble-assistant);
  color: var(--aipet-color-text-1);
  border: 1px solid var(--aipet-color-border);
}

.msg--system .msg__bubble {
  background: transparent;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
  font-style: italic;
  border: 0;
}

.msg__text {
  display: inline;
}

/* 流式光标：圆点呼吸（P1 节奏精修）。
   1.4s 周期更柔，sine 近似曲线，0% 起点降到 0.25 + scale 0.8 给"收缩"感更明显。 */
.msg__cursor {
  display: inline-block;
  width: 7px;
  height: 7px;
  margin-left: 6px;
  margin-bottom: 1px;
  border-radius: 50%;
  background: currentColor;
  vertical-align: baseline;
  opacity: 0.5;
  animation: aipet-bubble-pulse 1.4s cubic-bezier(0.45, 0, 0.55, 1) infinite;
}

@keyframes aipet-bubble-pulse {
  0%,
  100% {
    opacity: 0.25;
    transform: scale(0.8);
  }
  45% {
    opacity: 0.9;
    transform: scale(1);
  }
}

.msg__status-tag {
  margin-top: var(--aipet-space-1);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}
</style>
