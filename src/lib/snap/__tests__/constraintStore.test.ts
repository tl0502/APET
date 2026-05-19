// S1 constraintStore 单测（ADR-020 *Updated* I1 + I2）。
//
// 覆盖：set / get / delete / list / dependentsOf / wouldCycle / I1 替换 / self-loop reject。

import { beforeEach, describe, expect, it } from 'vitest'
import { ConstraintStore } from '../constraintStore'
import type { SnapConstraint } from '../types'

const c = (sourceId: string, targetId: string): SnapConstraint => ({
  sourceId,
  targetId,
  sourceEdge: 'left',
  targetEdge: 'right',
  offset: 0,
  enabled: true,
  createdAt: Date.now(),
})

describe('ConstraintStore — basic set/get/delete', () => {
  let store: ConstraintStore
  beforeEach(() => {
    store = new ConstraintStore()
  })

  it('set 后 get 返回相同 constraint', () => {
    const ok = store.set(c('chat', 'pet'))
    expect(ok).toEqual({ ok: true })
    expect(store.get('chat')?.targetId).toBe('pet')
  })

  it('delete 后 get 返回 undefined', () => {
    store.set(c('chat', 'pet'))
    expect(store.delete('chat')).toBe(true)
    expect(store.get('chat')).toBeUndefined()
    expect(store.delete('chat')).toBe(false) // 二次 delete 返 false
  })

  it('list 返回所有 constraints', () => {
    store.set(c('chat', 'pet'))
    store.set(c('settings', 'pet'))
    expect(store.list()).toHaveLength(2)
  })

  it('size 反映当前条数', () => {
    expect(store.size()).toBe(0)
    store.set(c('chat', 'pet'))
    expect(store.size()).toBe(1)
  })

  it('clear 清空全部', () => {
    store.set(c('chat', 'pet'))
    store.set(c('settings', 'pet'))
    store.clear()
    expect(store.size()).toBe(0)
    expect(store.dependentsOf('pet')).toHaveLength(0)
  })
})

describe('ConstraintStore — I1: 每 source 至多 1 个 constraint', () => {
  let store: ConstraintStore
  beforeEach(() => {
    store = new ConstraintStore()
  })

  it('同 source 第二次 set → 替换前一个', () => {
    store.set(c('chat', 'pet'))
    store.set(c('chat', 'settings'))
    expect(store.size()).toBe(1)
    expect(store.get('chat')?.targetId).toBe('settings')
  })

  it('I1 替换后旧 target 的 dependentsOf 不再含 source', () => {
    store.set(c('chat', 'pet'))
    expect(store.dependentsOf('pet')).toHaveLength(1)
    store.set(c('chat', 'settings'))
    expect(store.dependentsOf('pet')).toHaveLength(0)
    expect(store.dependentsOf('settings')).toHaveLength(1)
  })
})

describe('ConstraintStore — dependentsOf', () => {
  let store: ConstraintStore
  beforeEach(() => {
    store = new ConstraintStore()
  })

  it('多 source 共享同一 target 全返回', () => {
    store.set(c('chat', 'pet'))
    store.set(c('settings', 'pet'))
    store.set(c('tasks', 'pet'))
    const deps = store.dependentsOf('pet')
    expect(deps).toHaveLength(3)
    expect(deps.map((d) => d.sourceId).sort()).toEqual(['chat', 'settings', 'tasks'])
  })

  it('无 dependent target → 空数组', () => {
    store.set(c('chat', 'pet'))
    expect(store.dependentsOf('settings')).toHaveLength(0)
  })

  it('delete 后 dependentsOf 自动清理', () => {
    store.set(c('chat', 'pet'))
    store.set(c('settings', 'pet'))
    store.delete('chat')
    expect(store.dependentsOf('pet').map((d) => d.sourceId)).toEqual(['settings'])
  })
})

