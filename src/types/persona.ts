/** PersonaService IPC 契约（与 src-tauri/src/commands/persona.rs::PersonaSummary 对齐）。 */
export interface PersonaSummary {
  id: string
  name: string
  version: string
  source: string
  /** 完整 .soul.md 正文（不含 frontmatter），供 ChatService 拼 system prompt。 */
  raw_markdown: string
}
