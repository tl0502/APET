// 磁吸窗口系统核心类型（ADR-020 *Updated 2026-05-18*，issue #30）。
//
// 拓扑：constraint-based partial mesh。每个窗最多 1 个出向 constraint（I1），
// 整个图无环（I2）→ 等价于森林（每窗有 0 或 1 个 anchor，多个窗可共享同一 anchor）。
//
// 坐标系：全部 logical pixel（与 Tauri LogicalPosition / window_state.rs 一致）。
// 跨多屏时同一份全局逻辑坐标（monitor.position().to_logical 已加 offset）。

export type Edge = 'left' | 'right' | 'top' | 'bottom'

/** Logical-pixel 矩形。x/y 是 outer 左上角。 */
export interface Rect {
  x: number
  y: number
  w: number
  h: number
}

/** 磁吸约束：source 的 sourceEdge 贴在 target 的 targetEdge 上，偏移 offset。
 *  sourceEdge / targetEdge 必须 opposite（left↔right / top↔bottom），由 candidates 评分时保证。
 *  offset：沿 anchor 边的"切向"偏移。
 *    - 垂直边（left/right）：offset 是 y 方向（source.y - anchor.y），正向下
 *    - 水平边（top/bottom）：offset 是 x 方向（source.x - anchor.x），正向右
 */
export interface SnapConstraint {
  sourceId: string
  targetId: string
  sourceEdge: Edge
  targetEdge: Edge
  offset: number
  enabled: boolean
  /** ms epoch，用于 candidates memoryBias 计算（30s 内 detach 反向惩罚） */
  createdAt: number
}

/** 单窗口注册信息（windowRegistry 内 module-level Map 单例存储）。 */
export interface WindowRegistration {
  /** Tauri window label（如 'pet' / 'chat'） */
  id: string
  rect: Rect
  visible: boolean
}

/** 候选 snap：drag 期间 candidates.ts 计算的一个潜在吸附点。 */
export interface SnapCandidate {
  /** #30 follow-up D：谁被移动到 finalRect。
   *  - source / group / secondary 拖动模式：= dragSession.sourceId（即被拖窗本身）
   *  - primary-attract 模式：= 某个 secondary 的 id（被反向吸到 primary 那个）
   *  caller commit / settle tween 都基于 movingId（而非 dragSession.sourceId）。 */
  movingId: string
  targetId: string
  sourceEdge: Edge
  targetEdge: Edge
  offset: number
  /** commit 后 source 应到达的 final rect（已 applyConstraint） */
  finalRect: Rect
  /** 评分越小越优（distance × 0.6 + overlapPenalty × 0.2 + (1 − memoryBias) × 0.2） */
  score: number
  /** T8 (#31 follow-up B)：edgeDistance 原始值（logical px），供 UI 计算渐进 intensity。
   *  candidate 进入 list 的前提是 distance ≤ TRIGGER_ZONE（geometry.ts），所以 ∈ [0, TRIGGER_ZONE]。 */
  distance: number
}

/** dragSession 状态机。Idle → Armed → Dragging → PreviewSnap → Committing → Idle / Cancel。
 *  PreviewSnap 与 Dragging 的区别仅在于是否有 candidate；几何上同一态。
 *
 *  T6 (#31 follow-up B)：新增 group-drag 状态。当被拖窗自身是其他窗的 anchor
 *  （constraintStore.dependentsOf(label).length > 0）且自己没有出向 constraint 时，
 *  pointerdown 直接进 group-drag —— 不算 candidate / 不写 constraint，仅靠 onMoved
 *  路径走 solver 平移所有 dependents（Winamp / VS docking 经典模式）。
 *
 *  Phase F (#31 follow-up C)：新增 committing 状态。commit() 写入 constraint 后进入此态，
 *  caller 跑 settle tween 期间状态机仍在；tween 完成 caller 显式调 endCommitting() 回 idle。
 *  ESC 在 committing 时也能 cancel（回滚到 fromRect / forestSnapshot）。 */
export type DragSessionState =
  | { kind: 'idle' }
  | {
      kind: 'armed'
      sourceId: string
      /** drag 开始前的全 forest Rect 快照，ESC 回滚用 */
      forestSnapshot: Map<string, Rect>
      armedAt: number
    }
  | {
      kind: 'dragging'
      sourceId: string
      forestSnapshot: Map<string, Rect>
    }
  | {
      kind: 'preview'
      sourceId: string
      forestSnapshot: Map<string, Rect>
      candidate: SnapCandidate
    }
  | {
      /** T6：anchor 角色拖动 — 平移所有 dependents，跳过 candidate / detach。 */
      kind: 'group-drag'
      sourceId: string
      forestSnapshot: Map<string, Rect>
    }
  | {
      /** Phase F (#31 follow-up C)：commit 已写 store，settle tween 进行中。
       *  fromRect: 松手时 source 的位置；toRect: applyConstraint 后 finalRect。
       *  t0: ms epoch，commit 时刻；caller 用此估算 tween 已跑多久（兜底诊断）。
       *  ESC 在此态 cancel → 回滚 forestSnapshot（tween 半路停在 fromRect 附近也接受）。 */
      kind: 'committing'
      sourceId: string
      forestSnapshot: Map<string, Rect>
      fromRect: Rect
      toRect: Rect
      t0: number
    }

/** 持久化用：snap:constraints KV 内 JSON 数组的单元素。 */
export interface PersistedConstraint extends SnapConstraint {
  /** schema version（future-proof，破坏性 schema 变更时 ++） */
  v: 1
}
