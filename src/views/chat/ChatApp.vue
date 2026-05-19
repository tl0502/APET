<script setup lang="ts">
// ChatApp：chat 窗口的 root（V3 多对话并发版）。
//
// V3 重构（2026-05-10，B13）—— 核心变化：
// - 状态模型：messages/draft/streaming 从单 ref 升级为 convStates: reactive Map<convId, ConvState>
//   每对话独立分桶；computed 投射当前 view（currentMessages / inputDraft / canCancelHere 等）
// - 多对话并发流式（ChatGPT 风格）：任意对话可同时在跑 stream；切换 view 不影响后台流
// - Channel 路由：每次 handleSend 创建的 channel.onmessage 闭包捕获 routedConvId/AssistantId，
//   事件直接写到对应桶；切走视图也不丢 token
// - draft 持久化：debounce 200ms 写到 config 表 KV chat:draft:<convId>，关窗保留
// - sidebar lockedIds: 任意有 in-flight 流的 convId 显示 spinner + 禁 rename/archive/delete
// - 输入框永远可编辑（即使 current view 流式中也可打草稿）；发送按钮在 current 流式中时灰
// - 取消按钮在 current = streaming 时可见；切走自动隐藏
//
// 内存：convStates 长期看会膨胀（访问过的对话都驻留消息）。M1 用户量小不是问题；
// M3 加 LRU 即可（保留最近 N 个对话的 in-memory state，其他切回时 reload）。
//
// 后端 0 改动：active_streams 是 HashMap、run_stream 是 detached spawn、每个 chat_send
// 自带 channel/conn —— 后端早已支持并发，只是前端单状态模型把它锁死了。

import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { ElButton, ElIcon, ElMessageBox } from 'element-plus'
import { Expand, Fold } from '@element-plus/icons-vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import { LogicalSize, getCurrentWindow } from '@tauri-apps/api/window'
import AppShell from '@/components/layouts/AppShell.vue'
import ChatInput from '@/components/chat/ChatInput.vue'
import ConversationSidebar from '@/components/chat/ConversationSidebar.vue'
import MessageList from '@/components/chat/MessageList.vue'
import { useToast } from '@/composables/useToast'
import { useSnapWindow } from '@/composables/useSnapWindow'
import { getConfig } from '@/services/config'
import SnapGhost from '@/components/SnapGhost.vue'
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
// T2a (#31)：拿 isPreviewAnchor 给 .chat-panel 套 .snap-preview class（拖 pet 接近 chat 时高亮）。
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

// Phase F (#31 follow-up C)：self-lean transform 应用到外层 .chat-window
// 不影响内层 .chat-panel 的 border-radius / box-shadow（plan §8 风险 #6）。
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

const personaName = ref<string>('')
const conversations = ref<ConversationSummary[]>([])
const conversationId = ref<string | null>(null)
/** P0 美化:会话栏折叠状态。默认展开,☰ 按钮切换。 */
const sidebarCollapsed = ref(false)
/** Header momo SVG 加载失败:img onError 翻 true,降级到首字母占位圆。
 *  注:persona 自定义头像（avatarsStore.personaAvatarUrl）失败由独立 personaImgFailed 标记，
 *  让"自定义 PNG 失败 → 退回 momo SVG → 再失败 → 退回 'M'"三层降级各自独立。
 *  L5 修复：URL 变化时复位 failed flag（用户重新导出后新 URL 应重试，不被旧 fail 卡住）。 */
const avatarFailed = ref(false)
const personaImgFailed = ref(false)

watch(
  () => avatarsStore.personaAvatarUrl,
  () => {
    personaImgFailed.value = false
  },
)
/** Header 副标题 micro-copy:替换通用"在线"文字。
 *  保持中性灰风格,无 emoji,语气拟人但克制。挂载时随机选一条(每窗口生命周期固定),
 *  桌宠"真正状态机"接入前的占位实现。 */
