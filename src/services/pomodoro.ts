// #28 PomodoroService 前端 IPC wrapper。
// 与 src-tauri/src/commands/pomodoro.rs 同步；6 命令。

import { invoke } from './ipc'
import type { ActiveSession, StartInput, StopOutput, TodayStats } from '@/types/pomodoro'

export function startPomodoro(input: StartInput): Promise<ActiveSession> {
  return invoke<ActiveSession>('pomodoro_start', { input })
}

export function pausePomodoro(): Promise<ActiveSession> {
  return invoke<ActiveSession>('pomodoro_pause')
}

export function resumePomodoro(): Promise<ActiveSession> {
  return invoke<ActiveSession>('pomodoro_resume')
}

export function stopPomodoro(): Promise<StopOutput> {
  return invoke<StopOutput>('pomodoro_stop')
}

export function getActivePomodoro(): Promise<ActiveSession | null> {
  return invoke<ActiveSession | null>('pomodoro_active')
}

export function getPomodoroTodayStats(): Promise<TodayStats> {
  return invoke<TodayStats>('pomodoro_today_stats')
}
