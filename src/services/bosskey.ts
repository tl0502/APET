// BossKey 前端 IPC 绑定（#42）— 摸鱼模式（隐藏 4 窗）。
//
// IPC 与 Rust commands::bosskey 同 schema：
//   bosskey_toggle → 返操作后的 hidden boolean
//   bosskey_rebind({ accelerator }) → 改快捷键
//   bosskey_is_hidden → 当前是否隐藏

import { invoke } from './ipc'

export async function bosskeyToggle(): Promise<boolean> {
  return invoke<boolean>('bosskey_toggle')
}

export async function bosskeyIsHidden(): Promise<boolean> {
  try {
    return await invoke<boolean>('bosskey_is_hidden')
  } catch (e) {
    console.warn('[bosskey] is_hidden failed, defaulting false:', e)
    return false
  }
}

export async function bosskeyRebind(accelerator: string): Promise<void> {
  await invoke<void>('bosskey_rebind', { accelerator })
}

/** 与 Rust 端 BOSSKEY_TOGGLED_EVENT / SHORTCUT_REGISTER_FAILED_EVENT 同步。 */
export const BOSSKEY_TOGGLED_EVENT = 'boss_key:toggled'

export interface BosskeyToggledPayload {
  hidden: boolean
}
