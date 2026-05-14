// AvatarsStore — chat 窗共享的用户 / 当前 active persona 头像 URL（#25/#26）。
//
// 替换早先 useAvatars composable：每条 MessageBubble 跑一次 useAvatars 会
// 重复挂 listener + 重复 IPC，~100 条历史消息 × 2 = 200 次 KV 读 + 200 个 listener。
// Pinia singleton 让所有 MessageBubble / ChatApp header 共享同一份 state，
// 整窗口生命周期只挂一份 listener。
//
// 数据来源：
// - `memory_get` 拉 KV：`user:avatar_path` + `persona:<active_id>:avatar_path`
// - 监听 `avatar:changed` event（跨窗口，settings 那边触发也会刷这里）
// - 监听 `persona:activated` event（切换 persona 后重读对应 avatar_path）
//
// cacheBust：URL 上附 `?v=<n>` 防 webview 缓存同路径覆盖时不刷新 img。
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  AVATAR_CHANGED_EVENT,
  type AvatarChangedPayload,
  USER_AVATAR_KEY,
  personaAvatarKey,
} from '@/services/avatars'
import { getMemory } from '@/services/memory'
import { getActivePersona } from '@/services/persona'

export const useAvatarsStore = defineStore('avatars', () => {
  const userAvatarUrl = ref<string | null>(null)
  const personaAvatarUrl = ref<string | null>(null)
  const activePersonaId = ref<string | null>(null)
  const loaded = ref(false)
  let cacheBust = 0
  let unlistenAvatar: UnlistenFn | null = null
  let unlistenPersona: UnlistenFn | null = null

  /** 绝对路径 → asset URL + ?v=<n> 防缓存。 */
  function toAssetUrl(path: string): string {
    const base = convertFileSrc(path)
    cacheBust += 1
    const sep = base.includes('?') ? '&' : '?'
    return `${base}${sep}v=${cacheBust}`
  }

  async function refreshUser() {
    try {
      const p = await getMemory(USER_AVATAR_KEY)
      userAvatarUrl.value = p ? toAssetUrl(p) : null
    } catch (e) {
      console.warn('[avatars store] refreshUser failed:', e)
      userAvatarUrl.value = null
    }
  }

  async function refreshPersona() {
    const pid = activePersonaId.value
    if (!pid) {
      personaAvatarUrl.value = null
      return
    }
    try {
      const p = await getMemory(personaAvatarKey(pid))
      personaAvatarUrl.value = p ? toAssetUrl(p) : null
    } catch (e) {
      console.warn('[avatars store] refreshPersona failed:', e)
      personaAvatarUrl.value = null
    }
  }

  async function refreshActivePersonaId() {
    try {
      activePersonaId.value = (await getActivePersona()).id
    } catch (e) {
      console.warn('[avatars store] getActivePersona failed:', e)
      activePersonaId.value = null
    }
  }

  /** chat 窗 onMounted 调一次；二次调安全（loaded flag 拦）。 */
  async function load() {
    if (loaded.value) return
    await refreshActivePersonaId()
    await Promise.all([refreshUser(), refreshPersona()])
    loaded.value = true
  }

  /** 启动期挂 listener；多次调安全。listener 跟随 Pinia singleton 全应用存活。 */
  async function ensureListener() {
    if (unlistenAvatar && unlistenPersona) return
    if (!unlistenAvatar) {
      try {
        unlistenAvatar = await listen<AvatarChangedPayload>(AVATAR_CHANGED_EVENT, (e) => {
          const payload = e.payload
          if (payload.kind === 'user') {
            void refreshUser()
          } else if (payload.kind === 'persona') {
            // 只刷新当前 active persona 的；其他 persona 变了不影响当前 chat 视图
            if (payload.personaId === activePersonaId.value) {
              void refreshPersona()
            }
          }
        })
      } catch (e) {
        console.warn('[avatars store] listen avatar:changed failed:', e)
      }
    }
    if (!unlistenPersona) {
      try {
        unlistenPersona = await listen('persona:activated', async () => {
          await refreshActivePersonaId()
          await refreshPersona()
        })
      } catch (e) {
        console.warn('[avatars store] listen persona:activated failed:', e)
      }
    }
  }

  return { userAvatarUrl, personaAvatarUrl, activePersonaId, loaded, load, ensureListener }
})
