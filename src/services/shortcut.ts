// ShortcutService IPC（#11 / #21 Step 3 / 收尾 #2）。
// - probeGlobalShortcut(s)：试 register/unregister 验证可用性（#21 Onboarding Step 3 冲突探测）
// - setShortcutChat(s)：改 chat 快捷键 + 持久化（onboarding "用这个" 提交 + 后续设置面板）
// - getChatShortcut()：读当前已注册的 chat 快捷键（null = 启动期 register 失败）
// - getChatRegisterStatus()：读启动期 register 留痕（App.vue mount 时查询，解决 setup
//   emit 早于 listener 挂载的 race；详 src-tauri/.../shortcuts.rs::ShortcutRegistry 注释）
import { invoke } from './ipc'
import type { ProbeResult, ShortcutRegisterFailedPayload } from '@/types/shortcut'

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

/**
 * 拉启动期 register chat 快捷键的留痕。
 * - 非 null = 启动期 register 失败且尚未通过 setShortcutChat 恢复（前端 toast 提示用户改键）
 * - null = 启动期 OK / 用户已成功改键
 */
export function getChatRegisterStatus(): Promise<ShortcutRegisterFailedPayload | null> {
  return invoke<ShortcutRegisterFailedPayload | null>('get_chat_register_status')
}
