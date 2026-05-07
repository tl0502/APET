// WindowService IPC（#9 settings + #10 pet 位置）。M1 阶段前端不主动调 get/save_pet_position
// （后端 Moved 自动管）；wrapper 给未来 M3 设置面板"重置位置 / 看当前位置"复用。
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
