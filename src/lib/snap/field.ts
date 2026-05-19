// Field 影响域计算（#31 follow-up C Phase A）。
//
// 理论背景（research finding）：
// - Disney anticipation principle：UI 反馈必须先于动作（NN/g pre-attach signaling）
// - 现 ATTACH_ZONE 60px 是吸附触发阈值，但用户在 60-120px 区间也应感知到"磁场存在"
// - field intensity 渐进 [0,1]：120px 外为 0，60px 内（已进入吸附区）为 1
//
// 设计要点：
// - 用 edge-to-edge 最近距离（与 candidates 同款几何）而不是中心距，对透明窗更直观
// - 同时考虑所有 visible 窗，取最近的；多窗时只对最近窗 highlight 一次（避免视觉杂乱）
// - 仅在拖动时计算（caller 控制；非拖动期不广播，省 emit）
// - 完全纯函数；reactive ref 由 useSnapWindow 维护

import { ATTACH_ZONE, edgeDistance, FIELD_RADIUS, projectionOverlap } from './geometry'
import type { Edge, Rect, WindowRegistration } from './types'

/** Field intensity ∈ [0, 1]：1 - distance / FIELD_RADIUS。
 *  - distance ≤ ATTACH_ZONE (60)：直接 1.0（已在吸附区，最强反馈）
 *  - ATTACH_ZONE < distance ≤ FIELD_RADIUS (120)：渐进 0→1
 *  - distance > FIELD_RADIUS：0（远离）
 *
 *  渐进段映射：x ∈ (60, 120] → intensity ∈ (0, 1) 线性外推。
 *  注：吸附区内固定 1.0 而非"撞顶 1.0" — 避免吸附后视觉抖动。 */
export function fieldIntensityFromDistance(distance: number): number {
  if (distance <= ATTACH_ZONE) return 1
  if (distance >= FIELD_RADIUS) return 0
  // 在 [ATTACH_ZONE, FIELD_RADIUS] 区间线性映射到 [1, 0]（注意：x=60→1, x=120→0）
  return 1 - (distance - ATTACH_ZONE) / (FIELD_RADIUS - ATTACH_ZONE)
}

/** 返回 source rect 到 anchor rect 的"对接距离"：找 4 个 opposite 边对中
 *  overlap > 0 的最小 edgeDistance；若没有任何 opposite 边对有 overlap（如对角错开），
 *  返回 Infinity（无 field 反馈）。
 *
 *  与 candidates.ts 同款几何，但不做 score / corner dead 过滤 — field 范围更宽容。 */
export function nearestEdgeDistance(source: Rect, anchor: Rect): number {
  const EDGE_PAIRS: ReadonlyArray<readonly [Edge, Edge]> = [
    ['left', 'right'],
    ['right', 'left'],
    ['top', 'bottom'],
    ['bottom', 'top'],
  ]
  let best = Number.POSITIVE_INFINITY
  for (const [sEdge, tEdge] of EDGE_PAIRS) {
    const overlap = projectionOverlap(source, anchor, sEdge, tEdge)
    if (overlap <= 0) continue
    const d = edgeDistance(source, anchor, sEdge, tEdge)
    if (d < best) best = d
  }
  return best
}

/** 给 source rect + 所有 registered visible 窗，算 field 应显示的最强 intensity（取最近窗）。
 *  返回 { intensity, anchorId }，无 anchor 在场或全部超 FIELD_RADIUS 时 intensity=0 / anchorId=null。 */
export function computeFieldIntensity(
  sourceId: string,
  sourceRect: Rect,
  registry: ReadonlyArray<WindowRegistration>,
): { intensity: number; anchorId: string | null } {
  let bestDist = Number.POSITIVE_INFINITY
  let bestId: string | null = null
  for (const w of registry) {
    if (w.id === sourceId) continue
    if (!w.visible) continue
    const d = nearestEdgeDistance(sourceRect, w.rect)
    if (d < bestDist) {
      bestDist = d
      bestId = w.id
    }
  }
  const intensity = fieldIntensityFromDistance(bestDist)
  return { intensity, anchorId: intensity > 0 ? bestId : null }
}

/** 跨 webview event 名：source 端 emit field intensity 给 anchor 端显示 halo。
 *  payload: { sourceId, anchorId | null, intensity ∈ [0,1] } */
export const FIELD_INTENSITY_EVT = 'snap:field-intensity'

export interface FieldIntensityPayload {
  sourceId: string
  anchorId: string | null
  intensity: number
}
