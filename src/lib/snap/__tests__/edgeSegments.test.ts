// edgeSegments 单测（#30 follow-up F）。
//
// 覆盖：
// - projectSourceOntoEdge：clamp 到边界 / 完全外侧返 null / 容差 1px
// - mergeSegments：基础合并 / 相邻贴合 / 完全包含 / 单元素
// - freeSegments：补集 / 边界（占满 / 完全空 / 左右贴边）
// - computeEdgeOccupancy：过滤 targetId/edge/visible/excludeSourceId
// - findFreePlacement：策略 1/2/3/4 各一例

import { describe, expect, it } from 'vitest'
import {
  computeEdgeOccupancy,
  findFreePlacement,
  freeSegments,
  mergeSegments,
  projectSourceOntoEdge,
  type Segment,
} from '../edgeSegments'
import type { Rect, SnapConstraint, WindowRegistration } from '../types'

const r = (x: number, y: number, w: number, h: number): Rect => ({ x, y, w, h })
const reg = (id: string, rect: Rect, visible = true): WindowRegistration => ({ id, rect, visible })
const c = (
  sid: string,
  tid: string,
  sEdge: SnapConstraint['sourceEdge'],
  tEdge: SnapConstraint['targetEdge'],
): SnapConstraint => ({
  sourceId: sid,
  targetId: tid,
  sourceEdge: sEdge,
  targetEdge: tEdge,
  offset: 0,
  enabled: true,
  createdAt: 0,
})

describe('projectSourceOntoEdge', () => {
  it('vertical edge (target.right)：返 source 的 y 投影减 target.y', () => {
    const source = r(100, 50, 200, 150)
    const target = r(0, 0, 100, 300) // target.right 在 x=100，length=300
    const seg = projectSourceOntoEdge(source, target, 'right')
    expect(seg).toEqual({ start: 50, end: 200 })
  })

  it('horizontal edge (target.bottom)：返 source 的 x 投影减 target.x', () => {
    const source = r(120, 200, 80, 50)
    const target = r(100, 0, 300, 200)
    const seg = projectSourceOntoEdge(source, target, 'bottom')
    expect(seg).toEqual({ start: 20, end: 100 })
  })

  it('部分越界：clamp 到 [0, totalLength]', () => {
    const source = r(0, -50, 100, 100) // y=-50..50，target.h=300
    const target = r(0, 0, 100, 300)
    const seg = projectSourceOntoEdge(source, target, 'left')
    expect(seg).toEqual({ start: 0, end: 50 })
  })

  it('完全外侧 → 返 null（容差 1px）', () => {
    const source = r(0, -500, 100, 100)
    const target = r(0, 0, 100, 300)
    expect(projectSourceOntoEdge(source, target, 'left')).toBeNull()
  })

  it('overlap = 0.5px (< 1) → 视作不占用返 null', () => {
    const source = r(0, -99.7, 100, 100) // y = -99.7..0.3，clamp 后 0..0.3
    const target = r(0, 0, 100, 300)
    expect(projectSourceOntoEdge(source, target, 'left')).toBeNull()
  })
})

describe('mergeSegments', () => {
  it('空输入 → 空输出', () => {
    expect(mergeSegments([])).toEqual([])
  })

  it('单元素 → 原样返回（深拷贝）', () => {
    const input: Segment[] = [{ start: 10, end: 50 }]
    const out = mergeSegments(input)
    expect(out).toEqual(input)
    expect(out[0]).not.toBe(input[0]) // 深拷贝
  })

  it('两段无重叠 → 按 start 升序保留', () => {
    expect(mergeSegments([{ start: 100, end: 200 }, { start: 0, end: 50 }])).toEqual([
      { start: 0, end: 50 },
      { start: 100, end: 200 },
    ])
  })

  it('重叠 → 合并', () => {
    expect(
      mergeSegments([
        { start: 0, end: 100 },
        { start: 80, end: 200 },
      ]),
    ).toEqual([{ start: 0, end: 200 }])
  })

  it('相邻贴合（start == 前 end）→ 合并', () => {
    expect(
      mergeSegments([
        { start: 0, end: 50 },
        { start: 50, end: 100 },
      ]),
    ).toEqual([{ start: 0, end: 100 }])
  })

  it('完全包含 → 合并取大', () => {
    expect(
      mergeSegments([
        { start: 0, end: 200 },
        { start: 50, end: 100 },
      ]),
    ).toEqual([{ start: 0, end: 200 }])
  })
})

describe('freeSegments', () => {
  it('完全空 → 单段 [0, total]', () => {
    expect(freeSegments({ totalLength: 300, occupied: [] })).toEqual([{ start: 0, end: 300 }])
  })

  it('完全占满 → 空数组', () => {
    expect(
      freeSegments({ totalLength: 300, occupied: [{ start: 0, end: 300 }] }),
    ).toEqual([])
  })

  it('中段占用 → 头尾两段空', () => {
    expect(
      freeSegments({ totalLength: 300, occupied: [{ start: 100, end: 200 }] }),
    ).toEqual([
      { start: 0, end: 100 },
      { start: 200, end: 300 },
    ])
  })

  it('左贴边占用 → 仅右段空', () => {
    expect(
      freeSegments({ totalLength: 300, occupied: [{ start: 0, end: 100 }] }),
    ).toEqual([{ start: 100, end: 300 }])
  })
})

