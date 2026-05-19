// 用户运动意图（velocity）跟踪与 candidate 评分偏置（#31 follow-up C Phase C）。
//
// 理论背景：Lank CHI'07 kinematic prediction 证 velocity 预测精度高 2×。
// 本模块把"用户运动方向"接入 candidate 评分：
//   - 同方向 candidate（user 正朝那条边甩）→ 评分更优先
//   - 垂直 / 反向 candidate → 不奖励（但也不惩罚，避免误伤）
//   - 静止 → 无 bias（回退到纯 distance + overlap + memory 评分）
//
// 设计权衡（plan §3 ChatGPT critique 已采纳）：
//   - velocity 限为 0.2 modifier 而非主导（其余 0.8 仍是 distance + overlap + memory）
//   - 5 帧 EWMA 平滑（α ≈ 0.333）防瞬时方向抖动
//   - V_MIN = 10 px/frame 静止阈，停下 → velocityBias=0
//   - 跨屏 spike 丢弃：单帧 |dx|>200px 或 dt>50ms 视坏帧，不更新 _v 但更新参考帧
//
// 全部纯函数 + 单 class；reactive 由 caller 管理。

import type { Edge } from './types'

export interface Vec2 {
  x: number
  y: number
}

/** 单帧位移阈值（px）。> 此值视为跨屏 spike → 丢弃此帧 velocity 更新。 */
export const SPIKE_DX = 200
/** 单帧时间阈值（ms）。> 此值视为 spike（窗口失焦后突然再拖）。 */
export const SPIKE_DT = 50
/** velocity magnitude 低于此值视为静止 → velocityBias 返 0。
 *  10 px/frame ≈ 600 px/s at 60Hz，桌面缓慢拖动也能识别（典型 ≥ 300 px/s）。 */
export const V_MIN = 10
/** 5 帧 EWMA 系数 α = 2/(N+1) = 2/6 ≈ 0.333。
 *  新帧权重 0.333，过去 4 帧累计权重 0.667；平滑但不滞后过多。 */
export const EWMA_ALPHA = 1 / 3

/** 5 帧 EWMA velocity tracker。每帧 update(x, y, now) → 返当前平滑后 velocity (px/frame)。
 *  spike 帧（跨屏跳变 / 长时间间隔）跳过 EWMA 但更新参考帧，避免无限 spike。 */
export class VelocityTracker {
  private _lastPos: Vec2 | null = null
  private _lastTime = 0
  private _v: Vec2 = { x: 0, y: 0 }

  /** 喂一帧 (x, y, now)。返回当前 EWMA 平滑后的 velocity。
   *  - 第一帧（_lastPos==null）：仅记录参考，返 (0,0)
   *  - spike 帧：更新参考帧但不更新 _v，返上一帧 _v
   *  - 正常帧：α × raw + (1-α) × _v_prev */
  update(x: number, y: number, now: number): Vec2 {
    if (this._lastPos === null) {
      this._lastPos = { x, y }
      this._lastTime = now
      return this._v
    }
    const dt = now - this._lastTime
    const dx = x - this._lastPos.x
    const dy = y - this._lastPos.y
    if (dt > SPIKE_DT || Math.abs(dx) > SPIKE_DX || Math.abs(dy) > SPIKE_DX) {
      // spike：更新参考帧但保留 _v，避免下一帧 dx 再次被识别为 spike
      this._lastPos = { x, y }
      this._lastTime = now
      return this._v
    }
    // EWMA: v_new = α × raw + (1-α) × v_prev
    this._v = {
      x: EWMA_ALPHA * dx + (1 - EWMA_ALPHA) * this._v.x,
      y: EWMA_ALPHA * dy + (1 - EWMA_ALPHA) * this._v.y,
    }
    this._lastPos = { x, y }
    this._lastTime = now
    return this._v
  }

  /** 当前 EWMA velocity（不喂新帧；用于 caller 在 onMove 之外读 v）。 */
  get velocity(): Vec2 {
    return { x: this._v.x, y: this._v.y }
  }

  /** velocity magnitude。静止判定用 < V_MIN。 */
  get speed(): number {
    return Math.sqrt(this._v.x * this._v.x + this._v.y * this._v.y)
  }

  /** 显式 reset：mouseup 时清状态，下次拖动从 0 重启。 */
  reset(): void {
    this._lastPos = null
    this._lastTime = 0
    this._v = { x: 0, y: 0 }
  }
}

/** sourceEdge → "source 整体朝哪个方向移动" 单位向量。
 *  source.right ↔ target.left 配对意味着 source 把 right 边贴向 target → source 整体 +x 移动。
 *  其他三向同理。 */
function sourceEdgeToDirection(edge: Edge): Vec2 {
  switch (edge) {
    case 'right':
      return { x: 1, y: 0 } // source.right → target.left，source 朝 +x
    case 'left':
      return { x: -1, y: 0 }
    case 'bottom':
      return { x: 0, y: 1 }
    case 'top':
      return { x: 0, y: -1 }
  }
}

/** velocityBias: source 当前速度与 sourceEdge 方向的 cosine 对齐度 ∈ [0, 1]。
 *  - 1 = 完全同向（"我正朝这条边的方向甩"）
 *  - 0 = 垂直 / 反向 / 静止
 *
 *  反向 cos<0 截断到 0：不惩罚逆向运动，只奖励同向 — 避免"拖回来"时 score 突然变差。 */
export function velocityBias(v: Vec2, sourceEdge: Edge): number {
  const speed = Math.sqrt(v.x * v.x + v.y * v.y)
  if (speed < V_MIN) return 0
  const dir = sourceEdgeToDirection(sourceEdge)
  const cos = (v.x * dir.x + v.y * dir.y) / speed
  return Math.max(0, cos)
}

/** velocityTerm: score 中加权项 (1 - velocityBias) ∈ [0, 1]。
 *  与 memoryTerm 同模型，越小越优先选（candidate）：
 *  - 静止（velocityBias=0）→ 1（无偏向，与"无 velocity 信息"等价）
 *  - 完全同向（velocityBias=1）→ 0（score 不增，candidate 更优先）
 *  - 垂直（velocityBias=0）→ 1（同静止处理）
 *
 *  candidates.ts score 公式：
 *    score = distNorm × 0.6 + overlapPenalty × 0.2 + memoryTerm × 0.2 + velocityTerm × 0.2
 *  （总权重 1.2，sort 单调性不受归一化影响 — 不显式除 1.2） */
export function velocityTerm(v: Vec2, sourceEdge: Edge): number {
  return 1 - velocityBias(v, sourceEdge)
}
