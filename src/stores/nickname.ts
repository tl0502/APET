// NicknameStore（M1，2026-05-09 重构后）：跨窗口共享 user_nickname 状态。
//
// 数据来源：
// - `nickname_get_user` 拉初值
// - `nickname:changed` event 推增量（service 层 set_user 自动 emit）
//
// 使用方：
// - 设置面板 NicknameForm：load + setUser + 转场开关 checkbox
// - 后续：拼 system prompt 时 ChatService 已直接走后端 nickname::get_user_nickname；store
//   主要为 UI 共享状态服务
//
// 已删除（2026-05-09）：
// - pet ref / setPet / restorePet / 'pet' event 分支
// - 宠物名字源唯一化为 .soul.md persona.name；chat 窗口标题改听 'persona:activated' 事件
//
// 2026-05-10：
// - listener 永久存活（Pinia singleton + 全应用共享），故移除 teardownListener；
//   早先版本暴露 teardown 但全应用无人调用，是死代码。
// - payload.which 收窄为 'user'、payload.value 收窄为 string（与后端 String 对齐）。
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getUserNickname, setUserNickname } from '@/services/nickname'
import type { NicknameChangedPayload } from '@/types/nickname'

export const useNicknameStore = defineStore('nickname', () => {
  const user = ref<string | null>(null)
  const loaded = ref(false)
  let unlisten: UnlistenFn | null = null

  async function load() {
    user.value = await getUserNickname()
    loaded.value = true
  }

  /** 启动期挂 nickname:changed listener；多次调安全（第二次起 no-op）。
   *  listener 跟随 Pinia singleton 全应用存活；无对应 teardown（M1 不需要）。 */
  async function ensureListener() {
    if (unlisten) return
    unlisten = await listen<NicknameChangedPayload>('nickname:changed', (e) => {
      // payload.which 类型已收窄为 'user'，理论分支唯一；保留显式 check 防御未来契约扩展。
      if (e.payload.which === 'user') {
        user.value = e.payload.value
      }
    })
  }

  async function setUser(name: string) {
    await setUserNickname(name)
    // service 层会 emit nickname:changed 同步本 store；此处不再手写 user.value=name
    // 避免 emit 顺序与 await 完成竞态导致脏值闪烁
  }

  return { user, loaded, load, ensureListener, setUser }
})
