// 磁吸几何工具（ADR-020 *Updated 2026-05-18*）。
//
// 全部纯函数，零依赖；vitest 直接覆盖。坐标系：logical pixel，
// y 正向向下（屏幕坐标系，与 Tauri / DOM 一致）。
//
// 设计权衡：
// - projection overlap 算法用区间交集（max-min），简单可靠；不走"圆角 hit testing"那条路（容差大）
// - applyConstraint 假设 sourceEdge ↔ targetEdge 已经 opposite（candidates.ts 评分阶段保证）；
//   非 opposite 配对在此层是 garbage in garbage out，不做防御

import type { Edge, Rect } from './types'

// ───── 几何常量 ─────

/** 拖动期 source 边与 anchor 边距离小于此值 → 进入吸附 trigger zone。
 *  #30 follow-up D：60 → 10（业界 PowerToys/Photoshop 经验值）。
 *  #30 follow-up D revision：10 太严苛、用户体验差，调到 25（用户实测决策 2026-05-19）。
 *  #31 follow-up C：等价 ATTACH_ZONE（首次吸附阈值）。保留 TRIGGER_ZONE 名作向后兼容别名。 */
export const TRIGGER_ZONE = 25
/** #31 follow-up C：首次吸附（未 docked → docked）距离阈值。等于 TRIGGER_ZONE。 */
export const ATTACH_ZONE = TRIGGER_ZONE
/** #31 follow-up C：脱钩距离阈值（docked → 仍 docked 直到距离超过 DETACH_ZONE）。
 *  1.8× ATTACH（业界 1.5-2× hysteresis 甜点；schmitt trigger 同款思路）。
 *  #30 follow-up D：新流程 secondary 首帧 detach 后此字段事实上很少触达，但保留向后兼容
 *  现单测 + 给 M3 keepAttached modifier 留接口。
 *
 *  @deprecated 生产路径已绕过（secondary 拖动首帧 detachAll）。仅在 candidates.ts 的
 *    dockedTargetId 入参生效，M3 之前不要依赖此 hysteresis 行为。 */
export const DETACH_ZONE = 45
/** #31 follow-up C：commit 后 N ms 内即使 distance > DETACH_ZONE 也保持 docked（防 race + 给用户
 *  视觉确认时间）。借鉴 Apple Eyes HIG 230ms hover-feedback 阈值。
 *
 *  @deprecated 同 DETACH_ZONE，#30 follow-up D 后不在生产路径触达；保留供 candidates.ts 单测。 */
export const TIME_LOCKOUT_MS = 200
/** #31 follow-up C：Field halo 影响域半径。chat 距 pet < FIELD_RADIUS 就开始反馈。
 *  #30 follow-up D：2× ATTACH，halo 在最后冲刺阶段出现（25→50）。 */
export const FIELD_RADIUS = 50
/** 角落死区：两条相邻边都进入 trigger zone 时不触发，避免角落抖动。
 *  #30 follow-up D：CORNER_DEAD ≈ 0.32× ATTACH（25 时 8px）。 */
export const CORNER_DEAD = 8
/** projection overlap 必须 ≥ max(MIN_OVERLAP, sourceEdge_length × OVERLAP_RATIO) 才允许吸附 */
export const MIN_OVERLAP = 72
export const OVERLAP_RATIO = 0.25
/** 30 秒内 detach 的窗，再次靠近时 memoryBias 反向惩罚（-0.5） */
export const DETACH_RELUCTANCE_MS = 30_000
/** 已有 attachment 的窗，靠近时 memoryBias 正向加权（+0.5） */
export const EXISTING_BIAS = 0.5
export const DETACH_BIAS = -0.5
/** candidate scoring 权重（ADR-020 *Updated*：distance × 0.6 + overlapPenalty × 0.2 + (1 − memoryBias) × 0.2）。
 *  #31 follow-up C：加 W_VELOCITY × velocityTerm × 0.2（同向运动加分）。
 *  总权重 = 1.2，sort 单调性不受归一化影响，不显式除 1.2。 */
export const W_DISTANCE = 0.6
export const W_OVERLAP = 0.2
export const W_MEMORY = 0.2
/** #31 follow-up C：velocity bias 权重。limited to 0.2 modifier，不主导（plan §3）。 */
export const W_VELOCITY = 0.2

// ───── Rect 工具 ─────