const MOODS = ['等你来', '刚刚醒了', '在听呢', '想你了', '安静等着', '随你说', '今天还好吗'] as const
const currentMood = ref<string>(MOODS[0])
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

  // 随机挑一条 mood(每窗口生命周期固定一次)
  currentMood.value = MOODS[Math.floor(Math.random() * MOODS.length)]

  try {
    personaName.value = (await getActivePersona()).name
  } catch (e) {
    // #10：早先只 console.warn，header 永久空白用户不知发生了什么。
    // toast 引导去设置面板（典型场景：persona 表被外部清空 / DB 迁移失败）。
    console.warn('[ChatApp] getActivePersona failed:', e)
    toast.error('未能加载当前人格，请到设置面板检查或激活一个人格', { duration: 5000 })
  }

  // P0 美化:载入用户昵称,user 气泡头像首字符依赖。失败 toast.error 已由 store 透出,本处兜底 warn。
  try {
    if (!nicknameStore.loaded) await nicknameStore.load()
    await nicknameStore.ensureListener()
  } catch (e) {
    console.warn('[ChatApp] nickname store load failed:', e)
  }

  // #25/#26 头像 store：拉 KV + 挂 avatar:changed / persona:activated 跨窗口 listener。
  // 失败仅 warn 不阻断；MessageBubble / header 会自动 fallback 到 momo SVG / 昵称首字符。
  try {
    if (!avatarsStore.loaded) await avatarsStore.load()
    await avatarsStore.ensureListener()
  } catch (e) {
    console.warn('[ChatApp] avatars store load failed:', e)
  }

  const unPersona = await listen('persona:activated', async () => {
    try {
      personaName.value = (await getActivePersona()).name
    } catch (e) {
      console.warn('[ChatApp] refresh persona name failed:', e)
    }
  })
  unlistenFns.push(unPersona)

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

/** 标题栏 ─ 按钮：最小化到任务栏（与 hideChat 的"进托盘"区分）。
 *  依赖 tauri.conf.json chat 窗口 skipTaskbar:false；否则 minimize 后窗口会找不回。 */
async function onMinimize() {
  try {
    await getCurrentWindow().minimize()
  } catch (e) {
    console.warn('[ChatApp] minimize failed:', e)
  }
}

function toggleSidebar() {
  sidebarCollapsed.value = !sidebarCollapsed.value
}

function msgOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}
</script>

