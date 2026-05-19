// Candidate evaluation（ADR-020 *Updated 2026-05-18*）。
//
// 给定 source rect + windowRegistry，找出所有满足 trigger zone 的合格 candidate
// 并按 score 升序排序。dragSession 在 Dragging/PreviewSnap 状态下每 onMoved 调用一次。
//
// 评分公式（来自 ADR-020 *Updated*，#31 follow-up C 扩展 velocity 项）：
//   score = distance_norm × 0.6
//         + overlapPenalty × 0.2
//         + (1 − memoryBias) × 0.2
//         + velocityTerm × 0.2
//
// 各项含义：
// - distance_norm: edgeDistance / TRIGGER_ZONE，clamp [0,1]，越小越好
// - overlapPenalty: 1 − overlap / threshold，clamp [0,1]，重叠越多越接近 0
// - memoryBias: +0.5（已存在 attachment）/ -0.5（30s 内 detach）/ 0（既无既无）
//   → (1 − memoryBias) ∈ [0.5, 1.5]，× 0.2 ∈ [0.1, 0.3]
// - velocityTerm (Phase C): 1 - cos(v, sEdge_direction) ∈ [0, 1]，静止时为 1（中性）。
//   同向运动 → 0（candidate 减分更优先）；垂直 / 反向 → 1（无奖励，不惩罚）。

import { constraintStore } from './constraintStore'
import {
  applyConstraint,
  ATTACH_ZONE,
  DETACH_ZONE,
  DETACH_BIAS,
  DETACH_RELUCTANCE_MS,
  EXISTING_BIAS,
  edgeDistance,
  inCornerDead,
  overlapThreshold,
  projectionOverlap,
  rectEdgeGeometry,
  TIME_LOCKOUT_MS,
  TRIGGER_ZONE,
  W_DISTANCE,
  W_MEMORY,
  W_OVERLAP,
  W_VELOCITY,
} from './geometry'
import { velocityTerm, type Vec2 } from './intent'
import type { Edge, Rect, SnapCandidate, WindowRegistration } from './types'

const EDGE_PAIRS: ReadonlyArray<readonly [Edge, Edge]> = [
  ['left', 'right'],
  ['right', 'left'],
  ['top', 'bottom'],
  ['bottom', 'top'],
] as const

function clamp01(v: number): number {
  return Math.min(1, Math.max(0, v))
}

// ───── DetachHistory ─────
// 单例：source→target 维度记录最近 detach 时间，30s 内再次靠近时 candidates 评分扣分。
//
// 设计：内存中 Map，进程重启清空（30s 短 TTL 不值得持久化）。

class DetachHistory {
  private _map = new Map<string, number>()

  static key(sourceId: string, targetId: string): string {
    return `${sourceId}->${targetId}`
  }

  recordDetach(sourceId: string, targetId: string, at: number = Date.now()): void {
    this._map.set(DetachHistory.key(sourceId, targetId), at)
  }

  isRecent(sourceId: string, targetId: string, now: number = Date.now()): boolean {
    const at = this._map.get(DetachHistory.key(sourceId, targetId))
    if (at === undefined) return false
    return now - at < DETACH_RELUCTANCE_MS
  }

  clear(): void {
    this._map.clear()
  }

  /** 测试 helper：当前条数 */
  size(): number {
    return this._map.size
  }
}

export const detachHistory = new DetachHistory()
export { DetachHistory }

// ───── findCandidates ─────

export interface FindCandidatesOptions {
  /** ms epoch，测试时注入；默认 Date.now() */
  now?: number
  /** 测试 / cross-window 同步时注入自定义 store，默认 module-level constraintStore */
  existingConstraintTargetId?: string
  /** 测试时注入自定义 history */
  detachHistoryInstance?: DetachHistory
  /** #31 follow-up C：当前 source 已 docked 到的 targetId（来自 constraintStore.get(sourceId)?.targetId）。
   *  传入后：对该 target 用 DETACH_ZONE（100）放宽距离阈值实现 hysteresis；其他 target 仍用 ATTACH_ZONE。
   *  undefined 表示未 docked（所有 target 走 ATTACH_ZONE）。 */
  dockedTargetId?: string
  /** #31 follow-up C：上次 commit 时间戳（来自 constraintStore.get(sourceId)?.createdAt）。
   *  now - createdAt < TIME_LOCKOUT_MS 时，dockedTargetId 永不脱钩（zone = Infinity），
   *  防止 commit 后 race 立即脱开 + 给用户视觉确认时间。 */
  dockedAt?: number
  /** #31 follow-up C Phase C：source 当前 EWMA 平滑后的 velocity (px/frame)。
   *  传入后：每个 edge pair 的 score 加 velocityTerm(v, sEdge) × W_VELOCITY（同向 -0.2，垂直/反向 +0）。
   *  undefined / 静止 → 与无 velocity 信息等价（velocityTerm=1，加权 +0.2 中性项 — 不影响相对排序）。 */
  velocity?: Vec2
}

/** 给定 source rect 与 windowRegistry，返回所有合格 candidate 按 score 升序排列。
 *
 *  过滤规则：
 *  - 跳过 self（target.id === sourceId）
 *  - 跳过 !visible
 *  - 跳过 inCornerDead（防角落抖动）
 *  - 跳过 edgeDistance > TRIGGER_ZONE
 *  - 跳过 overlap < overlapThreshold(sourceEdge.length)
 */
