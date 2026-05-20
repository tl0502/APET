// Forest-walk BFS solver（ADR-020 *Updated 2026-05-18*）。
//
// 不变量保证：I1 (每 source 单 constraint) + I2 (constraintStore 写入前 wouldCycle 检查)
// → 整个约束图必为森林 → BFS 不会重复访问同一节点 → 无需 Kahn topo sort。
//
// 用法：onMoved 触发时，调 solve([movedWindowId])，结果 map 是 {sourceId: newRect}，
// 调用方逐项 safeSetPosition() 更新真实窗口位置（safeSetPosition 内部 isInternalMove
// guard 防 setPosition → onMoved → solve 死循环）。
//
// 复杂度：O(N + E)，N = 节点数（窗口数），E = 边数（constraint 数）。
// 单人项目 N ≤ 6 → 常数级。

import { constraintStore as defaultStore, type ConstraintStore } from './constraintStore'
import { applyConstraint, applyVisualInset, reverseVisualInset } from './geometry'
import type { Rect } from './types'
import { windowRegistry as defaultRegistry, type WindowRegistry } from './windowRegistry'

export interface SolveDeps {
  registry?: WindowRegistry
  store?: ConstraintStore
}

/** 给定一组"已变更位置"的 root id，沿 constraint forest 向下推 dependent 窗的新 Rect。
 *
 *  - changedRoots 内的 id 不出现在返回 map 中（root 已由 caller 设位）
 *  - 仅 visible 的 source 才会被推算（避免 hidden 窗位置漂移）
 *  - anchor 缺失（registry 无对应记录 + newRects 无） → 该 constraint 跳过（degenerate）
 *
 *  返回 map 是 dependent 窗的新 Rect，caller 应：
 *  ```
 *  for (const [id, rect] of solve([changedId])) await safeSetPosition(id, rect.x, rect.y)
 *  ```
 */
export function solve(
  changedRoots: ReadonlyArray<string>,
  deps: SolveDeps = {},
): Map<string, Rect> {
  const registry = deps.registry ?? defaultRegistry
  const store = deps.store ?? defaultStore

  const newRects = new Map<string, Rect>()
  const queue: string[] = [...changedRoots]
  const visited = new Set<string>()

  while (queue.length > 0) {
    const id = queue.shift()!
    if (visited.has(id)) continue
    visited.add(id)

    const deps2 = store.dependentsOf(id)
    for (const c of deps2) {
      const sourceWin = registry.get(c.sourceId)
      // anchor rect 优先用 BFS 期间已计算的新位置（newRects），fallback 当前 registry 中的位置
      const anchorOsRect = newRects.get(c.targetId) ?? registry.get(c.targetId)?.rect
      if (!sourceWin || !anchorOsRect || !sourceWin.visible) continue
      if (!c.enabled) continue

      // #30 follow-up F：solver 也用 visual rect 算 applyConstraint，与 candidates.ts 同坐标系
      // 避免 "candidates 路径无 padding 间隙、solver 路径有 padding 间隙" 的不一致。
      // - anchor inset 优先从 registry 取（newRects 不带 inset，但同 anchor 的 inset 不变）
      const anchorReg = registry.get(c.targetId)
      const anchorVisualRect = applyVisualInset(anchorOsRect, anchorReg?.visualInset)
      const sourceVisualRect = applyVisualInset(sourceWin.rect, sourceWin.visualInset)

      const finalVisualRect = applyConstraint(sourceVisualRect, anchorVisualRect, c)
      const computed = reverseVisualInset(finalVisualRect, sourceWin.visualInset)
      newRects.set(c.sourceId, computed)
      queue.push(c.sourceId)
    }
  }

  return newRects
}
