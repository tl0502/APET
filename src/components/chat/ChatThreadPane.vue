<script setup lang="ts">
// ChatThreadPane (#33 phase D)：chat 业务 content/messages/composer 单独 pane。
//
// 共享给：
// - chat 磁吸窗 ChatBody.vue（与 ConversationListPane 双 pane 组装）
// - workspace DetailColumn.vue（chat 类别下渲染）
//
// 业务状态走 ConversationStore（Pinia singleton）；本组件承担：
// - 视图：content-header（≡ + identity + drag-handle + ✕）+ MessageList + ChatInput
// - 每实例独立 chrome：persona identity / avatar 三层降级 / ESC keydown
// - v-model inputDraft 双向绑定 store.draft
// - emits: close（仅磁吸窗 ✕ 用）/ toggleSidebar（仅磁吸窗 ≡ 用）
//
// ESC 仅在 showCloseButton=true 时挂（磁吸窗）；workspace 内由 WorkspaceApp 全局 keydown 接管。

import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import ChatInput from '@/components/chat/ChatInput.vue'
import MessageList from '@/components/chat/MessageList.vue'
import { useToast } from '@/composables/useToast'
import { useAvatarsStore } from '@/stores/avatars'
import { useConversationStore } from '@/stores/conversation'
import { useNicknameStore } from '@/stores/nickname'
import { getActivePersona } from '@/services/persona'

const props = withDefaults(
  defineProps<{
    /** 父容器是否激活（workspace 内为 currentCategory === 'chat'；磁吸窗永远 true）。
     *  当前仅 reserved 给未来 watch 暂停某些副作用（如 stream 视觉），phase D 不消费。 */
    panelActive?: boolean
    /** 是否渲染 content-header 右侧 ✕ 关闭按钮（磁吸窗 true，workspace false） */
    showCloseButton?: boolean
    /** 是否渲染 content-header 中央胶囊拖动块（磁吸窗 true 整窗拖动；workspace false） */
    showTitlebarDrag?: boolean
    /** 是否渲染 content-header 左侧 ≡ 折叠按钮（磁吸窗 true 控制 sidebar 折叠；workspace false） */
    showSidebarToggle?: boolean
    /** ≡ 按钮的 aria-pressed 状态（磁吸窗内由 ChatBody.sidebarCollapsed 透传） */
    sidebarCollapsed?: boolean
  }>(),
  {
    panelActive: true,
    showCloseButton: true,
    showTitlebarDrag: true,
    showSidebarToggle: true,
    sidebarCollapsed: false,
  },
)

const emit = defineEmits<{
  close: []
  toggleSidebar: []
}>()

const toast = useToast()
const nicknameStore = useNicknameStore()
const avatarsStore = useAvatarsStore()
const store = useConversationStore()

// === chrome state（每实例独立）===

const MOODS = ['等你来', '刚刚醒了', '在听呢', '想你了', '安静等着', '随你说', '今天还好吗'] as const
const currentMood = ref<string>(MOODS[0])
const personaName = ref<string>('')

const personaImgFailed = ref(false)
const avatarFailed = ref(false)
watch(
  () => avatarsStore.personaAvatarUrl,
  () => {
    personaImgFailed.value = false
  },
)

const personaInitial = computed(() => {
  const n = personaName.value?.trim()
  return n ? n.charAt(0).toUpperCase() : 'M'
})

// === v-model inputDraft：投射到 store 的 draft 字段 ===

const inputDraft = computed<string>({
  get: () => store.getDraft(store.activeId),
  set: (v) => store.setDraft(store.activeId, v),
})

async function handleSend() {
  const draftRaw = inputDraft.value.trim()
  await store.send(draftRaw)
}

async function handleCancel() {
  await store.cancel()
}

function handleClose() {
  emit('close')
}

function handleToggleSidebar() {
  emit('toggleSidebar')
}

// === lifecycle ===

const unlistenFns: UnlistenFn[] = []

function onGlobalKeydown(e: KeyboardEvent) {
  if (e.key !== 'Escape') return
  if (document.querySelector('.el-message-box, .el-dialog__wrapper, .el-overlay')) return
  const active = document.activeElement
  if (active instanceof Element && active.closest('.conv-item__rename-input')) return
  emit('close')
}

