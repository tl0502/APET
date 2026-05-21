<script setup lang="ts">
// ChatBody (#33 phase A)：chat 业务壳层（sidebar + content + composer）。
//
// 共享给：
// - src/views/chat/ChatApp.vue（chat 磁吸窗 — 全 chrome：showCloseButton=true / showTitlebarDrag=true）
// - src/panels/chat/ChatHubPanel.vue（Phase D — 无 chrome：showCloseButton=false / showTitlebarDrag=false）
//
// 业务状态全部走 useConversationStore（Pinia singleton）；本组件仅承担：
// - 视图渲染 + v-model inputDraft 双向绑定
// - 每实例独立的 chrome 状态（persona identity / avatar 三层降级 / sidebarCollapsed / ESC keydown）
// - ElMessageBox.confirm 删除二次确认（UI 阻塞交互，留组件层）
// - emit close 让外层处理 hideChat / 无效（HubPanel 不监听）

import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ElMessageBox } from 'element-plus'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import ChatInput from '@/components/chat/ChatInput.vue'
import ConversationSidebar from '@/components/chat/ConversationSidebar.vue'
import MessageList from '@/components/chat/MessageList.vue'
import { useToast } from '@/composables/useToast'
import { useAvatarsStore } from '@/stores/avatars'
import { useConversationStore } from '@/stores/conversation'
import { useNicknameStore } from '@/stores/nickname'
import { getActivePersona } from '@/services/persona'

interface Props {
  /** 父容器是否激活（HubPanel 用 dockview activePanel；磁吸窗永远 true）。
   *  Phase A 暂未消费；Phase D HubPanel 会用 watch 此 prop 决定是否暂停某些副作用。 */
  panelActive?: boolean
  /** 是否渲染 content-header 右侧 ✕ 关闭按钮（磁吸窗 true，HubPanel false 因为 dockview tab 自带）。 */
  showCloseButton?: boolean
  /** 是否渲染 content-header 中央胶囊拖动块（磁吸窗 true 整窗拖动；HubPanel false 走 dockview tab 拖）。 */
  showTitlebarDrag?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  panelActive: true,
  showCloseButton: true,
  showTitlebarDrag: true,
})

const emit = defineEmits<{
  close: []
}>()

const toast = useToast()
const nicknameStore = useNicknameStore()
const avatarsStore = useAvatarsStore()
const store = useConversationStore()

// === chrome state（每实例独立）===

const sidebarCollapsed = ref(false)

/** Sidebar identity micro-copy：替换通用"在线"文字。
 *  挂载时随机选一条（每窗口/panel 生命周期固定）；桌宠"真正状态机"接入前的占位实现。 */
const MOODS = ['等你来', '刚刚醒了', '在听呢', '想你了', '安静等着', '随你说', '今天还好吗'] as const
const currentMood = ref<string>(MOODS[0])
const personaName = ref<string>('')

/** 自定义 persona PNG 加载失败时降级到 momo SVG。URL 变化时复位 failed flag。 */
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

// === 删除二次确认（ElMessageBox 留这层）===

async function onDeleteConversation(id: string) {
  const target = store.conversations.find((c) => c.id === id)
  const label = target?.title?.trim() || '此对话'
  try {
    await ElMessageBox.confirm(`删除「${label}」及其所有消息？此操作不可撤销。`, '确认删除', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
      confirmButtonClass: 'el-button--danger',
    })
  } catch {
    return // 用户取消 / ESC
  }
  await store.remove(id)
}

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

function toggleSidebar() {
  sidebarCollapsed.value = !sidebarCollapsed.value
}

// === lifecycle ===

const unlistenFns: UnlistenFn[] = []

function onGlobalKeydown(e: KeyboardEvent) {
  if (e.key !== 'Escape') return
  // ESC 不"穿透"到全窗口隐藏 —— 弹窗 / 重命名 input 由其它组件 cancel
  if (document.querySelector('.el-message-box, .el-dialog__wrapper, .el-overlay')) return
  const active = document.activeElement
  if (active instanceof Element && active.closest('.conv-item__rename-input')) return
  emit('close')
}

