<script setup lang="ts">
// ChatApp：chat 窗口的 root（Desktop Chat Window Architecture 重构 Phase A）。
//
// 重构核心（与上版差异）：
// - DOM 平级 surface 结构：window-root → app-surface → titlebar-surface + app-body(sidebar + content-surface(message-scroll + floating-composer)) + SnapGhost
// - drop AppShell（chat 不再走 standalone 三段式 shell；AppShell.vue 本身不动，其他 4 窗口继续用）
// - 真透明窗 + CSS 14px 圆角（tauri transparent:true + chat.html 全链路透明 + .app-surface 唯一实体层）
// - Surface 阶梯：surface-0 (window bg) → 中间层 → surface-2 (content) → surface-3 (composer Phase C)
// - titlebar 48px window chrome（仅 ─ + ✕，左/中是 drag region 大片空白）
// - Phase A 暂不渲染人格 identity（Phase B 迁到 sidebar 顶）
//
// V3 状态模型（保留不变）：
// - 状态：messages/draft/streaming 从单 ref 升级为 convStates: reactive Map<convId, ConvState>
//   每对话独立分桶；computed 投射当前 view
// - 多对话并发流式：任意对话可同时在跑 stream；切换 view 不影响后台流
// - Channel 路由：每次 handleSend 创建的 channel.onmessage 闭包捕获 routedConvId/AssistantId
// - draft 持久化：debounce 200ms 写到 config 表 KV chat:draft:<convId>，关窗保留
// - sidebar lockedIds: 任意有 in-flight 流的 convId 显示 spinner + 禁 rename/archive/delete
// - 输入框永远可编辑；发送按钮在 current 流式中时灰；取消按钮在 current=streaming 时可见

import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { ElMessageBox } from 'element-plus'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import { LogicalSize, getCurrentWindow } from '@tauri-apps/api/window'
import ChatInput from '@/components/chat/ChatInput.vue'
import ConversationSidebar from '@/components/chat/ConversationSidebar.vue'
import MessageList from '@/components/chat/MessageList.vue'
import SnapGhost from '@/components/SnapGhost.vue'
import { useToast } from '@/composables/useToast'
import { useSnapWindow } from '@/composables/useSnapWindow'
import { getConfig } from '@/services/config'
import { useAvatarsStore } from '@/stores/avatars'
import { useNicknameStore } from '@/stores/nickname'
import {
  Channel,
  archiveConversation,
  cancelChat,
  createConversation,
  deleteConversation,
  getChatDraft,
  listConversations,
  loadChatHistory,
  renameConversation,
  sendChat,
  setActiveConversation,
  setChatDraft,
} from '@/services/chat'
import { getActivePersona } from '@/services/persona'
import { hideChat } from '@/services/window'
import type { ConversationSummary, Message, StreamEvent } from '@/types/chat'

const toast = useToast()
const nicknameStore = useNicknameStore()
const avatarsStore = useAvatarsStore()

// #30 磁吸窗口系统：chat 作为参与磁吸的窗口，挂 listener + dragSession + persistence。
// composable 内部 onMounted/onBeforeUnmount 注册时机与本组件一致。
// T2a (#31)：拿 isPreviewAnchor 给 .app-surface 套 .snap-preview class（拖 pet 接近 chat 时高亮）。
// T7 (#31 follow-up B)：previewEdgeFor + previewIntensityFor 渲染沿对接边的矩形 glow（替代圆形）。
// Phase A (#31 follow-up C)：isFieldAnchor + fieldIntensityFor 渲染 field halo
// Phase F (#31 follow-up C)：selfLean — 本窗拖动 + 在 field 内时朝 pet 方向 ≤3px 透传 CSS transform
const {
  isPreviewAnchor: chatIsPreviewAnchor,
  previewEdgeFor: chatPreviewEdge,
  previewIntensityFor: chatPreviewIntensity,
  isFieldAnchor: chatIsFieldAnchor,
  fieldIntensityFor: chatFieldIntensity,
  selfLean: chatSelfLean,
} = useSnapWindow('chat')

const chatSnapPreviewClass = computed(() => {
  const cls: Record<string, boolean> = {
    'snap-preview': chatIsPreviewAnchor.value,
    'snap-field-anchor': chatIsFieldAnchor.value,
  }
  if (chatIsPreviewAnchor.value && chatPreviewEdge.value) {
    cls[`snap-preview--edge-${chatPreviewEdge.value}`] = true
  }
  return cls
})
const chatSnapPreviewStyle = computed(() => ({
  '--snap-preview-intensity': String(chatPreviewIntensity.value),
  '--snap-field-intensity': String(chatFieldIntensity.value),
}))

// Phase F (#31 follow-up C)：self-lean transform 应用到最外层 .window-root
// 不影响内层 .app-surface 的 border-radius / box-shadow。
const chatLeanStyle = computed(() => {
  const lean = chatSelfLean.value
  if (!lean) return {}
  return { transform: `translate(${lean.dx.toFixed(2)}px, ${lean.dy.toFixed(2)}px)` }
})

// === 状态模型 ===

interface InflightBuffer {
  tokens: string[]
  done: { finishReason: string } | null
  error: { errorKind: string; message: string } | null
}

