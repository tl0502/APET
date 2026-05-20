// useFocusAOT — focus-driven 窗口层级（#30 follow-up H）。
//
// 业界 floating-panel-group 模式：app 的工具窗（chat / pomodoro / future settings / tasks）
// 平时 alwaysOnTop=false，被 focus 时升 topmost，失焦时降回 normal。pet 永远 topmost
// （Tier 1），保证桌宠始终在所有工具窗之上。
//
// 设计要点：
// - **keepTopmost 回调**：let caller 强制保持 topmost（pomodoro FOCUS 期专注模式不让步），
//   优先级高于 focus 状态。每次 onBlur 时调 → 真要降 AOT 前先问 caller 是否要保持
// - **幂等**：本地 lastAOT cache 防重复 setAlwaysOnTop（IPC 开销 + Tauri Issue #6568 多次 set 可能
//   触发窗位置抖动）
// - **平台一致**：Tauri 2 onFocusChanged 在 Windows/macOS/Linux 都触发（与 visibilitychange
//   不同——后者 WebView2 失灵）
// - **不接管 pomodoro 的 fullscreen 期 AOT**：caller 负责在 fullscreen 期不调本 composable
//   或让 keepTopmost 返特殊值。当前 PomodoroApp.toggleFullscreen 在进入全屏前关 AOT，本
//   composable 也会在 fullscreen 期被 caller 短路（PomodoroApp 实现里检查）

import { onBeforeUnmount, onMounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { UnlistenFn } from '@tauri-apps/api/event'

export interface UseFocusAOTOptions {
  /** 调用时机：本窗 blur 准备降 AOT 前。返 true → 保持 topmost（不调 setAlwaysOnTop(false)）。
   *  典型用例：pomodoro FOCUS/PAUSED_F 期返 true，让番茄专注期不被任何窗盖住。
   *  缺省 / 返 false → 走标准 focus-driven AOT（失焦降回 normal）。 */
  shouldKeepTopmost?: () => boolean
}

export interface FocusAOTApi {
  /** 主动同步一次 AOT 到当前 focus + keepTopmost 综合状态。
   *  caller（如 pomodoro）在 phase 变化时调用，让 keepTopmost 新值立刻生效。
   *  例：FOCUS→IDLE 转换时 phase 从"强制 topmost"变成"focus-driven"，
   *  立即调 resync() 让窗按当前 focus 状态决定 AOT。 */
  resync: () => Promise<void>
}

export function useFocusAOT(options: UseFocusAOTOptions = {}): FocusAOTApi {
  const shouldKeepTopmost = options.shouldKeepTopmost ?? (() => false)
  let unlistenFocus: UnlistenFn | null = null
  // 幂等 cache：null = 未初始化；true/false = 最后一次成功调用的值
  // 防 onFocusChanged 高频触发时重复 setAlwaysOnTop 引发位置抖动（Tauri Issue #6568）
  let lastAOT: boolean | null = null

  async function applyAOT(target: boolean): Promise<void> {
    if (lastAOT === target) return
    try {
      await getCurrentWindow().setAlwaysOnTop(target)
      lastAOT = target
    } catch (e) {
      console.warn(`[useFocusAOT] setAlwaysOnTop(${target}) failed:`, e)
    }
  }

  /** 综合 focus 状态 + keepTopmost 决定目标 AOT。
   *  - keepTopmost=true（pomodoro FOCUS 期）→ topmost 不动，无视 focus
   *  - focused=true → topmost
   *  - focused=false + keepTopmost=false → normal */
  async function syncAOT(focused: boolean): Promise<void> {
    const target = focused || shouldKeepTopmost()
    await applyAOT(target)
  }

  async function resync(): Promise<void> {
    try {
      const focused = await getCurrentWindow().isFocused()
      await syncAOT(focused)
    } catch (e) {
      console.warn('[useFocusAOT] resync isFocused() failed:', e)
    }
  }

  onMounted(async () => {
    const win = getCurrentWindow()
    try {
      // 初始同步：startup 期 windowState 可能与 OS 不一致，强制对齐
      const focused = await win.isFocused()
      await syncAOT(focused)
    } catch (e) {
      console.warn('[useFocusAOT] initial sync failed:', e)
    }
    try {
      unlistenFocus = await win.onFocusChanged(({ payload: focused }) => {
        void syncAOT(focused)
      })
    } catch (e) {
      console.warn('[useFocusAOT] onFocusChanged listen failed:', e)
    }
  })

  onBeforeUnmount(() => {
    unlistenFocus?.()
  })

  return { resync }
}
