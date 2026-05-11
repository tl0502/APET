// LLMProviders IPC binding（用户增补，参考 cc-switch UI；与 #12 单 namespace 路径冲突以本设计为准）。
//
// 7 个 IPC：
// - listProviders(): 列表（不含 api_key 明文）
// - getProvider(id): 单个详情（含 api_key，给 drawer 编辑模式回填用）
// - addProvider({name, apiKey, baseUrl, model}): 新建，返新 ULID；首条自动 active
// - updateProvider(id, partial): 部分更新（undefined 字段后端不动）
// - deleteProvider(id): 删除；激活的不允许删（后端 CannotDeleteActive）
// - activateProvider(id): 设当前激活
// - testProvider(id): 用对应配置发"你好"测试连通；不影响 active；返完整回复
//   错误形如 "AuthFailed: ..." → 前端 split(':', 2)[0] 拿 kind 做 UI 分支
//
// dev console 验证：
//   await window.__TAURI__.core.invoke('llm_list_providers')
//   const id = await window.__TAURI__.core.invoke('llm_add_provider', {
//     req: { name: 'OpenAI', apiKey: 'sk-...', baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini' }
//   })
//   await window.__TAURI__.core.invoke('llm_test_provider', { id })

import { invoke } from './ipc'
import type {
  AddProviderRequest,
  ProviderDetail,
  ProviderListItem,
  UpdateProviderRequest,
} from '@/types/llm_providers'

/** 与 src-tauri/src/commands/llm_providers.rs::LLM_TEST_DELTA_EVENT 对齐；可选订阅看流式。 */
export const LLM_TEST_DELTA_EVENT = 'llm:test:delta'

export function listProviders(): Promise<ProviderListItem[]> {
  return invoke<ProviderListItem[]>('llm_list_providers')
}

export function getProvider(id: string): Promise<ProviderDetail> {
  return invoke<ProviderDetail>('llm_get_provider', { id })
}

export function addProvider(req: AddProviderRequest): Promise<string> {
  return invoke<string>('llm_add_provider', { req })
}

export function updateProvider(id: string, req: UpdateProviderRequest): Promise<void> {
  return invoke<void>('llm_update_provider', { id, req })
}

export function deleteProvider(id: string): Promise<void> {
  return invoke<void>('llm_delete_provider', { id })
}

export function activateProvider(id: string): Promise<void> {
  return invoke<void>('llm_activate_provider', { id })
}

/** 测试连通；返 LLM 完整回复字符串（前端取前 40 字 toast preview）。 */
export function testProvider(id: string): Promise<string> {
  return invoke<string>('llm_test_provider', { id })
}

/**
 * 探测 OpenAI 兼容 provider 的 /models 端点；返模型 id 列表。
 *
 * 不依赖已保存的 provider —— ProviderDrawer 创建模式首次填完 baseUrl + apiKey 即可调用。
 * 错误格式与 testProvider 一致："AuthFailed: ..." / "Network: ..." 等；前端 split kind 做 UI 分支。
 */
export function probeModels(baseUrl: string, apiKey: string): Promise<string[]> {
  return invoke<string[]>('llm_probe_models', { baseUrl, apiKey })
}