onMounted(async () => {
  try {
    if (!nicknameStore.loaded) await nicknameStore.load()
    await nicknameStore.ensureListener()
  } catch (e) {
    console.warn('[ChatThreadPane] nickname store load failed:', e)
  }
  try {
    if (!avatarsStore.loaded) await avatarsStore.load()
    await avatarsStore.ensureListener()
  } catch (e) {
    console.warn('[ChatThreadPane] avatars store load failed:', e)
  }

  currentMood.value = MOODS[Math.floor(Math.random() * MOODS.length)]

  try {
    personaName.value = (await getActivePersona()).name
  } catch (e) {
    console.warn('[ChatThreadPane] getActivePersona failed:', e)
    toast.error('未能加载当前人格，请到设置面板检查或激活一个人格', { duration: 5000 })
  }

  try {
    const unPersona = await listen('persona:activated', async () => {
      try {
        personaName.value = (await getActivePersona()).name
      } catch (e) {
        console.warn('[ChatThreadPane] refresh persona name failed:', e)
      }
    })
    unlistenFns.push(unPersona)
  } catch (e) {
    console.warn('[ChatThreadPane] listen persona:activated failed:', e)
  }

  // ESC 仅在磁吸窗内挂（showCloseButton=true）；workspace 由 WorkspaceApp 全局 keydown 接管
  if (props.showCloseButton) {
    window.addEventListener('keydown', onGlobalKeydown)
  }

  await store.loadInitial()
})

onBeforeUnmount(() => {
  unlistenFns.forEach((u) => u())
  if (props.showCloseButton) {
    window.removeEventListener('keydown', onGlobalKeydown)
  }
  store.flushAllDrafts()
})

// panelActive 当前 phase D 未消费；保留 prop 给未来用
void props.panelActive
</script>

<template>
  <main class="content-surface">
    <header class="content-header">
      <button
        v-if="showSidebarToggle"
        class="content-header__toggle"
        :title="sidebarCollapsed ? '展开会话栏' : '收起会话栏'"
        :aria-label="sidebarCollapsed ? '展开会话栏' : '收起会话栏'"
        @click="handleToggleSidebar"
      >≡</button>

      <div class="content-header__identity">
        <div class="content-header__avatar" aria-hidden="true">
          <img
            v-if="avatarsStore.personaAvatarUrl && !personaImgFailed"
            :src="avatarsStore.personaAvatarUrl"
            alt=""
            class="content-header__avatar-img"
            @error="personaImgFailed = true"
          />
          <img
            v-else-if="!avatarFailed"
            src="/avatar/momo-avatar.svg"
            alt=""
            class="content-header__avatar-img"
            @error="avatarFailed = true"
          />
          <span v-else class="content-header__avatar-fallback">{{ personaInitial }}</span>
        </div>
        <div class="content-header__name-wrap">
          <span v-if="personaName" class="content-header__name">{{ personaName }}</span>
          <span v-else class="content-header__name content-header__name--placeholder" />
          <span class="content-header__status">
            <span class="content-header__status-dot" aria-hidden="true" />
            {{ currentMood }}
          </span>
        </div>
      </div>

      <div
        v-if="showTitlebarDrag"
        class="content-header__drag-handle"
        data-tauri-drag-region
        title="拖动窗口"
        aria-label="拖动窗口"
      />
      <div v-else class="content-header__drag-spacer" />

      <button
        v-if="showCloseButton"
        class="content-header__close"
        title="关闭（进托盘）"
        aria-label="关闭"
        @click="handleClose"
      >✕</button>
    </header>

    <div class="message-scroll-surface">
      <MessageList
        :messages="store.currentMessages"
        :streaming-message-id="store.currentStreamingMessageId"
      />
    </div>
    <div class="floating-composer">
      <ChatInput
        v-model="inputDraft"
        :input-disabled="false"
        :send-disabled="store.isCurrentStreaming"
        :show-cancel="store.canCancelHere"
        :cancelling="store.isCancellingHere"
        @send="handleSend"
        @cancel="handleCancel"
      />
    </div>
  </main>
</template>

