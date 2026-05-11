<script setup lang="ts">
// MessageList：messages 列表 + 自动滚动到底（issue #14）。
// - 用 <ul> 容纳 <MessageBubble>；列表本身可滚，外部 .aipet-shell__body 不滚
// - streaming 中 / 新消息追加时 → nextTick 后 scrollTop = scrollHeight 滚到底
// - 用户手动向上滚到非底部时不强制锁底（避免阅读历史时被新 token 拽走）；阈值 80px
import { computed, nextTick, ref, watch } from 'vue'
import MessageBubble from './MessageBubble.vue'
import type { Message } from '@/types/chat'

interface Props {
  messages: Message[]
  /** 当前正在流式输出的 assistant message id；为 null 表示无活跃流。 */
  streamingMessageId: string | null
}

const props = defineProps<Props>()

const scrollerRef = ref<HTMLUListElement | null>(null)
const stickToBottom = ref(true)

const lastMessageKey = computed(() => {
  if (props.messages.length === 0) return ''
  const last = props.messages[props.messages.length - 1]
  return `${last.id}:${last.content.length}`
})

function isNearBottom(): boolean {
  const el = scrollerRef.value
  if (!el) return true
  return el.scrollHeight - el.scrollTop - el.clientHeight < 80
}

function scrollToBottom() {
  const el = scrollerRef.value
  if (!el) return
  el.scrollTop = el.scrollHeight
}

function onScroll() {
  // 用户主动滚到非底部 → 暂停自动跟随；滚回底部附近 → 恢复
  stickToBottom.value = isNearBottom()
}

// messages.length 变（新增）/ 末尾消息 content 变（流式 token 追加）→ 滚到底
watch(lastMessageKey, async () => {
  if (!stickToBottom.value) return
  await nextTick()
  scrollToBottom()
})

// 初次挂载也滚到底（chat_history 拉历史进来时）
watch(
  () => props.messages.length,
  async (n, prev) => {
    if (prev === 0 && n > 0) {
      await nextTick()
      scrollToBottom()
      stickToBottom.value = true
    }
  },
  { immediate: true },
)
</script>

<template>
  <ul ref="scrollerRef" class="chat-messages" @scroll.passive="onScroll">
    <MessageBubble
      v-for="m in messages"
      :key="m.id"
      :message="m"
      :streaming="streamingMessageId === m.id"
    />
  </ul>
</template>

<style scoped>
.chat-messages {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
  margin: 0;
  padding: var(--aipet-space-4);
  overflow-y: auto;
  overflow-x: hidden;
  list-style: none;
  background: var(--aipet-color-bg);
  min-height: 0;
}
</style>