interface StreamSlot {
  /** null = prepare 期；非 null = stream 期（assistant placeholder 已 push 到 messages）。 */
  assistantId: string | null
  /** 早期事件缓冲（assistantId === null 期间到达的 delta/done/error）。
   *  assistantId 设值后此字段被 drain + 设 null。 */
  inflight: InflightBuffer | null
  /** 用户已点取消，等后端 done(cancelled) / error 抵达期间的 UI 中间态。
   *  ChatInput 据此把"取消"按钮文案改"取消中…"并 disable，避免重复点击。
   *  finalizeStream / handleStreamError 把 stream slot 整个置 null 时 cancelling 隐式消失。
   *  cancelChat IPC 抛错时复原为 false（让用户能再试）。 */
  cancelling: boolean
}

interface ConvState {
  messages: Message[]
  draft: string
  /** null = 该对话当前没有 in-flight 流；非 null = prepare 或 stream 中。 */
  stream: StreamSlot | null
}

const conversations = ref<ConversationSummary[]>([])
const conversationId = ref<string | null>(null)
/** Phase B 会用：sidebar 折叠状态（顶部 toggle 按钮控制）。 */
const sidebarCollapsed = ref(false)

/** Phase B：人格 identity 现在挂在 sidebar 顶部 chrome。
 *  personaName 由 getActivePersona() 异步加载；persona:activated 事件触发刷新。 */
const personaName = ref<string>('')

/** Sidebar identity micro-copy：替换通用"在线"文字。
 *  保持中性灰风格,无 emoji,语气拟人但克制。挂载时随机选一条(每窗口生命周期固定),
 *  桌宠"真正状态机"接入前的占位实现。 */
const MOODS = ['等你来', '刚刚醒了', '在听呢', '想你了', '安静等着', '随你说', '今天还好吗'] as const
const currentMood = ref<string>(MOODS[0])

/** 自定义 persona PNG（avatarsStore.personaAvatarUrl）加载失败时降级到 momo SVG。
 *  L5 修复：URL 变化时复位 failed flag（用户重新导出后新 URL 应重试，不被旧 fail 卡住）。
 *  布局 v2：identity 从 sidebar chrome 迁到 content-header，三层降级仍由 ChatApp 持有。 */
const personaImgFailed = ref(false)
const avatarFailed = ref(false)
watch(
  () => avatarsStore.personaAvatarUrl,
  () => {
    personaImgFailed.value = false
  },
)

/** 人格名首字符（fallback 'M'），content-header avatar 三层降级最后一档。 */
const personaInitial = computed(() => {
  const n = personaName.value?.trim()
  return n ? n.charAt(0).toUpperCase() : 'M'
})
/** V3 核心 state：所有按对话独立的状态。
 *  reactive(Map) 让 .set/.delete + 内部字段变化都触发依赖重渲。 */
const convStates = reactive(new Map<string, ConvState>())
/** 无 active conv 时（首启 0 conversations）的草稿 fallback。
 *  首次发送 sendChat resolve 后会附到新建的 conv，pendingDraft 清空。 */
const pendingDraft = ref('')
/** #13：首启 0 conversations 路径的 in-flight 防御。sourceConvId === null 时
 *  下面 stream slot 不预占（无 conv id 可挂），用本独立 ref 拦住"await sendChat
 *  期间用户又输入新内容并按 Enter"导致并发 chat_send 建出两个 conversation 的边角。 */
const firstSendInFlight = ref(false)

const HISTORY_LIMIT = 50
const SIDEBAR_LIMIT = 50
const DRAFT_DEBOUNCE_MS = 200
/** #3 修复：首启路径（sourceConvId === null）发送时，realConvId 要等 sendChat resolve
 *  才知道。在那之前已经到达的 channel 早期事件需要一个落点；占位 key 挂个 stream slot
 *  在 convStates 里，事件路由到这里，resolve 后整体迁移到 realConvId。
 *
 *  早先版本在 onmessage 里看到 routedConvId === null 直接丢弃，理论上"IPC resolve 永远先于
 *  channel"——但这个假设在网络极快的本地 LLM（Ollama）+ 微秒级 IPC 抖动下会破，第一个字符可能丢。 */
const PENDING_CONV_KEY = '__pending_first_send__'

function ensureConvState(id: string): ConvState {
  let s = convStates.get(id)
  if (!s) {
    s = { messages: [], draft: '', stream: null }
    convStates.set(id, s)
  }
  return s
}

const currentMessages = computed<Message[]>(() => {
  if (!conversationId.value) return []
  return convStates.get(conversationId.value)?.messages ?? []
})

/** 当前 view 中流式 assistant 的 messageId（用于 MessageList 的 ▌ 光标）。 */
const currentStreamingMessageId = computed<string | null>(() => {
  if (!conversationId.value) return null
  return convStates.get(conversationId.value)?.stream?.assistantId ?? null
})

/** 流式中的 conversation id 集合 —— sidebar lockedIds + 锁判断都看这个。
 *  prepare 期（stream 已 set 但 assistantId 还 null）也算 streaming。 */
const streamingConvIds = computed<Set<string>>(() => {
  const set = new Set<string>()
  convStates.forEach((s, id) => {
    if (s.stream !== null) set.add(id)
  })
  return set
})

/** 当前 view 的对话是否处于 in-flight（prepare 或 stream）—— 控制发送按钮置灰。 */
const isCurrentStreaming = computed(() => {
  if (!conversationId.value) return false
  return convStates.get(conversationId.value)?.stream != null
})

/** 取消按钮可见：当前 view 是流式对话 + 已进入 stream phase（assistantId 已设）。
 *  prepare 期（assistantId 还 null）后端没给 ID 没法 cancel，按钮也不显。 */
const canCancelHere = computed(() => {
  if (!conversationId.value) return false
  const s = convStates.get(conversationId.value)?.stream
  return s != null && s.assistantId !== null
})

