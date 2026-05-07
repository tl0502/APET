// LLMService IPC（#12，dev console 验证用；#13 ChatService MVP 上线后正式消费）。
//
// 6 个函数：
// - setOpenaiApiKey(key)：写 config KV（key=`llm:openai:api_key`，明文 M1，M3 G 迁 secrets+DPAPI）
// - getOpenaiApiKeySet()：返 boolean
// - setOpenaiConfig({ api_key?, base_url?, model? })：CUSTOM provider 用，partial update
//   三键（DeepSeek / Moonshot / Qwen / Ollama / 任意 OpenAI 兼容端点）
// - getOpenaiConfig()：返 { api_key_set, base_url, model }，缺省 fallback OpenAI 默认
// - chatSendTest(input)：单轮调 LLM，收完整字符串返；同时后端 emit `chat:test:delta`
// - cancelTest()：触发活跃 chat_send_test 的 CancellationToken
//
// dev console 验证（settings 窗口 / pet 窗口任一 DevTools，withGlobalTauri 已启）：
//
// ① OpenAI 直连：
//   await window.__TAURI__.core.invoke('set_openai_api_key', { key: 'sk-...' })
//
// ② CUSTOM provider（DeepSeek 示例）：
//   await window.__TAURI__.core.invoke('set_openai_config', {
//     config: { api_key: 'sk-...', base_url: 'https://api.deepseek.com', model: 'deepseek-chat' }
//   })
//   await window.__TAURI__.core.invoke('get_openai_config')
//
// ③ 流式可视化 + 完整结果：
//   const un = await window.__TAURI__.event.listen('chat:test:delta', e => console.log('[delta]', e.payload))
//   await window.__TAURI__.core.invoke('chat_send_test', { input: '你好' })
//   un()
//
// ④ 取消：
//   const p = window.__TAURI__.core.invoke('chat_send_test', { input: '写一首长诗' })
//   await new Promise(r => setTimeout(r, 200))
//   await window.__TAURI__.core.invoke('cancel_test')
//   try { await p } catch (e) { console.log(e.message ?? e) }   // "Cancelled: ..."

import { invoke } from './ipc'

export const CHAT_TEST_DELTA_EVENT = 'chat:test:delta'

export interface OpenaiConfigUpdate {
  api_key?: string
  base_url?: string
  model?: string
}

export interface OpenaiConfigSnapshot {
  api_key_set: boolean
  base_url: string
  model: string
}

export function setOpenaiApiKey(key: string): Promise<void> {
  return invoke<void>('set_openai_api_key', { key })
}

export function getOpenaiApiKeySet(): Promise<boolean> {
  return invoke<boolean>('get_openai_api_key_set')
}

export function setOpenaiConfig(config: OpenaiConfigUpdate): Promise<void> {
  return invoke<void>('set_openai_config', { config })
}

export function getOpenaiConfig(): Promise<OpenaiConfigSnapshot> {
  return invoke<OpenaiConfigSnapshot>('get_openai_config')
}

export function chatSendTest(input: string): Promise<string> {
  return invoke<string>('chat_send_test', { input })
}

export function cancelTest(): Promise<void> {
  return invoke<void>('cancel_test')
}
