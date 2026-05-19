// S3 solver 单测（ADR-020 *Updated*）。
//
// 覆盖：空 input / 单链 / 分叉 / 多 root / !visible 跳过 / 缺 anchor 跳过 /
//      enabled=false 跳过 / BFS 内部 newRects 消费正确性（链条第二跳用第一跳的新位置）。

import { beforeEach, describe, expect, it } from 'vitest'
import { ConstraintStore } from '../constraintStore'
import { solve } from '../solver'
import type { Rect, SnapConstraint } from '../types'
import { WindowRegistry } from '../windowRegistry'

const r = (x: number, y: number, w: number, h: number): Rect => ({ x, y, w, h })

const c = (
  sourceId: string,
  targetId: string,
  overrides: Partial<SnapConstraint> = {},
): SnapConstraint => ({
  sourceId,
  targetId,
  sourceEdge: 'left',
  targetEdge: 'right',
  offset: 0,
  enabled: true,
  createdAt: 0,
  ...overrides,
})

let registry: WindowRegistry
let store: ConstraintStore

beforeEach(() => {
  registry = new WindowRegistry()
  store = new ConstraintStore()
})

describe('solve — degenerate', () => {
  it('空 changedRoots → 空 map', () => {
    expect(solve([], { registry, store }).size).toBe(0)
  })

  it('无 dependent (constraintStore 空) → 空 map', () => {
    registry.upsert({ id: 'pet', rect: r(0, 0, 320, 320), visible: true })
    expect(solve(['pet'], { registry, store }).size).toBe(0)
  })

  it('changedRoots 内的 id 不出现在结果中', () => {
    registry.upsert({ id: 'pet', rect: r(0, 0, 320, 320), visible: true })
    registry.upsert({ id: 'chat', rect: r(320, 0, 320, 320), visible: true })
    store.set(c('chat', 'pet'))
    const result = solve(['pet'], { registry, store })
    expect(result.has('pet')).toBe(false)
    expect(result.has('chat')).toBe(true)
  })
})

describe('solve — 单链', () => {
  it('A → B → C：solve([A]) 推 B 和 C', () => {
    // A 在原点 320×320；B 贴 A 右边；C 贴 B 右边
    registry.upsert({ id: 'A', rect: r(0, 0, 320, 320), visible: true })
    registry.upsert({ id: 'B', rect: r(320, 0, 320, 320), visible: true })
    registry.upsert({ id: 'C', rect: r(640, 0, 320, 320), visible: true })
    store.set(c('B', 'A')) // B 贴 A 右边 (B.left ↔ A.right, offset 0)
    store.set(c('C', 'B')) // C 贴 B 右边

    // 模拟 A 被拖到 (100, 50)
    registry.updateRect('A', r(100, 50, 320, 320))
    const result = solve(['A'], { registry, store })

    expect(result.get('B')).toEqual(r(420, 50, 320, 320))
    expect(result.get('C')).toEqual(r(740, 50, 320, 320)) // C 用 B 的"新"位置算
  })
})

describe('solve — 分叉', () => {
  it('A 是公共 anchor，B 和 C 都依赖 A', () => {
    registry.upsert({ id: 'A', rect: r(0, 0, 320, 320), visible: true })
    registry.upsert({ id: 'B', rect: r(320, 0, 320, 320), visible: true })
    registry.upsert({ id: 'C', rect: r(0, 320, 320, 320), visible: true })
    store.set(c('B', 'A', { sourceEdge: 'left', targetEdge: 'right', offset: 0 })) // B 贴 A 右
    store.set(c('C', 'A', { sourceEdge: 'top', targetEdge: 'bottom', offset: 0 })) // C 贴 A 下

    registry.updateRect('A', r(100, 50, 320, 320))
    const result = solve(['A'], { registry, store })

    expect(result.get('B')).toEqual(r(420, 50, 320, 320))
    expect(result.get('C')).toEqual(r(100, 370, 320, 320))
  })
})

