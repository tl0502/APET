// S4 internalMove guard 单测。
//
// 覆盖：markInternal 立即生效 / rAF 后释放 / 多窗独立 / clearInternal。

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { clearInternal, isInternalMove, markInternal } from '../internalMove'

beforeEach(() => {
  vi.useFakeTimers()
  clearInternal()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('internalMove guard', () => {
  it('markInternal 立即生效', () => {
    markInternal('pet')
    expect(isInternalMove('pet')).toBe(true)
  })

  it('未 mark 的窗 → false', () => {
    markInternal('pet')
    expect(isInternalMove('chat')).toBe(false)
  })

  it('rAF 后自动释放（jsdom 同步 rAF）', async () => {
    markInternal('pet')
    expect(isInternalMove('pet')).toBe(true)
    // jsdom rAF 通常异步触发；vi.useFakeTimers 应覆盖 rAF
    await vi.runAllTimersAsync()
    expect(isInternalMove('pet')).toBe(false)
  })

  it('多窗独立释放', async () => {
    markInternal('pet')
    markInternal('chat')
    expect(isInternalMove('pet')).toBe(true)
    expect(isInternalMove('chat')).toBe(true)
    await vi.runAllTimersAsync()
    expect(isInternalMove('pet')).toBe(false)
    expect(isInternalMove('chat')).toBe(false)
  })

  it('clearInternal 立即清空全部', () => {
    markInternal('pet')
    markInternal('chat')
    clearInternal()
    expect(isInternalMove('pet')).toBe(false)
    expect(isInternalMove('chat')).toBe(false)
  })
})