/** 用户已点取消等后端收尾的 UI 中间态（ChatInput 据此把按钮文案改"取消中…"并 disable）。 */
const isCancellingHere = computed(() => {
  if (!conversationId.value) return false
  return convStates.get(conversationId.value)?.stream?.cancelling ?? false
})

/** v-model 双向绑定的 inputDraft：投射到当前 conv 的 draft 字段。
 *  无 active conv 时降级到 pendingDraft。 */
const inputDraft = computed<string>({
  get: () => {
    if (!conversationId.value) return pendingDraft.value
    return convStates.get(conversationId.value)?.draft ?? ''
  },
  set: (v) => {
    if (!conversationId.value) {
      pendingDraft.value = v
      return
    }
    const state = ensureConvState(conversationId.value)
    state.draft = v
    scheduleDraftPersist(conversationId.value, v)
  },
})

// === draft 持久化（debounced）===

const draftTimers = new Map<string, ReturnType<typeof setTimeout>>()

function scheduleDraftPersist(convId: string, draft: string) {
  const existing = draftTimers.get(convId)
  if (existing) clearTimeout(existing)
  const timer = setTimeout(() => {
    draftTimers.delete(convId)
    void setChatDraft(convId, draft).catch((e) => {
      console.warn('[ChatApp] setChatDraft failed:', e)
    })
  }, DRAFT_DEBOUNCE_MS)
  draftTimers.set(convId, timer)
}

/** 切换前 / 关窗前 强制 flush 一次未触发的 debounce，保证最后几个字也落库。 */
function flushDraftIfPending(convId: string) {
  const t = draftTimers.get(convId)
  if (!t) return
  clearTimeout(t)
  draftTimers.delete(convId)
  const state = convStates.get(convId)
  if (state) {
    void setChatDraft(convId, state.draft).catch((e) => {
      console.warn('[ChatApp] flushDraft failed:', e)
    })
  }
}

const unlistenFns: UnlistenFn[] = []

onMounted(async () => {
  // #30 Windows 11 transparency bug workaround：transparent:true + decorations:false 时
  // 首次绘制 webview 背景为白色，直到首次 resize 才变透明（Tauri #4881 / #10318 / #8308）。
  // 启动期主动 set_size(currentSize) 触发一次 redraw 规避。
  try {
    const w = getCurrentWindow()
    const sz = await w.outerSize()
    const scale = await w.scaleFactor()
    const logical = sz.toLogical(scale)
    await w.setSize(new LogicalSize(logical.width, logical.height))
  } catch (e) {
    console.warn('[ChatApp] transparency redraw workaround failed:', e)
  }

  // T10 (#31 follow-up B)：AOT 前端兜底 — chat 是 lazy webview 显示后才 mount，
  // 此时启动期 backend apply_initial_always_on_top 可能已跑过但 chat webview
  // 当时未 ready。主动读 KV 应用一次 + listen 后续切换。
  try {
    const raw = await getConfig('window:always_on_top')
    const v = raw === null ? true : raw === 'true'
    await getCurrentWindow().setAlwaysOnTop(v)
  } catch (e) {
    console.warn('[ChatApp] initial setAlwaysOnTop failed:', e)
  }
  try {
    const unlistenAot = await listen<boolean>('window:always-on-top:changed', async (ev) => {
      try {
        await getCurrentWindow().setAlwaysOnTop(ev.payload)
      } catch (e) {
        console.warn('[ChatApp] AOT changed listen apply failed:', e)
      }
    })
    unlistenFns.push(unlistenAot)
  } catch (e) {
    console.warn('[ChatApp] listen AOT changed failed:', e)
  }

  // store init：nickname / avatar store 是 MessageBubble 跨窗口共享依赖；chat 窗 mount 时
  // 主动 load + ensureListener，避免 MessageBubble 渲染时拿不到数据。
  try {
    if (!nicknameStore.loaded) await nicknameStore.load()
    await nicknameStore.ensureListener()
  } catch (e) {
    console.warn('[ChatApp] nickname store load failed:', e)
  }
  try {
    if (!avatarsStore.loaded) await avatarsStore.load()
    await avatarsStore.ensureListener()
  } catch (e) {
    console.warn('[ChatApp] avatars store load failed:', e)
  }

  // Phase B 人格 identity：随机挑一条 mood（每窗口生命周期固定一次）+ 加载当前人格名 +
  // listen persona:activated 跨窗口刷新（设置面板切换人格后立即同步 sidebar identity）。
  currentMood.value = MOODS[Math.floor(Math.random() * MOODS.length)]

  try {
    personaName.value = (await getActivePersona()).name
  } catch (e) {
    // #10：早先只 console.warn，header 永久空白用户不知发生了什么。
    // toast 引导去设置面板（典型场景：persona 表被外部清空 / DB 迁移失败）。
    console.warn('[ChatApp] getActivePersona failed:', e)
    toast.error('未能加载当前人格，请到设置面板检查或激活一个人格', { duration: 5000 })
  }

  try {
    const unPersona = await listen('persona:activated', async () => {
      try {
        personaName.value = (await getActivePersona()).name
      } catch (e) {
        console.warn('[ChatApp] refresh persona name failed:', e)
      }
    })
    unlistenFns.push(unPersona)
  } catch (e) {
    console.warn('[ChatApp] listen persona:activated failed:', e)
  }

  window.addEventListener('keydown', onGlobalKeydown)

  await refreshConversations()
  if (conversations.value.length > 0) {
    await switchConversation(conversations.value[0].id)
  }
})

