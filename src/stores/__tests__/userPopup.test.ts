// userPopup store 单测 — 5 case
//
// 覆盖：open / close / setNav / 默认 activeNav / 重复 open 不抖

import { setActivePinia, createPinia } from 'pinia'
import { beforeEach, describe, expect, it } from 'vitest'

import { useUserPopupStore } from '../userPopup'

beforeEach(() => {
  setActivePinia(createPinia())
})

describe('userPopup store', () => {
  it('case 1: 默认 isOpen=false, activeNav="profile"', () => {
    const store = useUserPopupStore()
    expect(store.isOpen).toBe(false)
    expect(store.activeNav).toBe('profile')
  })

  it('case 2: open() 翻 isOpen + 默认进 profile', () => {
    const store = useUserPopupStore()
    store.setNav('about') // 模拟上次留在 about
    store.close()
    store.open()
    expect(store.isOpen).toBe(true)
    expect(store.activeNav).toBe('profile') // 每次重新进 profile（spec §4.3）
  })

  it('case 3: close() 翻 isOpen=false', () => {
    const store = useUserPopupStore()
    store.open()
    store.close()
    expect(store.isOpen).toBe(false)
  })

  it('case 4: setNav 切 nav 但不影响 isOpen', () => {
    const store = useUserPopupStore()
    store.open()
    store.setNav('help')
    expect(store.activeNav).toBe('help')
    expect(store.isOpen).toBe(true)
    store.setNav('about')
    expect(store.activeNav).toBe('about')
  })

  it('case 5: setNav 拒绝 disabled nav id（保持当前 activeNav 不变）', () => {
    const store = useUserPopupStore()
    store.open()
    store.setNav('profile')
    store.setNav('account') // disabled
    expect(store.activeNav).toBe('profile') // 不动
    store.setNav('privacy') // disabled
    expect(store.activeNav).toBe('profile')
    store.setNav('notifications') // disabled
    expect(store.activeNav).toBe('profile')
  })
})
