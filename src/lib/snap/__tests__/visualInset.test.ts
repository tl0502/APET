// visualInset 单测（#30 follow-up F）。
//
// 覆盖：
// - applyVisualInset：基础内缩 / 缺省 / clamp 到非负
// - reverseVisualInset：与 applyVisualInset 互逆
// - findCandidates 在带 visualInset 的窗下产生的 finalRect.x/y 仍是 OS rect 坐标
//   且两窗 visual 边贴合（OS 边之间有 left+right inset 的间距，符合 padding 间隙消除）

import { beforeEach, describe, expect, it } from 'vitest'
import { findCandidates } from '../candidates'
import { constraintStore } from '../constraintStore'
import { applyVisualInset, reverseVisualInset } from '../geometry'
import type { Rect, WindowRegistration } from '../types'

const r = (x: number, y: number, w: number, h: number): Rect => ({ x, y, w, h })
const reg = (
  id: string,
  rect: Rect,
  visualInset?: { top: number; right: number; bottom: number; left: number },
  visible = true,
): WindowRegistration => ({ id, rect, visible, ...(visualInset ? { visualInset } : {}) })

beforeEach(() => {
  constraintStore.clear()
})

describe('applyVisualInset / reverseVisualInset', () => {
  it('applyVisualInset 无 inset → 原 rect', () => {
    const src = r(10, 20, 100, 200)
    expect(applyVisualInset(src)).toEqual(src)
  })

  it('applyVisualInset 均匀 12px → 各方向内缩 12', () => {
    const src = r(0, 0, 100, 100)
    expect(applyVisualInset(src, { top: 12, right: 12, bottom: 12, left: 12 })).toEqual({
      x: 12,
      y: 12,
      w: 76,
      h: 76,
    })
  })

  it('applyVisualInset 不对称 inset → 各方向独立内缩', () => {
    const src = r(0, 0, 100, 100)
    expect(applyVisualInset(src, { top: 5, right: 10, bottom: 15, left: 20 })).toEqual({
      x: 20,
      y: 5,
      w: 70,
      h: 80,
    })
  })

  it('applyVisualInset 超大 inset → w/h clamp 到 0 不变负', () => {
    const src = r(0, 0, 50, 50)
    expect(applyVisualInset(src, { top: 100, right: 100, bottom: 100, left: 100 })).toEqual({
      x: 100,
      y: 100,
      w: 0,
      h: 0,
    })
  })

  it('reverseVisualInset 与 applyVisualInset 互逆', () => {
    const src = r(100, 100, 200, 300)
    const inset = { top: 8, right: 12, bottom: 8, left: 12 }
    const back = reverseVisualInset(applyVisualInset(src, inset), inset)
    expect(back).toEqual(src)
  })

  it('reverseVisualInset 无 inset → 原 rect', () => {
    const v = r(50, 50, 80, 80)
    expect(reverseVisualInset(v)).toEqual(v)
  })
})

// findCandidates 集成：chat（12px inset）吸 pet（无 inset）
// 视觉贴合 → OS rect 边之间应留 12px（chat 的 padding）
describe('findCandidates — visualInset 集成（消除 padding 间隙）', () => {
  it('两窗均无 inset → 行为同旧版（finalRect 紧贴 OS 边）', () => {
    const pet = reg('pet', r(0, 0, 320, 320))
    // chat 想吸 pet.right；chat 在 (323, 0)，距 3 px
    const chat = r(323, 0, 320, 320)
    const cands = findCandidates('chat', chat, [pet])
    expect(cands).toHaveLength(1)
    // finalRect.x = pet.right = 320（OS 紧贴，无间隙）
    expect(cands[0]?.finalRect.x).toBe(320)
    expect(cands[0]?.finalRect.y).toBe(0)
  })

  it('chat 有 12px inset，pet 无 inset → finalRect.x = pet.right - chat.inset.left = 320 - 12 = 308', () => {
    // 语义：chat visual rect = OS rect 内缩 12 → chat 的"视觉左边"在 OS.x + 12 位置
    // 贴 pet 视觉右边（= pet OS.right = 320） → chat visual.x = 320 → chat OS.x = 320 - 12 = 308
    const pet = reg('pet', r(0, 0, 320, 320)) // 无 inset
    const chatInset = { top: 12, right: 12, bottom: 12, left: 12 }
    // chat 想吸 pet.right；放在 distance 较近的位置
    const chatOs = r(330, 0, 320, 320) // visual 边距 pet.right 较近
    // 必须先把 chat 注册到 registry 让 findCandidates 拿到 inset
    const chatReg = reg('chat', chatOs, chatInset)
    const cands = findCandidates('chat', chatOs, [pet, chatReg])
    expect(cands.length).toBeGreaterThan(0)
    const cand = cands.find((c) => c.targetId === 'pet')
    expect(cand).toBeDefined()
    // OS finalRect.x = pet 视觉 right (320) - chat inset.left (12) = 308
    expect(cand!.finalRect.x).toBe(308)
  })

  it('chat 视觉边距 pet 视觉边在 ATTACH_ZONE 内 → 吸附（OS 距 < visual 距 因 chat inset 把视觉边推远）', () => {
    // 物理含义：chat OS.x=320 + inset.left=12 → chat visual.x = 332
    // pet visual.right = 320 → visual 距 = 332 - 320 = 12 < ATTACH 25 → 命中
    // 但 OS 距 = chat OS.x - pet OS.right = 0（OS 上 chat 紧贴 pet）— 物理上 chat 已与 pet 重叠
    // 这正说明 visualInset 模型避免了"OS 重叠但视觉留间隙"被误判为吸附冲突的 case
    const pet = reg('pet', r(0, 0, 320, 320))
    const chatInset = { top: 12, right: 12, bottom: 12, left: 12 }
    const chatOs = r(320, 0, 320, 320) // OS 紧贴
    const chatReg = reg('chat', chatOs, chatInset)
    const cands = findCandidates('chat', chatOs, [pet, chatReg])
    expect(cands.length).toBeGreaterThan(0)
    const cand = cands.find((c) => c.targetId === 'pet')
    expect(cand).toBeDefined()
    // distance 字段是 visual 距 = 12
    expect(cand!.distance).toBe(12)
  })
})
