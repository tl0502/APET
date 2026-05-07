// ShortcutService IPC（#11）。
// - probeGlobalShortcut(s)：试 register/unregister 验证可用性（给 #17 Onboarding Step 3）
// - setShortcutChat(s)：改 chat 快捷键 + 持久化（M1 stub，UI 推到 #17 / 后续 #9 增补）
import { invoke } from './ipc'
import type { ProbeResult } from '@/types/shortcut'

export function probeGlobalShortcut(shortcut: string): Promise<ProbeResult> {
  return invoke<ProbeResult>('probe_global_shortcut', { shortcut })
}

export function setShortcutChat(shortcut: string): Promise<void> {
  return invoke<void>('set_shortcut_chat', { shortcut })
}
