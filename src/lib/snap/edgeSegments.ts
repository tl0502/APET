// Edge-segment occupancy（#30 follow-up F）。
//
// 解决两个互相纠缠的问题：
//   1. 占用检测：A.right 已被 B 吸 → C 再吸 A.right 不能与 B 重叠
//   2. 错位磁吸：A 边长 300、B 占 [0,100)，C 拖近 A 时自动滑入 [100,300) 空段
//
// 模型：每条 target 边按"切向轴"（垂直边 = y、水平边 = x）形成 [0, edgeLength]
// 区间，已吸附 source 各占用 [srcStart, srcEnd] 一段。candidate 评估时按 source
// 投影位置查询"该位置是否落在空段 + 空段长度是否够 overlap threshold"，必要时
// 把 source 的切向位置滑入相邻空段（offset 自动调整）。
//
// 设计决策：
// - 区段坐标系是"target 边的 local 切向坐标"（0 = anchor start, length = anchor end），
//   而非世界坐标——这样 anchor 移动时区段不需重算（target rect 变化时只重算 source projection）
// - 空段查找首选"包含 source 当前投影中心"的段（鼠标在哪 → 滑入哪），fallback 最近段
// - allowSelf：caller 传 currentSourceId，refresh / re-attach 时排除自己的旧占用（否则一定冲突）

import { applyVisualInset, rectEdgeGeometry } from './geometry'
import type { Edge, Rect, SnapConstraint, WindowRegistration } from './types'

/** 切向区段：[start, end]，target 边 local 坐标系（0 = 边起点）。 */
export interface Segment {
  start: number
  end: number
}

/** target 某条边上已被占用的 source 投影区段。
 *  按 start 升序、不重叠（合并）。 */
export interface EdgeOccupancy {
  /** target 边总长（垂直边 = target.h；水平边 = target.w） */
  totalLength: number
  /** 合并后的占用区段，按 start 升序 */
  occupied: Segment[]
}

/** 把 source 在 target 边上的投影换算成 target-local 切向区段（clamp 到 [0, totalLength]）。
 *  - 垂直边（left/right）：投影是 source.y .. source.y+source.h，local = source.y - target.y
 *  - 水平边（top/bottom）：投影是 source.x .. source.x+source.w，local = source.x - target.x
 *  完全落在边外（剪切后 end ≤ start）→ 返 null。 */
export function projectSourceOntoEdge(
  source: Rect,
  target: Rect,
  targetEdge: Edge,
): Segment | null {
  let rawStart: number
  let rawEnd: number
  let total: number
  if (targetEdge === 'left' || targetEdge === 'right') {
    rawStart = source.y - target.y
    rawEnd = source.y + source.h - target.y
    total = target.h
  } else {
    rawStart = source.x - target.x
    rawEnd = source.x + source.w - target.x
    total = target.w
  }
  const start = Math.max(0, Math.min(total, rawStart))
  const end = Math.max(0, Math.min(total, rawEnd))
  if (end - start < 1) return null // 小于 1px 视作不占用（容差）
  return { start, end }
}

/** 计算 target 某条边上的占用情况（聚合所有 `c.targetId === targetId && c.targetEdge === edge`
 *  的 constraints）。
 *  - excludeSourceId：评估某 source 的 candidate 时排除自己的旧占用
 *  - 返回区段已按 start 升序、相邻重叠合并
 *  - #30 follow-up F：内部统一用 visualRect（target / source 都按各自 visualInset 内缩），
 *    与 candidates.ts 评估时的 projection 坐标系一致 */
export function computeEdgeOccupancy(
  targetId: string,
  targetEdge: Edge,
  constraints: ReadonlyArray<SnapConstraint>,
  registry: ReadonlyArray<WindowRegistration>,
  excludeSourceId?: string,
): EdgeOccupancy {
  const targetReg = registry.find((w) => w.id === targetId)
  const targetVisualRect = targetReg
    ? applyVisualInset(targetReg.rect, targetReg.visualInset)
    : null
  const totalLength = targetVisualRect ? rectEdgeGeometry(targetVisualRect, targetEdge).length : 0

  if (!targetReg || !targetVisualRect || totalLength <= 0) {
    return { totalLength, occupied: [] }
  }

  const raw: Segment[] = []
  for (const c of constraints) {
    if (c.targetId !== targetId) continue
    if (c.targetEdge !== targetEdge) continue
    if (excludeSourceId !== undefined && c.sourceId === excludeSourceId) continue
    const src = registry.find((w) => w.id === c.sourceId)
    if (!src || !src.visible) continue
    const srcVisualRect = applyVisualInset(src.rect, src.visualInset)
    const seg = projectSourceOntoEdge(srcVisualRect, targetVisualRect, targetEdge)
    if (seg) raw.push(seg)
  }
  return { totalLength, occupied: mergeSegments(raw) }
}

