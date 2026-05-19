// useSnapWindow composable（ADR-020 *Updated 2026-05-18*，issue #30 S6）。
//
// 每个参与磁吸的窗口（pet / chat）在 root 组件 onMounted 调一次：
//   useSnapWindow('pet') / useSnapWindow('chat')
//
// 职责：
//   1. 注册自己到 windowRegistry + 监听 tauri://move/resize 更新 rect
//   2. 跨 webview 同步：emit/listen 'snap:registry-update' 广播 rect 给其他 webview
//   3. 跨 webview 同步：emit/listen 'snap:constraint-changed' 触发 reload from KV
//   4. listen tauri://move 入口走 isInternalMove guard 防解器自递归
//   5. 监听 window pointerdown on [data-tauri-drag-region] → dragSession.arm
//   6. 监听 keydown ESC → dragSession.cancel + 回滚 forest snapshot
//   7. 200ms 无 onMoved watchdog → drag end heuristic → dragSession.commit + persistence.save
//   8. 1s Armed timeout → dragSession.checkArmedTimeout
//   9. pet 窗负责启动期 load persistence + initial solve（其他窗收 snap:constraint-changed 后 reload）
//
// 跨 webview 死循环安全：
//   - isInternalMove guard 是 webview-local；跨 webview 不生效
//   - 但 I2 forest 保证 solver 路径不形成循环（chat onMoved → solve([chat]) → dependentsOf(chat)=[]
//     → 空 map → 终止）；只在有环时才会死循环，而 I2 禁了环

import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  LogicalPosition,
  getAllWindows,
  getCurrentWindow,
  type Window,
} from '@tauri-apps/api/window'
import { computed, onBeforeUnmount, onMounted, ref, watch, type ComputedRef } from 'vue'

import { detachHistory, findCandidates, findReverseAttract } from '@/lib/snap/candidates'
import { constraintStore } from '@/lib/snap/constraintStore'
import {
  ARMED_TIMEOUT_MS,
  dragSession,
  previewAnchorId,
  previewEdge,
  previewIntensity,
} from '@/lib/snap/dragSession'
import {
  computeFieldIntensity,
  FIELD_INTENSITY_EVT,
  type FieldIntensityPayload,
} from '@/lib/snap/field'
import { VelocityTracker } from '@/lib/snap/intent'
import { isInternalMove, markInternal } from '@/lib/snap/internalMove'
import { loadPersistedConstraints, persistConstraints } from '@/lib/snap/persistence'
import { solve } from '@/lib/snap/solver'
import type { Edge, Rect, SnapCandidate } from '@/lib/snap/types'
import { windowRegistry } from '@/lib/snap/windowRegistry'

const DRAG_END_TIMEOUT_MS = 200
const PET_LABEL = 'pet'
/** #30 follow-up D：硬编码 primary 身份。pet 唯一是主体（用户唯一可塑造的人格）；
 *  其他所有窗（chat / future settings / tasks）都是 secondary。
 *  primary drag (无 dependents) → primary-attract 反向吸引附近 secondary；
 *  secondary drag → 首帧 detachAll 立即脱钩（用户认为"我要把这个挪走"）。 */
const PRIMARY_LABEL = PET_LABEL
const REGISTRY_BROADCAST_EVT = 'snap:registry-update'
const CONSTRAINT_CHANGED_EVT = 'snap:constraint-changed'
/** T2a (#31)：preview anchor 切换的跨 webview 广播事件名。
 *  T7 (#31 follow-up B)：payload 由 string|null 扩展为 PreviewAnchorPayload，承载 edge + intensity。 */
const PREVIEW_ANCHOR_EVT = 'snap:preview-anchor'

interface PreviewAnchorPayload {
  anchorId: string | null
  edge: Edge | null
  intensity: number
}

/** T2a / T7：webview-local 镜像，记录"当前 preview anchor 是哪个窗 + 哪条边 + 多亮"。
 *  - 本窗 dragSession 触发时由 watch 三个 ref → emit + 写入本地
 *  - 其他 webview 触发时由 listen(PREVIEW_ANCHOR_EVT) 写入本地
 *  各窗 root 组件用 isPreviewAnchor / previewEdgeFor / previewIntensityValue computed 渲染 .snap-preview class */
const previewAnchorGlobal = ref<string | null>(null)
const previewEdgeGlobal = ref<Edge | null>(null)
const previewIntensityGlobal = ref<number>(0)

