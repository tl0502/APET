/** MemoryService IPC 契约（KV 偏好表，架构 §339 memory 表）。 */
export interface MemoryItem {
  key: string
  value: string
  /** 'user_set' | 'inferred'（来源标识，UI 决定是否提示用户"AI 推断"）。 */
  source: string
  updated_at: string
}
