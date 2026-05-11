// ShortcutService IPC（#11 / #21 Step 3）。
// - probeGlobalShortcut(s)：试 register/unregister 验证可用性（#21 Onboarding Step 3 冲突探测）
// - setShortcutChat(s)：改 chat 快捷键 + 持久化（onboarding "用这个" 提交 + 后续设置面板）
// - getChatShortcut()：读当前已注册的 chat 快捷键（null = 启动期 register 失败）
import { invoke } from './ipc'
import type { ProbeResult } from '@/types/shortcut'

export function probeGlobalShortcut(shortcut: string): Promise<ProbeResult> {
  return invoke<ProbeResult>('probe_global_shortcut', { shortcut })
}

export function setShortcutChat(shortcut: string): Promise<void> {
  return invoke<void>('set_shortcut_chat', { shortcut })
}

/**
 * 拉当前已注册的 chat 快捷键。
 * - 字符串 = 启动期 register 成功，这是当前生效值
 * - null = 启动期 register 失败（系统占用 / 平台异常），当前无快捷键
 *
 * 用于 Onboarding Step 3 同步显示：避免前端硬编码 default 与后端实际状态漂移。
 */
export function getChatShortcut(): Promise<string | null> {
  return invoke<string | null>('get_chat_shortcut')
}