/** Phase A (#31 follow-up C)：field intensity 跨 webview 镜像。
 *  - 拖动中的 source 端 onMove → computeFieldIntensity → emit FIELD_INTENSITY_EVT
 *  - 所有窗 listen → 更新本地镜像；anchorId === label 时该窗 root 显示 halo
 *  - 拖动结束（idle）→ 显式 emit { anchorId: null, intensity: 0 } 清状态
 *  非拖动期不广播，避免无效流量。 */
const fieldAnchorGlobal = ref<string | null>(null)
const fieldIntensityGlobal = ref<number>(0)

/** Phase F (#31 follow-up C)：当前 source 角色拖动窗的 label（self-lean 用）。
 *  - source 端 onMove userDragging 分支：set 为 label
 *  - scheduleDragEnd / ESC / 状态机退出 dragging/preview：clear 为 null
 *  - chat 自己拖动 + 在 field 内（intensity > 0）→ selfLean computed 算 transform 向 pet 方向 ≤3px
 *  注：这个 ref 是 webview-local（不跨 webview emit）— self-lean 是本窗 root transform，无需广播。 */
const sourceDragLabelGlobal = ref<string | null>(null)

export interface SnapWindowApi {
  /** 当前窗是否是 preview anchor（拖动中的窗准备吸到本窗） */
  isPreviewAnchor: ComputedRef<boolean>
  /** T7 (#31 follow-up B)：preview 时本窗哪条边亮 glow（targetEdge）；非 preview 状态恒 null */
  previewEdgeFor: ComputedRef<Edge | null>
  /** T7：渐进 intensity ∈ [0, 1]；非 preview / 非本窗 anchor 时恒 0 */
  previewIntensityFor: ComputedRef<number>
  /** Phase A (#31 follow-up C)：本窗是 field anchor（拖动中的窗在本窗影响域内）→ 显示 halo */
  isFieldAnchor: ComputedRef<boolean>
  /** Phase A：field halo 强度 ∈ [0, 1]；本窗不是 field anchor 时恒 0 */
  fieldIntensityFor: ComputedRef<number>
  /** Phase F (#31 follow-up C)：本窗作为 source 拖动 + 在 field 内 → 朝 anchor 偏移 (dx,dy) ≤ 3px。
   *  null = 不显示 lean（非拖动 / 不在 field 内）。 */
  selfLean: ComputedRef<{ dx: number; dy: number } | null>
}

interface RegistryUpdatePayload {
  id: string
  rect: Rect
  visible: boolean
}

async function readWindowRect(w: Window): Promise<Rect> {
  const phys = await w.outerPosition()
  const sz = await w.outerSize()
  const scale = await w.scaleFactor()
  const lp = phys.toLogical(scale)
  const ls = sz.toLogical(scale)
  return { x: lp.x, y: lp.y, w: ls.width, h: ls.height }
}

async function safeSetPosition(targetLabel: string, x: number, y: number): Promise<void> {
  const all = await getAllWindows()
  const target = all.find((w) => w.label === targetLabel)
  if (!target) return
  markInternal(targetLabel)
  await target.setPosition(new LogicalPosition(x, y))
}

/** Phase F (#31 follow-up C)：commit 后 settle tween — "4 帧线性抵达 + 2 帧 1.5px 反向回正"。
 *  替代之前的 cubic ease-out 6 帧。"settle" 不引入 spring physics（plan §9 anti-checklist），
 *  纯帧序：
 *    - 帧 1-4 @ 25ms：从 fromRect 线性到达 toRect
 *    - 帧 5 @ 25ms：toRect - 1.5px × unit(dx,dy)（沿运动反方向微撤，模拟"卡入"过冲反弹）
 *    - 帧 6 @ 25ms：回到 toRect
 *  总 ~150ms，与现实 mouse-up 反馈节奏一致。
 *
 *  极短距离（< SETTLE_MIN_DIST 6px）跳过反向阶段，单帧抵达。 */
const SETTLE_MIN_DIST = 6
const SETTLE_AMOUNT = 1.5
const SETTLE_FRAME_MS = 25

