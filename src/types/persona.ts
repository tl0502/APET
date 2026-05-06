/** PersonaService IPC 契约（与 src-tauri/src/services/persona.rs::PersonaSummary 对齐）。 */
export interface PersonaSummary {
  id: string
  name: string
  version: string
  source: string
  /** 完整 .soul.md 正文（不含 frontmatter），供 ChatService 拼 system prompt。 */
  raw_markdown: string
}

/**
 * `persona:activated` event payload（与 nickname:changed 同款契约）。
 *
 * 当 commands::persona::persona_activate 成功后 emit；payload 是 persona id 字符串。
 * 监听场景：M3 设置面板（独立窗口）切人格 → 角色窗 listen 后刷新 PetCanvas / system prompt。
 */
export type PersonaActivatedPayload = string
