/** PersonaService IPC 契约（与 src-tauri/src/services/persona.rs::PersonaSummary 对齐）。 */
export interface PersonaSummary {
  id: string
  snapshot_id: string
  name: string
  version: string
  source: string
  /** 完整 .soul.md 正文（不含 frontmatter），供 ChatService 拼 system prompt。 */
  raw_markdown: string
}

/**
 * PersonaListItem：list 用，不带 raw_markdown（减少 IPC 数据量）。
 *
 * 与 src-tauri/src/services/persona.rs::PersonaListItem 对齐。
 * 后端排序：is_active DESC + id ASC（active 卡片排第一，便于 onboarding picker 默认聚焦）。
 */
export interface PersonaListItem {
  id: string
  name: string
  version: string
  source: string
  is_active: boolean
}

/**
 * `persona:activated` event payload（与 nickname:changed 同款契约）。
 *
 * 当 commands::persona::persona_activate 成功后 emit；payload 是 persona id 字符串。
 * 监听场景：M3 设置面板（独立窗口）切人格 → 角色窗 listen 后刷新 PetCanvas / system prompt。
 */
export type PersonaActivatedPayload = string

export type PersonaDiagnosticSeverity = 'error' | 'warning' | 'info'

export interface PersonaDiagnostic {
  code: string
  severity: PersonaDiagnosticSeverity
  message: string
}

export interface PersonaDraftValidationResult {
  diagnostics: PersonaDiagnostic[]
  blocking: boolean
  token_estimate: number
}

export interface PersonaSaveResult {
  persona_id: string
  snapshot_id: string
  version: string
  activated: boolean
  diagnostics: PersonaDiagnostic[]
}

export interface SoulRuntimeProfile {
  identity_prompt: string
  style_prompt: string
  examples: string[]
  initiative_config: Record<string, unknown>
  memory_policy: Record<string, unknown>
  ui_metadata: Record<string, unknown>
  source_kind: string
  source_hash: string
}

export interface PersonaSnapshotActivatedPayload {
  personaId: string
  snapshotId: string
}