onMounted(async () => {
  // store init：nickname / avatar store 是 MessageBubble 跨窗口共享依赖
  try {
    if (!nicknameStore.loaded) await nicknameStore.load()
    await nicknameStore.ensureListener()
  } catch (e) {
    console.warn('[ChatBody] nickname store load failed:', e)
  }
  try {
    if (!avatarsStore.loaded) await avatarsStore.load()
    await avatarsStore.ensureListener()
  } catch (e) {
    console.warn('[ChatBody] avatars store load failed:', e)
  }

  currentMood.value = MOODS[Math.floor(Math.random() * MOODS.length)]

  try {
    personaName.value = (await getActivePersona()).name
  } catch (e) {
    console.warn('[ChatBody] getActivePersona failed:', e)
    toast.error('未能加载当前人格，请到设置面板检查或激活一个人格', { duration: 5000 })
  }

  try {
    const unPersona = await listen('persona:activated', async () => {
      try {
        personaName.value = (await getActivePersona()).name
      } catch (e) {
        console.warn('[ChatBody] refresh persona name failed:', e)
      }
    })
    unlistenFns.push(unPersona)
  } catch (e) {
    console.warn('[ChatBody] listen persona:activated failed:', e)
  }

  window.addEventListener('keydown', onGlobalKeydown)

  await store.loadInitial()
})

onBeforeUnmount(() => {
  unlistenFns.forEach((u) => u())
  window.removeEventListener('keydown', onGlobalKeydown)
  // 关窗前 flush 所有未触发的 draft debounce（保证不丢最后几个字）
  store.flushAllDrafts()
})

// panelActive 当前 phase A 未消费；保留 prop 在 phase D HubPanel 时使用
void props.panelActive
</script>

<template>
  <div class="app-body">
    <ConversationSidebar
      :conversations="store.conversations"
      :active-id="store.activeId"
      :locked-ids="store.streamingConvIds"
      :collapsed="sidebarCollapsed"
      @select="store.switchTo"
      @create="store.create"
      @rename="store.rename"
      @archive="store.archive"
      @delete="onDeleteConversation"
    />

    <main class="content-surface">
      <header class="content-header">
        <button
          class="content-header__toggle"
          :title="sidebarCollapsed ? '展开会话栏' : '收起会话栏'"
          :aria-label="sidebarCollapsed ? '展开会话栏' : '收起会话栏'"
          @click="toggleSidebar"
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
        <!-- HubPanel 不显胶囊：dockview tab 自带拖；header 收尾让 ✕（若有）右贴 -->
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
  </div>
</template>

<style scoped>
/* === content-header（content-surface 顶部 window chrome）===
   48-56px header：≡ + identity + 拖动块 + ✕（可选）。
   半透明 surface-blur + backdrop-filter 让背后 dot-grid 透出（frosted）；
   底部 hairline border-faint 弱分隔；z-index:1 + relative 让 blur 生效。 */
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

/* === 胶囊拖动块（仅 chat 磁吸窗）===
   Tauri data-tauri-drag-region 不被子元素继承；用独立元素挂；
   flex:1 吃掉 identity 与 ✕ 之间剩余空间；::before 渲染 pill 作"拖动手柄"提示。 */
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

/* HubPanel 无 drag：保留 flex:1 让 ✕（若存在）右贴 / 整体居中 */
.content-header__drag-spacer {
  flex: 1 1 auto;
}

/* === app-body（sidebar + content 水平容器）=== */
.app-body {
  width: 100%;
  height: 100%;
  display: flex;
  min-height: 0;
}

/* === content-surface（消息区+composer 列容器）===
   surface 阶梯 L2：比 sidebar(L1) 亮、比 bg(L0) 暗一档。
   relative positioning 给 floating-composer 提供定位上下文。 */
.content-surface {
  flex: 1 1 auto;
  background: var(--aipet-color-surface);
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  position: relative;
}

/* === message-scroll-surface（消息滚动容器外壳）===
   本元素不再是滚动容器，滚动职责下放给内部 .chat-messages (ul) 唯一负责。
   dot-grid 背景仍保留 = "桌面壁纸"不随消息滚动，符合桌面 IM 习惯。 */
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

/* === floating-composer（浮卡容器）===
   上下 padding 对称（space-3）让浮卡视觉中心居中；
   左右 space-4 与 app-surface 圆角呼应（不贴边）。 */
.floating-composer {
  flex: 0 0 auto;
  padding: var(--aipet-space-3) var(--aipet-space-4);
}
</style>