<template>
  <!-- #30 磁吸：chat 现走 transparent:false + decorations:false 路径（与 pomodoro 一致）：
       Win11 OS 自动提供 ~8px 系统圆角，CSS .chat-panel border-radius:8px 与之对齐做内部裁剪。
       T2a + T7 (#31 follow-up B)：preview anchor 命中时 .chat-panel 加 .snap-preview class
       + 沿对接边 modifier 显示矩形 glow（覆盖式 box-shadow，外向部分会被 OS 窗口边界剪掉）。
       Phase F (#31 follow-up C)：.chat-window 接 chatLeanStyle transform 实现 self-lean
       （朝 pet 方向 ≤3px 微偏），不影响 .chat-panel layout。 -->
  <div class="chat-window" :style="chatLeanStyle">
    <!-- Phase B (#31 follow-up C)：Ghost slot 提示。preview 状态时显示"松手会落这"
         outline，相对 chat 当前位置偏移 ≤ 60px（webview 外被裁，可见偏移传达落点方向）。 -->
    <SnapGhost source-label="chat" />
    <div class="chat-panel" :class="chatSnapPreviewClass" :style="chatSnapPreviewStyle">
      <!-- Windows 风格窗口标题栏（28px 极简）：只保留 ─ 最小化 + ✕ 关闭。
           整片 data-tauri-drag-region（左侧大片空白即拖动区）；两个按钮 false 排除。
           ─ 调 minimize（skipTaskbar:false 后走标准 Win 任务栏路径）；
           ✕ 走 handleHide → 现有 hideChat IPC（进托盘，与之前一致）。 -->
      <div class="chat-titlebar" data-tauri-drag-region>
        <div class="chat-titlebar__sysbtns">
          <button
            class="chat-titlebar__btn chat-titlebar__btn--min"
            title="最小化"
            aria-label="最小化"
            data-tauri-drag-region="false"
            @click="onMinimize"
          >─</button>
          <button
            class="chat-titlebar__btn chat-titlebar__btn--close"
            title="关闭（进托盘）"
            aria-label="关闭"
            data-tauri-drag-region="false"
            @click="handleHide"
          >✕</button>
        </div>
      </div>
      <div class="chat-root">
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

    <div class="chat-main">
      <AppShell variant="standalone">
        <template #header>
          <ElButton
            link
            class="chat-header__sidebar-toggle"
            :title="sidebarCollapsed ? '展开会话栏' : '收起会话栏'"
            :aria-label="sidebarCollapsed ? '展开会话栏' : '收起会话栏'"
            data-tauri-drag-region="false"
            @click="toggleSidebar"
          >
            <ElIcon>
              <Expand v-if="sidebarCollapsed" />
              <Fold v-else />
            </ElIcon>
          </ElButton>

          <div class="chat-header__identity">
            <div class="chat-header__avatar" aria-hidden="true">
              <!-- 三层降级：persona 自定义 PNG（#26 导出）→ momo SVG → 'M' 字符
                   onError 区分两层 flag，让"PNG 失败回退到 momo SVG"独立于"momo SVG 失败回退到字符" -->
              <img
                v-if="avatarsStore.personaAvatarUrl && !personaImgFailed"
                :src="avatarsStore.personaAvatarUrl"
                alt=""
                class="chat-header__avatar-img"
                @error="personaImgFailed = true"
              />
              <img
                v-else-if="!avatarFailed"
                src="/avatar/momo-avatar.svg"
                alt=""
                class="chat-header__avatar-img"
                @error="avatarFailed = true"
              />
              <span v-else class="chat-header__avatar-fallback">M</span>
            </div>
            <div class="chat-header__name-wrap">
              <span v-if="personaName" class="chat-header__title">{{ personaName }}</span>
              <span v-else class="chat-header__title chat-header__title--placeholder" />
              <span class="chat-header__status">
                <span class="chat-header__status-dot" aria-hidden="true" />
                {{ currentMood }}
              </span>
            </div>
          </div>
        </template>

        <MessageList :messages="currentMessages" :streaming-message-id="currentStreamingMessageId" />

        <template #footer>
          <ChatInput
            v-model="inputDraft"
            :input-disabled="false"
            :send-disabled="isCurrentStreaming"
            :show-cancel="canCancelHere"
            :cancelling="isCancellingHere"
            @send="handleSend"
            @cancel="handleCancel"
          />
        </template>
      </AppShell>
    </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* #30 chat-window：padding=0 让 chat-panel 占满 webview。
   transparent:false 切换后由 OS 提供 ~8px 圆角，外层不需再为 box-shadow 留 padding。
   Phase F (#31 follow-up C)：transform 由 :style 注入（chatLeanStyle 朝 pet 方向 ≤3px 微偏）。
   160ms ease 过渡让 lean 出入平滑，符合 plan §2 L2 节奏。 */
.chat-window {
  width: 100%;
  height: 100%;
  padding: 0;
  box-sizing: border-box;
  background: transparent;
  transition: transform 160ms var(--aipet-ease-standard, ease-out);
}

/* #30 内层 opaque 面板：圆角由 Win11 OS 提供（transparent:false + decorations:false 时
   DWMWA_WINDOW_CORNER_PREFERENCE 默认走 ROUND ≈ 8px，与 pomodoro 独立窗一致）。
   - CSS border-radius 设 8px 与 OS 对齐：仅用于裁剪内部子元素（chat-titlebar 顶角、
     ConversationSidebar 左下、ChatInput 右下），与 OS 圆角不冲突。
   - 不画 hairline / drop shadow：OS 圆角已自带系统级边界。
   - snap-preview modifier 仍保留 outset box-shadow（transparent:false 后会被 OS 窗口
     边界剪掉外向部分，inset 部分正常工作；后续可改 inset-only 优化，本次不动）。 */
.chat-panel {
  width: 100%;
  height: 100%;
  background: var(--aipet-color-bg, #ffffff);
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  transition: box-shadow 180ms var(--aipet-ease-standard, ease-out);
}

/* T2a + T7 (#31 follow-up B)：chat 是 preview anchor 时矩形 outline + 沿对接边内向 glow。
   .chat-panel 有 overflow:hidden + 8px 圆角；多层 box-shadow（spread 0 0 0 2px + 弥散 24px）
   模拟 outline，但 transparent:false 切换后外向部分会被 OS 窗口边界剪掉（inset glow 不受影响）；
   --snap-preview-intensity ∈ [0.25,1] 控制。 */
.chat-panel.snap-preview {
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
    0 8px 32px -8px rgba(0, 0, 0, 0.3),
    0 2px 8px -2px rgba(0, 0, 0, 0.15);
}

/* Phase A (#31 follow-up C)：chat 作为 field anchor 时显示渐进 halo。
   仅在 distance ∈ (60, 120] 段出现；进入 < 60px (preview) 时 .snap-preview 会覆盖此样式。
   1px subtle outline + 弥散 32px 主色外光晕营造"磁场氛围"。 */
.chat-panel.snap-field-anchor:not(.snap-preview) {
  box-shadow:
    0 0 0 1px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-field-intensity, 0) * 25%),
        transparent
      ),
    0 0 32px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-field-intensity, 0) * 22%),
        transparent
      ),
    0 8px 32px -8px rgba(0, 0, 0, 0.25),
    0 2px 8px -2px rgba(0, 0, 0, 0.1);
}
.chat-panel.snap-preview--edge-right {
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
    0 8px 32px -8px rgba(0, 0, 0, 0.3);
}
.chat-panel.snap-preview--edge-left {
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
    0 8px 32px -8px rgba(0, 0, 0, 0.3);
}
.chat-panel.snap-preview--edge-top {
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
    0 8px 32px -8px rgba(0, 0, 0, 0.3);
}
.chat-panel.snap-preview--edge-bottom {
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
    0 8px 32px -8px rgba(0, 0, 0, 0.3);
}

