// VelocityTracker + velocityBias 单测（#31 follow-up C Phase C）。
//
// 覆盖：
// - VelocityTracker: 第一帧 (0,0) / spike 丢弃 / V_MIN 静止 / EWMA 平滑 / reset
// - velocityBias: 静止返 0 / 同向返 1 / 反向截 0 / 垂直返 0 / 各 edge 方向
// - velocityTerm: 1 - velocityBias 关系

import { describe, expect, it } from 'vitest'
import {
  EWMA_ALPHA,
  SPIKE_DT,
  SPIKE_DX,
  V_MIN,
  VelocityTracker,
  velocityBias,
  velocityTerm,
  type Vec2,
} from '../intent'

describe('VelocityTracker — 基础', () => {
  it('第一帧 update → velocity = (0, 0)（只记录参考帧，不算 velocity）', () => {
    const t = new VelocityTracker()
    const v = t.update(0, 0, 1000)
    expect(v).toEqual({ x: 0, y: 0 })
    expect(t.speed).toBe(0)
  })

  it('reset 后清空状态，下一次 update 视为第一帧', () => {
    const t = new VelocityTracker()
    t.update(0, 0, 1000)
    t.update(50, 0, 1016)
    expect(t.speed).toBeGreaterThan(0)
    t.reset()
    expect(t.speed).toBe(0)
    const v = t.update(100, 100, 2000)
    expect(v).toEqual({ x: 0, y: 0 })
  })
})

describe('VelocityTracker — EWMA 平滑', () => {
  it('稳定方向连续 update → velocity 趋向真实位移（5 帧后 ~87% raw）', () => {
    const t = new VelocityTracker()
    t.update(0, 0, 1000) // 第一帧建立参考
    // 每帧 +10px x，16ms（60fps）
    for (let i = 1; i <= 5; i++) {
      t.update(i * 10, 0, 1000 + i * 16)
    }
    // EWMA α=1/3 收敛：v_n = raw × (1 - (1-α)^n)
    // 5 帧：1 - (2/3)^5 = 1 - 32/243 ≈ 0.868 → v ≈ 8.68
    const expected = 10 * (1 - Math.pow(2 / 3, 5))
    expect(t.velocity.x).toBeCloseTo(expected, 3)
    expect(t.velocity.x).toBeGreaterThan(0.8 * 10) // > 80%
    expect(t.velocity.x).toBeLessThan(10) // 未到 100%
    expect(t.velocity.y).toBeCloseTo(0, 5)
  })

  it('10 帧后接近 raw（>98%）', () => {
    const t = new VelocityTracker()
    t.update(0, 0, 1000)
    for (let i = 1; i <= 10; i++) {
      t.update(i * 10, 0, 1000 + i * 16)
    }
    // 10 帧：1 - (2/3)^10 ≈ 0.983 → v ≈ 9.83
    expect(t.velocity.x).toBeGreaterThan(0.98 * 10)
  })

  it('单帧位移立即体现（不滞后到 0）', () => {
    const t = new VelocityTracker()
    t.update(0, 0, 1000)
    const v = t.update(30, 0, 1016) // dx=30, dt=16ms
    // α × 30 = 10
    expect(v.x).toBeCloseTo(EWMA_ALPHA * 30, 5)
  })

  it('方向反转 → velocity 渐进切到反向（不瞬切）', () => {
    const t = new VelocityTracker()
    t.update(0, 0, 1000)
    // 先 +x 累积
    for (let i = 1; i <= 5; i++) t.update(i * 20, 0, 1000 + i * 16)
    const vBefore = t.velocity.x
    expect(vBefore).toBeGreaterThan(0)
    // 反转 -x 单帧
    t.update(0, 0, 1100) // dx = -20*5 = -100, 但 dt=4ms?
    // 实际 dt = 1100 - 1080 = 20ms（< SPIKE_DT 50），dx = -100（< SPIKE_DX 200）
    const vAfter = t.velocity.x
    // EWMA: 1/3 × (-100) + 2/3 × vBefore ≈ -33.3 + 2/3 × 18.x ≈ -21
    expect(vAfter).toBeLessThan(vBefore) // 至少减少
    // 没有瞬切到 -100（被平滑）
    expect(vAfter).toBeGreaterThan(-100)
  })
})