onBeforeUnmount(() => {
  unlistenFns.forEach((u) => u())
  window.removeEventListener('keydown', onGlobalKeydown)
  // 关窗前 flush 所有未触发的 draft debounce（保证不丢最后几个字）
  draftTimers.forEach((_, id) => flushDraftIfPending(id))
})

function onGlobalKeydown(e: KeyboardEvent) {
  if (e.key !== 'Escape') return
  // #2 修复：ESC 不能"穿透"到全窗口隐藏——
  // - 弹窗在（ElMessageBox / ElDialog / ElOverlay portal）→ 让 EP 自己 cancel
  // - 重命名 input 聚焦 → 让 sidebar 的 cancelRename 处理
  // 早先版本不分场景一律 hideChat，导致"按 ESC 取消删除确认时连窗口一起藏"。
  if (document.querySelector('.el-message-box, .el-dialog__wrapper, .el-overlay')) return
  const active = document.activeElement
  if (active instanceof Element && active.closest('.conv-item__rename-input')) return
  void hideChat()
}

async function refreshConversations() {
  try {
    conversations.value = await listConversations(SIDEBAR_LIMIT)
  } catch (e) {
    console.warn('[ChatApp] listConversations failed:', e)
  }
}

/** #5 修复：switchConversation race 守护。用户快速点 A→B 时，A 和 B 的 IPC
 *  Promise.all 顺序不可控；如果 B 先回 A 后回，最终 conversationId 会落在 A
 *  （"先点的"），而用户预期是 B。给每次调用发号，过期请求直接吞掉即可。 */
let switchSeq = 0

async function switchConversation(id: string) {
  if (id === conversationId.value) return
  const mySeq = ++switchSeq

  // flush 当前 view 的 draft 再切
  if (conversationId.value) {
    flushDraftIfPending(conversationId.value)
  }

  try {
    // 之前没 cache 过的对话：从 DB 拉历史 + 草稿。
    // 已 cache（流式中切走又切回）：直接用 in-memory state，避免 reload 把流式中的 assistant
    // content 清回 DB 的 placeholder 空状态。
    const existing = convStates.get(id)
    if (!existing) {
      const [records, draftFromDb] = await Promise.all([
        loadChatHistory(id, HISTORY_LIMIT),
        getChatDraft(id),
      ])
      if (mySeq !== switchSeq) return // 我已过期，让最新的 switch win
      const state = ensureConvState(id)
      state.messages = records
      state.draft = draftFromDb ?? ''
    }
    // 已存在的 state：什么都不做（messages 内存已最新；assistant 流式正在写桶）
    if (mySeq !== switchSeq) return
    conversationId.value = id
    await setActiveConversation(id)
  } catch (e) {
    if (mySeq === switchSeq) toast.error(`切换会话失败：${msgOf(e)}`)
  }
}

async function onCreateConversation() {
  try {
    const newId = await createConversation()
    if (conversationId.value) flushDraftIfPending(conversationId.value)
    ensureConvState(newId)
    conversationId.value = newId
    await refreshConversations() // sidebar 立即包含新行（已被后端置顶）
  } catch (e) {
    toast.error(`新建会话失败：${msgOf(e)}`)
  }
}

async function fallbackAfterActiveGone() {
  if (conversations.value.length > 0) {
    await switchConversation(conversations.value[0].id)
  } else {
    conversationId.value = null
  }
}

async function onRenameConversation(payload: { id: string; title: string }) {
  // 流式中也允许 rename：title 写到 conversations 表，与 messages / prepare tx 互不读；
  // 不锁可以避免"prepare 期取消按钮还没显、想改名又被拦"的几百毫秒假死。
  // archive / delete 仍保留 lock，因为它们会让 prepare 内的 INSERT 撞 FK。
  try {
    await renameConversation(payload.id, payload.title)
    await refreshConversations()
  } catch (e) {
    toast.error(`重命名失败：${msgOf(e)}`)
  }
}

async function onArchiveConversation(id: string) {
  if (streamingConvIds.value.has(id)) {
    toast.warn('该对话流式中，请先取消再归档')
    return
  }
  const wasActive = id === conversationId.value
  try {
    await archiveConversation(id)
    convStates.delete(id) // 归档后从内存清出
    await refreshConversations()
    toast.success('已归档')
    if (wasActive) await fallbackAfterActiveGone()
  } catch (e) {
    toast.error(`归档失败：${msgOf(e)}`)
  }
}

async function onDeleteConversation(id: string) {
  if (streamingConvIds.value.has(id)) {
    toast.warn('该对话流式中，请先取消再删除')
    return
  }
  const target = conversations.value.find((c) => c.id === id)
  const label = target?.title?.trim() || '此对话'
  try {
    await ElMessageBox.confirm(`删除「${label}」及其所有消息？此操作不可撤销。`, '确认删除', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
      confirmButtonClass: 'el-button--danger',
    })
  } catch {
    // 用户点取消 / ESC → ElMessageBox reject；不报错不刷新
    return
  }
  const wasActive = id === conversationId.value
  try {
    await deleteConversation(id)
    convStates.delete(id) // 删除后从内存清出（draft KV 已被后端级联删）
    await refreshConversations()
    if (wasActive) await fallbackAfterActiveGone()
  } catch (e) {
    toast.error(`删除失败：${msgOf(e)}`)
  }
}

// === Stream 事件处理（按 convId 路由）===

function appendToMessage(convId: string, targetMsgId: string, token: string) {
  const state = convStates.get(convId)
  if (!state) return
  const idx = state.messages.findIndex((m) => m.id === targetMsgId)
  if (idx === -1) return
  state.messages[idx] = {
    ...state.messages[idx],
    content: state.messages[idx].content + token,
  }
}

