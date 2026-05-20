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
import { loadPersistedConstraints, persistAndBroadcastConstraints } from '@/lib/snap/persistence'
import { isPrimary, PRIMARY_LABEL } from '@/lib/snap/roles'
import { solve } from '@/lib/snap/solver'
import { snapSyncConstraints, type RustVisualInset } from '@/services/config'
import type { Edge, Rect, SnapCandidate } from '@/lib/snap/types'
import { windowRegistry } from '@/lib/snap/windowRegistry'

const DRAG_END_TIMEOUT_MS = 200
/** 跨 webview 广播 rect / visible 变化的事件名。
 *  外部 caller（如 PomodoroApp 全屏 toggle 时手动改 visible）也需引用此常量，
 *  导出避免硬编码字面量漂移。 */
export const REGISTRY_BROADCAST_EVT = 'snap:registry-update'
/** B2 修复：新窗 mount 时 emit 此事件让现存窗回播自己的 rect。
 *  解决"chat 懒窗 reopen 时已经错过了 pet 的初始 REGISTRY_BROADCAST" race —— chat 自己 emit hello，
 *  pet 的 listener 收到后回播 pet rect，chat 就能在 loadPersistedConstraints 之前拿到 pet rect。
 *  payload 即新窗 label（senderId）；接收端用此避免回播给 sender 自己。 */
const REGISTRY_HELLO_EVT = 'snap:registry-hello'
const CONSTRAINT_CHANGED_EVT = 'snap:constraint-changed'
/** T2a (#31)：preview anchor 切换的跨 webview 广播事件名。
 *  T7 (#31 follow-up B)：payload 由 string|null 扩展为 PreviewAnchorPayload，承载 edge + intensity。 */
const PREVIEW_ANCHOR_EVT = 'snap:preview-anchor'

interface PreviewAnchorPayload {
  /** A4 修复：emit 端 webview label；接收端自过滤，避免 self-echo 触发本地 ref 重复赋值。 */
  senderId: string
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
  /** P1 修复 (review 2)：caller 在外部改动 constraintStore（如 PomodoroApp 全屏 detach）后调，
   *  把全量 constraints 推到 Rust SnapState，避免依赖跨 webview broadcast 延迟才让 Rust 端
   *  收敛——本 webview emit 的 constraint-changed 会被 A4 senderId 自过滤跳过自己。 */
  syncRustSnap: () => Promise<void>
  /** P6 修复：caller 在外部改动本窗 visible（如 PomodoroApp 进入/退出全屏）后调，
   *  统一通过 composable 内部 broadcastSelfRect 走，保证 payload schema（含 visualInset 等）
   *  与本模块其他 emit 路径一致。 */
  broadcastSelfRect: (rect: Rect, visible: boolean) => Promise<void>
}

interface RegistryUpdatePayload {
  id: string
  rect: Rect
  visible: boolean
  /** #30 follow-up F：sender 自己的 visualInset，让接收端注册时同步贴边几何参数。
   *  缺省 / undefined 表示该窗无 padding（与之前行为兼容）。 */
  visualInset?: { top: number; right: number; bottom: number; left: number }
}

/** A4 修复：所有跨 webview event 携带 senderId（emit 端的 webview label），
 *  接收端 if (p.senderId === label) return 自过滤，避免 self-echo 触发副作用。
 *  REGISTRY_BROADCAST 原本用 payload.id 自过滤（id === label 即 sender），延续此约定；
 *  PREVIEW_ANCHOR / CONSTRAINT_CHANGED / FIELD_INTENSITY 需要显式 senderId 字段。 */
