// useInteractionRaycaster：物理交互状态机（#40，模块 N 主干）。
//
// 范围（ADR-025 lock）：
// - hitbox = AABB 单 body（PetCanvas 整窗 raycast = body 命中；M3+ 接 Bone Proxy 才扩 4 hitbox）
// - 5 事件路由：click / dblclick / longpress / rclick / drag
// - drag 走单独路径 recordDragCount（30s 滑窗 ≥3 触发抗议，Rust 端统一管 emit）
// - 右键 contextmenu 不走 startDragging（保留默认 webview 行为关，自绘菜单浮层由调用方 mount）
//
// 状态机:
//   pointerdown(0) → 启动 longpress 600ms timer + 记 startTime/pos
//     ├─ pointermove > 5px → 取消 longpress, 标 dragStarted, 调 startDragging() + recordDragCount(+1)
//     ├─ longpress 600ms 到 → 取消 click 候选, dispatch('longpress'), 标 longpressFired
//     └─ pointerup
//          ├─ dragStarted → 结束（drag 起点已通过 recordDragCount 上报）
//          ├─ longpressFired → 结束
//          └─ 纯 click：
//                ├─ 距上次 click <300ms → dblclick（取消 pending click timer）
//                └─ 否则 → schedule click dispatch 在 300ms 后（等 dblclick 窗口过）
//   pointerdown(2) / contextmenu → dispatch('rclick') + emit 自定义事件让父组件开菜单
//
// 与 useSnapWindow 协作（lessons #12 / #13 已有约束）：
//   useSnapWindow.onPointerDown 在 capture phase 已 arm dragSession；
//   本 composable 在 bubble phase 听 pointerdown，threshold 跨过才真正调 startDragging。
//   未跨 threshold 用户松手 / 长按时，snap 的 armed dragSession 1s 自动 timeout → ok。

import { onBeforeUnmount, onMounted, ref, type Ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  dispatchInteraction,
  recordDragCount,
  type ReactionEntry,
} from '@/services/interaction'
import { cancelWander } from '@/services/livingPet'

/** drag 阈值：pointermove 累计位移超过此像素值即判定为拖动起点。 */
const DRAG_THRESHOLD_PX = 5
/** 长按阈值（毫秒）：pointerdown 到 pointerup 超过此值 + 未达 drag 阈值 = longpress。 */
const LONGPRESS_MS = 600
/** 双击阈值（毫秒）：两次 pointerup 间隔 <= 此值 = dblclick。 */
const DBLCLICK_MS = 300

export interface InteractionContextMenuEvent {
  /** webview 视口坐标（不是 canvas 内）。父组件用此定位浮层菜单。 */
  x: number
  y: number
  /** Rust 派发返回的反应条目；rclick 走 dispatch 但 action_id=tilt_head 不强制播动作。 */
  reaction: ReactionEntry | null
}

export interface UseInteractionRaycasterOptions {
  /** 当前 webview window label，用于 recordDragCount 多窗维度（M2 仅 'pet'）。 */
  windowLabel: string
  /** 是否启用（onboarding 等场景应传 false 跳过物理交互）。 */
  enabled?: () => boolean
}

export interface UseInteractionRaycasterReturn {
  /** 右键打开菜单事件：父组件 watch / emit hook 自绘菜单浮层。 */
  contextMenu: Ref<InteractionContextMenuEvent | null>
  /** 关闭菜单（父组件点击外部 / Esc 时调）。 */
  closeContextMenu: () => void
  /** dev 期诊断：当前内部状态。 */
  debug: Ref<{
    longpressArmed: boolean
    dragStarted: boolean
    longpressFired: boolean
  }>
}

/**
 * 把 5 事件状态机挂到 targetRef 元素。
 *
 * 调用方在 setup 阶段拿到 ref，本 composable onMounted 时 attach listener，
 * onBeforeUnmount 自动 detach。重复挂载（HMR）会先 cleanup 旧 listener。
 */