export function findCandidates(
  sourceId: string,
  sourceRect: Rect,
  registry: ReadonlyArray<WindowRegistration>,
  opts: FindCandidatesOptions = {},
): SnapCandidate[] {
  const now = opts.now ?? Date.now()
  const history = opts.detachHistoryInstance ?? detachHistory
  const existingTargetId =
    opts.existingConstraintTargetId ?? constraintStore.get(sourceId)?.targetId
  // #31 follow-up C：hysteresis 状态推导
  // - dockedTargetId 显式传入 优先；缺省值从 existingTargetId 反推（向后兼容现 caller）
  const dockedTargetId = opts.dockedTargetId ?? existingTargetId ?? null
  // - 200ms time lockout：dockedAt 显式 / 从 store.createdAt 反推
  const dockedAt = opts.dockedAt ?? constraintStore.get(sourceId)?.createdAt
  const timeLocked =
    dockedAt !== undefined && now - dockedAt < TIME_LOCKOUT_MS

  const out: SnapCandidate[] = []

  for (const target of registry) {
    if (target.id === sourceId) continue
    if (!target.visible) continue
    if (inCornerDead(sourceRect, target.rect)) continue

    // #31 follow-up C：per-target distance zone（hysteresis 核心）
    // - target 是当前 docked 目标 + 在 200ms time lockout 内 → Infinity（永不放手）
    // - target 是当前 docked 目标（已超 lockout）→ DETACH_ZONE 100（粘性，需拖更远才脱）
    // - target 是其他窗 → ATTACH_ZONE 60（首次吸附阈值）
    let zone: number
    if (target.id === dockedTargetId) {
      zone = timeLocked ? Number.POSITIVE_INFINITY : DETACH_ZONE
    } else {
      zone = ATTACH_ZONE
    }

    for (const [sEdge, tEdge] of EDGE_PAIRS) {
      const distance = edgeDistance(sourceRect, target.rect, sEdge, tEdge)
      if (distance > zone) continue

      const sGeo = rectEdgeGeometry(sourceRect, sEdge)
      const overlap = projectionOverlap(sourceRect, target.rect, sEdge, tEdge)
      const threshold = overlapThreshold(sGeo.length)
      if (overlap < threshold) continue

      // offset 反推：candidates 评分阶段就把"切向偏移"算好，commit 时直接用
      // 例：sourceEdge='right' / targetEdge='left' → 两条都垂直边，offset 是 y 方向
      //     offset = source.y - anchor.y = sGeo.start - tGeo.start
      const tGeo = rectEdgeGeometry(target.rect, tEdge)
      const offset = sGeo.start - tGeo.start

      const finalRect = applyConstraint(sourceRect, target.rect, {
        sourceEdge: sEdge,
        targetEdge: tEdge,
        offset,
      })

      const distNorm = clamp01(distance / TRIGGER_ZONE)
      const overlapPenalty = clamp01(1 - overlap / threshold)

      let memoryBias = 0
      if (existingTargetId === target.id) {
        memoryBias = EXISTING_BIAS
      } else if (history.isRecent(sourceId, target.id, now)) {
        memoryBias = DETACH_BIAS
      }
      const memoryTerm = 1 - memoryBias // ∈ [0.5, 1.5]

      // #31 follow-up C Phase C：velocity 同向偏置。
      // velocity undefined / 静止 → velocityTerm=1（与 memoryTerm 同模型；中性项不影响排序，
      // 只在 velocity 同向时减分 → candidate 更优先）。
      const vTerm = opts.velocity ? velocityTerm(opts.velocity, sEdge) : 1

      const score =
        distNorm * W_DISTANCE +
        overlapPenalty * W_OVERLAP +
        memoryTerm * W_MEMORY +
        vTerm * W_VELOCITY

      out.push({
        movingId: sourceId,
        targetId: target.id,
        sourceEdge: sEdge,
        targetEdge: tEdge,
        offset,
        finalRect,
        score,
        // T8 (#31 follow-up B)：UI 用此算渐进 intensity（沿对接边 glow）
        distance,
      })
    }
  }

  out.sort((a, b) => a.score - b.score)
  return out
}

// ───── findReverseAttract（#30 follow-up D） ─────
//
// 反向吸引：当 primary（pet）拖动且 pet 没有 dependents 时，从 secondary 视角找它们
// 该不该吸到 primary 上。对每个 visible secondary，调用 findCandidates(secondaryId,
// secondaryRect, [primaryReg])（registry 仅含 primary），把得到的 candidates 合并 + sort。
// candidate.movingId === 该 secondary id，candidate.targetId === primary id。
//
// 设计要点：
// - 不传 velocity（primary drag 时 velocity 是 primary 的，反向语义复杂；MVP 不接）
// - secondary 自己已有 constraint（出向）时 → 评分含 EXISTING/DETACH memoryBias（与正向一致）
// - primary-attract 在 commit 时只取最佳一个（findReverseAttract 返 sorted list，caller 取 [0]）
// - 已 attached 到该 primary 的 secondary 不会进 candidate list（被 dragSession.commit 写入时
//   replace 同 source 的旧 constraint，事实上等价 no-op 不影响）

export function findReverseAttract(
  primaryId: string,
  primaryRect: Rect,
  registry: ReadonlyArray<WindowRegistration>,
  opts: Pick<FindCandidatesOptions, 'now' | 'detachHistoryInstance'> = {},
): SnapCandidate[] {
  const primaryReg: WindowRegistration = { id: primaryId, rect: primaryRect, visible: true }
  const out: SnapCandidate[] = []
  for (const sec of registry) {
    if (sec.id === primaryId) continue
    if (!sec.visible) continue
    // 仅 primary 作为可吸附 target 的 mini registry
    const cands = findCandidates(sec.id, sec.rect, [primaryReg], opts)
    for (const c of cands) out.push(c)
  }
  out.sort((a, b) => a.score - b.score)
  return out
}