function finalizeStream(convId: string, messageId: string, finishReason: string) {
  const state = convStates.get(convId)
  if (!state) return
  state.stream = null
  if (finishReason === 'offline_rule') {
    const idx = state.messages.findIndex((m) => m.id === messageId)
    if (idx !== -1) {
      state.messages[idx] = { ...state.messages[idx], mode: 'offline_rule' }
    }
  } else if (finishReason === 'cancelled') {
    // 后端语义：partial 为空 → DELETE DB 行；非空 → UPDATE mode='cancelled'。
    // 前端跟上：空 partial splice 删气泡（不留空泡），非空同步 mode 让 MessageBubble
    // 显「（已取消）」小标。两边视图与 DB 对齐。
    const idx = state.messages.findIndex((m) => m.id === messageId)
    if (idx !== -1) {
      const target = state.messages[idx]
      if (target.content === '') {
        state.messages.splice(idx, 1)
      } else {
        state.messages[idx] = { ...target, mode: 'cancelled' }
      }
    }
  }
  void refreshConversations()
}

function handleStreamError(
  convId: string,
  messageId: string,
  errorKind: string,
  errorMsg: string,
) {
  const state = convStates.get(convId)
  if (state) {
    const idx = state.messages.findIndex((m) => m.id === messageId)
    if (idx !== -1) {
      state.messages.splice(idx, 1)
    }
    state.stream = null
  }
  toast.error(errorHint(errorKind, errorMsg), { duration: 5000 })
  void refreshConversations()
}

/** 把后端错误 raw 字符串里的标准 OpenAI envelope `{"error":{"message":"..."}}` 抽出来；
 *  抽不到（非 JSON / 字段缺失）就截断到 200 字符。BadRequest / 默认分支用此函数避免
 *  长英文 + JSON 糊到 toast 里给中文用户看。 */
function extractFriendlyMessage(raw: string): string {
  const idx = raw.indexOf('{')
  if (idx !== -1) {
    try {
      const obj = JSON.parse(raw.slice(idx))
      const msg = obj?.error?.message
      if (typeof msg === 'string' && msg.length > 0) return msg
    } catch {
      /* fall through to truncate */
    }
  }
  return raw.length > 200 ? raw.slice(0, 200) + '…' : raw
}

function errorHint(kind: string, raw: string): string {
  // 所有错误一律 console.error 打 raw，方便开发者排错（toast 仅 friendly 文案）
  console.error('[ChatApp] stream error:', kind, raw)
  const friendly = extractFriendlyMessage(raw)
  switch (kind) {
    case 'AuthFailed':
      return 'API Key 错误或已失效，请到设置面板更新'
    case 'RateLimit':
      return '请求过于频繁，稍后再试'
    case 'BadRequest':
      return `请求被拒绝：${friendly}`
    case 'ParseError':
      return '响应解析失败（可能 base_url / model 配置不兼容；详见控制台）'
    default:
      return `出错了：${friendly}`
  }
}

// === 发送 / 取消 ===

