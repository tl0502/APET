// DragSession 状态机（ADR-020 *Updated 2026-05-18*）。
//
// 状态：Idle → Armed → Dragging → PreviewSnap → Commit / Cancel(ESC)
//
// T6 (#31 follow-up B)：新增 group-drag 状态。当被拖窗自身是 anchor（其他窗的 target）
// 且自己无出向 constraint 时，caller arm({mode:'group'}) → 进入 group-drag —— 不算 candidate /
// commit 时不写 constraint，仅靠 useSnapWindow onMoved 路径走 solver 平移 dependents。
//
// 调用约定（useSnapWindow 内部时序）：
//   1. 用户 pointerdown on data-tauri-drag-region → arm(label, snapshot, {mode})
//      mode 由 caller 按 constraintStore.dependentsOf(label).length > 0 && get(label) === null 判定
//   2. 后端 startDragging 接管 OS drag；前端等待第一个 tauri://move 事件
//   3. 每个 onMoved 触发 → caller 算 findCandidates(rect, registry)，
//      然后 onUserMove(label, cands[0] ?? null) 推进状态机
//      （group-drag 状态时 onUserMove 不算 candidate，是 no-op）
//   4. 200ms 无 onMoved → caller 调 commit() 写 constraint（如 preview 有 candidate）
//      （group-drag 状态时 commit 仅状态回 Idle，不写 constraint）
//   5. ESC keydown 期间任意状态 → cancel() 返 forestSnapshot，caller 用此 snapshot 调
//      safeSetPosition 把 forest 全部窗回滚到 drag 前位置
//   6. Armed 超 1s 无 onMoved → 自动回 Idle（用户 click 没拖），caller 应定时调 checkArmedTimeout
//
// 副作用：
// - commit: 写 constraintStore（仅 preview）；若替换了原 constraint → detachHistory 记录旧 target
// - cancel: 不写 store，仅返回 forestSnapshot 给 caller 回滚
//
// 测试可注入 stores 避免 module singleton 污染。

import { ref, type Ref } from 'vue'

import { detachHistory as defaultDetachHistory } from './candidates'
import { constraintStore as defaultStore, type ConstraintStore } from './constraintStore'
import { TRIGGER_ZONE } from './geometry'
import type { DetachHistory } from './candidates'
import type { DragSessionState, Edge, Rect, SnapCandidate, SnapConstraint } from './types'

export interface CommitResult {
  /** 实际写入的 constraint（无 candidate 时为 null） */
  committedConstraint: SnapConstraint | null
  /** commit 替换了原 constraint 时的旧 (sourceId, targetId)，用于 detachHistory 显式提示 */
  detached: { sourceId: string; targetId: string } | null
}

export interface DragSessionDeps {
  store?: ConstraintStore
  history?: DetachHistory
}

/** T6 (#31 follow-up B) + #30 follow-up D：arm 模式
 *  - 'source' 默认：被拖窗作 source 走 candidate 检测 / commit 写 source→target
 *  - 'group'：被拖窗是 anchor（其他窗的 target），平移 dependents，commit no-op
 *  - 'primary-attract' (#30 follow-up D)：primary 拖动 + 无 dependents → 反向吸引附近 secondary。
 *      candidate 由 caller 经 findReverseAttract 算得（candidate.movingId = secondary id），
 *      commit 时写 secondary→primary。dragSession.sourceId 仍是 primary（被拖窗），
 *      但 commit 写入 constraint 用 candidate.movingId 作 source。 */
export type ArmMode = 'source' | 'group' | 'primary-attract'

/** Armed → Idle 自动超时（用户 click 没拖） */
export const ARMED_TIMEOUT_MS = 1000

/** T2a (#31)：preview 期的 anchor windowId 镜像 reactive ref，供 useSnapWindow + 各窗
 *  根组件 watch → 切换 .snap-preview CSS class。preview 之外的状态恒为 null。 */
export const previewAnchorId: Ref<string | null> = ref(null)
/** T7 (#31 follow-up B)：preview 期 anchor 哪条边被靠近（targetEdge），UI 沿这条边画 glow。 */
export const previewEdge: Ref<Edge | null> = ref(null)
/** T7 (#31 follow-up B)：preview 渐进 intensity ∈ [0, 1]：1 - distance/TRIGGER_ZONE，越近越亮。 */
export const previewIntensity: Ref<number> = ref(0)
/** Phase B (#31 follow-up C)：preview 期 source 松手后将到达的 finalRect。
 *  source 端 SnapGhost 用此渲染"我会落在这里"的 ghost outline（相对当前 source rect 偏移）。
 *  非 preview 状态恒 null。 */