interface ConstraintChangedPayload {
  senderId: string
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

/** 返回 true = tween 完整跑完到 toRect；false = 中途被 ESC / 新 drag 打断。
 *  B1 修复：原来用 `dragSession.state.kind === 'idle'` 判定 tween 是否完整，
 *  但 ESC 也会让 state 变 idle，caller 无法区分「我刚 endCommitting」和「我被 ESC 提前打断」。
 *  现 caller 直接根据本函数返回值判断，是否回写 final 到 registry。 */
async function tweenToRect(
  sourceId: string,
  fromRect: Rect,
  toRect: Rect,
): Promise<boolean> {
  const dx = toRect.x - fromRect.x
  const dy = toRect.y - fromRect.y
  const dist = Math.sqrt(dx * dx + dy * dy)
  // 极小距离直接到位（同位置时 dx≈0 / dy≈0）
  if (dist < 0.5) {
    await safeSetPosition(sourceId, toRect.x, toRect.y)
    return true
  }
  // 短距离跳过反向阶段
  if (dist < SETTLE_MIN_DIST) {
    for (let i = 1; i <= 4; i++) {
      // Phase F：ESC cancel 时 dragSession 已回 idle，tween 中止
      if (dragSession.state.kind !== 'committing') return false
      const t = i / 4
      await safeSetPosition(sourceId, fromRect.x + dx * t, fromRect.y + dy * t)
      if (i < 4) await new Promise((r) => setTimeout(r, SETTLE_FRAME_MS))
    }
    return true
  }
  // 帧 1-4：线性抵达 toRect
  for (let i = 1; i <= 4; i++) {
    if (dragSession.state.kind !== 'committing') return false
    const t = i / 4
    await safeSetPosition(sourceId, fromRect.x + dx * t, fromRect.y + dy * t)
    await new Promise((r) => setTimeout(r, SETTLE_FRAME_MS))
  }
  if (dragSession.state.kind !== 'committing') return false
  // 帧 5：沿运动反方向 1.5px（settle 微撤）
  const ux = dx / dist
  const uy = dy / dist
  await safeSetPosition(sourceId, toRect.x - SETTLE_AMOUNT * ux, toRect.y - SETTLE_AMOUNT * uy)
  await new Promise((r) => setTimeout(r, SETTLE_FRAME_MS))
  if (dragSession.state.kind !== 'committing') return false
  // 帧 6：回正 toRect
  await safeSetPosition(sourceId, toRect.x, toRect.y)
  return true
}

export interface UseSnapWindowOptions {
  /** #30 follow-up F：OS rect 内"视觉可见层"的内缩量（logical px）。
   *  用于有 CSS padding 让 box-shadow 溢出 .app-surface 的窗 — 不传则按全 0
   *  （M2 实际窗口 chat/pet/pomodoro/settings/tasks 均无 inset；保留 API 供 M3 复用）。
   *  candidates / occupancy / solver 全程用 visual rect 做贴边几何，避免 padding 间隙。 */
  visualInset?: { top: number; right: number; bottom: number; left: number }
}

export function useSnapWindow(
  label: string,
  options: UseSnapWindowOptions = {},
): SnapWindowApi {
  const visualInset = options.visualInset
  let unlistenMove: UnlistenFn | null = null
  let unlistenResize: UnlistenFn | null = null
  let unlistenRegistry: UnlistenFn | null = null
  let unlistenHello: UnlistenFn | null = null
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
        // B3 修复：persist + broadcast 原子 IPC（Rust 端串行写 KV → emit），
        // 替代之前的 await persistConstraints() + await emit() 两步。
        await persistAndBroadcastConstraints(label)
        // #30 follow-up I：commit 后同步 Rust SnapState（Rust solver 在下次 Moved 接管 group-drag）
        await syncRustSnap()
        // Phase F：committed 时进 committing state，跑 settle tween，完成后 endCommitting
        if (result.committedConstraint && movingRectAtMouseup) {
          const c = result.committedConstraint
          const targetReg = windowRegistry.get(c.targetId)
          if (targetReg) {
            // #30 follow-up F：用 currentCand.finalRect（candidates.ts 已经反推 visual→OS）。
            // 原本本地 applyConstraintToRect 重算会绕过 visualInset 模型，造成 chat 等带 padding
            // 的窗在 commit 时位置算回"贴 OS rect 边"而非"贴 visual rect 边"，padding 间隙复现。
            const final = currentCand?.finalRect
              ?? applyConstraintToRect(movingRectAtMouseup, targetReg.rect, c)
            // c.sourceId 已等于 movingId（commit 写入时用了 candidate.movingId）
            const tweenCompleted = await tweenToRect(c.sourceId, movingRectAtMouseup, final)
            // tween 完成（或 ESC 中断后）显式回 idle
            dragSession.endCommitting()
            // B1 修复：只在 tween 完整跑完时才回写 final 到 registry。
            // ESC 中断时 tweenToRect 返 false，ESC handler 已用 forestSnapshot 写回 fromRect 到 registry，
            // 这里不应再覆盖（之前用 state.kind==='idle' 判定，但 ESC 也让 state 变 idle，错误覆盖）。
            if (tweenCompleted) {
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
    await emit(REGISTRY_BROADCAST_EVT, {
      id: label,
      rect,
      visible,
      visualInset,
    } satisfies RegistryUpdatePayload)
  }

  /** #30 follow-up I：把 constraintStore 全量 + 各窗 visualInset 推到 Rust 端 SnapState。
   *  Rust 端在 Moved 事件后接管 BFS solver + 批量 set_position，避免前端 group-drag 路径
   *  N 次 setPosition IPC 在 Windows 上排队导致的链式抖动。
   *
   *  caller 路径：commit 后 / detach 后 / persistence load 完 / removeAllInvolving 后。
   *  失败仅 warn — Rust state 不同步只是回退到前端 solver（仍能工作，只是会抖）。
   *
   *  幂等：每次全量 sync，Rust 端 sync_constraints 内部 clear+rewrite，调用频率受限于
   *  constraint 变化频率（drag commit 节奏 ≤ 1Hz），不在 onMoved 高频路径上。 */
  async function syncRustSnap(): Promise<void> {
    const all = constraintStore.list().map((c) => ({
      sourceId: c.sourceId,
      targetId: c.targetId,
      sourceEdge: c.sourceEdge,
      targetEdge: c.targetEdge,
      offset: c.offset,
    }))
    const insets: Record<string, RustVisualInset> = {}
    for (const reg of windowRegistry.list()) {
      if (reg.visualInset) {
        insets[reg.id] = reg.visualInset
      }
    }
    try {
      await snapSyncConstraints(all, insets)
    } catch (e) {
      console.warn('[useSnapWindow] snapSyncConstraints failed (fallback to JS solver):', e)
    }
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
      st.kind === 'group-drag' && st.draggedId === label
    const userDragging =
      (st.kind === 'armed' || st.kind === 'dragging' || st.kind === 'preview') &&
      st.draggedId === label

    if (groupDragging) {
      // T6 (#31 follow-up B)：anchor 拖动 — 走 solver 把 dependents 平移跟随，
      // 不算 candidate / 不写 constraint。dragEnd 200ms watchdog 仍 schedule，
      // 让 dragSession 在松手后回 idle（commit no-op，但状态机要走完）。
      //
      // #30 follow-up I：Rust 端 SnapState 已接管 BFS solver + 批量 set_position。
      // 前端这里完全短路 — 不再 solve / setPosition / emit REGISTRY_BROADCAST，
      // 避免双方同时写位置造成"前端追 Rust"的 ping-pong 抖动（windowRegistry rect 也会
      // 在 dep 窗自己的 onMoved 路径上被对应 webview 更新并广播，跨 webview 同步走那条）。
      //
      // 旧前端实现（Promise.all + inflight throttle）见 git log；实测 N=2 链 30Hz、
      // N=3 链 22Hz、严重视觉抖动。换 Rust 端后 60Hz 稳定无 IPC 排队（Win32 SetWindowPos μs 级）。
      //
      // 仅保留 scheduleDragEnd 让状态机正常退出（armed→dragging→idle）。
      scheduleDragEnd()
    } else if (userDragging) {
      // 被拖窗作为 source — 算 candidates → 推 dragSession
      // Phase C (#31 follow-up C)：先 update velocity，再传给 findCandidates。
      // 用 rect.x/rect.y 作为输入（不用中心，相对方向不影响 velocity 朝向）。
      velocityTracker.update(rect.x, rect.y, Date.now())
      // Phase F (#31 follow-up C)：标记本窗为 self-lean source
      sourceDragLabelGlobal.value = label

      // B6 修复：secondary first-frame detachAll 已在 onPointerDown 同步执行（见下方分支），
      // 不再依赖此处 onMoved 时 st.kind === 'armed' 的判定。
      // 原本依赖"第一个 onMoved 仍处于 armed"的判定逻辑脆弱：若 onMoved 在 armed 阶段就直接命中
      // candidate，state 会跳过 armed → dragging 进 preview，detachAll 跳过 → 与设计意图不符。

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
      // B8 修复：armedTimer 在第一个 onMoved 时主动清除。
      // 原本依赖 1s 后 checkArmedTimeout 自检 state.kind === 'armed' 返 false 自然失效，
      // 每次 drag 都泄漏一个不必要的 timer 到 1.1s 后才回收。
      if (armedTimer !== null) {
        clearTimeout(armedTimer)
        armedTimer = null
      }
      dragSession.onUserMove(label, best)

      // Phase A (#31 follow-up C)：拖动时算 field intensity → emit 给所有 webview。
      // field 用整个 registry（包括非 candidate 阈值的窗），让 chat 在 100px 距离也能"感觉到 pet"。
      // bypass 时也跳过 field halo（视觉上吸附被禁用，halo 也别再渲染）
      //
      // D3 throttle：只在 anchorId 变化或 intensity 差值 ≥ 0.02 时 emit。
      // onMoved 大约 60Hz，原方案每帧 emit → IPC 流量浪费；现在仅 anchor 切换 / 显著变化时广播。
      if (!bypassSnapForCurrentDrag) {
        const fi = computeFieldIntensity(label, rect, windowRegistry.list())
        const anchorChanged = fi.anchorId !== fieldAnchorGlobal.value
        const intensityChanged = Math.abs(fi.intensity - fieldIntensityGlobal.value) >= 0.02
        if (anchorChanged || intensityChanged) {
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
      // 非用户拖（wander / 程序 setPosition）路径。
      //
      // #30 follow-up I：Rust 端 on_window_moved 已接管 BFS solver + 批量 set_position，
      // 前端这里**也短路**（与 group-drag 同理）。pet wander 后端直接 set_position 触发 Moved
      // → Rust solver 自动推所有 dep，不需要再走前端 solve + 4 次 IPC。
      //
      // dep 窗的 windowRegistry rect 同步靠 dep 自己 onMoved 路径上的 broadcastSelfRect
      // （Rust set_position dep 后 dep 的 webview 收到 Moved 事件 → 自己更新 + emit REGISTRY_BROADCAST）。
      //
      // 旧前端实现见 git log。
    }
  }

  function onRegistryUpdate(p: RegistryUpdatePayload): void {
    if (!p || p.id === label) return // 自己的广播不回灌
    const existing = windowRegistry.get(p.id)
    if (existing) {
      windowRegistry.updateRect(p.id, p.rect)
      windowRegistry.updateVisible(p.id, p.visible)
      // #30 follow-up F：visualInset 也同步（窗启动期首次广播 / 后续 inset 不会变，但写入幂等）
      if (p.visualInset !== undefined) {
        windowRegistry.upsert({
          id: p.id,
          rect: p.rect,
          visible: p.visible,
          visualInset: p.visualInset,
        })
      }
    } else {
      windowRegistry.upsert({
        id: p.id,
        rect: p.rect,
        visible: p.visible,
        visualInset: p.visualInset,
      })
    }
  }

  async function onConstraintChanged(): Promise<void> {
    constraintStore.clear()
    // B1 fix (#30 follow-up D review)：不清 detachHistory。
    // 之前 clear() 在这里 → 自己 emit CONSTRAINT_CHANGED 自己也收到 → 清掉刚记的 30s 反向惩罚 →
    // 每次 detach 都自动失效。30s 惩罚是 webview-local 内存，与 store 持久化语义无关，不应联动。
    await loadPersistedConstraints()
    await cleanupDirtyPrimaryOutbound()
    // #30 follow-up I：onConstraintChanged 是跨 webview reload 路径，本窗 store 已 reload，
    // 同步 Rust SnapState 确保它跟得上（每个 webview 都会调一次，Rust 端 sync_constraints 幂等）
    await syncRustSnap()
  }

  /** #30 follow-up D：primary 角色不应该有 outbound constraint（所有 commit 路径都写 secondary→primary）。
   *  发现 pet→? 脏数据时自动清除 + persist + broadcast。原因：
   *  - KV 历史脏数据（plan 设计前的测试遗留）
   *  - 某 webview 的 store reload race
   *  幂等：清理后 persist 把全局 KV 修正，所有 webview 下次 reload 都干净。 */
  async function cleanupDirtyPrimaryOutbound(): Promise<void> {
    const dirty = constraintStore.get(PRIMARY_LABEL)
    if (!dirty) return
    console.warn(
      `[snap] cleanup: primary '${PRIMARY_LABEL}' has illegal outbound ` +
        `${dirty.sourceId}->${dirty.targetId} — removing`,
    )
    constraintStore.delete(PRIMARY_LABEL)
    try {
      // B3 修复：原子 persist+broadcast
      await persistAndBroadcastConstraints(label)
      // #30 follow-up I：cleanup 后同步 Rust SnapState
      await syncRustSnap()
    } catch (e) {
      console.warn('[snap] cleanup persist/emit failed:', e)
    }
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
    //   - primary + 已有 dependents + 无出向：group-drag（拖 anchor 平移整族）
    //   - primary + 无 dependents + 无出向：primary-attract（拖 primary 反向吸引附近 secondary）
    //   - 其他：source（首帧 detachAll，立即脱钩）
    //
    // primary 脏状态自愈：primary 理论不该有 outbound（I3：constraint.sourceId 永远不是 primary）。
    // pre-#30 follow-up D 版本可能写过 pet→? 进 KV → 启动 load 后污染 store → mode 判定走错。
    // 这里同步 delete + fire-and-forget persist + emit，下面 mode 计算用清理后的状态。
    const labelIsPrimary = isPrimary(label)
    let hasOut = constraintStore.get(label) !== null
    if (labelIsPrimary && hasOut) {
      const dirty = constraintStore.get(label)
      console.warn(
        `[snap] primary ${label} has illegal outbound ` +
          `${dirty?.sourceId}->${dirty?.targetId} — fire-and-forget cleanup`,
      )
      constraintStore.delete(label)
      hasOut = false
      void (async (): Promise<void> => {
        try {
          // B3 修复：原子 persist+broadcast，避免 emit 比 KV 写早抵达其他 webview
          // 导致其他 webview 重新 load 时读到含脏 outbound 的旧 KV。
          await persistAndBroadcastConstraints(label)
          // #30 follow-up I：脏 primary outbound 清理后同步 Rust SnapState
          await syncRustSnap()
        } catch (err) {
          console.warn('[snap] onPointerDown cleanup async failed:', err)
        }
      })()
    }
    const deps = constraintStore.dependentsOf(label)
    const hasDeps = deps.length > 0
    let mode: 'source' | 'group' | 'primary-attract'
    if (labelIsPrimary && hasDeps && !hasOut) {
      mode = 'group'
    } else if (labelIsPrimary && !hasDeps && !hasOut) {
      mode = 'primary-attract'
    } else {
      mode = 'source'
    }
    currentDragMode = mode
    if (import.meta.env.DEV) {
      const storeDump = constraintStore
        .list()
        .map((c) => `${c.sourceId}->${c.targetId}`)
        .join(',')
      console.log(
        `[snap] pointerdown label=${label} mode=${mode} ` +
          `deps=[${deps.map((c) => c.sourceId).join(',')}] ` +
          `hasOut=${hasOut} bypass=${e.shiftKey || e.ctrlKey} ` +
          `store=[${storeDump}]`,
      )
    }

    // Escape hatch：Shift 或 Ctrl 按下时本次 drag 完全跳过吸附（Photoshop / Figma 直觉）
    bypassSnapForCurrentDrag = e.shiftKey || e.ctrlKey

    // B6 修复：secondary source-drag 时同步执行 detachAll（不依赖第一个 onMoved 的 armed 态判定）。
    // 仅 source 模式 + 非 bypass 走此路径；primary-attract / group / bypass 跳过。
    // 同步执行的好处：armed → preview 跳跃（极近距离点击直接命中 candidate）也能正确脱钩。
    if (mode === 'source' && !bypassSnapForCurrentDrag) {
      const removed = constraintStore.removeAllInvolving(label)
      if (removed.length > 0) {
        if (import.meta.env.DEV) {
          console.log(
            `[snap] pointerdown ${label} source-mode detachAll removed=` +
              removed.map((c) => `${c.sourceId}->${c.targetId}`).join(','),
          )
        }
        const now = Date.now()
        for (const c of removed) {
          // 30s 反向惩罚：防止用户拖一下就被原 anchor 吸回
          detachHistory.recordDetach(c.sourceId, c.targetId, now)
        }
        // fire-and-forget B3 原子 IPC（pointerdown 不能 async，且后续 arm dragSession 不依赖 KV 写完成）
        void persistAndBroadcastConstraints(label)
          .then(() => syncRustSnap()) // #30 follow-up I：detachAll 后同步 Rust SnapState
          .catch((err) => {
            console.warn('[snap] pointerdown detachAll persist failed:', err)
          })
      }
    }

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
      windowRegistry.upsert({ id: label, rect, visible, visualInset })
      await emit(REGISTRY_BROADCAST_EVT, {
        id: label,
        rect,
        visible,
        visualInset,
      } satisfies RegistryUpdatePayload)
      // B2 修复：emit hello 让现存窗回播自己的 rect。
      // 解决"chat 懒窗 reopen 时已错过 pet 的初始 REGISTRY_BROADCAST"竞争：chat 自己启动期
      // 不知道 pet 的 rect，loadPersistedConstraints 时 windowRegistry.get('pet')=undefined
      // → constraint 被当作 anchor-missing drop 掉。hello 出去后，pet listener 会回播 pet rect。
      await emit(REGISTRY_HELLO_EVT, label)
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

    // B2 修复：监听陌生窗 hello，回播自己的 rect。
    // payload 是 sender label；sender === self 的 hello（自己刚 emit 的）跳过。
    try {
      unlistenHello = await listen<string>(REGISTRY_HELLO_EVT, async (ev) => {
        const senderId = ev.payload
        if (!senderId || senderId === label) return
        try {
          const rect = await readWindowRect(getCurrentWindow())
          const visible = await getCurrentWindow().isVisible()
          await emit(REGISTRY_BROADCAST_EVT, {
            id: label,
            rect,
            visible,
            visualInset,
          } satisfies RegistryUpdatePayload)
        } catch (e) {
          console.warn(`[useSnapWindow] hello-back to ${senderId} failed:`, e)
        }
      })
    } catch (e) {
      console.warn('[useSnapWindow] listen hello failed:', e)
    }

    try {
      unlistenConstraint = await listen<ConstraintChangedPayload | null>(
        CONSTRAINT_CHANGED_EVT,
        (ev) => {
          // A4 修复：自过滤 senderId === label，避免 sender 自己 clear+reload（已 persist 过）。
          // 旧 payload 是 null（兼容历史 emit('snap:constraint-changed', null)）—— 仍处理，
          // 但建议 emit 端逐步迁移到 { senderId } 形式。
          if (ev.payload && ev.payload.senderId === label) return
          void onConstraintChanged()
        },
      )
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
            senderId: label,
            anchorId: newAnchor,
            edge: newEdge,
            intensity: newIntensity,
          } satisfies PreviewAnchorPayload)
        } catch (e) {
          console.warn('[useSnapWindow] emit preview-anchor failed:', e)
        }
      },
    )

    // 监听其他 webview 广播的 preview anchor 切换。A4 修复：自过滤 senderId === label
    try {
      unlistenPreviewAnchor = await listen<PreviewAnchorPayload>(PREVIEW_ANCHOR_EVT, (ev) => {
        const p = ev.payload
        if (!p) {
          previewAnchorGlobal.value = null
          previewEdgeGlobal.value = null
          previewIntensityGlobal.value = 0
          return
        }
        if (p.senderId === label) return // 自回声跳过
        previewAnchorGlobal.value = p.anchorId
        previewEdgeGlobal.value = p.edge
        previewIntensityGlobal.value = p.intensity
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

    // #30 follow-up D review (B2+B3)：所有 webview 启动期都 load KV → 写本地 store。
    //   - 之前只有 pet 启动 load，chat webview 的 store 永远是 empty 直到第一次 emit
    //   - 改后：每个 webview 自己 load → 即使 emit 路径出 race / 顺序问题也有 self-sufficient 数据
    // 延迟 300ms 等其他 webview 注册自己（cross-webview broadcast 有传播时延）。
    // primary 还额外负责 initial solve + emit CONSTRAINT_CHANGED 让其他 webview 同步。
    setTimeout(async () => {
      try {
        const r = await loadPersistedConstraints()
        console.log(`[snap] ${label} loaded ${r.loaded} constraint(s), dropped ${r.dropped}`)
        // #30 follow-up D：自检清除历史脏数据 primary→?（plan 设计前的遗留）
        await cleanupDirtyPrimaryOutbound()
        if (isPrimary(label)) {
          // primary：跑 initial solve 把 attached 窗摆到正确位置
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
          // B3 fix：primary 启动 load+solve 完后 emit CONSTRAINT_CHANGED 让其他 webview reload。
          // 解决 chat 早于 pet startup 时本地 load 还没拿到完整 registry 导致 anchor-missing drop 的 race。
          // 此处只是信号事件（KV 没改），不需要走 persistAndBroadcastConstraints；直接 emit 带 senderId。
          // A4 修复：payload 带 senderId 让 listener 自过滤；primary 本地 store 已经在 solve 前 load 过。
          try {
            await emit(CONSTRAINT_CHANGED_EVT, { senderId: label } satisfies ConstraintChangedPayload)
          } catch (e) {
            console.warn(`[snap] ${label} startup constraint-changed emit failed:`, e)
          }
        }
      } catch (e) {
        console.error(`[snap] ${label} startup load + solve failed:`, e)
      }
    }, 300)
  })

  // #30 follow-up G：webview hide / show 同步。
  // 关键：WebView2 在 window.hide() 时 **不会** 触发 DOM visibilitychange（已知 Tauri/
  // WebView2 bug，参 issues #6864 / #9524 / #10592 — document.visibilityState 永远为
  // 'visible'）。所以必须靠后端 Rust 主动 emit 'window:visibility-changed' 通知前端。
  //
  // 不同步会导致 windowRegistry 内本窗 visible 永远 true：
  //   - 其他窗 findCandidates 仍把本窗当 anchor 候选 → 拖近隐形窗仍被吸附
  //   - 其他窗 edgeOccupancy 仍把本窗当占用 → 拒绝合法 candidate
  //   - solver 仍把本窗的 dep 推到本窗旁
  //
  // 关键：只同步 visible 标志，不删 constraint。用户期望"关掉 chat 一会儿再打开
  // 还吸在 pet 旁"——constraint 保留，重开后 onMounted 启动期 solver 会按 constraint
  // 把 chat 摆回 pet 旁。
  let unlistenVisibility: UnlistenFn | null = null
  const VISIBILITY_EVT = 'window:visibility-changed'
  interface VisibilityPayload {
    label: string
    visible: boolean
  }
  const handleVisibilityChange = async (target: string, nextVisible: boolean): Promise<void> => {
    const reg = windowRegistry.get(target)
    if (!reg) {
      // 之前没注册过的窗（例如 hello 阶段还没收到 rect），先 upsert 一个最小 entry。
      // P4 修复 (review 2)：placeholder 永远 visible=false，直到第一次真正的 REGISTRY_BROADCAST
      // 带 rect 抵达后 onRegistryUpdate 才把 visible 修正。理由：0×0 rect 进 candidates.ts 评估
      // 时 rectEdgeGeometry.length=0 → computeEdgeOccupancy 返空 → 浪费一次 evaluation 周期；
      // findCandidates 顶头的 `if (!target.visible) continue` 把它干净跳过。
      if (target === label) return // 自己未注册不正常，跳过
      windowRegistry.upsert({
        id: target,
        rect: { x: 0, y: 0, w: 0, h: 0 },
        visible: false,
      })
      return
    }
    if (reg.visible === nextVisible) return // 幂等
    windowRegistry.updateVisible(target, nextVisible)

    // 本窗自己 hide 时：清拖动残留（用户拖到一半时被外部 hide 的极端 case）
    if (target === label && !nextVisible) {
      dragSession.cancel()
      currentDragMode = null
      bypassSnapForCurrentDrag = false
      if (armedTimer !== null) {
        clearTimeout(armedTimer)
        armedTimer = null
      }
      clearDragEndTimer()
      if (fieldAnchorGlobal.value === label) {
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
      if (sourceDragLabelGlobal.value === label) {
        sourceDragLabelGlobal.value = null
      }
      if (previewAnchorGlobal.value === label) {
        previewAnchorGlobal.value = null
        previewEdgeGlobal.value = null
        previewIntensityGlobal.value = 0
        try {
          await emit(PREVIEW_ANCHOR_EVT, {
            senderId: label,
            anchorId: null,
            edge: null,
            intensity: 0,
          } satisfies PreviewAnchorPayload)
        } catch {
          /* ignore */
        }
      }
    }
    // 别窗成为 anchor / source 时本窗有缓存的 preview/field 状态指向它 → 清掉
    // （别窗已经 hide 的极端 race；它本地的 emit clear 还未到达 / 已发但乱序）
    if (target !== label && !nextVisible) {
      if (fieldAnchorGlobal.value === target) {
        fieldAnchorGlobal.value = null
        fieldIntensityGlobal.value = 0
      }
      if (previewAnchorGlobal.value === target) {
        previewAnchorGlobal.value = null
        previewEdgeGlobal.value = null
        previewIntensityGlobal.value = 0
      }
    }
  }
  try {
    // 同步上下文（useSnapWindow 函数主体）无法 await — 用 .then 桥接 listen Promise
    listen<VisibilityPayload>(VISIBILITY_EVT, (ev) => {
      if (!ev.payload) return
      void handleVisibilityChange(ev.payload.label, ev.payload.visible)
    })
      .then((fn) => {
        unlistenVisibility = fn
      })
      .catch((e) => {
        console.warn('[useSnapWindow] listen visibility-changed failed:', e)
      })
  } catch (e) {
    console.warn('[useSnapWindow] listen visibility-changed setup failed:', e)
  }

  onBeforeUnmount(() => {
    unlistenMove?.()
    unlistenResize?.()
    unlistenRegistry?.()
    unlistenHello?.()
    unlistenConstraint?.()
    unlistenPreviewAnchor?.()
    unlistenField?.()
    unlistenVisibility?.()
    stopPreviewAnchorWatch?.()
    if (escHandler) window.removeEventListener('keydown', escHandler, true)
    if (pointerDownHandler) window.removeEventListener('pointerdown', pointerDownHandler, true)
    clearDragEndTimer()
    if (armedTimer !== null) clearTimeout(armedTimer)
  })

  return {
    isPreviewAnchor,
    previewEdgeFor,
    previewIntensityFor,
    isFieldAnchor,
    fieldIntensityFor,
    selfLean,
    syncRustSnap,
    broadcastSelfRect,
  }
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
