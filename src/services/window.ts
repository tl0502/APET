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