async function tweenToRect(
  sourceId: string,
  fromRect: Rect,
  toRect: Rect,
): Promise<void> {
  const dx = toRect.x - fromRect.x
  const dy = toRect.y - fromRect.y
  const dist = Math.sqrt(dx * dx + dy * dy)
  // 极小距离直接到位（同位置时 dx≈0 / dy≈0）
  if (dist < 0.5) {
    await safeSetPosition(sourceId, toRect.x, toRect.y)
    return
  }
  // 短距离跳过反向阶段
  if (dist < SETTLE_MIN_DIST) {
    for (let i = 1; i <= 4; i++) {
      // Phase F：ESC cancel 时 dragSession 已回 idle，tween 中止
      if (dragSession.state.kind !== 'committing') return
      const t = i / 4
      await safeSetPosition(sourceId, fromRect.x + dx * t, fromRect.y + dy * t)
      if (i < 4) await new Promise((r) => setTimeout(r, SETTLE_FRAME_MS))
    }
    return
  }
  // 帧 1-4：线性抵达 toRect
  for (let i = 1; i <= 4; i++) {
    if (dragSession.state.kind !== 'committing') return
    const t = i / 4
    await safeSetPosition(sourceId, fromRect.x + dx * t, fromRect.y + dy * t)
    await new Promise((r) => setTimeout(r, SETTLE_FRAME_MS))
  }
  if (dragSession.state.kind !== 'committing') return
  // 帧 5：沿运动反方向 1.5px（settle 微撤）
  const ux = dx / dist
  const uy = dy / dist
  await safeSetPosition(sourceId, toRect.x - SETTLE_AMOUNT * ux, toRect.y - SETTLE_AMOUNT * uy)
  await new Promise((r) => setTimeout(r, SETTLE_FRAME_MS))
  if (dragSession.state.kind !== 'committing') return
  // 帧 6：回正 toRect
  await safeSetPosition(sourceId, toRect.x, toRect.y)
}