describe('computeEdgeOccupancy', () => {
  it('过滤 targetId/edge：只统计 targetId === query 且 targetEdge === query 的 constraint', () => {
    const target = reg('t', r(0, 0, 200, 300))
    const s1 = reg('s1', r(200, 0, 100, 100)) // s1.left ↔ t.right 投影 [0,100]
    const s2 = reg('s2', r(-100, 100, 100, 50)) // s2.right ↔ t.left 投影 [100,150]，不该计入 right
    const constraints = [c('s1', 't', 'left', 'right'), c('s2', 't', 'right', 'left')]
    const occ = computeEdgeOccupancy('t', 'right', constraints, [target, s1, s2])
    expect(occ.totalLength).toBe(300)
    expect(occ.occupied).toEqual([{ start: 0, end: 100 }]) // 只 s1
  })

  it('过滤 !visible source', () => {
    const target = reg('t', r(0, 0, 200, 300))
    const s1 = reg('s1', r(200, 0, 100, 100), false)
    const occ = computeEdgeOccupancy('t', 'right', [c('s1', 't', 'left', 'right')], [target, s1])
    expect(occ.occupied).toEqual([])
  })

  it('excludeSourceId：跳过该 source 的旧占用（自己重新评估时用）', () => {
    const target = reg('t', r(0, 0, 200, 300))
    const me = reg('me', r(200, 0, 100, 100))
    const constraints = [c('me', 't', 'left', 'right')]
    // 不传 exclude → 自己被算占用
    const occInclude = computeEdgeOccupancy('t', 'right', constraints, [target, me])
    expect(occInclude.occupied).toEqual([{ start: 0, end: 100 }])
    // 传 exclude='me' → 自己被排除
    const occExclude = computeEdgeOccupancy('t', 'right', constraints, [target, me], 'me')
    expect(occExclude.occupied).toEqual([])
  })

  it('target 不在 registry → totalLength=0, occupied=[]', () => {
    const occ = computeEdgeOccupancy('missing', 'right', [], [])
    expect(occ).toEqual({ totalLength: 0, occupied: [] })
  })

  it('多 source 占用同一边 → 合并区段', () => {
    const target = reg('t', r(0, 0, 200, 400))
    const s1 = reg('s1', r(200, 0, 100, 100)) // 投影 [0,100]
    const s2 = reg('s2', r(200, 90, 100, 100)) // 投影 [90,190]（重叠）
    const occ = computeEdgeOccupancy(
      't',
      'right',
      [c('s1', 't', 'left', 'right'), c('s2', 't', 'left', 'right')],
      [target, s1, s2],
    )
    expect(occ.occupied).toEqual([{ start: 0, end: 190 }]) // 合并
  })
})

describe('findFreePlacement — 策略覆盖', () => {
  it('策略 1：完整落在 free 段 → 返原 start（不滑动）', () => {
    const occ = { totalLength: 300, occupied: [{ start: 0, end: 100 }] }
    // proj [150, 250]，完整在 free [100,300] 内
    expect(findFreePlacement({ start: 150, end: 250 }, occ, 100)).toBe(150)
  })

  it('策略 2：中心在 free 但左侧越界 → 推到段 start', () => {
    const occ = { totalLength: 300, occupied: [{ start: 0, end: 100 }] }
    // proj [50, 200]，中心 125 在 free [100,300]，但 start 50 < 100 → 推到 100
    expect(findFreePlacement({ start: 50, end: 200 }, occ, 150)).toBe(100)
  })

  it('策略 2：中心在 free 但右侧越界 → 拉到段 end - projLen', () => {
    const occ = { totalLength: 300, occupied: [{ start: 200, end: 300 }] }
    // proj [100, 250]，中心 175 在 free [0, 200]，但 end 250 > 200 → 拉到 200 - 150 = 50
    expect(findFreePlacement({ start: 100, end: 250 }, occ, 150)).toBe(50)
  })

  it('策略 3：中心在 occupied → 找最近能容下 neededLen 的 free 段', () => {
    const occ = {
      totalLength: 600,
      occupied: [
        { start: 100, end: 200 },
        { start: 350, end: 450 },
      ],
    }
    // proj [150, 200]，中心 175 在 occupied [100,200]
    // free 段：[0,100]、[200,350]、[450,600]
    // projLen=50；needed=50；选最近 = 中心 275 的段 [200,350]（距 100 < 距 50）
    // projCenter(175) ≤ 段中心(275) → 贴 start = 200
    expect(findFreePlacement({ start: 150, end: 200 }, occ, 50)).toBe(200)
  })

  it('策略 4：没有任何 free 段容下 neededLen → null', () => {
    const occ = {
      totalLength: 300,
      occupied: [
        { start: 0, end: 100 },
        { start: 150, end: 300 },
      ],
    }
    // free 只有 [100,150]，长度 50 < neededLen 100
    expect(findFreePlacement({ start: 0, end: 100 }, occ, 100)).toBeNull()
  })

  it('neededLen > totalLength → null', () => {
    const occ = { totalLength: 100, occupied: [] }
    expect(findFreePlacement({ start: 0, end: 100 }, occ, 200)).toBeNull()
  })

  it('完全空边 → 返原 start', () => {
    const occ = { totalLength: 300, occupied: [] }
    expect(findFreePlacement({ start: 50, end: 150 }, occ, 100)).toBe(50)
  })
})