/* === 窗口标题栏（28px 极简，Win11 / VSCode 风格）===
   只保留 ─ 最小化 + ✕ 关闭；左侧大片空白即拖动区。
   - 整片 data-tauri-drag-region；两个 button 标记 false 排除拖动。
   - 不画自己的背景色 / border：避免在 chat-panel 8px 圆角顶部叠出"独立矩形色块"
     破坏圆角观感。caption button 直接浮在 chat-panel bg 上（Win11/VSCode 主流做法）。
   - 系统按钮 46×28：宽度沿用 Win 标准；hover 浅灰 / close hover 红 #c42b1c。
   - chat-panel 本身 overflow:hidden + 8px border-radius 自动裁好标题栏顶角。 */
.chat-titlebar {
  flex: 0 0 28px;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  user-select: none;
}

.chat-titlebar__sysbtns {
  display: flex;
  height: 100%;
}

.chat-titlebar__btn {
  width: 46px;
  height: 28px;
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

.chat-titlebar__btn:hover {
  background: color-mix(in srgb, var(--aipet-color-text-1) 10%, transparent);
}

.chat-titlebar__btn:active {
  background: color-mix(in srgb, var(--aipet-color-text-1) 18%, transparent);
}

/* Win11 标准 close-button red：hover 红底白图标，深色模式同色（Win 本身不分主题）。 */
.chat-titlebar__btn--close:hover {
  background: #c42b1c;
  color: #ffffff;
}

.chat-titlebar__btn--close:active {
  background: #a01e15;
  color: #ffffff;
}

.chat-root {
  display: flex;
  width: 100%;
  /* 上层多了 28px .chat-titlebar，本元素改 flex 拉伸（min-height:0 让子元素 overflow 工作）。
     原 height:100% 在 chat-panel(flex column) 下会撑出 overflow 把 titlebar 顶飞。 */
  flex: 1 1 auto;
  min-height: 0;
}

/* AppShell 在独立窗口（settings/tasks）下用 min-height:100vh 兜底，但 chat 是嵌在
   chat-panel 内的（chat-window padding:0 → chat-panel 占满 webview → 内部高度 =
   viewport - 28px titlebar），100vh 会撑爆把 footer 顶出 overflow:hidden 之外。
   chat 局部撤销该约束并改 flex 拉伸。 */
.chat-panel :deep(.aipet-shell--standalone) {
  min-height: 0;
  flex: 1 1 auto;
  height: 100%;
}

.chat-main {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

/* === Header（人格信息栏，44px）=== */
/* AppShell 的 header 现在是"人格信息栏"职责：≡ + 头像 + 人格名 + mood。
   窗口控制（拖动 / 最小化 / 关闭）已迁到上层 .chat-titlebar，本层不再承担。 */
.chat-header__sidebar-toggle {
  flex: 0 0 auto;
  width: 36px;
  height: 36px;
  padding: 0;
  color: var(--aipet-color-text-2);
  border: 1px solid transparent;
  border-radius: var(--aipet-radius-base);
  transition: color var(--aipet-duration-fast) var(--aipet-ease-standard),
    background-color var(--aipet-duration-fast) var(--aipet-ease-standard),
    border-color var(--aipet-duration-fast) var(--aipet-ease-standard),
    transform var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.chat-header__sidebar-toggle :deep(.el-icon) {
  font-size: 18px;
}

.chat-header__sidebar-toggle:hover {
  color: var(--aipet-color-primary);
  background: color-mix(in srgb, var(--aipet-color-primary) 12%, transparent);
  border-color: color-mix(in srgb, var(--aipet-color-primary) 35%, transparent);
  transform: scale(1.08);
}

.chat-header__sidebar-toggle:active {
  transform: scale(0.96);
}

.chat-header__identity {
  flex: 1 1 auto;
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
  min-width: 0;
  margin-left: var(--aipet-space-2);
}

.chat-header__avatar {
  flex: 0 0 auto;
  width: 36px;
  height: 36px;
  border-radius: 50%;
  overflow: hidden;
  /* 用 surface-soft 让头像在 header 上浮起一档(亮:#f5f5f5 / 暗:#1c1c1c) */
  background: var(--aipet-color-surface-soft);
  border: 1px solid var(--aipet-color-border);
  display: flex;
  align-items: center;
  justify-content: center;
  /* 桌宠头像:hover 时 4 度倾斜 + 微放大,给"她看了你一眼"的反应 */
  transition: transform 0.6s var(--aipet-ease-emphasized);
  cursor: default;
}

.chat-header__avatar:hover {
  transform: rotate(4deg) scale(1.04);
}

.chat-header__avatar-img {
  width: 100%;
  height: 100%;
  display: block;
}

.chat-header__avatar-fallback {
  font-size: 16px;
  font-weight: 600;
  color: var(--aipet-color-primary);
}

.chat-header__name-wrap {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 1px;
  min-width: 0;
}

.chat-header__title {
  font-size: var(--aipet-font-size-base);
  font-weight: 600;
  color: var(--aipet-color-text-1);
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* personaName 异步加载期间占位（保持 header 高度稳定，不出现晃动） */
.chat-header__title--placeholder {
  min-height: 1em;
}

.chat-header__status {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
  line-height: 1;
}

.chat-header__status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--aipet-color-online);
  /* 慢呼吸(1.5s),呼应流式光标节奏;桌宠"她在的"暗示。 */
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
</style>