export const previewFinalRect: Ref<Rect | null> = ref(null)

class DragSession {
  private _state: DragSessionState = { kind: 'idle' }
  private _deps: DragSessionDeps

  constructor(deps: DragSessionDeps = {}) {
    this._deps = deps
  }

  get state(): DragSessionState {
    return this._state
  }

  /** PreviewSnap 状态时的当前 candidate；其他状态返 null。 */
  get currentCandidate(): SnapCandidate | null {
    return this._state.kind === 'preview' ? this._state.candidate : null
  }

  /** T2a (#31)：所有 _state 变更后调用，同步 previewAnchorId / previewEdge / previewIntensity / previewFinalRect。
   *  preview 时反映 candidate 信息；其他状态恒 null / 0。 */
  private _syncReactive(): void {
    if (this._state.kind === 'preview') {
      const c = this._state.candidate
      previewAnchorId.value = c.targetId
      previewEdge.value = c.targetEdge
      // T7 (#31 follow-up B)：用 distance 算渐进 intensity，clamp [0.25, 1] 留个最低可见底
      previewIntensity.value = Math.max(0.25, Math.min(1, 1 - c.distance / TRIGGER_ZONE))
      // Phase B (#31 follow-up C)：暴露 finalRect 供 SnapGhost 渲染
      previewFinalRect.value = c.finalRect
    } else {
      previewAnchorId.value = null
      previewEdge.value = null
      previewIntensity.value = 0
      previewFinalRect.value = null
    }
  }

  /** 用户开始拖动（pointerdown on drag-region）。
   *  forestSnapshot 是 caller 在 arm 前从 windowRegistry 全量 snapshot 出来的所有窗 Rect。
   *
   *  T6 (#31 follow-up B)：mode='group' 时进 group-drag（被拖窗是 anchor，平移 dependents）；
   *  mode='source'（默认）走原 armed 流程（被拖窗找 candidate）。 */
  arm(
    sourceId: string,
    forestSnapshot: Map<string, Rect>,
    armedAtOrOpts?: number | { mode?: ArmMode; now?: number },
    legacyOpts?: { mode?: ArmMode },
  ): void {
    // 兼容旧签名 arm(sourceId, snap, now?: number)：测试 / 调用方传 number 仍工作
    let now: number
    let mode: ArmMode
    if (typeof armedAtOrOpts === 'number') {
      now = armedAtOrOpts
      mode = legacyOpts?.mode ?? 'source'
    } else {
      now = armedAtOrOpts?.now ?? Date.now()
      mode = armedAtOrOpts?.mode ?? 'source'
    }

    if (mode === 'group') {
      this._state = {
        kind: 'group-drag',
        sourceId,
        forestSnapshot: new Map(forestSnapshot),
      }
    } else {
      this._state = {
        kind: 'armed',
        sourceId,
        forestSnapshot: new Map(forestSnapshot),
        armedAt: now,
      }
    }
    this._syncReactive()
  }

  /** Armed 超 1s 没 onMoved → 自动回 Idle。
   *  返回 true 表示发生了超时回滚（caller 可记 telemetry）。 */
  checkArmedTimeout(now: number = Date.now()): boolean {
    if (this._state.kind === 'armed' && now - this._state.armedAt > ARMED_TIMEOUT_MS) {
      this._state = { kind: 'idle' }
      this._syncReactive()
      return true
    }
    return false
  }

  /** 收到一个 onMoved 事件（已确认是用户拖动，非 isInternalMove）。
   *  candidate 为本帧 findCandidates 的最佳结果（或 null = 无吸附候选）。
   *
   *  T6 (#31 follow-up B)：group-drag 状态下不算 candidate，直接 return（useSnapWindow
   *  走另一条 solver 路径推 dependents，不需经状态机）。
   *
   *  Phase F (#31 follow-up C)：committing 状态（settle tween 中）也忽略 — caller 用
   *  markInternal 兜底 tween 帧间不触发 onMove，但极端 race 时此处再守一道。 */
  onUserMove(sourceId: string, candidate: SnapCandidate | null): void {
    if (this._state.kind === 'idle') return
    if (this._state.kind === 'group-drag') return // anchor 拖动不参与 candidate 评分
    if (this._state.kind === 'committing') return // settle tween 中，忽略残余 onMove
    if (this._state.sourceId !== sourceId) return

    if (this._state.kind === 'armed') {
      this._state = {
        kind: 'dragging',
        sourceId,
        forestSnapshot: this._state.forestSnapshot,
      }
    }

    if (candidate) {
      this._state = {
        kind: 'preview',
        sourceId,
        forestSnapshot: this._state.forestSnapshot,
        candidate,
      }
    } else if (this._state.kind === 'preview') {
      // 失去 candidate → 回 Dragging
      this._state = {
        kind: 'dragging',
        sourceId,
        forestSnapshot: this._state.forestSnapshot,
      }
    }
    this._syncReactive()
  }

