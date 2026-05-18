// WindowService IPC（#9 settings + #10 pet 位置 + #14 chat 显隐）。M1 阶段前端不主动调
// get/save_pet_position（后端 Moved 自动管）；wrapper 给未来 M3 设置面板"重置位置 / 看
// 当前位置"复用。
import { invoke } from './ipc'
import type { LastPosition } from '@/types/window'

export function showSettings(): Promise<void> {
  return invoke<void>('settings_show')
}

export function hideSettings(): Promise<void> {
  return invoke<void>('settings_hide')
}

export function getPetPosition(): Promise<LastPosition | null> {
  return invoke<LastPosition | null>('get_pet_position')
}

export function savePetPosition(pos: LastPosition): Promise<void> {
  return invoke<void>('save_pet_position', { pos })
}

/** 显示 chat 窗口（#14；接 #11 全局快捷键、#15 设置面板"立即试聊"等入口）。 */
export function showChat(): Promise<void> {
  return invoke<void>('chat_show')
}

/** 隐藏 chat 窗口（不销毁，保留 messages state；ESC / 关闭按钮路径）。 */
export function hideChat(): Promise<void> {
  return invoke<void>('chat_hide')
}

/** 切换 chat 窗口可见性（#11 shortcut:chat 主路径）。 */
export function toggleChat(): Promise<void> {
  return invoke<void>('chat_toggle')
}

/** #22 任务窗口（提醒/番茄/待办）show/hide/toggle。与 settings 同款"关 = hide"。 */
export function showTasks(): Promise<void> {
  return invoke<void>('tasks_show')
}

export function hideTasks(): Promise<void> {
  return invoke<void>('tasks_hide')
}

export function toggleTasks(): Promise<void> {
  return invoke<void>('tasks_toggle')
}

/**
 * #28 follow-up 番茄独立窗口（Pomotroid 型，紧凑 360×480）。
 * 与 tasks 同款"关 = hide"（首次关闭时后端 emit `pomodoro:hide_hint` 提示用户后台仍在计时）。
 *
 * 入口：托盘菜单「番茄...」/ tasks tab「在独立窗口打开 ↗」按钮 / pomodoro_start 后端自动 show。
 * 位置记忆：setup 阶段还原到 KV `window:pomodoro:last_position`（visible:false 下 set_position 防闪动）。
 * alwaysOnTop：tauri.conf.json 默认 false；PomodoroApp.vue listen pomodoro:state_changed 按 phase 切换。
 */
export function showPomodoro(): Promise<void> {
  return invoke<void>('pomodoro_show')
}

export function hidePomodoro(): Promise<void> {
  return invoke<void>('pomodoro_hide')
}

export function togglePomodoro(): Promise<void> {
  return invoke<void>('pomodoro_toggle')
}

/**
 * #24 视角档位（pet 角色窗）。
 * - 'half'：320×320 等比，相机看胸口（默认；对话/表情场景）
 * - 'full'：320×512（1:1.6），相机看全身（装扮/动作场景）
 *
 * 跨窗口同步契约：set 后后端会广播 `pet:view-changed` 事件，pet 窗的 App.vue
 * listen 该事件调 `runtime.setView()`；settings 自己不需要 listen（自身改的不用回灌）。
 */
export type PetViewPreset = 'half' | 'full'

/** preset → 窗口逻辑像素尺寸。与 Rust `preset_to_size` 对齐（跨语言常量两份）。 */
export const PET_VIEW_SIZES: Record<PetViewPreset, { width: number; height: number }> = {
  half: { width: 320, height: 320 },
  full: { width: 320, height: 512 },
}

/** 视角切换 Tauri event 名；后端 emit 时使用同名字面量。 */
export const PET_VIEW_CHANGED_EVENT = 'pet:view-changed'

export function getPetViewPreset(): Promise<PetViewPreset> {
  return invoke<PetViewPreset>('get_pet_view_preset')
}

export function setPetViewPreset(preset: PetViewPreset): Promise<void> {
  return invoke<void>('set_pet_view_preset', { preset })
}
