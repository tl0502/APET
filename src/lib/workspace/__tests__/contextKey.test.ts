// ContextKeyService 单测（4 case）

import { beforeEach, describe, expect, it, vi } from 'vitest'

import { ContextKeyService } from '../contextKey'

let ctx: ContextKeyService
beforeEach(() => {
  ctx = new ContextKeyService()
})

describe('ContextKeyService', () => {
  it('set + get 单 key 往返', () => {
    ctx.set('a', 1)
    expect(ctx.get('a')).toBe(1)
    ctx.set('b', 'hello')
    expect(ctx.get('b')).toBe('hello')
    expect(ctx.get('nonexistent')).toBeUndefined()
    // asMap 返回当前快照（迭代 + 只读）
    const map = ctx.asMap()
    expect(map.get('a')).toBe(1)
    expect(map.get('b')).toBe('hello')
  })

  it('subscribe 单 key → set 触发 cb 一次；same value 不触发', () => {
    const cb = vi.fn()
    ctx.subscribe('a', cb)
    ctx.set('a', 1)
    expect(cb).toHaveBeenCalledTimes(1)
    expect(cb).toHaveBeenCalledWith('a')

    // same value（Object.is）不触发
    ctx.set('a', 1)
    expect(cb).toHaveBeenCalledTimes(1)

    // 变更触发
    ctx.set('a', 2)
    expect(cb).toHaveBeenCalledTimes(2)
  })

  it('subscribe 多 key → 任一变更触发；无关 key 不触发', () => {
    const cb = vi.fn()
    ctx.subscribe(['x', 'y'], cb)
    ctx.set('x', 1)
    expect(cb).toHaveBeenCalledTimes(1)
    expect(cb).toHaveBeenLastCalledWith('x')
    ctx.set('y', 'foo')
    expect(cb).toHaveBeenCalledTimes(2)
    expect(cb).toHaveBeenLastCalledWith('y')
    // 无关 key
    ctx.set('z', 999)
    expect(cb).toHaveBeenCalledTimes(2)
  })

  it('unsubscribe → 后续变更不触发', () => {
    const cb = vi.fn()
    const unsub = ctx.subscribe(['a', 'b'], cb)
    ctx.set('a', 1)
    expect(cb).toHaveBeenCalledTimes(1)

    unsub()
    ctx.set('a', 2)
    ctx.set('b', 3)
    expect(cb).toHaveBeenCalledTimes(1)
  })
})
