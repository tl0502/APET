<script setup lang="ts">
// MessageList：消息分组渲染 + 自动滚动到底（Phase D 重构）。
//
// 关键变化：
// - 相邻同 role 且时间差 ≤ 5 分钟的消息合并为一个 group：单 avatar（顶部）+ bubble stack（多个紧贴气泡）
//   + 单时间戳（底部，取 group 末条）。Discord/Telegram Desktop 风。
// - 头像三层降级逻辑（assistant 自定义 PNG → momo SVG → 'M'；user 自定义 PNG → 昵称首字符）
//   从 MessageBubble 上移到 group 容器层。MessageBubble 退化为"纯气泡 + 流式光标 + status-tag"。
// - bubble 不再带 CSS 尾巴（与桌面 IM 一致；role 区分仍靠 group alignment + 气泡颜色）。
// - streaming 仍指向单个 bubble id；只让目标气泡显示 cursor，group 渲染稳定不抖。
//
// 滚动逻辑保留：watch lastMessageKey（末条 id + content.length）+ stickToBottom 阈值 80px。
// TransitionGroup 用 tag="ul" 渲染；真实 DOM 在 .$el 上，scroll 操作通过 getEl() helper 取出。
import { computed, nextTick, ref, watch } from 'vue'
import MessageBubble from './MessageBubble.vue'
import { useAvatarsStore } from '@/stores/avatars'
import { useNicknameStore } from '@/stores/nickname'
import type { Message } from '@/types/chat'

interface Props {
  messages: Message[]
  /** 当前正在流式输出的 assistant message id；为 null 表示无活跃流。 */
  streamingMessageId: string | null
}

const props = defineProps<Props>()

const nicknameStore = useNicknameStore()
const avatarsStore = useAvatarsStore()

// === 头像三层降级（从 MessageBubble 整段上移） ===
// L5 修复保留：URL 变化时复位 failed flag（用户重新导出/上传头像后新 URL 应该重试）。
const personaImgFailed = ref(false)
const momoSvgFailed = ref(false)
const userImgFailed = ref(false)

watch(
  () => avatarsStore.personaAvatarUrl,
  () => {
    personaImgFailed.value = false
  },
)
watch(
  () => avatarsStore.userAvatarUrl,
  () => {
    userImgFailed.value = false
  },
)

const userInitial = computed(() => {
  const n = nicknameStore.user?.trim()
  if (!n) return '我'
  return Array.from(n)[0] ?? '我'
})

const assistantAvatarSrc = computed<string | null>(() => {
  if (avatarsStore.personaAvatarUrl && !personaImgFailed.value) return avatarsStore.personaAvatarUrl
  if (!momoSvgFailed.value) return '/avatar/momo-avatar.svg'
  return null
})

const userAvatarSrc = computed<string | null>(() => {
  if (avatarsStore.userAvatarUrl && !userImgFailed.value) return avatarsStore.userAvatarUrl
  return null
})

function onAssistantImgError() {
  if (avatarsStore.personaAvatarUrl && !personaImgFailed.value) {
    personaImgFailed.value = true
  } else {
    momoSvgFailed.value = true
  }
}

// === Message Grouping ===

interface MessageGroup {
  /** key：用第一条 message id 派生，保证 TransitionGroup 在 push 新组时不重排已有组 */
  id: string
  role: 'user' | 'assistant' | 'system'
  messages: Message[]
}

/** 5 分钟：相邻同 role 在此窗口内合并；超出则切新组。 */
const GROUP_TIME_WINDOW_MS = 5 * 60 * 1000

const groups = computed<MessageGroup[]>(() => {
  const out: MessageGroup[] = []
  for (const m of props.messages) {
    const last = out[out.length - 1]
    if (last && last.role === m.role) {
      const lastMsg = last.messages[last.messages.length - 1]
      const lastTime = new Date(lastMsg.created_at).getTime()
      const mTime = new Date(m.created_at).getTime()
      // Number.isNaN 防御：created_at 解析失败时回退为合并（避免单条消息独立成组）
      const gap =
        Number.isNaN(lastTime) || Number.isNaN(mTime) ? 0 : Math.abs(mTime - lastTime)
      if (gap <= GROUP_TIME_WINDOW_MS) {
        last.messages.push(m)
        continue
      }
    }
    out.push({ id: `g-${m.id}`, role: m.role, messages: [m] })
  }
  return out
})

function groupTimeLabel(g: MessageGroup): string {
  const last = g.messages[g.messages.length - 1]
  const dt = new Date(last.created_at)
  if (Number.isNaN(dt.getTime())) return ''
  const hh = String(dt.getHours()).padStart(2, '0')
  const mm = String(dt.getMinutes()).padStart(2, '0')
  return `${hh}:${mm}`
}

function groupTimeTitle(g: MessageGroup): string {
  return g.messages[g.messages.length - 1].created_at
}

// === 滚动 ===

const scrollerRef = ref<{ $el?: HTMLUListElement } | null>(null)
const stickToBottom = ref(true)
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
  stickToBottom.value = isNearBottom()
}

