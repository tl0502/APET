// Constraint store（ADR-020 *Updated 2026-05-18* I1 + I2）。
//
// 不变量：
// - I1：每个 source 至多 1 个 constraint（同 source 第二次 set 替换前一个）
// - I2：commit 前 wouldCycle 检查 — 沿 attachedTo 链 (target → target's target → ...) 追到 self 即 reject
//
// I1 + I2 → 图必为森林（无重复出向边 + 无环）→ solver 不需要 Kahn topo sort，BFS 即足够。

import type { SnapConstraint } from './types'

export type SetResult = { ok: true } | { ok: false; reason: 'cycle' | 'self-loop' }

class ConstraintStore {
  /** sourceId → constraint（I1：每 source 单出向边） */
  private _bySource = new Map<string, SnapConstraint>()
  /** targetId → Set<sourceId>（反向索引，dependentsOf 用） */
  private _byTarget = new Map<string, Set<string>>()

  /** 写入 constraint。
   *  - sourceId === targetId → reject 'self-loop'
   *  - wouldCycle → reject 'cycle'
   *  - 同 source 已有 constraint → 替换（先清旧的反向索引） */
  set(c: SnapConstraint): SetResult {
    if (c.sourceId === c.targetId) return { ok: false, reason: 'self-loop' }
    if (this.wouldCycle(c.sourceId, c.targetId)) return { ok: false, reason: 'cycle' }

    const old = this._bySource.get(c.sourceId)
    if (old) {
      const targetSet = this._byTarget.get(old.targetId)
      targetSet?.delete(c.sourceId)
      if (targetSet && targetSet.size === 0) this._byTarget.delete(old.targetId)
    }

    this._bySource.set(c.sourceId, c)
    let bucket = this._byTarget.get(c.targetId)
    if (!bucket) {
      bucket = new Set()
      this._byTarget.set(c.targetId, bucket)
    }
    bucket.add(c.sourceId)

    return { ok: true }
  }

  get(sourceId: string): SnapConstraint | undefined {
    return this._bySource.get(sourceId)
  }

  /** 删除 sourceId 的 constraint。返回是否实际删除。 */
  delete(sourceId: string): boolean {
    const c = this._bySource.get(sourceId)
    if (!c) return false
    this._bySource.delete(sourceId)
    const targetSet = this._byTarget.get(c.targetId)
    targetSet?.delete(sourceId)
    if (targetSet && targetSet.size === 0) this._byTarget.delete(c.targetId)
    return true
  }

  list(): SnapConstraint[] {
    return Array.from(this._bySource.values())
  }

  /** 所有 target===id 的 constraints（id 变了时 solver 要推这些 source）。 */
  dependentsOf(targetId: string): SnapConstraint[] {
    const sourceIds = this._byTarget.get(targetId)
    if (!sourceIds || sourceIds.size === 0) return []
    const out: SnapConstraint[] = []
    for (const sid of sourceIds) {
      const c = this._bySource.get(sid)
      if (c) out.push(c)
    }
    return out
  }

  /** 沿 attachedTo 链向上追：target → target's target → ...，到 sourceId 即环。
   *  也防御现存图已有环（guard set，多走一步不会无限循环）。 */
  wouldCycle(sourceId: string, targetId: string): boolean {
    let cursor: string | undefined = targetId
    const guard = new Set<string>()
    while (cursor !== undefined) {
      if (cursor === sourceId) return true
      if (guard.has(cursor)) return true
      guard.add(cursor)
      cursor = this._bySource.get(cursor)?.targetId
    }
    return false
  }

  /** #30 follow-up D：删除涉及 label 的 constraints，返被删 list。
   *  用于"拖子体时立即脱钩"流程：caller 拿返回 list 走 detachHistory.recordDetach
   *  让 30s 反向惩罚生效。
   *
   *  E1 修复 (2026-05-19)：默认只删出向（拖子体场景的正确语义）。
   *  之前"出向+入向都删"在 M3 多窗时会误伤——例如 settings 已吸到 chat 上，用户只想轻挪 chat，
   *  结果 settings 也被脱钩。入向删除应是显式动作（"detach incoming"），不应在常规拖动里自动发生。
   *
   *  options.includeInbound = true 时保留原行为（M3 显式"清空 chat 的所有依附关系"场景预留）。 */
  removeAllInvolving(
    label: string,
    options: { includeInbound?: boolean } = {},
  ): SnapConstraint[] {
    const removed: SnapConstraint[] = []
    // 出向：label 作为 source 的 constraint（拖子体时正确：用户要拖走它，断开它对 anchor 的依附）
    const out = this._bySource.get(label)
    if (out) {
      removed.push(out)
      this.delete(label)  // 复用现有 delete 维护双索引一致
    }
    // 入向：所有 target === label 的 constraints（仅 includeInbound:true 时删；
    //   常规拖动不应触达——其他窗依附我，我挪一下不代表它们也要脱钩）
    if (options.includeInbound) {
      const inSourceIds = this._byTarget.get(label)
      if (inSourceIds && inSourceIds.size > 0) {
        // 复制成 array 防 delete 时 mutate iteration
        for (const sid of Array.from(inSourceIds)) {
          const c = this._bySource.get(sid)
          if (c) {
            removed.push(c)
            this.delete(sid)
          }
        }
      }
    }
    return removed
  }

  clear(): void {
    this._bySource.clear()
    this._byTarget.clear()
  }

  /** 测试 helper：当前 constraint 总数。 */
  size(): number {
    return this._bySource.size
  }
}

export const constraintStore = new ConstraintStore()
export { ConstraintStore }
