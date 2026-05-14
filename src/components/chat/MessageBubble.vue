<script setup lang="ts">
// MessageBubble：单条消息气泡（issue #14 + P0 美化 + #25/#26 自定义头像）。
// - 三种 role 视觉差异:
//   * user: 靠右 + 28px 头像（KV `user:avatar_path` → fallback 昵称首字符）+ 紫色单色 + 白字
//   * assistant: 靠左 + 28px 头像（KV `persona:<active_id>:avatar_path` → fallback /avatar/momo-avatar.svg）+ 奶油白底
//   * system: 居中灰条（M1 几乎不出现）
// - 自定义头像通过 useAvatars composable 跨窗口热刷新（avatar:changed event）
// - streaming=true: 在 content 末尾追加圆点呼吸光标（CSS animation）
// - mode='offline_rule': 底部加灰色小标"（离线模板）"
// - mode='cancelled': 底部加灰色小标"（已取消）"
// - 时间戳 hh:mm 显示 + title 看完整 ISO
import { computed, ref, watch } from 'vue'
import { useAvatarsStore } from '@/stores/avatars'
import { useNicknameStore } from '@/stores/nickname'
import type { Message } from '@/types/chat'

interface Props {
  message: Message
  streaming?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  streaming: false,
})

const nicknameStore = useNicknameStore()
const avatarsStore = useAvatarsStore()

const roleClass = computed(() => `msg msg--${props.message.role}`)
const isOffline = computed(() => props.message.mode === 'offline_rule')
const isCancelled = computed(() => props.message.mode === 'cancelled')
const isAssistant = computed(() => props.message.role === 'assistant')
const isUser = computed(() => props.message.role === 'user')

// 各类型头像加载失败标志:img onError 时翻 true,降级到下一层 fallback。
// assistant: 自定义 URL → momo SVG → "M" 字符
// user: 自定义 URL → 昵称首字符
//
// L5 修复：URL 变化时复位 failed flag —— 用户重新导出/上传头像后新 URL 应该重试加载，
// 不应该因为之前 onError 一次就永久卡在 fallback。watch URL 翻 true→新值 时复位。
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

/** 用户昵称首字符;Array.from 处理 emoji / 中文 code point。fallback '我'。 */
const userInitial = computed(() => {
  const n = nicknameStore.user?.trim()
  if (!n) return '我'
  return Array.from(n)[0] ?? '我'
})

/** assistant 实际显示哪个头像源：自定义 URL（在且未失败）→ momo SVG（未失败）→ 字符 fallback。 */
const assistantAvatarSrc = computed<string | null>(() => {
  if (avatarsStore.personaAvatarUrl && !personaImgFailed.value) return avatarsStore.personaAvatarUrl
  if (!momoSvgFailed.value) return '/avatar/momo-avatar.svg'
  return null
})

/** user 头像源：自定义 URL（在且未失败）→ null（走字符 fallback）。 */
const userAvatarSrc = computed<string | null>(() => {
  if (avatarsStore.userAvatarUrl && !userImgFailed.value) return avatarsStore.userAvatarUrl
  return null
})

// hh:mm 简化展示;hover title 显示原始 ISO（dev 排查用）。
const timeLabel = computed(() => {
  const dt = new Date(props.message.created_at)
  if (Number.isNaN(dt.getTime())) return ''
  const hh = String(dt.getHours()).padStart(2, '0')
  const mm = String(dt.getMinutes()).padStart(2, '0')
  return `${hh}:${mm}`
})

/** 区分两层 fallback：自定义 PNG 失败 → 标 personaImgFailed；momo SVG 失败 → 标 momoSvgFailed。
 *  bind 时根据当前 assistantAvatarSrc 判断 onError 该改哪个 flag。 */
function onAssistantImgError() {
  if (avatarsStore.personaAvatarUrl && !personaImgFailed.value) {
    personaImgFailed.value = true
  } else {
    momoSvgFailed.value = true
  }
}
</script>

<template>
  <li :class="roleClass">
    <!-- assistant 左侧头像 -->
    <div v-if="isAssistant" class="msg__avatar msg__avatar--assistant" aria-hidden="true">
      <img
        v-if="assistantAvatarSrc"
        :src="assistantAvatarSrc"
        alt=""
        class="msg__avatar-img"
        @error="onAssistantImgError"
      />
      <span v-else class="msg__avatar-fallback">M</span>
    </div>

    <!-- user 右侧头像:自定义图片 → 昵称首字符占位 -->
    <div v-if="isUser" class="msg__avatar msg__avatar--user" aria-hidden="true">
      <img
        v-if="userAvatarSrc"
        :src="userAvatarSrc"
        alt=""
        class="msg__avatar-img"
        @error="userImgFailed = true"
      />
      <span v-else class="msg__avatar-initial">{{ userInitial }}</span>
    </div>

    <div class="msg__column">
      <div class="msg__bubble">
        <span class="msg__text"
          >{{ message.content
          }}<span v-if="streaming" class="msg__cursor" aria-hidden="true" /></span>
        <div v-if="isOffline" class="msg__status-tag">（离线模板）</div>
        <div v-else-if="isCancelled" class="msg__status-tag">（已取消）</div>
      </div>
      <time class="msg__time" :title="message.created_at">{{ timeLabel }}</time>
    </div>
  </li>