export function useSnapWindow(label: string): SnapWindowApi {
  let unlistenMove: UnlistenFn | null = null
  let unlistenResize: UnlistenFn | null = null
  let unlistenRegistry: UnlistenFn | null = null
  let unlistenConstraint: UnlistenFn | null = null
  let unlistenPreviewAnchor: UnlistenFn | null = null
  let unlistenField: UnlistenFn | null = null
  let stopPreviewAnchorWatch: (() => void) | null = null
  let dragEndTimer: ReturnType<typeof setTimeout> | null = null
  let armedTimer: ReturnType<typeof setTimeout> | null = null
  let escHandler: ((e: KeyboardEvent) => void) | null = null
  let pointerDownHandler: ((e: PointerEvent) => void) | null = null

  // Phase C (#31 follow-up C)：source 拖动期 velocity 跟踪。
  // - 仅 userDragging 路径 update（非 user move 不影响 velocity）
  // - findCandidates 传 tracker.velocity，让评分根据"用户朝哪甩"偏向同向 candidate
  // - 拖动结束 / ESC cancel 时 reset，避免上次的 velocity 残留干扰下次
  const velocityTracker = new VelocityTracker()

  // #30 follow-up D：当前 drag 的角色模式。onPointerDown 时设；scheduleDragEnd /
  // ESC / armed timeout 时清。useSnapWindow 闭包变量（非 dragSession state） —
  //   - 'source'：被拖窗作 source（secondary drag），首帧 detachAll + findCandidates
  //   - 'group'：anchor 拖动（primary drag with dependents），solver 平移 dependents
  //   - 'primary-attract'：primary drag without dependents，findReverseAttract 反向吸引 secondary
  //   - null：未在拖动 / 拖动已结束（与 dragSession.idle 同步）
  let currentDragMode: 'source' | 'group' | 'primary-attract' | null = null

  // #30 follow-up D：本次 drag 是否跳过吸附（escape hatch）。
  // - onPointerDown 时根据 e.shiftKey / e.ctrlKey 设置
  // - scheduleDragEnd 末尾 / ESC handler / armedTimer 触发都 reset
  // - true 时：onMove 强制 candidate=null（不进 preview / 不写新 constraint）；
  //   detachAll 跳过（保留已存在 constraint，符合"只想小幅挪一下"用例）
  let bypassSnapForCurrentDrag = false

  // T2a (#31)：暴露给 root 组件 :class 用，命中时挂 .snap-preview
  const isPreviewAnchor = computed(() => previewAnchorGlobal.value === label)
  // T7 (#31 follow-up B)：仅本窗为 anchor 时返 edge / intensity，否则恒 null / 0
  const previewEdgeFor = computed<Edge | null>(() =>
    previewAnchorGlobal.value === label ? previewEdgeGlobal.value : null,
  )
  const previewIntensityFor = computed<number>(() =>
    previewAnchorGlobal.value === label ? previewIntensityGlobal.value : 0,
  )
  // Phase A (#31 follow-up C)：本窗作为 field anchor 的反应式 API
  const isFieldAnchor = computed(() => fieldAnchorGlobal.value === label)
  const fieldIntensityFor = computed<number>(() =>
    fieldAnchorGlobal.value === label ? fieldIntensityGlobal.value : 0,
  )
  // Phase F (#31 follow-up C)：本窗作为 source + 在 field 内 → 朝 anchor 方向 lean ≤ 3px
  //   - 仅当 sourceDragLabelGlobal === label（本窗正在 source 拖动）
  //   - 且 fieldAnchorGlobal 非 null + intensity > 0（field 内）
  //   - 用 windowRegistry 现 rect 算两中心向量 → 单位化 × intensity × 3
  //   - 完全静止 / anchor 不在 registry → null（CSS 不应用 transform）
  const selfLean = computed<{ dx: number; dy: number } | null>(() => {
    if (sourceDragLabelGlobal.value !== label) return null
    const anchorId = fieldAnchorGlobal.value
    if (!anchorId || fieldIntensityGlobal.value <= 0) return null
    const self = windowRegistry.get(label)
    const anchor = windowRegistry.get(anchorId)
    if (!self || !anchor) return null
    const dx = anchor.rect.x + anchor.rect.w / 2 - (self.rect.x + self.rect.w / 2)
    const dy = anchor.rect.y + anchor.rect.h / 2 - (self.rect.y + self.rect.h / 2)
    const mag = Math.sqrt(dx * dx + dy * dy)
    if (mag < 1) return null
    const maxOffset = 3 // plan §2 L2：≤ 3px
    const k = (fieldIntensityGlobal.value * maxOffset) / mag
    return { dx: dx * k, dy: dy * k }
  })

  function clearDragEndTimer(): void {
    if (dragEndTimer !== null) {
      clearTimeout(dragEndTimer)
      dragEndTimer = null
    }
  }

  function scheduleDragEnd(): void {
    clearDragEndTimer()
    dragEndTimer = setTimeout(async () => {
      dragEndTimer = null
      // #30 follow-up D：commit 用 candidate.movingId 的 rect 作 fromRect。
      // - source / secondary drag：movingId === label（被拖窗自身）→ label rect
      // - primary-attract：movingId === secondary id → 该 secondary 的 rect（被反向吸的那个）
      // - 无 candidate（dragging / group）：fallback label rect（commit 内部不进 committing）
      const currentCand = dragSession.currentCandidate
      const movingId = currentCand?.movingId ?? label
      const movingRectAtMouseup = windowRegistry.get(movingId)?.rect
      const result = dragSession.commit(Date.now(), movingRectAtMouseup)
      if (result.committedConstraint || result.detached) {
        await persistConstraints()
        await emit(CONSTRAINT_CHANGED_EVT, null)
        // Phase F：committed 时进 committing state，跑 settle tween，完成后 endCommitting
        if (result.committedConstraint && movingRectAtMouseup) {
          const c = result.committedConstraint
          const targetReg = windowRegistry.get(c.targetId)
          if (targetReg) {
            const final = applyConstraintToRect(movingRectAtMouseup, targetReg.rect, c)
            // c.sourceId 已等于 movingId（commit 写入时用了 candidate.movingId）
            await tweenToRect(c.sourceId, movingRectAtMouseup, final)
            // tween 完成（或 ESC 中断后）显式回 idle
            dragSession.endCommitting()
            // 只在 tween 完整完成时才更新 registry（ESC 中断时 ESC handler 已写回 fromRect 到 registry）
            if (dragSession.state.kind === 'idle') {
              windowRegistry.updateRect(c.sourceId, final)
              await emit(REGISTRY_BROADCAST_EVT, {
                id: c.sourceId,
                rect: final,
                visible: true,
              } satisfies RegistryUpdatePayload)
            }
          } else {
            // target 不在 registry → 兜底直接回 idle
            dragSession.endCommitting()
          }
        }
      }
      // Phase A (#31 follow-up C)：拖动结束 → 清 field halo（无论是否 commit）
      if (fieldAnchorGlobal.value !== null || fieldIntensityGlobal.value !== 0) {
        fieldAnchorGlobal.value = null
        fieldIntensityGlobal.value = 0
        try {
          await emit(FIELD_INTENSITY_EVT, {
            sourceId: label,
            anchorId: null,
            intensity: 0,
          } satisfies FieldIntensityPayload)
        } catch (e) {
          console.warn('[useSnapWindow] emit field-intensity clear failed:', e)
        }
      }
      // Phase C (#31 follow-up C)：拖动结束 → reset velocity tracker，下次拖动从 0 重启
      velocityTracker.reset()
      // Phase F (#31 follow-up C)：清 self-lean source 标记（若本窗是 source）
      if (sourceDragLabelGlobal.value === label) {
        sourceDragLabelGlobal.value = null
      }
      // #30 follow-up D：清 mode + bypass，下次 drag 从干净状态开始
      currentDragMode = null
      bypassSnapForCurrentDrag = false
    }, DRAG_END_TIMEOUT_MS)
  }

  async function broadcastSelfRect(rect: Rect, visible: boolean): Promise<void> {
    windowRegistry.updateRect(label, rect)
    windowRegistry.updateVisible(label, visible)
    await emit(REGISTRY_BROADCAST_EVT, { id: label, rect, visible } satisfies RegistryUpdatePayload)
  }

  async function onMoved(): Promise<void> {
    if (isInternalMove(label)) return
    const w = getCurrentWindow()
    let rect: Rect
    try {
      rect = await readWindowRect(w)
    } catch (e) {
      console.error('[useSnapWindow] readWindowRect failed:', e)
      return
    }
    await broadcastSelfRect(rect, true)

    const st = dragSession.state
    const groupDragging =
      st.kind === 'group-drag' && st.sourceId === label
    const userDragging =
      (st.kind === 'armed' || st.kind === 'dragging' || st.kind === 'preview') &&
      st.sourceId === label

    if (groupDragging) {
      // T6 (#31 follow-up B)：anchor 拖动 — 走 solver 把 dependents 平移跟随，
      // 不算 candidate / 不写 constraint。dragEnd 200ms watchdog 仍 schedule，
      // 让 dragSession 在松手后回 idle（commit no-op，但状态机要走完）。
      const result = solve([label])
      for (const [id, r] of result) {
        await safeSetPosition(id, r.x, r.y)
        windowRegistry.updateRect(id, r)
        await emit(REGISTRY_BROADCAST_EVT, {
          id,
          rect: r,
          visible: true,
        } satisfies RegistryUpdatePayload)
      }
      scheduleDragEnd()
    } else if (userDragging) {
      // 被拖窗作为 source — 算 candidates → 推 dragSession
      // Phase C (#31 follow-up C)：先 update velocity，再传给 findCandidates。
      // 用 rect.x/rect.y 作为输入（不用中心，相对方向不影响 velocity 朝向）。
      velocityTracker.update(rect.x, rect.y, Date.now())
      // Phase F (#31 follow-up C)：标记本窗为 self-lean source
      sourceDragLabelGlobal.value = label

      // #30 follow-up D：secondary 首帧（armed → dragging）detachAll 立即脱钩。
      // 只 source 模式 + 非 bypass 走此路径；primary-attract / group / bypass 跳过。
      // 在 dragSession.onUserMove 之前做，因为 onUserMove 会推 armed → dragging/preview。
      if (
        st.kind === 'armed' &&
        currentDragMode === 'source' &&
        !bypassSnapForCurrentDrag
      ) {
        const removed = constraintStore.removeAllInvolving(label)
        if (removed.length > 0) {
          const now = Date.now()
          for (const c of removed) {
            // 30s 反向惩罚：防止用户拖一下就被原 anchor 吸回
            detachHistory.recordDetach(c.sourceId, c.targetId, now)
          }
          try {
            await persistConstraints()
            await emit(CONSTRAINT_CHANGED_EVT, null)
          } catch (e) {
            console.warn('[useSnapWindow] detachAll persist/emit failed:', e)
          }
        }
      }

      // 按模式找 candidate（bypass 时强制 null）
      let best: SnapCandidate | null = null
      if (!bypassSnapForCurrentDrag) {
        let cands: SnapCandidate[]
        if (currentDragMode === 'primary-attract') {
          // 反向吸引：从每个 secondary 视角看是否要吸到 primary
          cands = findReverseAttract(label, rect, windowRegistry.list())
        } else {
          // source 模式：被拖窗找 anchor candidate
          cands = findCandidates(label, rect, windowRegistry.list(), {
            velocity: velocityTracker.velocity,
          })
        }
        best = cands[0] ?? null
      }
      dragSession.onUserMove(label, best)

      // Phase A (#31 follow-up C)：拖动时算 field intensity → emit 给所有 webview。
      // field 用整个 registry（包括非 candidate 阈值的窗），让 chat 在 100px 距离也能"感觉到 pet"。
      // bypass 时也跳过 field halo（视觉上吸附被禁用，halo 也别再渲染）
      if (!bypassSnapForCurrentDrag) {
        const fi = computeFieldIntensity(label, rect, windowRegistry.list())
        fieldAnchorGlobal.value = fi.anchorId
        fieldIntensityGlobal.value = fi.intensity
        try {
          await emit(FIELD_INTENSITY_EVT, {
            sourceId: label,
            anchorId: fi.anchorId,
            intensity: fi.intensity,
          } satisfies FieldIntensityPayload)
        } catch (e) {
          console.warn('[useSnapWindow] emit field-intensity failed:', e)
        }
      } else if (fieldAnchorGlobal.value !== null || fieldIntensityGlobal.value !== 0) {
        // bypass 期持续清 field halo（之前帧可能已 emit）
        fieldAnchorGlobal.value = null
        fieldIntensityGlobal.value = 0
        try {
          await emit(FIELD_INTENSITY_EVT, {
            sourceId: label,
            anchorId: null,
            intensity: 0,
          } satisfies FieldIntensityPayload)
        } catch {
          /* ignore */
        }
      }
      scheduleDragEnd()
    } else {
      // 非用户拖（wander / 程序 setPosition）→ solver 推 dependent 窗
      const result = solve([label])
      for (const [id, r] of result) {
        await safeSetPosition(id, r.x, r.y)
        windowRegistry.updateRect(id, r)
        await emit(REGISTRY_BROADCAST_EVT, {
          id,
          rect: r,
          visible: true,
        } satisfies RegistryUpdatePayload)
      }
    }
  }

  function onRegistryUpdate(p: RegistryUpdatePayload): void {
    if (!p || p.id === label) return // 自己的广播不回灌
    const existing = windowRegistry.get(p.id)
    if (existing) {
      windowRegistry.updateRect(p.id, p.rect)
      windowRegistry.updateVisible(p.id, p.visible)
    } else {
      windowRegistry.upsert({ id: p.id, rect: p.rect, visible: p.visible })
    }
  }

  async function onConstraintChanged(): Promise<void> {
    constraintStore.clear()
    detachHistory.clear()
    await loadPersistedConstraints()
  }

  function onEscKey(e: KeyboardEvent): void {
    if (e.key !== 'Escape') return
    const snap = dragSession.cancel()
    if (!snap) return
    clearDragEndTimer()
    void (async (): Promise<void> => {
      for (const [id, rect] of snap) {
        await safeSetPosition(id, rect.x, rect.y)
        windowRegistry.updateRect(id, rect)
        await emit(REGISTRY_BROADCAST_EVT, {
          id,
          rect,
          visible: true,
        } satisfies RegistryUpdatePayload)
      }
      // Phase A (#31 follow-up C)：ESC 取消也清 field halo
      if (fieldAnchorGlobal.value !== null || fieldIntensityGlobal.value !== 0) {
        fieldAnchorGlobal.value = null
        fieldIntensityGlobal.value = 0
        try {
          await emit(FIELD_INTENSITY_EVT, {
            sourceId: label,
            anchorId: null,
            intensity: 0,
          } satisfies FieldIntensityPayload)
        } catch (e) {
          console.warn('[useSnapWindow] emit field-intensity ESC clear failed:', e)
        }
      }
      // Phase C (#31 follow-up C)：ESC 取消也 reset velocity tracker
      velocityTracker.reset()
      // Phase F (#31 follow-up C)：ESC 取消也清 self-lean source 标记
      if (sourceDragLabelGlobal.value === label) {
        sourceDragLabelGlobal.value = null
      }
      // #30 follow-up D：ESC 取消也清 mode + bypass
      currentDragMode = null
      bypassSnapForCurrentDrag = false
    })()
  }

  function onPointerDown(e: PointerEvent): void {
    if (e.button !== 0) return
    const tgt = e.target as HTMLElement | null
    if (!tgt) return
    // 命中条件（任一）：
    //   1. [data-tauri-drag-region]：chat header / AppShell header 标题栏
    //   2. [data-snap-drag-trigger]（T4 #31）：pet 整窗 VRM 区（PetCanvas.pet-stage）
    // 排除 [data-no-drag] / [data-tauri-drag-region="false"]（reminder 气泡按钮 / 关闭按钮）
    const dragRegion = tgt.closest('[data-tauri-drag-region], [data-snap-drag-trigger]')
    if (!dragRegion) return
    if (tgt.closest('[data-no-drag]')) return
    if (tgt.closest('[data-tauri-drag-region="false"]')) return

    // #30 follow-up D：三模式判定（primary/secondary 角色模型）
    //   - primary + 已有 dependents：group-drag（拖 anchor 平移整族）
    //   - primary + 无 dependents：primary-attract（拖 primary 反向吸引附近 secondary）
    //   - 其他（secondary 拖动）：source（首帧 detachAll，立即脱钩）
    const isPrimary = label === PRIMARY_LABEL
    const hasDeps = constraintStore.dependentsOf(label).length > 0
    const hasOut = constraintStore.get(label) !== null
    let mode: 'source' | 'group' | 'primary-attract'
    if (isPrimary && hasDeps && !hasOut) {
      mode = 'group'
    } else if (isPrimary && !hasDeps) {
      mode = 'primary-attract'
    } else {
      mode = 'source'
    }
    currentDragMode = mode

    // Escape hatch：Shift 或 Ctrl 按下时本次 drag 完全跳过吸附（Photoshop / Figma 直觉）
    bypassSnapForCurrentDrag = e.shiftKey || e.ctrlKey

    // arm dragSession + snapshot forest
    const snap = new Map<string, Rect>()
    for (const w of windowRegistry.list()) {
      snap.set(w.id, w.rect)
    }
    // dragSession 自身仅区分 group / 非 group（commit 用 candidate.movingId 推断 sourceId）。
    // primary-attract / source 都走 armed→dragging→preview 同一状态机。
    const dragSessionMode: 'source' | 'group' = mode === 'group' ? 'group' : 'source'
    dragSession.arm(label, snap, { mode: dragSessionMode })

    // 启动 Armed 超时检查（仅非 group 模式有 armed 态；group-drag 无 armed 不超时）
    if (armedTimer !== null) clearTimeout(armedTimer)
    if (mode !== 'group') {
      armedTimer = setTimeout(() => {
        armedTimer = null
        if (dragSession.checkArmedTimeout()) {
          // 用户 click 没拖 → 也清本地 mode + bypass 状态
          currentDragMode = null
          bypassSnapForCurrentDrag = false
        }
      }, ARMED_TIMEOUT_MS + 100)
    }
  }

  onMounted(async () => {
    const win = getCurrentWindow()

    // 注册自己 + 立即广播
    try {
      const rect = await readWindowRect(win)
      const visible = await win.isVisible()
      windowRegistry.upsert({ id: label, rect, visible })
      await emit(REGISTRY_BROADCAST_EVT, {
        id: label,
        rect,
        visible,
      } satisfies RegistryUpdatePayload)
    } catch (e) {
      console.error('[useSnapWindow] register self failed:', e)
    }

    // 各类 listener
    try {
      unlistenMove = await win.onMoved(() => {
        void onMoved()
      })
    } catch (e) {
      console.warn('[useSnapWindow] onMoved listen failed:', e)
    }

    try {
      unlistenResize = await win.onResized(async () => {
        try {
          const r = await readWindowRect(win)
          await broadcastSelfRect(r, true)
          // T1 (#31)：尺寸变化（如 pet view_preset half↔full 切换）后追加 solver，
          // 把 attached dependent 窗按新 anchor 边重摆，避免 onResized 与 onMoved 之间
          // 的 race（pet 是 set_size + set_position 两步，onMoved 路径才推 dep；
          // 此处兜底 onResized 也推一次，solver 幂等不会双写错位）。
          const solveResult = solve([label])
          for (const [id, rect] of solveResult) {
            await safeSetPosition(id, rect.x, rect.y)
            windowRegistry.updateRect(id, rect)
            await emit(REGISTRY_BROADCAST_EVT, {
              id,
              rect,
              visible: true,
            } satisfies RegistryUpdatePayload)
          }
        } catch (e) {
          console.error('[useSnapWindow] onResized handler failed:', e)
        }
      })
    } catch (e) {
      console.warn('[useSnapWindow] onResized listen failed:', e)
    }

    try {
      unlistenRegistry = await listen<RegistryUpdatePayload>(REGISTRY_BROADCAST_EVT, (ev) => {
        if (ev.payload) onRegistryUpdate(ev.payload)
      })
    } catch (e) {
      console.warn('[useSnapWindow] listen registry failed:', e)
    }

    try {
      unlistenConstraint = await listen(CONSTRAINT_CHANGED_EVT, () => {
        void onConstraintChanged()
      })
    } catch (e) {
      console.warn('[useSnapWindow] listen constraint-changed failed:', e)
    }

    // T2a (#31) + T7 (#31 follow-up B)：本窗 dragSession previewAnchorId / Edge / Intensity 变化
    // → 同步本地三个 global ref + emit 给其他 webview。watch 任一变化都触发 emit。
    stopPreviewAnchorWatch = watch(
      [previewAnchorId, previewEdge, previewIntensity],
      async ([newAnchor, newEdge, newIntensity]) => {
        previewAnchorGlobal.value = newAnchor
        previewEdgeGlobal.value = newEdge
        previewIntensityGlobal.value = newIntensity
        try {
          await emit(PREVIEW_ANCHOR_EVT, {
            anchorId: newAnchor,
            edge: newEdge,
            intensity: newIntensity,
          } satisfies PreviewAnchorPayload)
        } catch (e) {
          console.warn('[useSnapWindow] emit preview-anchor failed:', e)
        }
      },
    )

    // 监听其他 webview 广播的 preview anchor 切换（本窗 emit 也会回声，统一覆盖即可）
    try {
      unlistenPreviewAnchor = await listen<PreviewAnchorPayload>(PREVIEW_ANCHOR_EVT, (ev) => {
        const p = ev.payload
        if (!p) {
          previewAnchorGlobal.value = null
          previewEdgeGlobal.value = null
          previewIntensityGlobal.value = 0
        } else {
          previewAnchorGlobal.value = p.anchorId
          previewEdgeGlobal.value = p.edge
          previewIntensityGlobal.value = p.intensity
        }
      })
    } catch (e) {
      console.warn('[useSnapWindow] listen preview-anchor failed:', e)
    }

    // Phase A (#31 follow-up C)：监听其他 webview 广播的 field intensity（本窗拖动时
    // 自己 emit + 本窗 onMove 内已写本地，listen 主要给非拖动窗（anchor 端）显示 halo）
    try {
      unlistenField = await listen<FieldIntensityPayload>(FIELD_INTENSITY_EVT, (ev) => {
        const p = ev.payload
        if (!p) {
          fieldAnchorGlobal.value = null
          fieldIntensityGlobal.value = 0
        } else {
          fieldAnchorGlobal.value = p.anchorId
          fieldIntensityGlobal.value = p.intensity
        }
      })
    } catch (e) {
      console.warn('[useSnapWindow] listen field-intensity failed:', e)
    }

    escHandler = onEscKey
    window.addEventListener('keydown', escHandler, true)
    pointerDownHandler = onPointerDown
    window.addEventListener('pointerdown', pointerDownHandler, true)

    // pet 窗负责启动期 load persistence + initial solve
    // 延迟 300ms 等其他 webview 也注册到自己的 registry（cross-webview event 有 race）
    if (label === PET_LABEL) {
      setTimeout(async () => {
        try {
          const r = await loadPersistedConstraints()
          console.log(`[snap] loaded ${r.loaded} constraint(s), dropped ${r.dropped}`)
          // 触发一次 solver 把 attached 窗摆到位
          const rootIds = windowRegistry.list().map((w) => w.id)
          const solveResult = solve(rootIds)
          for (const [id, rect] of solveResult) {
            await safeSetPosition(id, rect.x, rect.y)
            windowRegistry.updateRect(id, rect)
            await emit(REGISTRY_BROADCAST_EVT, {
              id,
              rect,
              visible: true,
            } satisfies RegistryUpdatePayload)
          }
        } catch (e) {
          console.error('[snap] startup load + solve failed:', e)
        }
      }, 300)
    }
  })

  onBeforeUnmount(() => {
    unlistenMove?.()
    unlistenResize?.()
    unlistenRegistry?.()
    unlistenConstraint?.()
    unlistenPreviewAnchor?.()
    unlistenField?.()
    stopPreviewAnchorWatch?.()
    if (escHandler) window.removeEventListener('keydown', escHandler, true)
    if (pointerDownHandler) window.removeEventListener('pointerdown', pointerDownHandler, true)
    clearDragEndTimer()
    if (armedTimer !== null) clearTimeout(armedTimer)
  })

  return { isPreviewAnchor, previewEdgeFor, previewIntensityFor, isFieldAnchor, fieldIntensityFor, selfLean }
}

// ───── helper（避免 useSnapWindow 直接 import geometry，防 circular dep 担心） ─────

function applyConstraintToRect(
  source: Rect,
  anchor: Rect,
  c: { sourceEdge: 'left' | 'right' | 'top' | 'bottom'; targetEdge: 'left' | 'right' | 'top' | 'bottom'; offset: number },
): Rect {
  switch (c.sourceEdge) {
    case 'left':
      return { x: anchor.x + anchor.w, y: anchor.y + c.offset, w: source.w, h: source.h }
    case 'right':
      return { x: anchor.x - source.w, y: anchor.y + c.offset, w: source.w, h: source.h }
    case 'top':
      return { x: anchor.x + c.offset, y: anchor.y + anchor.h, w: source.w, h: source.h }
    case 'bottom':
      return { x: anchor.x + c.offset, y: anchor.y - source.h, w: source.w, h: source.h }
  }
}
