// userPopup store（#37 2026-05-21 重设计）：管理 in-workspace 用户 popup 的开关 + nav 选中。
//
// 设计：
// - 6 个 nav 项：profile / account / privacy / notifications / help / about
// - 其中 account / privacy / notifications 是 disabled 占位（spec §4.3）
// - 每次 open() 都从 profile 开始（不记忆 activeNav 跨次打开，spec §4.3）
// - setNav 自带 disabled 守卫，UI 层无需重复判断

import { defineStore } from 'pinia'
import { ref } from 'vue'

export type PopupNavId =
  | 'profile'
  | 'account'
  | 'privacy'
  | 'notifications'
  | 'help'
  | 'about'

const DISABLED_NAV_IDS: readonly PopupNavId[] = [
  'account',
  'privacy',
  'notifications',
] as const

export const useUserPopupStore = defineStore('userPopup', () => {
  const isOpen = ref(false)
  const activeNav = ref<PopupNavId>('profile')

  function open() {
    activeNav.value = 'profile' // 每次都重置（spec §4.3）
    isOpen.value = true
  }

  function close() {
    isOpen.value = false
  }

  function setNav(id: PopupNavId) {
    if (DISABLED_NAV_IDS.includes(id)) return
    activeNav.value = id
  }

  function isDisabled(id: PopupNavId): boolean {
    return DISABLED_NAV_IDS.includes(id)
  }

  return { isOpen, activeNav, open, close, setNav, isDisabled }
})