export function rectCenter(r: Rect): { cx: number; cy: number } {
  return { cx: r.x + r.w / 2, cy: r.y + r.h / 2 }
}

/** 返回 rect 某条边的几何信息：
 *  - level: 该边所在的"主轴"坐标（垂直边 = x；水平边 = y）
 *  - start/end: 该边沿"次轴"的起止
 *  - length: end - start
 */
export interface EdgeGeometry {
  level: number
  start: number
  end: number
  length: number
}

export function rectEdgeGeometry(r: Rect, edge: Edge): EdgeGeometry {
  switch (edge) {
    case 'left':
      return { level: r.x, start: r.y, end: r.y + r.h, length: r.h }
    case 'right':
      return { level: r.x + r.w, start: r.y, end: r.y + r.h, length: r.h }
    case 'top':
      return { level: r.y, start: r.x, end: r.x + r.w, length: r.w }
    case 'bottom':
      return { level: r.y + r.h, start: r.x, end: r.x + r.w, length: r.w }
  }
}

/** sourceEdge 与 targetEdge 是否方向相反（snap 配对的几何前提）。 */
export function isOppositeEdge(s: Edge, t: Edge): boolean {
  return (
    (s === 'left' && t === 'right') ||
    (s === 'right' && t === 'left') ||
    (s === 'top' && t === 'bottom') ||
    (s === 'bottom' && t === 'top')
  )
}

/** sourceEdge 与 targetEdge 沿次轴方向的重叠长度（≥0）。
 *  调用方应先 isOppositeEdge 检查；否则结果几何上无意义。 */
export function projectionOverlap(
  source: Rect,
  target: Rect,
  sourceEdge: Edge,
  targetEdge: Edge,
): number {
  const s = rectEdgeGeometry(source, sourceEdge)
  const t = rectEdgeGeometry(target, targetEdge)
  return Math.max(0, Math.min(s.end, t.end) - Math.max(s.start, t.start))
}

/** 两条 opposite 边的 level 距离（主轴方向，可为负 — drag 过头时） */
export function edgeDistance(
  source: Rect,
  target: Rect,
  sourceEdge: Edge,
  targetEdge: Edge,
): number {
  const s = rectEdgeGeometry(source, sourceEdge)
  const t = rectEdgeGeometry(target, targetEdge)
  return Math.abs(s.level - t.level)
}

/** 给定 sourceRect / anchorRect / constraint，算出 source 应该到达的 final Rect。
 *  尺寸保留（仅锁位置）。sourceEdge / targetEdge 必须 opposite，由调用方保证。 */
export function applyConstraint(
  source: Rect,
  anchor: Rect,
  c: { sourceEdge: Edge; targetEdge: Edge; offset: number },
): Rect {
  switch (c.sourceEdge) {
    case 'left':
      // targetEdge 必是 right → anchor 右边 → source.x = anchor.x + anchor.w
      return { x: anchor.x + anchor.w, y: anchor.y + c.offset, w: source.w, h: source.h }
    case 'right':
      return { x: anchor.x - source.w, y: anchor.y + c.offset, w: source.w, h: source.h }
    case 'top':
      return { x: anchor.x + c.offset, y: anchor.y + anchor.h, w: source.w, h: source.h }
    case 'bottom':
      return { x: anchor.x + c.offset, y: anchor.y - source.h, w: source.w, h: source.h }
  }
}

/** projection overlap 阈值：max(MIN_OVERLAP, edge_length × OVERLAP_RATIO)。 */
export function overlapThreshold(edgeLength: number): number {
  return Math.max(MIN_OVERLAP, edgeLength * OVERLAP_RATIO)
}

/** 当前 source 是否在 anchor 的某条边的"角落死区"内（两条相邻边都触发 trigger zone）。 */
export function inCornerDead(source: Rect, anchor: Rect): boolean {
  // 检查 source 中心到 anchor 4 个角的最小距离，<= CORNER_DEAD 视为死区
  const sc = rectCenter(source)
  const corners = [
    { x: anchor.x, y: anchor.y },
    { x: anchor.x + anchor.w, y: anchor.y },
    { x: anchor.x, y: anchor.y + anchor.h },
    { x: anchor.x + anchor.w, y: anchor.y + anchor.h },
  ]
  return corners.some((c) => {
    const dx = sc.cx - c.x
    const dy = sc.cy - c.y
    return Math.sqrt(dx * dx + dy * dy) <= CORNER_DEAD
  })
}
