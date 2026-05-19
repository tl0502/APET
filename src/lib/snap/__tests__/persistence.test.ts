// S5 persistence 单测（loadPersistedConstraints / persistConstraints）。
//
// 注入 PersistenceDeps mock，覆盖：
// - 空 KV (null) → 0/0
// - 非 JSON → clear KV + 0/0
// - 非数组 → clear KV + 0/0
// - 有效条 + anchor in registry → loaded
// - schema v 不匹配 → dropped
// - anchor missing → dropped
// - persist 写 KV 内容正确

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { constraintStore } from '../constraintStore'
import {
  loadPersistedConstraints,
  persistConstraints,
  SCHEMA_V,
  SNAP_KV_KEY,
} from '../persistence'
import type { PersistedConstraint } from '../types'
import { windowRegistry } from '../windowRegistry'

const validItem = (overrides: Partial<PersistedConstraint> = {}): PersistedConstraint => ({
  sourceId: 'chat',
  targetId: 'pet',
  sourceEdge: 'left',
  targetEdge: 'right',
  offset: 0,
  enabled: true,
  createdAt: 1000,
  v: SCHEMA_V,
  ...overrides,
})

beforeEach(() => {
  constraintStore.clear()
  windowRegistry.clear()
})

describe('loadPersistedConstraints', () => {
  it('KV 为 null → 0/0', async () => {
    const getKv = vi.fn().mockResolvedValue(null)
    const setKv = vi.fn()
    const result = await loadPersistedConstraints({ getKv, setKv })
    expect(result).toEqual({ loaded: 0, dropped: 0 })
    expect(setKv).not.toHaveBeenCalled()
  })

  it('KV 为空串 → 0/0', async () => {
    const getKv = vi.fn().mockResolvedValue('')
    const setKv = vi.fn()
    expect(await loadPersistedConstraints({ getKv, setKv })).toEqual({ loaded: 0, dropped: 0 })
  })

  it('KV 非法 JSON → clear KV + 0/0', async () => {
    const getKv = vi.fn().mockResolvedValue('not-json')
    const setKv = vi.fn().mockResolvedValue(undefined)
    const result = await loadPersistedConstraints({ getKv, setKv })
    expect(result).toEqual({ loaded: 0, dropped: 0 })
    expect(setKv).toHaveBeenCalledWith(SNAP_KV_KEY, '[]')
  })

  it('JSON 非数组 → clear KV + 0/0', async () => {
    const getKv = vi.fn().mockResolvedValue('{"a":1}')
    const setKv = vi.fn().mockResolvedValue(undefined)
    const result = await loadPersistedConstraints({ getKv, setKv })
    expect(result).toEqual({ loaded: 0, dropped: 0 })
    expect(setKv).toHaveBeenCalledWith(SNAP_KV_KEY, '[]')
  })

  it('有效条 + anchor 在 registry → loaded=1', async () => {
    windowRegistry.upsert({ id: 'pet', rect: { x: 0, y: 0, w: 320, h: 320 }, visible: true })
    windowRegistry.upsert({ id: 'chat', rect: { x: 320, y: 0, w: 320, h: 320 }, visible: true })
    const getKv = vi.fn().mockResolvedValue(JSON.stringify([validItem()]))
    const setKv = vi.fn()
    const result = await loadPersistedConstraints({ getKv, setKv })
    expect(result).toEqual({ loaded: 1, dropped: 0 })
    expect(constraintStore.get('chat')?.targetId).toBe('pet')
  })

  it('schema v 不匹配 → dropped', async () => {
    windowRegistry.upsert({ id: 'pet', rect: { x: 0, y: 0, w: 320, h: 320 }, visible: true })
    const item = validItem({ v: 999 as unknown as 1 })
    const getKv = vi.fn().mockResolvedValue(JSON.stringify([item]))
    const setKv = vi.fn()
    const result = await loadPersistedConstraints({ getKv, setKv })
    expect(result).toEqual({ loaded: 0, dropped: 1 })
    expect(constraintStore.size()).toBe(0)
  })

  it('anchor missing in registry → dropped', async () => {
    // registry 不含 pet
    const getKv = vi.fn().mockResolvedValue(JSON.stringify([validItem()]))
    const setKv = vi.fn()
    const result = await loadPersistedConstraints({ getKv, setKv })
    expect(result).toEqual({ loaded: 0, dropped: 1 })
  })

  it('类型不匹配（缺字段 / wrong type）→ dropped', async () => {
    windowRegistry.upsert({ id: 'pet', rect: { x: 0, y: 0, w: 320, h: 320 }, visible: true })
    const broken = [
      { sourceId: 'a' }, // 缺字段
      { ...validItem(), sourceEdge: 'invalid' }, // 非法 edge
      { ...validItem(), offset: 'string' }, // 类型错
    ]
    const getKv = vi.fn().mockResolvedValue(JSON.stringify(broken))
    const setKv = vi.fn()
    const result = await loadPersistedConstraints({ getKv, setKv })
    expect(result.loaded).toBe(0)
    expect(result.dropped).toBe(3)
  })

  it('多条混合：1 valid + 1 dropped (anchor miss) + 1 dropped (schema v)', async () => {
    windowRegistry.upsert({ id: 'pet', rect: { x: 0, y: 0, w: 320, h: 320 }, visible: true })
    const items = [
      validItem(),
      validItem({ sourceId: 'settings', targetId: 'missing-anchor' }),
      validItem({ sourceId: 'tasks', v: 99 as unknown as 1 }),
    ]
    const getKv = vi.fn().mockResolvedValue(JSON.stringify(items))
    const setKv = vi.fn()
    const result = await loadPersistedConstraints({ getKv, setKv })
    expect(result).toEqual({ loaded: 1, dropped: 2 })
  })

  it('getKv 抛错 → 不阻塞，返 0/0', async () => {
    const getKv = vi.fn().mockRejectedValue(new Error('IPC down'))
    const setKv = vi.fn()
    const result = await loadPersistedConstraints({ getKv, setKv })
    expect(result).toEqual({ loaded: 0, dropped: 0 })
    expect(setKv).not.toHaveBeenCalled()
  })
})