async function handleSend() {
  // 当前 view 流式中 → 输入框可编辑但发送按钮已灰；二次防御
  if (isCurrentStreaming.value) return
  const draftRaw = inputDraft.value.trim()
  if (draftRaw.length === 0) return

  // 捕获 send 时的 conv id（可能为 null：首启 0 conversations）
  const sourceConvId = conversationId.value

  // #13 首启路径防御：sourceConvId === null 时下面 stream slot 不预占，
  // 用 firstSendInFlight 拦住二次进入；非 null 路径走 stream slot 预占（与下方一致）。
  if (sourceConvId === null && firstSendInFlight.value) return

  // 在 source conv 上预占 stream slot（标记 prepare 期）→ sidebar 立即转 spinner。
  // 首启路径（sourceConvId === null）改预占在 PENDING_CONV_KEY 上（#3 修复），让早期
  // channel 事件能落入 inflight buffer 而不是被丢弃。
  let stateAtSend: ConvState
  if (sourceConvId) {
    stateAtSend = ensureConvState(sourceConvId)
    if (stateAtSend.stream !== null) return // 双击防御：已经在 in-flight
    stateAtSend.stream = {
      assistantId: null,
      inflight: { tokens: [], done: null, error: null },
      cancelling: false,
    }
  } else {
    firstSendInFlight.value = true
    stateAtSend = ensureConvState(PENDING_CONV_KEY)
    stateAtSend.stream = {
      assistantId: null,
      inflight: { tokens: [], done: null, error: null },
      cancelling: false,
    }
  }

  // 清输入草稿；立即 flush 空字符串到 DB（避免 race 复读旧草稿）
  inputDraft.value = ''
  if (sourceConvId) flushDraftIfPending(sourceConvId)

  // 乐观 push user 气泡（仅当 sourceConvId 存在；首启路径 send resolve 后再 push 到 realConvId state）
  const nowIso = new Date().toISOString()
  const userTempId = `pending-user-${Date.now()}`
  if (sourceConvId !== null) {
    stateAtSend.messages.push({
      id: userTempId,
      conversation_id: sourceConvId,
      role: 'user',
      content: draftRaw,
      mode: 'online',
      created_at: nowIso,
    })
  }

  // === Channel 闭包路由 ===
  // routedConvId: 始终非 null —— 非首启路径 = sourceConvId；首启路径 = PENDING_CONV_KEY，
  //               sendChat resolve 后会改写为 realConvId。
  // routedAssistantId: assistant placeholder push 后设值；之前事件落 inflight buffer
  let routedConvId: string = sourceConvId ?? PENDING_CONV_KEY
  let routedAssistantId: string | null = null

  const channel = new Channel<StreamEvent>()
  channel.onmessage = (msg) => {
    const state = convStates.get(routedConvId)
    if (!state || !state.stream) return // conv 已被删 / stream 已被清；丢弃

    switch (msg.type) {
      case 'delta':
        if (routedAssistantId !== null) {
          appendToMessage(routedConvId, routedAssistantId, msg.token)
        } else if (state.stream.inflight) {
          state.stream.inflight.tokens.push(msg.token)
        }
        break
      case 'done':
        if (routedAssistantId !== null) {
          finalizeStream(routedConvId, routedAssistantId, msg.finishReason)
        } else if (state.stream.inflight) {
          state.stream.inflight.done = { finishReason: msg.finishReason }
        }
        break
      case 'error':
        if (routedAssistantId !== null) {
          handleStreamError(routedConvId, routedAssistantId, msg.errorKind, msg.message)
        } else if (state.stream.inflight) {
          state.stream.inflight.error = { errorKind: msg.errorKind, message: msg.message }
        }
        break
    }
  }

  try {
    const result = await sendChat(draftRaw, sourceConvId ?? undefined, channel)
    const realConvId = result.conversationId
    const isNewConv = realConvId !== sourceConvId

    // 首启路径：sourceConvId === null，backend 帮建了 realConvId；把 PENDING_CONV_KEY 上
    // 占位的 stream slot（含 inflight buffer 内已收到的早期 tokens / done / error）整体搬到
    // realConvId 的 state 上（#3 修复）。后续的 placeholder push + drain 跑在 realState 上时，
    // realState.stream 就是同一个 slot 引用，buffer 内容全部保留。
    if (sourceConvId === null) {
      const state = ensureConvState(realConvId)
      const pendingState = convStates.get(PENDING_CONV_KEY)
      if (pendingState?.stream) {
        state.stream = pendingState.stream
      } else {
        // 防御兜底：PENDING_CONV_KEY 在 prepare 阶段已 set；不会到这里
        state.stream = {
          assistantId: null,
          inflight: { tokens: [], done: null, error: null },
          cancelling: false,
        }
      }
    }

    routedConvId = realConvId
    const realState = convStates.get(realConvId)!

    // 回填 user message 的真实 ID（先前乐观 push 的 userTempId 替换）
    const userIdx = realState.messages.findIndex((m) => m.id === userTempId)
    if (userIdx !== -1) {
      realState.messages[userIdx] = {
        ...realState.messages[userIdx],
        id: result.userMessageId,
        conversation_id: realConvId,
      }
    } else if (sourceConvId === null) {
      // 首启路径之前没 push 过乐观 user；现在补
      realState.messages.push({
        id: result.userMessageId,
        conversation_id: realConvId,
        role: 'user',
        content: draftRaw,
        mode: 'online',
        created_at: nowIso,
      })
    }

    // push assistant placeholder + drain 早期 token / done / error
    const earlyTokens = realState.stream?.inflight?.tokens ?? []
    realState.messages.push({
      id: result.messageId,
      conversation_id: realConvId,
      role: 'assistant',
      content: earlyTokens.join(''),
      mode: 'online',
      created_at: new Date().toISOString(),
    })

    if (realState.stream) {
      realState.stream.assistantId = result.messageId
      const earlyDone = realState.stream.inflight?.done
      const earlyError = realState.stream.inflight?.error
      realState.stream.inflight = null // drain 完成
      routedAssistantId = result.messageId

      if (earlyError) {
        handleStreamError(realConvId, result.messageId, earlyError.errorKind, earlyError.message)
      } else if (earlyDone) {
        finalizeStream(realConvId, result.messageId, earlyDone.finishReason)
      }
    }

    // 首启路径同步 conversationId.value，并迁移 pendingDraft（#4 修复：用户在 await sendChat
    // 期间可能继续往输入框打字，conversationId 还是 null，输入会落到 pendingDraft；切到真
    // conv 之前先把 pendingDraft 搬到 realState.draft，否则 input 会瞬间显示空白）。
    if (sourceConvId === null) {
      const realState = ensureConvState(realConvId)
      if (pendingDraft.value !== '') {
        realState.draft = pendingDraft.value
        void setChatDraft(realConvId, pendingDraft.value).catch((e) => {
          console.warn('[ChatApp] migrate pendingDraft to realConvId failed:', e)
        })
      }
      convStates.delete(PENDING_CONV_KEY) // 清理占位 state
      conversationId.value = realConvId
      pendingDraft.value = ''
    }

    if (isNewConv) {
      await refreshConversations()
    }
  } catch (e) {
    // sendChat 抛 = 后端 prepare 阶段失败（active provider 缺失 / api_key 空 等）
    const userIdx = stateAtSend.messages.findIndex((m) => m.id === userTempId)
    if (userIdx !== -1) stateAtSend.messages.splice(userIdx, 1)
    stateAtSend.stream = null
    // #3 配套：首启路径占位在 PENDING_CONV_KEY 上，失败也要清掉，否则下一轮 send 会撞到旧 slot
    if (sourceConvId === null) {
      convStates.delete(PENDING_CONV_KEY)
    }
    // 复原输入草稿（让用户能改了重发）
    inputDraft.value = draftRaw
    toast.error(`发送失败：${msgOf(e)}`, { duration: 5000 })
    // #9 修复：ensure_active_conversation 可能在 tx 之前已创建一个空 conv 行；
    // 若 build_messages / build_provider 失败，DB 留下空 conv，但 sidebar 不会自动看到。
    // 这里补一次 refresh，保证侧边栏立刻同步（不需要重启窗口才看到）。
    void refreshConversations()
  } finally {
    // #13 首启路径：sendChat 成功 / 失败都要清 in-flight ref，否则下一次发送被永久拦
    if (sourceConvId === null) {
      firstSendInFlight.value = false
    }
  }
}