</template>

<style scoped>
.msg {
  display: flex;
  gap: var(--aipet-space-2);
  max-width: 78%;
  list-style: none;
}

.msg--user {
  align-self: flex-end;
  flex-direction: row-reverse;
}

.msg--user .msg__column {
  align-items: flex-end;
}

.msg--assistant {
  align-self: flex-start;
}

.msg--system {
  align-self: center;
  flex-direction: column;
  align-items: center;
  max-width: 95%;
}

/* 头像通用容器 */
.msg__avatar {
  flex: 0 0 auto;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-top: 2px;
  user-select: none;
}

/* assistant: 用 surface-soft 从主区 bg 浮起一档(亮:#f5f5f5 / 暗:#1c1c1c),细灰边 */
.msg__avatar--assistant {
  background: var(--aipet-color-surface-soft);
  border: 1px solid var(--aipet-color-border);
}

/* user: 同 assistant 几何对称;首字符占位 */
.msg__avatar--user {
  background: var(--aipet-color-surface-soft);
  border: 1px solid var(--aipet-color-border);
}

.msg__avatar-img {
  width: 100%;
  height: 100%;
  display: block;
}

.msg__avatar-fallback,
.msg__avatar-initial {
  font-size: 13px;
  font-weight: 600;
  color: var(--aipet-color-primary);
  /* 中文字符容易偏大,稍微收一下 */
  line-height: 1;
}

.msg__column {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-1);
  min-width: 0;
}

.msg__bubble {
  position: relative;
  padding: 10px var(--aipet-space-3);
  border-radius: var(--aipet-radius-bubble);
  font-size: 15px;
  line-height: 1.5;
  word-break: break-word;
  white-space: pre-wrap;
  /* Vercel 风:不用阴影,所有气泡都靠 1px border 划界 */
}

/* 气泡尾巴:CSS 三角(无 SVG 资源依赖)。
 *  assistant 左 → 指向左头像;user 右 → 指向右头像。
 *  对齐逻辑:头像 28px + margin-top 2px → 中心 16px(从 msg 顶部),
 *    bubble 顶部 = msg 顶部(flex column,bubble 是 column 第一个 child),
 *    尾巴自身 12px 高 → top = 16 - 6 = 10px,垂直中心严格对齐头像中心。
 *    不用 bottom:N —— bubble 高度因消息长度变化,bottom 定位会让尾巴飘。
 *  assistant 用 ::before 描边 + ::after 内填两层,与 1px border 视觉连续;
 *  user 单色,一层 ::before 即可。 */
.msg__bubble::before,
.msg__bubble::after {
  content: '';
  position: absolute;
  top: 10px;
  width: 0;
  height: 0;
  border-style: solid;
}

.msg--assistant .msg__bubble::before {
  left: -7px;
  border-width: 6px 7px 6px 0;
  border-color: transparent var(--aipet-color-border) transparent transparent;
}

.msg--assistant .msg__bubble::after {
  left: -6px;
  border-width: 5px 6px 5px 0;
  border-color: transparent var(--aipet-color-bubble-assistant) transparent transparent;
}

.msg--user .msg__bubble::before {
  right: -7px;
  border-width: 6px 0 6px 7px;
  border-color: transparent transparent transparent var(--aipet-color-bubble-user);
}

.msg--user .msg__bubble::after,
.msg--system .msg__bubble::before,
.msg--system .msg__bubble::after {
  display: none;
}

.msg--user .msg__bubble {
  /* Vercel 风:user 气泡单色紫(品牌色焦点),无渐变 */
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

/* 流式光标:圆点呼吸(P1 节奏精修)。
   1.4s 周期更柔(原 1.1s 偏机械),sine 近似曲线(0.45/0/0.55/1)替代 ease-in-out,
   0% 起点降到 0.25 + scale 0.8 给"收缩"感更明显,峰移到 45% 让"扩张更快、收缩更慢"。 */
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

.msg__time {
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
  user-select: none;
}
</style>
