/** ConsentService IPC 类型（与 src-tauri/src/services/consent.rs 对齐）。 */

/** consent.method 枚举（schema 守护：'soul_pledge' | 'classic'）。 */
export type ConsentMethod = 'soul_pledge' | 'classic'

/**
 * consent 表单行（与 services/consent.rs::ConsentRecord 对齐）。
 *
 * `accepted_at` 仅当 `granted === true` 时为字符串（用户真同意时间，RFC3339）；
 * `granted === false` 时为 null（service 层归一，防前端误读 schema seed 占位时间）。
 */
export interface ConsentRecord {
  granted: boolean
  method: ConsentMethod
  version: number
  accepted_at: string | null
}

/**
 * 启动期路由 / Onboarding 状态机判定结果（与 services/consent.rs::ConsentStatus 对齐）。
 *
 * Rust 端 `#[serde(rename_all = "snake_case")]` 让 enum 序列化为：
 * - `"match"` / `"not_granted"` （unit variant 直接字符串）
 * - `{ "need_reconsent": { stored_version, current_version } }` （含数据 variant）
 *
 * 语义：
 * - `match`：stored_version >= CURRENT_CONSENT_VERSION（含降级场景，不强制重新同意）
 * - `need_reconsent`：stored_version < CURRENT_CONSENT_VERSION（升级路径需用户确认）
 * - `not_granted`：从未同意
 */
export type ConsentStatus =
  | 'match'
  | 'not_granted'
  | { need_reconsent: { stored_version: number; current_version: number } }
