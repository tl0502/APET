/** Shortcut event/IPC 契约（与 src-tauri/src/services/shortcuts.rs 对齐）。 */

/** `shortcut:chat` event payload。当前 source 仅 'global_shortcut'；未来 onboarding / hub 触发可扩展。 */
export interface ShortcutChatPayload {
  source: 'global_shortcut'
  timestamp_ms: number
}

/** `shortcut:register-failed` event payload。启动期注册失败 / 改快捷键失败时 emit。 */
export interface ShortcutRegisterFailedPayload {
  shortcut: string
  error: string
}

/** `probe_global_shortcut` IPC 返回。 */
export interface ProbeResult {
  available: boolean
  error: string | null
}