// 流式 token 增长（最后一条 content.length 变化）→ 若已锁底则跟随滚到底。
watch(lastMessageKey, async () => {
  if (!stickToBottom.value) return
  await nextTick()
  scrollToBottom()
})

// 消息数变化 = 新消息追加：
// - 用户刚发送（末条 role===user）→ 无条件强制滚到底 + 重新锁底，覆盖之前阅读历史时的 stickToBottom=false。
//   ChatGPT 网页同样行为：发完消息 user 气泡立即顶到视野底，开始等 assistant。
// - assistant placeholder 刚 push（消息数 +1 但末条 role===assistant + content=空）→ 不在这里
//   触发滚动；交给 lastMessageKey watcher 在 token 流入时跟随。
// - 切对话 / 初次挂载（prev===0 && n>0）→ 拉历史完成滚到底 + 锁底。
watch(
  () => props.messages.length,
  async (n, prev) => {
    const prevLen = prev ?? 0
    if (prevLen === 0 && n > 0) {
      await nextTick()
      scrollToBottom()
      stickToBottom.value = true
      return
    }
    if (n > prevLen) {
      const last = props.messages[n - 1]
      if (last?.role === 'user') {
        stickToBottom.value = true
        await nextTick()
        scrollToBottom()
      } else if (last?.role === 'assistant' && stickToBottom.value) {
        await nextTick()
        scrollToBottom()
      }
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
    <li
      v-for="g in groups"
      :key="g.id"
      :class="['msg-group', `msg-group--${g.role}`]"
    >
      <!-- assistant 头像 -->
      <div v-if="g.role === 'assistant'" class="msg-group__avatar" aria-hidden="true">
        <img
          v-if="assistantAvatarSrc"
          :src="assistantAvatarSrc"
          alt=""
          class="msg-group__avatar-img"
          @error="onAssistantImgError"
        />
        <span v-else class="msg-group__avatar-fallback">M</span>
      </div>
      <!-- user 头像 -->
      <div v-else-if="g.role === 'user'" class="msg-group__avatar" aria-hidden="true">
        <img
          v-if="userAvatarSrc"
          :src="userAvatarSrc"
          alt=""
          class="msg-group__avatar-img"
          @error="userImgFailed = true"
        />
        <span v-else class="msg-group__avatar-initial">{{ userInitial }}</span>
      </div>

      <div class="msg-group__main">
        <ul class="msg-group__stack">
          <MessageBubble
            v-for="m in g.messages"
            :key="m.id"
            :message="m"
            :streaming="streamingMessageId === m.id"
          />
        </ul>
        <time
          v-if="g.role !== 'system'"
          class="msg-group__time"
          :title="groupTimeTitle(g)"
        >{{ groupTimeLabel(g) }}</time>
      </div>
    </li>
  </TransitionGroup>
</template>

<style scoped>
.chat-messages {
  flex: 1 1 auto;
  /* 唯一滚动容器：父级 .message-scroll-surface 已改为 flex column 不滚，本元素 overflow-y 才真正生效。
     min-height: 0 是 flex column 嵌套常坑——不设的话 flex:1 1 auto 在内容超出时不会收缩，
     scrollHeight === clientHeight，scrollTop 设置无效。 */
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
  margin: 0;
  /* 底部多 8px 让最后一条消息不贴 floating-composer 顶边，留呼吸空间 */
  padding: var(--aipet-space-5) var(--aipet-space-5) var(--aipet-space-3);
  overflow-y: auto;
  overflow-x: hidden;
  list-style: none;
  background: transparent;
}

/* P1：新组进场。只动 opacity + translateY，不动 layout 维度。 */
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

/* === message group === */
.msg-group {
  display: flex;
  gap: var(--aipet-space-2);
  max-width: 78%;
  list-style: none;
}

.msg-group--user {
  align-self: flex-end;
  flex-direction: row-reverse;
}

.msg-group--assistant {
  align-self: flex-start;
}

.msg-group--system {
  align-self: center;
  max-width: 95%;
}

.msg-group__avatar {
  flex: 0 0 auto;
  width: 32px;
  height: 32px;
  border-radius: 50%;
  overflow: hidden;
  background: var(--aipet-color-surface-soft);
  border: 1px solid var(--aipet-color-border);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-top: 2px;
  user-select: none;
}

.msg-group__avatar-img {
  width: 100%;
  height: 100%;
  display: block;
}

.msg-group__avatar-fallback,
.msg-group__avatar-initial {
  font-size: 14px;
  font-weight: 600;
  color: var(--aipet-color-primary);
  line-height: 1;
}

.msg-group__main {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-1);
  min-width: 0;
  flex: 0 1 auto;
}

.msg-group--user .msg-group__main {
  align-items: flex-end;
}

/* bubble stack：连续气泡 3px 紧贴，桌面 IM 风。 */
.msg-group__stack {
  display: flex;
  flex-direction: column;
  gap: 3px;
  list-style: none;
  margin: 0;
  padding: 0;
}

.msg-group--user .msg-group__stack {
  align-items: flex-end;
}

.msg-group--assistant .msg-group__stack {
  align-items: flex-start;
}

.msg-group__time {
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
  user-select: none;
}

/* === 主区 empty state（保留原样） === */
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
  object-fit: cover;
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