  /** drag end heuristic（200ms 无 onMoved）触发：
   *  - preview → 写 constraint；若替换原 constraint，记 detach
   *  - dragging（无 candidate）→ 不写 constraint，原 constraint 保留
   *  - armed → 不写（用户没真拖）
   *  - group-drag → 不写（anchor 拖动；T6 #31 follow-up B）
   *  - idle → no-op
   *
   *  Phase F (#31 follow-up C)：preview 写入成功 + 调用方提供 sourceRectAtMouseup
   *  → 进 committing 状态（settle tween 进行时不回 idle，让 ESC 仍可 cancel）。
   *  无 sourceRect 参数 → 退化为原行为直接回 idle（向后兼容测试 / 调用方）。
   *  caller 跑完 tween 后须调 endCommitting() 回 idle。
   *
   *  #30 follow-up D：constraint 的 sourceId = candidate.movingId（不是 dragSession.sourceId）。
   *  - secondary drag (movingId === dragSession.sourceId)：行为不变
   *  - primary-attract (movingId === 某 secondary id)：写入 secondary→primary constraint。
   *  sourceRectAtMouseup 也应是 movingId 的 rect（caller 责任传对）。 */
  commit(now: number = Date.now(), sourceRectAtMouseup?: Rect): CommitResult {
    const store = this._deps.store ?? defaultStore
    const history = this._deps.history ?? defaultDetachHistory

    const result: CommitResult = { committedConstraint: null, detached: null }

    if (this._state.kind === 'preview') {
      const c = this._state.candidate
      // #30 follow-up D：constraint.sourceId 用 candidate.movingId（兼容 primary-attract）
      const newCon: SnapConstraint = {
        sourceId: c.movingId,
        targetId: c.targetId,
        sourceEdge: c.sourceEdge,
        targetEdge: c.targetEdge,
        offset: c.offset,
        enabled: true,
        createdAt: now,
      }
      const old = store.get(c.movingId)
      if (old && old.targetId !== c.targetId) {
        history.recordDetach(old.sourceId, old.targetId, now)
        result.detached = { sourceId: old.sourceId, targetId: old.targetId }
      }
      const r = store.set(newCon)
      if (r.ok) {
        result.committedConstraint = newCon
      }
      // Phase F (#31 follow-up C)：写入成功 + 有 sourceRect → 进 committing
      // 让 caller 的 settle tween 期间 ESC 仍可 cancel；tween 完成 caller 调 endCommitting()。
      // #30 follow-up D：committing.sourceId = movingId（即 settle tween 要移动的窗）
      if (result.committedConstraint && sourceRectAtMouseup) {
        this._state = {
          kind: 'committing',
          sourceId: c.movingId,
          forestSnapshot: this._state.forestSnapshot,
          fromRect: sourceRectAtMouseup,
          toRect: c.finalRect,
          t0: now,
        }
        this._syncReactive()
        return result
      }
      // cycle / self-loop reject 时 committedConstraint 仍为 null（应该不发生 — findCandidates
      // 不应返回触发 cycle 的 candidate）
    }
    // group-drag / armed / dragging / idle / preview(无 sourceRect) 路径：不进 committing，直接回 idle

    this._state = { kind: 'idle' }
    this._syncReactive()
    return result
  }

  /** Phase F (#31 follow-up C)：caller settle tween 完成后调用。
   *  - committing → idle（清状态，下次 drag 可启动）
   *  - 其他状态 → no-op（防误调用） */
  endCommitting(): void {
    if (this._state.kind === 'committing') {
      this._state = { kind: 'idle' }
      this._syncReactive()
    }
  }

  /** ESC 触发：返回 forest snapshot 供 caller 回滚位置。状态回 Idle。
   *  返 null 表示当前在 Idle 状态没有可回滚的 snapshot。 */
  cancel(): Map<string, Rect> | null {
    if (this._state.kind === 'idle') return null
    const snap = this._state.forestSnapshot
    this._state = { kind: 'idle' }
    this._syncReactive()
    return snap
  }

  /** 显式 reset（测试 / 边角 case） */
  reset(): void {
    this._state = { kind: 'idle' }
    this._syncReactive()
  }
}

export const dragSession = new DragSession()
export { DragSession }
