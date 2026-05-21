/** Shortcut event/IPC 契约（与 src-tauri/src/services/shortcuts.rs 对齐）。 */

/** `shortcut:chat` event payload。当前 source 仅 'global_shortcut'；未来 onboarding / hub 触发可扩展。 */
export interface ShortcutChatPayload {
  source: 'global_shortcut'
  timestamp_ms: number
}

/** `shortcut:register-failed` event payload。启动期注册失败 / 改快捷键失败时 emit。
 *  `kind` 让前端按业务分发（chat / workspace），不依赖 `shortcut` 字面比对
 *  （M2 改键 UI 上线后，字符串会变；kind 是稳定语义标签）。 */
export interface ShortcutRegisterFailedPayload {
  kind: 'chat' | 'workspace'
  shortcut: string
  error: string
}

/** `probe_global_shortcut` IPC 返回。 */
export interface ProbeResult {
  available: boolean
  error: string | null
}
