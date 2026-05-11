/** LLM Providers IPC 类型（与 src-tauri/src/services/llm_providers.rs 对齐）。 *
 * Rust 端通过 `#[serde(rename_all = "camelCase")]` 把 snake_case 字段重命名为 camelCase；
 * 因此 TS 端字段全 camelCase（与 services/chat.ts SendResult 同款约定）。 */

/** 列表项（不含 api_key 明文）— 设置面板列表用。 */
export interface ProviderListItem {
  id: string
  name: string
  baseUrl: string
  model: string
  /** 是否已设置 api_key（永远不返明文）。 */
  hasApiKey: boolean
  isActive: boolean
}

/** 详情项（含 api_key 明文）— drawer 编辑模式回填用；调用方应避免持久化到长生命周期 state。 */
export interface ProviderDetail {
  id: string
  name: string
  apiKey: string
  baseUrl: string
  model: string
  isActive: boolean
}

/** add_provider 入参。 */
export interface AddProviderRequest {
  name: string
  apiKey: string
  baseUrl: string
  model: string
}

/** update_provider 部分更新入参；undefined 字段后端不动。 */
export interface UpdateProviderRequest {
  name?: string
  apiKey?: string
  baseUrl?: string
  model?: string
}