describe('solve — 多 root', () => {
  it('两条独立链同时被推（changedRoots 含两个 root）', () => {
    registry.upsert({ id: 'A', rect: r(0, 0, 320, 320), visible: true })
    registry.upsert({ id: 'B', rect: r(320, 0, 320, 320), visible: true })
    registry.upsert({ id: 'X', rect: r(1000, 1000, 320, 320), visible: true })
    registry.upsert({ id: 'Y', rect: r(1320, 1000, 320, 320), visible: true })
    store.set(c('B', 'A'))
    store.set(c('Y', 'X'))

    registry.updateRect('A', r(50, 60, 320, 320))
    registry.updateRect('X', r(2000, 2000, 320, 320))
    const result = solve(['A', 'X'], { registry, store })

    expect(result.get('B')).toEqual(r(370, 60, 320, 320))
    expect(result.get('Y')).toEqual(r(2320, 2000, 320, 320))
  })
})

describe('solve — 跳过路径', () => {
  it('!visible source 不被推', () => {
    registry.upsert({ id: 'A', rect: r(0, 0, 320, 320), visible: true })
    registry.upsert({ id: 'B', rect: r(320, 0, 320, 320), visible: false })
    store.set(c('B', 'A'))
    registry.updateRect('A', r(100, 0, 320, 320))
    expect(solve(['A'], { registry, store }).has('B')).toBe(false)
  })

  it('!enabled constraint 跳过', () => {
    registry.upsert({ id: 'A', rect: r(0, 0, 320, 320), visible: true })
    registry.upsert({ id: 'B', rect: r(320, 0, 320, 320), visible: true })
    store.set(c('B', 'A', { enabled: false }))
    registry.updateRect('A', r(100, 0, 320, 320))
    expect(solve(['A'], { registry, store }).has('B')).toBe(false)
  })

  it('anchor missing in registry → 跳过', () => {
    // A 在 store 是 B 的 target，但 registry 不含 A
    registry.upsert({ id: 'B', rect: r(320, 0, 320, 320), visible: true })
    store.set(c('B', 'A'))
    // 即使 A 在 changedRoots 中，没 anchor rect → 跳过
    expect(solve(['A'], { registry, store }).has('B')).toBe(false)
  })

  it('source missing in registry → 跳过', () => {
    registry.upsert({ id: 'A', rect: r(0, 0, 320, 320), visible: true })
    store.set(c('B', 'A')) // B 不在 registry
    expect(solve(['A'], { registry, store }).has('B')).toBe(false)
  })
})

describe('solve — BFS 中链条传播正确性', () => {
  it('A → B → C, solve([A])：C 的 anchor (B) 取 newRects 里的"新 B"而非旧 registry B', () => {
    // 故意让 registry 里 B 的位置"过时"，新位置应通过 newRects 传播
    registry.upsert({ id: 'A', rect: r(0, 0, 320, 320), visible: true })
    registry.upsert({ id: 'B', rect: r(9999, 9999, 320, 320), visible: true }) // 过时位置
    registry.upsert({ id: 'C', rect: r(8888, 8888, 320, 320), visible: true })
    store.set(c('B', 'A'))
    store.set(c('C', 'B'))

    registry.updateRect('A', r(100, 0, 320, 320))
    const result = solve(['A'], { registry, store })

    // B 新位置应 = A + 320 = 420
    expect(result.get('B')).toEqual(r(420, 0, 320, 320))
    // C 新位置应基于 B 的新位置 (420)，不是 registry 里旧 9999
    expect(result.get('C')).toEqual(r(740, 0, 320, 320))
  })
})

describe('solve — 不修改输入 registry', () => {
  it('solver 不写 registry，只返回 newRects', () => {
    registry.upsert({ id: 'A', rect: r(0, 0, 320, 320), visible: true })
    registry.upsert({ id: 'B', rect: r(320, 0, 320, 320), visible: true })
    store.set(c('B', 'A'))
    registry.updateRect('A', r(100, 0, 320, 320))
    solve(['A'], { registry, store })
    // registry 中 B 仍是旧位置
    expect(registry.get('B')?.rect).toEqual(r(320, 0, 320, 320))
  })
})
