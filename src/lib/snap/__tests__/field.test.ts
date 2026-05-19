// Field intensity 单测（#31 follow-up C Phase A）。
//
// 覆盖 fieldIntensityFromDistance 边界、nearestEdgeDistance（含无 overlap 情况）、
// computeFieldIntensity（多窗取最近 / self 跳过 / invisible 跳过）。

import { describe, expect, it } from 'vitest'
import {
  FIELD_INTENSITY_EVT,
  computeFieldIntensity,
  fieldIntensityFromDistance,
  nearestEdgeDistance,
} from '../field'
import { ATTACH_ZONE, FIELD_RADIUS } from '../geometry'
import type { Rect, WindowRegistration } from '../types'

const r = (x: number, y: number, w: number, h: number): Rect => ({ x, y, w, h })
const reg = (id: string, rect: Rect, visible = true): WindowRegistration => ({ id, rect, visible })

describe('fieldIntensityFromDistance', () => {
  it('distance ≤ ATTACH_ZONE → 1.0（已在吸附区）', () => {
    expect(fieldIntensityFromDistance(0)).toBe(1)
    expect(fieldIntensityFromDistance(5)).toBe(1)
    expect(fieldIntensityFromDistance(ATTACH_ZONE)).toBe(1)
  })

  it('distance ≥ FIELD_RADIUS → 0', () => {
    expect(fieldIntensityFromDistance(FIELD_RADIUS)).toBe(0)
    expect(fieldIntensityFromDistance(200)).toBe(0)
    expect(fieldIntensityFromDistance(Infinity)).toBe(0)
  })

  it('ATTACH ~ FIELD_RADIUS 中点 → 0.5', () => {
    const mid = (ATTACH_ZONE + FIELD_RADIUS) / 2 // 15 (= (10+20)/2)
    expect(fieldIntensityFromDistance(mid)).toBeCloseTo(0.5, 5)
  })

  it('线性单调递减 in (ATTACH, FIELD_RADIUS)', () => {
    // #30 follow-up D revision：ATTACH=25 / FIELD_RADIUS=50 → 区间 (25, 50)
    const a = fieldIntensityFromDistance(30)
    const b = fieldIntensityFromDistance(37)
    const c = fieldIntensityFromDistance(44)
    expect(a).toBeGreaterThan(b)
    expect(b).toBeGreaterThan(c)
  })
})

describe('nearestEdgeDistance', () => {
  it('两窗 y 完全对齐 + 右排 20px gap → 20', () => {
    expect(nearestEdgeDistance(r(0, 0, 320, 320), r(340, 0, 320, 320))).toBe(20)
  })

  it('两窗 x 完全对齐 + 下排 50px gap → 50', () => {
    expect(nearestEdgeDistance(r(0, 0, 320, 320), r(0, 370, 320, 320))).toBe(50)
  })

  it('对角错开（无 overlap）→ Infinity', () => {
    // source 右下、anchor 左上，没有任何 opposite 边对有 overlap
    expect(nearestEdgeDistance(r(500, 500, 320, 320), r(0, 0, 320, 320))).toBe(
      Number.POSITIVE_INFINITY,
    )
  })

  it('部分 overlap：y 重叠 40px (small)，水平距 10px → 10', () => {
    // source y=[0,320]，anchor y=[280,600]，overlap y = [280,320] = 40 > 0
    expect(nearestEdgeDistance(r(0, 0, 320, 320), r(330, 280, 320, 320))).toBe(10)
  })
})

describe('computeFieldIntensity', () => {
  it('空 registry → intensity 0, anchorId null', () => {
    const result = computeFieldIntensity('s', r(0, 0, 320, 320), [])
    expect(result.intensity).toBe(0)
    expect(result.anchorId).toBeNull()
  })

  it('单 anchor 在吸附区（dist=5）→ intensity 1, anchorId 命中', () => {
    const result = computeFieldIntensity('s', r(0, 0, 320, 320), [
      reg('t', r(325, 0, 320, 320)), // dist = 5
    ])
    expect(result.intensity).toBe(1)
    expect(result.anchorId).toBe('t')
  })

  it('单 anchor 在 field 中段（dist=mid）→ intensity ~0.5', () => {
    // mid = (25+50)/2 = 37.5；用浮点 rect 让 dist 严格 == mid
    const mid = (ATTACH_ZONE + FIELD_RADIUS) / 2
    const result = computeFieldIntensity('s', r(0, 0, 320, 320), [
      reg('t', r(320 + mid, 0, 320, 320)),
    ])
    expect(result.intensity).toBeCloseTo(0.5, 5)
    expect(result.anchorId).toBe('t')
  })

  it('单 anchor 远 (dist=50) → intensity 0, anchorId null', () => {
    const result = computeFieldIntensity('s', r(0, 0, 320, 320), [
      reg('t', r(370, 0, 320, 320)), // dist = 50 > FIELD_RADIUS 20
    ])
    expect(result.intensity).toBe(0)
    expect(result.anchorId).toBeNull()
  })

  it('多 anchor → 取距离最近的', () => {
    const result = computeFieldIntensity('s', r(0, 0, 320, 320), [
      reg('far', r(335, 0, 320, 320)), // dist = 15，intensity 0.5
      reg('near', r(325, 0, 320, 320)), // dist = 5 < ATTACH_ZONE，intensity 1
    ])
    expect(result.anchorId).toBe('near')
    expect(result.intensity).toBe(1)
  })

  it('self（source.id === target.id）跳过', () => {
    const result = computeFieldIntensity('s', r(0, 0, 320, 320), [
      reg('s', r(325, 0, 320, 320)), // 同 id 即使近也跳
    ])
    expect(result.intensity).toBe(0)
  })

  it('invisible 窗跳过', () => {
    const result = computeFieldIntensity('s', r(0, 0, 320, 320), [
      reg('t', r(325, 0, 320, 320), false), // !visible
    ])
    expect(result.intensity).toBe(0)
  })
})

describe('FIELD_INTENSITY_EVT', () => {
  it('event 名稳定（不要乱改，前后端协定）', () => {
    expect(FIELD_INTENSITY_EVT).toBe('snap:field-intensity')
  })
})
