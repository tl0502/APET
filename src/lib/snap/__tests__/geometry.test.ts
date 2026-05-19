// S1 geometry 单测（ADR-020 *Updated*）。
//
// 覆盖：rectEdgeGeometry / projectionOverlap / edgeDistance / isOppositeEdge
//      / applyConstraint / overlapThreshold / inCornerDead。

import { describe, expect, it } from 'vitest'
import {
  applyConstraint,
  CORNER_DEAD,
  edgeDistance,
  inCornerDead,
  isOppositeEdge,
  MIN_OVERLAP,
  OVERLAP_RATIO,
  overlapThreshold,
  projectionOverlap,
  rectCenter,
  rectEdgeGeometry,
} from '../geometry'
import type { Rect } from '../types'

const rect = (x: number, y: number, w: number, h: number): Rect => ({ x, y, w, h })

describe('rectCenter', () => {
  it('returns geometric center', () => {
    expect(rectCenter(rect(0, 0, 100, 100))).toEqual({ cx: 50, cy: 50 })
    expect(rectCenter(rect(100, 200, 320, 320))).toEqual({ cx: 260, cy: 360 })
  })
})

describe('rectEdgeGeometry', () => {
  const r = rect(100, 200, 320, 240)
  it('left edge: level=x, range=[y, y+h]', () => {
    expect(rectEdgeGeometry(r, 'left')).toEqual({ level: 100, start: 200, end: 440, length: 240 })
  })
  it('right edge: level=x+w', () => {
    expect(rectEdgeGeometry(r, 'right')).toEqual({ level: 420, start: 200, end: 440, length: 240 })
  })
  it('top edge: level=y, range=[x, x+w]', () => {
    expect(rectEdgeGeometry(r, 'top')).toEqual({ level: 200, start: 100, end: 420, length: 320 })
  })
  it('bottom edge: level=y+h', () => {
    expect(rectEdgeGeometry(r, 'bottom')).toEqual({ level: 440, start: 100, end: 420, length: 320 })
  })
})

describe('isOppositeEdge', () => {
  it('left↔right / top↔bottom are opposite', () => {
    expect(isOppositeEdge('left', 'right')).toBe(true)
    expect(isOppositeEdge('right', 'left')).toBe(true)
    expect(isOppositeEdge('top', 'bottom')).toBe(true)
    expect(isOppositeEdge('bottom', 'top')).toBe(true)
  })
  it('same-axis or same-edge are not opposite', () => {
    expect(isOppositeEdge('left', 'left')).toBe(false)
    expect(isOppositeEdge('left', 'top')).toBe(false)
    expect(isOppositeEdge('right', 'bottom')).toBe(false)
  })
})

describe('projectionOverlap', () => {
  // source 在左，target 在右；source.right ↔ target.left
  const source = rect(0, 100, 320, 320) // y range [100, 420]
  it('full overlap when ranges equal', () => {
    const target = rect(400, 100, 320, 320) // y range [100, 420]
    expect(projectionOverlap(source, target, 'right', 'left')).toBe(320)
  })
  it('partial overlap', () => {
    const target = rect(400, 200, 320, 320) // y range [200, 520] — 与 source 重叠 [200, 420] = 220
    expect(projectionOverlap(source, target, 'right', 'left')).toBe(220)
  })
  it('zero overlap when ranges disjoint', () => {
    const target = rect(400, 500, 320, 320) // y range [500, 820] — 与 [100, 420] 不交
    expect(projectionOverlap(source, target, 'right', 'left')).toBe(0)
  })
  it('horizontal edges: source.bottom ↔ target.top', () => {
    const s = rect(100, 0, 320, 240) // x range [100, 420]
    const t = rect(200, 240, 320, 240) // x range [200, 520] — 重叠 [200, 420] = 220
    expect(projectionOverlap(s, t, 'bottom', 'top')).toBe(220)
  })
})