<style scoped>
.content-header {
  position: relative;
  z-index: 1;
  flex: 0 0 56px;
  display: flex;
  align-items: center;
  background: var(--aipet-color-surface-blur);
  backdrop-filter: blur(12px) saturate(180%);
  -webkit-backdrop-filter: blur(12px) saturate(180%);
  border-bottom: 1px solid var(--aipet-color-border-faint);
  user-select: none;
}

.content-header__toggle {
  flex: 0 0 56px;
  width: 56px;
  height: 56px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--aipet-color-text-2);
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
  padding: 4px;
  margin: 0;
  border-radius: 0;
  transition: background-color 100ms ease, color 100ms ease;
}

.content-header__toggle:hover {
  background: color-mix(in srgb, var(--aipet-color-text-1) 6%, transparent);
  color: var(--aipet-color-text-1);
}

.content-header__toggle:active {
  background: color-mix(in srgb, var(--aipet-color-text-1) 12%, transparent);
}

.content-header__identity {
  flex: 0 1 auto;
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
  padding: 0 var(--aipet-space-3);
  min-width: 0;
}

.content-header__avatar {
  flex: 0 0 auto;
  width: 32px;
  height: 32px;
  border-radius: 50%;
  overflow: hidden;
  background: var(--aipet-color-bg);
  border: 1px solid var(--aipet-color-border);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 0.6s var(--aipet-ease-emphasized);
}

.content-header__avatar:hover {
  transform: rotate(4deg) scale(1.04);
}

.content-header__avatar-img {
  width: 100%;
  height: 100%;
  display: block;
}

.content-header__avatar-fallback {
  font-size: 14px;
  font-weight: 600;
  color: var(--aipet-color-primary);
}

.content-header__name-wrap {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 1px;
  min-width: 0;
}

.content-header__name {
  font-size: var(--aipet-font-size-base);
  font-weight: 500;
  color: var(--aipet-color-text-1);
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.content-header__name--placeholder {
  min-height: 1em;
}

.content-header__status {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
  line-height: 1;
}

.content-header__status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--aipet-color-online);
  animation: aipet-status-pulse 1.5s var(--aipet-ease-standard) infinite;
}

@keyframes aipet-status-pulse {
  0%, 100% { opacity: 0.55; }
  50% { opacity: 1; }
}

.content-header__close {
  flex: 0 0 auto;
  width: 46px;
  height: 56px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--aipet-color-text-2);
  font-size: 13px;
  font-family: 'Segoe Fluent Icons', 'Segoe MDL2 Assets', system-ui, sans-serif;
  cursor: pointer;
  padding: 0;
  margin: 0;
  transition: background-color 100ms ease, color 100ms ease;
}

.content-header__close:hover {
  background: #c42b1c;
  color: #ffffff;
}

.content-header__close:active {
  background: #a01e15;
  color: #ffffff;
}

.content-header__drag-handle {
  flex: 1 1 auto;
  align-self: stretch;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: grab;
  min-width: var(--aipet-space-6);
}

.content-header__drag-handle::before {
  content: '';
  width: 64px;
  height: 4px;
  border-radius: 999px;
  background: var(--aipet-color-border);
  transition: background 120ms ease, width 120ms ease;
}

.content-header__drag-handle:hover::before {
  background: var(--aipet-color-border-strong);
  width: 80px;
}

.content-header__drag-handle:active {
  cursor: grabbing;
}

.content-header__drag-handle:active::before {
  background: var(--aipet-color-text-3);
}

.content-header__drag-spacer {
  flex: 1 1 auto;
}

.content-surface {
  flex: 1 1 auto;
  background: var(--aipet-color-surface);
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  position: relative;
}

.message-scroll-surface {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background-image: radial-gradient(
    circle at 1px 1px,
    color-mix(in srgb, var(--aipet-color-text-3) 15%, transparent) 1px,
    transparent 1px
  );
  background-size: 24px 24px;
  background-position: 0 0;
}

:global(html.dark) .message-scroll-surface {
  background-image: radial-gradient(
    circle at 1px 1px,
    color-mix(in srgb, var(--aipet-color-text-3) 20%, transparent) 1px,
    transparent 1px
  );
}

.floating-composer {
  flex: 0 0 auto;
  padding: var(--aipet-space-3) var(--aipet-space-4);
}
</style>