/** 合并重叠 / 相邻区段（按 start 升序输出）。 */
export function mergeSegments(segs: ReadonlyArray<Segment>): Segment[] {
  if (segs.length === 0) return []
  const sorted = [...segs].sort((a, b) => a.start - b.start)
  const out: Segment[] = [{ ...sorted[0]! }]
  for (let i = 1; i < sorted.length; i++) {
    const cur = sorted[i]!
    const last = out[out.length - 1]!
    if (cur.start <= last.end) {
      // 重叠 / 相邻（含相切）→ 合并
      last.end = Math.max(last.end, cur.end)
    } else {
      out.push({ ...cur })
    }
  }
  return out
}

/** 取占用的补集（free 区段），按 start 升序。 */
export function freeSegments(occ: EdgeOccupancy): Segment[] {
  const out: Segment[] = []
  let cursor = 0
  for (const seg of occ.occupied) {
    if (seg.start > cursor) out.push({ start: cursor, end: seg.start })
    cursor = Math.max(cursor, seg.end)
  }
  if (cursor < occ.totalLength) out.push({ start: cursor, end: occ.totalLength })
  return out
}

/** 给定 source 投影 + 边占用 + 所需长度，在 free 区段中找最佳放置点。
 *
 *  策略（按优先级）：
 *  1. source 当前投影完整落在某个 free 段 → 不动（offset 不变）
 *  2. source 投影中心点落在某个 free 段（但部分越界）→ 尝试在该段内"贴边滑入"
 *     （要么把 source.start 推到 free.start，要么把 source.end 拉到 free.end）
 *  3. 投影中心在 occupied 段 → 找"最近能容纳 neededLength"的 free 段，对齐其 start
 *  4. 没有任何 free 段长度 ≥ neededLength → 返 null
 *
 *  返回值：source 在该边的"目标 start 位置"（target-local 切向坐标）。
 *  caller 用此减去原 projection.start 即得 offset 调整量。 */
export function findFreePlacement(
  projection: Segment,
  occ: EdgeOccupancy,
  neededLength: number,
): number | null {
  const free = freeSegments(occ)
  const projLen = projection.end - projection.start
  const projCenter = (projection.start + projection.end) / 2

  if (neededLength > occ.totalLength) return null

  // 1. 完整落在某段
  for (const seg of free) {
    if (projection.start >= seg.start && projection.end <= seg.end) {
      return projection.start
    }
  }

  // 2. 中心在某 free 段 → 段内贴边滑入（保持 projLen 不变）
  for (const seg of free) {
    if (projCenter >= seg.start && projCenter <= seg.end) {
      const segLen = seg.end - seg.start
      if (segLen < neededLength) continue // 该段太窄
      // 尽量保持原相对位置：若靠左越界推右；靠右越界拉左
      if (projection.start < seg.start) return seg.start
      if (projection.end > seg.end) return seg.end - projLen
      return projection.start
    }
  }

  // 3. 找最近的"能容纳 neededLength"的 free 段（按段中心距 projCenter 排序）
  const candidates = free
    .filter((s) => s.end - s.start >= neededLength)
    .map((s) => ({
      seg: s,
      dist: Math.abs((s.start + s.end) / 2 - projCenter),
    }))
    .sort((a, b) => a.dist - b.dist)
  if (candidates.length === 0) return null
  const best = candidates[0]!.seg
  // 对齐到段的近端：projCenter 比段中心小 → 贴 start；否则贴 end - projLen
  if (projCenter <= (best.start + best.end) / 2) return best.start
  return best.end - projLen
}
