<script setup lang="ts">
// MessageList:messages 列表 + 自动滚动到底(issue #14)。
// - 用 <TransitionGroup tag="ul"> 容纳 <MessageBubble>;列表本身可滚,外部 .aipet-shell__body 不滚
// - streaming 中 / 新消息追加时 → nextTick 后 scrollTop = scrollHeight 滚到底
// - 用户手动向上滚到非底部时不强制锁底(避免阅读历史时被新 token 拽走);阈值 80px
//
// P1 进场动画(bubble-enter-*):
// - 新消息(新 key)从 translateY(8px) + opacity 0 淡入到位,180ms ease-standard
// - 只动 opacity + transform,不动 height/margin/padding,保证 scrollHeight 在动画启动前已稳定
// - streaming token 增长不重挂 DOM(key 不变),不重复触发动画
// - 切对话整批 key 替换:不加 leave 动画,让旧消息瞬切,新消息按需进场
// - prefers-reduced-motion 由 chat.css 全局兜底
//
// TransitionGroup 用 tag="ul" 渲染:ref 拿到的是组件实例,真实 <ul> DOM 在 .$el 上,
// scroll 操作通过 getEl() helper 取出。
import { computed, nextTick, ref, watch } from 'vue'
import MessageBubble from './MessageBubble.vue'
import type { Message } from '@/types/chat'

interface Props {
  messages: Message[]
  /** 当前正在流式输出的 assistant message id;为 null 表示无活跃流。 */
  streamingMessageId: string | null
}

const props = defineProps<Props>()

// TransitionGroup 实例 ref;DOM 通过 .$el 取。
const scrollerRef = ref<{ $el?: HTMLUListElement } | null>(null)
const stickToBottom = ref(true)
/** empty state 头像 onError 兜底标志。 */
const emptyAvatarFailed = ref(false)

function getEl(): HTMLUListElement | null {
  return (scrollerRef.value?.$el ?? null) as HTMLUListElement | null
}

const lastMessageKey = computed(() => {
  if (props.messages.length === 0) return ''
  const last = props.messages[props.messages.length - 1]
  return `${last.id}:${last.content.length}`
})

function isNearBottom(): boolean {
  const el = getEl()
  if (!el) return true
  return el.scrollHeight - el.scrollTop - el.clientHeight < 80
}

function scrollToBottom() {
  const el = getEl()
  if (!el) return
  el.scrollTop = el.scrollHeight
}

function onScroll() {
  // 用户主动滚到非底部 → 暂停自动跟随;滚回底部附近 → 恢复
  stickToBottom.value = isNearBottom()
}

// messages.length 变(新增)/ 末尾消息 content 变(流式 token 追加)→ 滚到底
watch(lastMessageKey, async () => {
  if (!stickToBottom.value) return
  await nextTick()
  scrollToBottom()
})

// 初次挂载也滚到底(chat_history 拉历史进来时)
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
  <div v-if="messages.length === 0" class="chat-empty" aria-live="polite">
    <img
      v-if="!emptyAvatarFailed"
      src="/avatar/momo-avatar.svg"
      alt=""
      class="chat-empty__avatar"
      @error="emptyAvatarFailed = true"
    />
    <span v-else class="chat-empty__avatar chat-empty__avatar--fallback">M</span>
    <h2 class="chat-empty__title">还在等你说点什么呢…</h2>
    <p class="chat-empty__hint">Enter 发送 · Shift+Enter 换行</p>
  </div>
  <TransitionGroup
    v-else
    ref="scrollerRef"
    tag="ul"
    name="bubble"
    class="chat-messages"
    @scroll.passive="onScroll"
  >
    <MessageBubble
      v-for="m in messages"
      :key="m.id"
      :message="m"
      :streaming="streamingMessageId === m.id"
    />
  </TransitionGroup>
</template>

<style scoped>
.chat-messages {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
  margin: 0;
  padding: var(--aipet-space-5) var(--aipet-space-5);
  overflow-y: auto;
  overflow-x: hidden;
  list-style: none;
  background: transparent;
  min-height: 0;
}

/* P1:新气泡进场。只动 opacity + translateY,不动 layout 维度,
   保证 scrollHeight 在动画启动前已稳定,不跟 scrollTop=scrollHeight 抢节奏。 */
.bubble-enter-active {
  transition: opacity var(--aipet-duration-base) var(--aipet-ease-standard),
    transform var(--aipet-duration-base) var(--aipet-ease-standard);
}

.bubble-enter-from {
  opacity: 0;
  transform: translateY(8px);
}

.bubble-enter-to {
  opacity: 1;
  transform: translateY(0);
}

/* === 主区 empty state ===
 * 仅当 conv 0 消息时显示;头像 + 一句桌宠语气 + 操作提示。
 * 不挂 TransitionGroup,纯静态(进场动画走 opacity fade,400ms,延迟 80ms 给布局稳定)。
 */
.chat-empty {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--aipet-space-3);
  padding: var(--aipet-space-8);
  text-align: center;
  user-select: none;
  animation: aipet-empty-fade 400ms var(--aipet-ease-standard) 80ms both;
}

.chat-empty__avatar {
  width: 72px;
  height: 72px;
  border-radius: 50%;
  background: var(--aipet-color-surface-soft);
  border: 1px solid var(--aipet-color-border);
  display: flex;
  align-items: center;
  justify-content: center;
  /* SVG 等比缩放铺满 */
  object-fit: cover;
  /* hover 时轻微"看你一眼"反应:头部右倾 4 度 + 0.6s */
  transition: transform 0.6s var(--aipet-ease-emphasized);
}

.chat-empty__avatar:hover {
  transform: rotate(4deg) scale(1.04);
}

.chat-empty__avatar--fallback {
  font-size: 32px;
  font-weight: 700;
  color: var(--aipet-color-primary);
}

.chat-empty__title {
  margin: 0;
  font-size: var(--aipet-font-size-xl);
  font-weight: 600;
  color: var(--aipet-color-text-1);
  line-height: var(--aipet-line-height-display);
}

.chat-empty__hint {
  margin: 0;
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
}

@keyframes aipet-empty-fade {
  from {
    opacity: 0;
    transform: translateY(4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