async function handleCancel() {
  // 取消的是当前 view 那个流的 assistantId（按 canCancelHere 守护，view ≠ streaming 时按钮已隐）
  if (!conversationId.value) return
  const slot = convStates.get(conversationId.value)?.stream
  if (!slot?.assistantId) return
  if (slot.cancelling) return // 双击防御
  slot.cancelling = true
  try {
    await cancelChat(slot.assistantId)
  } catch (e) {
    console.warn('[ChatApp] cancelChat failed:', e)
    // IPC 失败 → 复原 cancelling，让用户能再点；流可能仍在跑
    slot.cancelling = false
  }
  // 成功路径不复位 cancelling：等后端 done(cancelled) / error 抵达，
  // finalizeStream / handleStreamError 把 stream slot 整体置 null，cancelling 隐式消失。
}

async function handleHide() {
  await hideChat()
}

/** Phase B：sidebar 顶部 chrome 的 ≡ 折叠按钮触发；折叠时 sidebar 缩到 48px 窄列保留入口。 */
function toggleSidebar() {
  sidebarCollapsed.value = !sidebarCollapsed.value
}

function msgOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}
</script>

<template>
  <!-- Desktop Chat Window Architecture（布局 v2 — 抽屉式）：
       window-root → app-surface → app-body[sidebar + content-surface[content-header + messages + composer]] + SnapGhost
       - 全宽 titlebar-surface 删除（只剩 ✕ 浪费空间）；window chrome 三件套（≡ + identity + ✕）下沉到
         content-surface 顶部，仅占右半宽，sidebar 顶部回归纯净（创建对话 + 历史记录）。
       - identity 区（avatar + 人格名 + mood）挂 data-tauri-drag-region 补足整窗拖动入口。
       - sidebar 抽屉语义：collapsed = width:0 完全收起（toggle 入口在 content-header）。 -->
  <div class="window-root" :style="chatLeanStyle">
    <SnapGhost source-label="chat" />
    <div class="app-surface" :class="chatSnapPreviewClass" :style="chatSnapPreviewStyle">
      <div class="app-body">
        <ConversationSidebar
          :conversations="conversations"
          :active-id="conversationId"
          :locked-ids="streamingConvIds"
          :collapsed="sidebarCollapsed"
          @select="switchConversation"
          @create="onCreateConversation"
          @rename="onRenameConversation"
          @archive="onArchiveConversation"
          @delete="onDeleteConversation"
        />

        <main class="content-surface">
          <!-- content-header：48px。四段式：
               ≡ 抽屉开关 / identity (avatar + name + mood) / 胶囊拖动块 / ✕ 关闭。
               拖动入口：胶囊块是独立 data-tauri-drag-region 元素（flex:1 抢占剩余空间），
               中间有 pill 视觉提示告诉用户"这里能拖"。
               header 整体不挂 drag-region 是因为子元素（按钮/img/span）不继承该属性，
               早先做法只剩元素间缝隙能拖。 -->
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

            <!-- 胶囊拖动块：data-tauri-drag-region 挂在本元素上；flex:1 抢占剩余空间让拖动区域够大；
                 ::before 渲染一段灰色 pill 作为"拖动手柄"视觉提示。 -->
            <div
              class="content-header__drag-handle"
              data-tauri-drag-region
              title="拖动窗口"
              aria-label="拖动窗口"
            />

            <button
              class="content-header__close"
              title="关闭（进托盘）"
              aria-label="关闭"
              @click="handleHide"
            >✕</button>
          </header>

          <div class="message-scroll-surface">
            <MessageList
              :messages="currentMessages"
              :streaming-message-id="currentStreamingMessageId"
            />
          </div>
          <div class="floating-composer">
            <ChatInput
              v-model="inputDraft"
              :input-disabled="false"
              :send-disabled="isCurrentStreaming"
              :show-cancel="canCancelHere"
              :cancelling="isCancellingHere"
              @send="handleSend"
              @cancel="handleCancel"
            />
          </div>
        </main>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* === window-root（全透明缓冲层）===
   100% × 100% 占满 webview。padding 给 .app-surface 的 box-shadow-float 留显示空间，
   避免阴影被 webview 边界裁。Phase F transform 注入到本元素（最外层不影响内部 layout）。 */
.window-root {
  width: 100%;
  height: 100%;
  padding: 12px;
  box-sizing: border-box;
  background: transparent;
  transition: transform 160ms var(--aipet-ease-standard);
}

/* === app-surface（唯一实体层，L0 windowbg）===
   14px CSS 圆角 + overflow:hidden 把所有子内容裁成圆角。
   transparent 窗口 + 此元素 opaque → 圆角外为透明 webview → 桌面透出。
   这是"真"圆角窗的标准做法（Discord/Telegram/Arc）。
   shadow-float 提供浮起感；snap-preview modifier 在被拖目标时覆盖式注入边描线 + glow。 */
.app-surface {
  width: 100%;
  height: 100%;
  background: var(--aipet-color-bg);
  border-radius: 14px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: var(--aipet-shadow-float);
  transition: box-shadow 180ms var(--aipet-ease-standard);
}

