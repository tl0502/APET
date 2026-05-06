/** NicknameService IPC 事件 payload（架构 §711 'nickname:changed'）。 */
export interface NicknameChangedPayload {
  /** 'pet' 或 'user'。 */
  which: 'pet' | 'user'
  /** 新值；null 表示置空（fallback 到默认）。 */
  value: string | null
}