describe('ConstraintStore — I2: wouldCycle 路径覆盖', () => {
  let store: ConstraintStore
  beforeEach(() => {
    store = new ConstraintStore()
  })

  it('self-loop（source === target）→ set reject', () => {
    const r = store.set(c('pet', 'pet'))
    expect(r).toEqual({ ok: false, reason: 'self-loop' })
  })

  it('A → B → A 第二条 reject cycle', () => {
    store.set(c('A', 'B'))
    const r = store.set(c('B', 'A'))
    expect(r).toEqual({ ok: false, reason: 'cycle' })
    expect(store.size()).toBe(1) // 第二条没 set 进
  })

  it('3 节点环：A → B → C → A reject', () => {
    store.set(c('A', 'B'))
    store.set(c('B', 'C'))
    const r = store.set(c('C', 'A'))
    expect(r).toEqual({ ok: false, reason: 'cycle' })
  })

  it('forest 链 A → B → C 允许（无环）', () => {
    expect(store.set(c('A', 'B'))).toEqual({ ok: true })
    expect(store.set(c('B', 'C'))).toEqual({ ok: true })
    expect(store.size()).toBe(2)
  })

  it('跨树无环：A → C, B → C 允许（多 source 共 target）', () => {
    expect(store.set(c('A', 'C'))).toEqual({ ok: true })
    expect(store.set(c('B', 'C'))).toEqual({ ok: true })
  })

  it('wouldCycle 直接调用：空图 → false', () => {
    expect(store.wouldCycle('A', 'B')).toBe(false)
  })

  it('wouldCycle 直接调用：A → B 时 wouldCycle(B, A) === true', () => {
    store.set(c('A', 'B'))
    expect(store.wouldCycle('B', 'A')).toBe(true)
  })

  it('wouldCycle: A→B, B→C, wouldCycle(C, A) === true (长链回路)', () => {
    store.set(c('A', 'B'))
    store.set(c('B', 'C'))
    expect(store.wouldCycle('C', 'A')).toBe(true)
  })
})

// #30 follow-up D：removeAllInvolving — 拖子体时立即脱钩流程。
// E1 修复 (2026-05-19)：默认只删出向；入向需 options.includeInbound:true。
//
// 出向删除是拖子体的正确语义（用户拖走自己，断它对 anchor 的依附）。
// 入向删除会误伤其他依附我的窗，仅在显式"清空所有依附关系"场景下需要。
describe('ConstraintStore — removeAllInvolving (#30 follow-up D, E1 修复)', () => {
  let store: ConstraintStore
  beforeEach(() => {
    store = new ConstraintStore()
  })

  it('默认只删出向 constraint（label 作 source）', () => {
    store.set(c('chat', 'pet'))
    const removed = store.removeAllInvolving('chat')
    expect(removed).toHaveLength(1)
    expect(removed[0]?.sourceId).toBe('chat')
    expect(removed[0]?.targetId).toBe('pet')
    expect(store.get('chat')).toBeUndefined()
    expect(store.size()).toBe(0)
  })

  it('默认 NOT 删入向（拖目标 anchor 不应让依附它的窗脱钩）', () => {
    store.set(c('chat', 'pet'))
    store.set(c('settings', 'pet'))
    // pet 出向无，仅入向有；默认不删入向
    const removed = store.removeAllInvolving('pet')
    expect(removed).toHaveLength(0)
    expect(store.size()).toBe(2)
    // 入向仍在
    expect(store.dependentsOf('pet').map((c) => c.sourceId).sort()).toEqual([
      'chat',
      'settings',
    ])
  })

  it('includeInbound:true → 删 label 入向 constraints（多个 source 共指 label）', () => {
    store.set(c('chat', 'pet'))
    store.set(c('settings', 'pet'))
    store.set(c('tasks', 'pet'))
    const removed = store.removeAllInvolving('pet', { includeInbound: true })
    expect(removed).toHaveLength(3)
    const sourceIds = removed.map((c) => c.sourceId).sort()
    expect(sourceIds).toEqual(['chat', 'settings', 'tasks'])
    // 都被删
    expect(store.size()).toBe(0)
    expect(store.dependentsOf('pet')).toEqual([])
  })

  it('includeInbound:true → 同时删 label 的出向 + 入向', () => {
    // 链：chat → pet → desktop（pet 同时是 chat 的 target 与 desktop 的 source）
    store.set(c('chat', 'pet'))
    store.set(c('pet', 'desktop'))
    const removed = store.removeAllInvolving('pet', { includeInbound: true })
    expect(removed).toHaveLength(2)
    expect(store.get('chat')).toBeUndefined()
    expect(store.get('pet')).toBeUndefined()
    expect(store.size()).toBe(0)
  })

  it('默认仅删 label 出向：链 chat→pet→desktop，拖 pet → 仅 pet→desktop 被删', () => {
    store.set(c('chat', 'pet'))
    store.set(c('pet', 'desktop'))
    const removed = store.removeAllInvolving('pet')
    expect(removed).toHaveLength(1)
    expect(removed[0]?.sourceId).toBe('pet')
    expect(removed[0]?.targetId).toBe('desktop')
    // chat→pet 仍在
    expect(store.get('chat')?.targetId).toBe('pet')
    expect(store.size()).toBe(1)
  })

  it('label 未参与任何 constraint → 返空 list，store 不变', () => {
    store.set(c('A', 'B'))
    const removed = store.removeAllInvolving('Z')
    expect(removed).toHaveLength(0)
    expect(store.size()).toBe(1)
    expect(store.get('A')?.targetId).toBe('B')
  })
})