/* Snap-preview state（拖 pet 接近 chat 时反馈）：
   覆盖式 box-shadow（含 2px primary 描边 + 24px primary glow + 浮起阴影）。
   transparent:true + window-root padding:12 留足空间不被裁。 */
.app-surface.snap-preview {
  box-shadow:
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 75%),
        transparent
      ),
    0 0 24px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 40%),
        transparent
      ),
    var(--aipet-shadow-float);
}
.app-surface.snap-preview--edge-right {
  box-shadow:
    inset -3px 0 22px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 60%),
        transparent
      ),
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 75%),
        transparent
      ),
    0 0 24px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 40%),
        transparent
      ),
    var(--aipet-shadow-float);
}
.app-surface.snap-preview--edge-left {
  box-shadow:
    inset 3px 0 22px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 60%),
        transparent
      ),
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 75%),
        transparent
      ),
    0 0 24px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 40%),
        transparent
      ),
    var(--aipet-shadow-float);
}
.app-surface.snap-preview--edge-top {
  box-shadow:
    inset 0 3px 22px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 60%),
        transparent
      ),
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 75%),
        transparent
      ),
    0 0 24px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 40%),
        transparent
      ),
    var(--aipet-shadow-float);
}
.app-surface.snap-preview--edge-bottom {
  box-shadow:
    inset 0 -3px 22px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 60%),
        transparent
      ),
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 75%),
        transparent
      ),
    0 0 24px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 40%),
        transparent
      ),
    var(--aipet-shadow-float);
}

/* === content-header（content-surface 顶部 48px window chrome）===
   布局 v2：删掉全宽 titlebar-surface 后，window chrome 三件套（≡ + identity + ✕）
   下沉到 content-surface 顶部，只占右半宽（sidebar 侧不再有 chrome 干扰）。
   整条 header 是 data-tauri-drag-region；左 ≡ / 右 ✕ 按钮挂 ="false" 反向声明保证点击。
   背景与 content-surface 一致（L2 surface），无 border-bottom 避免分隔感。 */
.content-header {
  flex: 0 0 48px;
  display: flex;
  align-items: center;
  background: var(--aipet-color-surface);
  user-select: none;
}

.content-header__toggle {
  flex: 0 0 48px;
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--aipet-color-text-2);
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
  padding: 0;
  margin: 0;
  transition: background-color 100ms ease, color 100ms ease;
}

.content-header__toggle:hover {
  background: color-mix(in srgb, var(--aipet-color-text-1) 8%, transparent);
  color: var(--aipet-color-text-1);
}

.content-header__toggle:active {
  background: color-mix(in srgb, var(--aipet-color-text-1) 14%, transparent);
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
  /* 跟 sidebar 旧版一致的轻微 hover 转动 */
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
  font-size: var(--aipet-font-size-sm);
  font-weight: 600;
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
  0%,
  100% {
    opacity: 0.55;
  }
  50% {
    opacity: 1;
  }
}

.content-header__close {
  flex: 0 0 auto;
  width: 46px;
  height: 48px;
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

/* === 胶囊拖动块 ===
   Tauri 的 data-tauri-drag-region 不被子元素继承，所以早先做法（header 上挂）只剩
   子元素之间的缝隙能拖。改用独立 drag-handle 元素：flex:1 吃掉 identity 与 ✕ 之间的
   剩余空间，本元素挂 data-tauri-drag-region；::before 渲染一段 pill 作为视觉提示。 */
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

/* === app-body（sidebar + content 水平容器）===
   占 app-surface 剩余高度。min-height:0 让内部子组件 overflow 工作。 */
.app-body {
  flex: 1 1 auto;
  display: flex;
  min-height: 0;
}

/* === content-surface（L2，消息区+composer 列容器）===
   surface 阶梯 L2：light=#fafafa / dark=#262626，比 sidebar(L1) 亮、比 bg(L0) 暗一档。
   relative positioning 给 floating-composer 提供定位上下文（C 阶段会用）。 */
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
   注意：本元素不再是滚动容器（早先双层 overflow 导致 scrollToBottom 操作内层 ul 但实际滚动条
   挂在本层 div 上 → 新消息被滚到视野下方"被遮掩"）。改为 flex column 容器，把滚动职责下放给
   内部 .chat-messages (ul) 唯一负责。
   dot-grid 背景仍保留在本层 = "桌面壁纸"不随消息滚动，符合桌面 IM 习惯。 */
.message-scroll-surface {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background-image: radial-gradient(
    circle at 1px 1px,
    color-mix(in srgb, var(--aipet-color-text-3) 25%, transparent) 1px,
    transparent 1px
  );
  background-size: 24px 24px;
  background-position: 0 0;
}

:global(html.dark) .message-scroll-surface {
  background-image: radial-gradient(
    circle at 1px 1px,
    color-mix(in srgb, var(--aipet-color-text-3) 30%, transparent) 1px,
    transparent 1px
  );
}

/* === floating-composer（Phase C 浮卡容器）===
   padding 给 ChatInput 浮卡四周留 breathing space：
   - 顶部 12px 让浮卡与 message-scroll 视觉拉开（上向阴影朝消息区柔和扩散）
   - 左右 16px 与 app-surface 14px 圆角呼应（避免浮卡贴边）
   - 底部 16px 让浮卡距 app-surface 圆角下边沿有呼吸 */
.floating-composer {
  flex: 0 0 auto;
  padding: var(--aipet-space-3) var(--aipet-space-4) var(--aipet-space-4);
}
</style>
