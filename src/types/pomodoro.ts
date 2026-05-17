// #28 PomodoroService 前端类型与常量。
// 与 src-tauri/src/services/pomodoro.rs 同步：Phase / ActiveSession / IPC 入参出参 / 事件名。

export type PomodoroPhase = 'FOCUS' | 'PAUSED_F' | 'REST' | 'PAUSED_R'

export interface ActiveSession {
  sessionId: string
  phase: PomodoroPhase
  focusMin: number
  restMin: number
  sessionStartedAt: string
  phaseStartedAt: string
  phasePlannedEnd: string
  lastTickAt: string
  pauseStartedAt: string | null
  accumulatedPauseMs: number
  sleepCompensationMs: number
}

export interface StartInput {
  focusMin?: number
  restMin?: number
}

export interface StopOutput {
  status: 'completed' | 'cancelled'
  completionRatio: number
  focusMinActual: number
  sessionId: string
}

export interface TodayStats {
  completed: number
  cancelled: number
  totalFocusMin: number
}

export interface PomodoroTickPayload {
  sessionId: string
  phase: PomodoroPhase
  remainingMs: number
  focusMin: number
  restMin: number
}

/** state_changed 事件：phase=null 表示进入 IDLE（active 已清）。 */
export interface PomodoroStateChangedPayload {
  phase: PomodoroPhase | null
}

export interface PomodoroFocusStartedPayload {
  sessionId: string
  focusMin: number
}

export interface PomodoroFocusEndedPayload {
  sessionId: string
  completed: boolean
  interruptedBy?: string
}

export interface PomodoroRestStartedPayload {
  sessionId: string
  restMin: number
}

export interface PomodoroRestEndedPayload {
  sessionId: string
}

// === 事件名常量（与后端 EVENT_* 字面量一致） ===

export const POMODORO_TICK_EVENT = 'pomodoro:tick'
export const POMODORO_STATE_CHANGED_EVENT = 'pomodoro:state_changed'
export const POMODORO_FOCUS_STARTED_EVENT = 'pomodoro:focus_started'
export const POMODORO_FOCUS_ENDED_EVENT = 'pomodoro:focus_ended'
export const POMODORO_REST_STARTED_EVENT = 'pomodoro:rest_started'
export const POMODORO_REST_ENDED_EVENT = 'pomodoro:rest_ended'

// === 取值范围（与后端 MIN/MAX_*_MIN 一致） ===

export const FOCUS_MIN_RANGE = { min: 5, max: 90, step: 5 } as const
export const REST_MIN_RANGE = { min: 1, max: 30, step: 1 } as const
export const DEFAULT_FOCUS_MIN = 25
export const DEFAULT_REST_MIN = 5
export const MIN_COMPLETE_RATIO = 0.3

// === 预设（plan 决策 #9 + 用户答 Q7） ===

export interface PomodoroPreset {
  id: string
  label: string
  focusMin: number
  restMin: number
}

export const POMODORO_PRESETS: readonly PomodoroPreset[] = [
  { id: 'classic', label: '经典 25/5', focusMin: 25, restMin: 5 },
  { id: 'deep', label: '深度 50/10', focusMin: 50, restMin: 10 },
] as const

// === Phase 元信息（UI 反复用） ===

export interface PhaseMeta {
  emoji: string
  label: string
  /** ElProgress 颜色 token */
  color: string
  /** 是否处于"运行/暂停"任一态（用于 UI 按钮显示） */
  active: boolean
  isPaused: boolean
  isFocusLike: boolean
}

export function getPhaseMeta(phase: PomodoroPhase | null): PhaseMeta {
  switch (phase) {
    case 'FOCUS':
      return {
        emoji: '🎯',
        label: '专注中',
        color: 'var(--aipet-color-primary)',
        active: true,
        isPaused: false,
        isFocusLike: true,
      }
    case 'PAUSED_F':
      return {
        emoji: '⏸',
        label: '专注暂停',
        color: 'var(--aipet-color-text-3)',
        active: true,
        isPaused: true,
        isFocusLike: true,
      }
    case 'REST':
      return {
        emoji: '🌿',
        label: '休息中',
        color: '#22c55e',
        active: true,
        isPaused: false,
        isFocusLike: false,
      }
    case 'PAUSED_R':
      return {
        emoji: '⏸',
        label: '休息暂停',
        color: 'var(--aipet-color-text-3)',
        active: true,
        isPaused: true,
        isFocusLike: false,
      }
    default:
      return {
        emoji: '🌙',
        label: '未开始',
        color: 'var(--aipet-color-text-3)',
        active: false,
        isPaused: false,
        isFocusLike: false,
      }
  }
}

/** ms → "mm:ss"（用于倒计时显示） */
export function formatRemainingMs(ms: number): string {
  const totalSec = Math.max(0, Math.ceil(ms / 1000))
  const m = Math.floor(totalSec / 60)
  const s = totalSec % 60
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
}