describe('persistConstraints', () => {
  it('store 空 → 写 "[]"', async () => {
    const getKv = vi.fn()
    const setKv = vi.fn().mockResolvedValue(undefined)
    await persistConstraints({ getKv, setKv })
    expect(setKv).toHaveBeenCalledWith(SNAP_KV_KEY, '[]')
  })

  it('store 含 2 条 → 写 JSON 数组 w/ schema v', async () => {
    constraintStore.set({
      sourceId: 'chat',
      targetId: 'pet',
      sourceEdge: 'left',
      targetEdge: 'right',
      offset: 0,
      enabled: true,
      createdAt: 1000,
    })
    constraintStore.set({
      sourceId: 'settings',
      targetId: 'pet',
      sourceEdge: 'top',
      targetEdge: 'bottom',
      offset: 100,
      enabled: true,
      createdAt: 2000,
    })
    const getKv = vi.fn()
    const setKv = vi.fn().mockResolvedValue(undefined)
    await persistConstraints({ getKv, setKv })
    expect(setKv).toHaveBeenCalledTimes(1)
    const [key, value] = setKv.mock.calls[0]!
    expect(key).toBe(SNAP_KV_KEY)
    const parsed = JSON.parse(value)
    expect(parsed).toHaveLength(2)
    expect(parsed[0].v).toBe(SCHEMA_V)
    expect(parsed[1].v).toBe(SCHEMA_V)
  })

  it('setKv 抛错 → 不抛出（fire-and-log）', async () => {
    const getKv = vi.fn()
    const setKv = vi.fn().mockRejectedValue(new Error('IPC fail'))
    await expect(persistConstraints({ getKv, setKv })).resolves.toBeUndefined()
  })
})