describe('VelocityTracker — spike 丢弃', () => {
  it('单帧 dx > SPIKE_DX (200) → 视为 spike，velocity 不更新但参考帧更新', () => {
    const t = new VelocityTracker()
    t.update(0, 0, 1000)
    t.update(30, 0, 1016) // 正常帧，v.x ≈ 10
    const vBefore = t.velocity.x
    expect(vBefore).toBeGreaterThan(0)
    // spike：dx = 300，本帧丢弃
    const vSpike = t.update(330, 0, 1032)
    expect(vSpike.x).toBe(vBefore) // 完全没变
    // 但参考帧已更新到 (330, 0)，下一帧 dx 从这里算
    const vNext = t.update(340, 0, 1048) // dx=10
    expect(vNext.x).toBeCloseTo(EWMA_ALPHA * 10 + (1 - EWMA_ALPHA) * vBefore, 5)
  })

  it('单帧 dt > SPIKE_DT (50) → 视为 spike（窗口失焦后突然拖）', () => {
    const t = new VelocityTracker()
    t.update(0, 0, 1000)
    t.update(30, 0, 1016)
    const vBefore = t.velocity.x
    // spike：dt = 100ms
    const vSpike = t.update(40, 0, 1116)
    expect(vSpike.x).toBe(vBefore)
  })

  it('spike 后下一帧立即可识别为正常（不会陷在 spike 循环）', () => {
    const t = new VelocityTracker()
    t.update(0, 0, 1000)
    t.update(300, 0, 1010) // spike (dx=300)
    expect(t.velocity).toEqual({ x: 0, y: 0 }) // 无前置 v 历史
    // 下一帧 normal：dx = 10
    const vNext = t.update(310, 0, 1026)
    expect(vNext.x).toBeCloseTo(EWMA_ALPHA * 10, 5)
  })

  it('y 方向 spike 也识别', () => {
    const t = new VelocityTracker()
    t.update(0, 0, 1000)
    t.update(0, 30, 1016)
    const vBefore = t.velocity.y
    const vSpike = t.update(0, 330, 1032) // dy = 300
    expect(vSpike.y).toBe(vBefore)
  })
})

describe('velocityBias — cosine alignment', () => {
  it('静止（speed < V_MIN）→ 返 0（任何 edge）', () => {
    const v: Vec2 = { x: 5, y: 0 } // |v| = 5 < V_MIN 10
    expect(velocityBias(v, 'right')).toBe(0)
    expect(velocityBias(v, 'left')).toBe(0)
    expect(velocityBias(v, 'top')).toBe(0)
    expect(velocityBias(v, 'bottom')).toBe(0)
  })

  it('完全同向 → 1', () => {
    // source 朝 +x，sourceEdge='right' → 同向
    expect(velocityBias({ x: 50, y: 0 }, 'right')).toBeCloseTo(1, 5)
    expect(velocityBias({ x: -50, y: 0 }, 'left')).toBeCloseTo(1, 5)
    expect(velocityBias({ x: 0, y: 50 }, 'bottom')).toBeCloseTo(1, 5)
    expect(velocityBias({ x: 0, y: -50 }, 'top')).toBeCloseTo(1, 5)
  })

  it('反向 → 截断到 0（不惩罚 detach 方向运动）', () => {
    expect(velocityBias({ x: -50, y: 0 }, 'right')).toBe(0)
    expect(velocityBias({ x: 50, y: 0 }, 'left')).toBe(0)
    expect(velocityBias({ x: 0, y: -50 }, 'bottom')).toBe(0)
    expect(velocityBias({ x: 0, y: 50 }, 'top')).toBe(0)
  })

  it('垂直 → 返 0（与反向同处理）', () => {
    expect(velocityBias({ x: 0, y: 50 }, 'right')).toBeCloseTo(0, 5)
    expect(velocityBias({ x: 50, y: 0 }, 'top')).toBeCloseTo(0, 5)
  })

  it('45 度同向 → cos = √2/2 ≈ 0.707', () => {
    // v = (30, 30) 朝右下；sourceEdge='right' 朝 +x
    // cos = 30 / (30√2) = 1/√2 ≈ 0.707
    expect(velocityBias({ x: 30, y: 30 }, 'right')).toBeCloseTo(Math.SQRT1_2, 3)
  })

  it('30 度偏 → cos = cos(30°) ≈ 0.866', () => {
    // sourceEdge='right' (+x)，v 与 +x 夹角 30°
    const speed = 50
    const v = { x: speed * Math.cos((30 * Math.PI) / 180), y: speed * Math.sin((30 * Math.PI) / 180) }
    expect(velocityBias(v, 'right')).toBeCloseTo(Math.cos((30 * Math.PI) / 180), 3)
  })
})

describe('velocityTerm — score 用的反向项', () => {
  it('1 - velocityBias 关系', () => {
    // 同向 → 0
    expect(velocityTerm({ x: 50, y: 0 }, 'right')).toBeCloseTo(0, 5)
    // 反向 → 1
    expect(velocityTerm({ x: -50, y: 0 }, 'right')).toBe(1)
    // 静止 → 1
    expect(velocityTerm({ x: 0, y: 0 }, 'right')).toBe(1)
    expect(velocityTerm({ x: 5, y: 0 }, 'right')).toBe(1) // < V_MIN
    // 垂直 → 1
    expect(velocityTerm({ x: 0, y: 50 }, 'right')).toBeCloseTo(1, 5)
  })

  it('45 度同向 → 1 - √2/2 ≈ 0.293', () => {
    expect(velocityTerm({ x: 30, y: 30 }, 'right')).toBeCloseTo(1 - Math.SQRT1_2, 3)
  })
})

describe('常量稳定性（防意外改动）', () => {
  it('SPIKE_DX = 200', () => {
    expect(SPIKE_DX).toBe(200)
  })
  it('SPIKE_DT = 50', () => {
    expect(SPIKE_DT).toBe(50)
  })
  it('V_MIN = 10', () => {
    expect(V_MIN).toBe(10)
  })
  it('EWMA_ALPHA = 1/3', () => {
    expect(EWMA_ALPHA).toBeCloseTo(1 / 3, 5)
  })
})