export function useInteractionRaycaster(
  targetRef: Ref<HTMLElement | null>,
  opts: UseInteractionRaycasterOptions,
): UseInteractionRaycasterReturn {
  const isEnabled = opts.enabled ?? (() => true)

  const contextMenu = ref<InteractionContextMenuEvent | null>(null)
  const debug = ref({
    longpressArmed: false,
    dragStarted: false,
    longpressFired: false,
  })

  // 单 pointerdown session 状态（pointerup / 状态机退出时全部清零）。
  let downX = 0
  let downY = 0
  let downTime = 0
  let longpressTimer: number | null = null
  let pendingClickTimer: number | null = null
  let lastClickAt = 0
  let dragStarted = false
  let longpressFired = false
  let pointerActive = false

  function clearTimers() {
    if (longpressTimer !== null) {
      window.clearTimeout(longpressTimer)
      longpressTimer = null
    }
  }

  function resetSession() {
    clearTimers()
    pointerActive = false
    dragStarted = false
    longpressFired = false
    debug.value.longpressArmed = false
    debug.value.dragStarted = false
    debug.value.longpressFired = false
  }

  function onPointerDown(event: PointerEvent) {
    if (!isEnabled()) return

    // 右键：单独路径（不进入 click/drag 状态机）。
    if (event.button === 2) {
      // 不 preventDefault（contextmenu 事件由 onContextMenu 统一拦），允许浏览器派发 contextmenu。
      // 真正的菜单打开在 onContextMenu 内做（兼容键盘 ContextMenu 键）。
      return
    }
    if (event.button !== 0) return

    // closest('[data-no-drag]') 兜底：与 PetCanvas 现有 startDragging 路径一致，
    // 让 reminder bubble 等按钮区不触发桌宠交互。
    if ((event.target as HTMLElement | null)?.closest('[data-no-drag]')) return

    // 用 capture phase 否则 useSnapWindow 的 capture-arm 会先跑、bubble 跑到这里再 arm。
    // 但 useSnapWindow 自己也是 capture 上注册，时序约定不可乱动；保持 bubble 即可，
    // useSnapWindow 不依赖本 composable 的事件，互相独立。
    resetSession()
    pointerActive = true
    downX = event.clientX
    downY = event.clientY
    downTime = performance.now()

    // 启动长按 timer
    longpressTimer = window.setTimeout(() => {
      if (!pointerActive || dragStarted) return
      longpressFired = true
      debug.value.longpressFired = true
      // 在调 dispatch 前取消 wander tween（与拖动同款副作用，避免长按时模型还在飘）。
      void cancelWander().catch(() => {})
      void dispatchInteraction('longpress', 'body')
    }, LONGPRESS_MS) as unknown as number
    debug.value.longpressArmed = true
  }

  async function startDraggingIfNeeded() {
    // 与现有 PetCanvas 隐式契约一致：先 cancelWander 再 startDragging。
    void cancelWander().catch(() => {})
    try {
      await getCurrentWindow().startDragging()
    } catch (e) {
      console.error('[interaction] startDragging failed:', e)
    }
  }

  function onPointerMove(event: PointerEvent) {
    if (!pointerActive || dragStarted || longpressFired) return
    const dx = event.clientX - downX
    const dy = event.clientY - downY
    if (dx * dx + dy * dy < DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX) return

    // 跨过 drag 阈值：标 dragStarted、取消 longpress、上报一次拖动起点、调 OS startDragging。
    dragStarted = true
    debug.value.dragStarted = true
    clearTimers()
    // recordDragCount 内部失败仅 console.warn，不阻塞 startDragging。
    void recordDragCount(opts.windowLabel, 1)
    // 单次 drag 起点也走一次 dispatch 让 emit 链路可观测（reaction_table.drag.body 默认无 mood/template）。
    void dispatchInteraction('drag', 'body')
    void startDraggingIfNeeded()
  }

  function onPointerUp(_event: PointerEvent) {
    if (!pointerActive) return
    const elapsed = performance.now() - downTime
    clearTimers()
    debug.value.longpressArmed = false

    // drag 起点已在 onPointerMove 内上报；松手仅清状态。
    if (dragStarted) {
      resetSession()
      return
    }
    // 长按已触发，松手不再当 click。
    if (longpressFired) {
      resetSession()
      return
    }
    // 微动作但未达 drag 阈值 & 未达 longpress：当 click 处理（含 dblclick 检测）。
    // 限制：< 1s 的 pointerdown 才视为有效 click（防止 webview 卡顿场景误触）。
    if (elapsed < 1000) {
      const now = performance.now()
      if (now - lastClickAt <= DBLCLICK_MS) {
        // dblclick：取消上一次 pending click 派发，改派 dblclick。
        if (pendingClickTimer !== null) {
          window.clearTimeout(pendingClickTimer)
          pendingClickTimer = null
        }
        lastClickAt = 0
        void dispatchInteraction('dblclick', 'body')
      } else {
        lastClickAt = now
        // 等 DBLCLICK_MS 看是否会有第二次 click。其间若再有 click → 上面分支换 dblclick。
        pendingClickTimer = window.setTimeout(() => {
          pendingClickTimer = null
          void dispatchInteraction('click', 'body')
        }, DBLCLICK_MS) as unknown as number
      }
    }
    resetSession()
  }

  function onPointerCancel(_event: PointerEvent) {
    // OS 接管拖动 / 视图失焦：等同 pointerup 但不派发 click。
    resetSession()
  }

  function onContextMenu(event: MouseEvent) {
    if (!isEnabled()) return
    // 阻止 webview 默认浏览器右键菜单（dev 期也屏蔽，避免与自绘菜单同时出现）。
    event.preventDefault()
    // dispatch + open 自绘菜单。dispatch 失败不阻塞菜单（菜单是核心 UX）。
    void dispatchInteraction('rclick', 'body').then((reaction) => {
      contextMenu.value = {
        x: event.clientX,
        y: event.clientY,
        reaction,
      }
    })
  }

  function closeContextMenu() {
    contextMenu.value = null
  }

  onMounted(() => {
    const el = targetRef.value
    if (!el) {
      console.warn('[interaction] target element not mounted — listeners not attached')
      return
    }
    el.addEventListener('pointerdown', onPointerDown)
    el.addEventListener('pointermove', onPointerMove)
    el.addEventListener('pointerup', onPointerUp)
    el.addEventListener('pointercancel', onPointerCancel)
    el.addEventListener('contextmenu', onContextMenu)
  })

  onBeforeUnmount(() => {
    const el = targetRef.value
    if (el) {
      el.removeEventListener('pointerdown', onPointerDown)
      el.removeEventListener('pointermove', onPointerMove)
      el.removeEventListener('pointerup', onPointerUp)
      el.removeEventListener('pointercancel', onPointerCancel)
      el.removeEventListener('contextmenu', onContextMenu)
    }
    if (pendingClickTimer !== null) {
      window.clearTimeout(pendingClickTimer)
      pendingClickTimer = null
    }
    clearTimers()
  })

  return {
    contextMenu,
    closeContextMenu,
    debug,
  }
}

/** export 给单测用的内部常量（vitest）。 */
export const __TEST_ONLY__ = {
  DRAG_THRESHOLD_PX,
  LONGPRESS_MS,
  DBLCLICK_MS,
}

// 仅供调用方在拿到 ReactionEntry 后判断是否触发气泡 / mood icon 视觉反馈。
// 不导出额外抽象，避免接口面爆炸。
export type { InteractionEventKind, ReactionEntry } from '@/services/interaction'