describe('edgeDistance', () => {
  it('right ↔ left: |source.right - target.left|', () => {
    const s = rect(0, 0, 320, 320) // right = 320
    const t = rect(340, 0, 320, 320) // left = 340
    expect(edgeDistance(s, t, 'right', 'left')).toBe(20)
  })
  it('overshoot case still abs', () => {
    const s = rect(0, 0, 320, 320) // right = 320
    const t = rect(310, 0, 320, 320) // left = 310 — source overshoots into target
    expect(edgeDistance(s, t, 'right', 'left')).toBe(10)
  })
})

describe('applyConstraint', () => {
  const source = rect(0, 0, 320, 320)
  const anchor = rect(400, 200, 480, 320)

  it('left ↔ right: source 贴 anchor 右边', () => {
    const out = applyConstraint(source, anchor, { sourceEdge: 'left', targetEdge: 'right', offset: 50 })
    // source.x = anchor.x + anchor.w = 880; source.y = anchor.y + offset = 250
    expect(out).toEqual({ x: 880, y: 250, w: 320, h: 320 })
  })
  it('right ↔ left: source 贴 anchor 左边', () => {
    const out = applyConstraint(source, anchor, { sourceEdge: 'right', targetEdge: 'left', offset: 0 })
    // source.x = anchor.x - source.w = 80; source.y = anchor.y + offset = 200
    expect(out).toEqual({ x: 80, y: 200, w: 320, h: 320 })
  })
  it('top ↔ bottom: source 贴 anchor 下边', () => {
    const out = applyConstraint(source, anchor, { sourceEdge: 'top', targetEdge: 'bottom', offset: -40 })
    // source.x = anchor.x + offset = 360; source.y = anchor.y + anchor.h = 520
    expect(out).toEqual({ x: 360, y: 520, w: 320, h: 320 })
  })
  it('bottom ↔ top: source 贴 anchor 上边', () => {
    const out = applyConstraint(source, anchor, { sourceEdge: 'bottom', targetEdge: 'top', offset: 100 })
    // source.x = anchor.x + offset = 500; source.y = anchor.y - source.h = -120
    expect(out).toEqual({ x: 500, y: -120, w: 320, h: 320 })
  })
  it('保留 source 尺寸（仅锁位置）', () => {
    const out = applyConstraint(source, anchor, { sourceEdge: 'left', targetEdge: 'right', offset: 0 })
    expect(out.w).toBe(source.w)
    expect(out.h).toBe(source.h)
  })
})

describe('overlapThreshold', () => {
  it('短边走 MIN_OVERLAP 兜底', () => {
    // 边长 100，100 * 0.25 = 25 < MIN_OVERLAP (72) → 72
    expect(overlapThreshold(100)).toBe(MIN_OVERLAP)
  })
  it('长边走 length × OVERLAP_RATIO', () => {
    // 边长 400，400 * 0.25 = 100 > MIN_OVERLAP → 100
    expect(overlapThreshold(400)).toBe(400 * OVERLAP_RATIO)
  })
  it('边界点：MIN_OVERLAP / OVERLAP_RATIO = 288，恰好相等', () => {
    expect(overlapThreshold(288)).toBe(MIN_OVERLAP)
  })
})

describe('inCornerDead', () => {
  const anchor = rect(400, 400, 320, 320) // corners at (400,400)(720,400)(400,720)(720,720)
  it('source 中心紧贴某个 corner → true', () => {
    const s = rect(395, 395, 10, 10) // center (400, 400) = top-left corner
    expect(inCornerDead(s, anchor)).toBe(true)
  })
  it('source 中心在边中点 → 不算 corner dead', () => {
    const s = rect(540, 395, 40, 10) // center (560, 400) — top 边中点附近，离 corner 距离 160 > 24
    expect(inCornerDead(s, anchor)).toBe(false)
  })
  it('source 中心刚好 CORNER_DEAD 像素外 → false', () => {
    const s = rect(400 + CORNER_DEAD + 1, 400 - 5, 10, 10) // center 距离 (400,400) > CORNER_DEAD
    expect(inCornerDead(s, anchor)).toBe(false)
  })
})
