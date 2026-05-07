// LLMService IPC（#12，dev console 验证用；#13 ChatService MVP 上线后正式消费）。
//
// 4 个函数对应 issue #12 验收标准：
// - setOpenaiApiKey(key)：写 config 表 KV（key=`llm:openai:api_key`，明文 M1，M3 G 迁 secrets+DPAPI）
// - getOpenaiApiKeySet()：返 boolean，永不返明文（dev DevTools 也不能拿）
// - chatSendTest(input)：单轮调 LLM，收完整字符串返；同时后端 emit `chat:test:delta` 给可视化
// - cancelTest()：触发活跃 chat_send_test 的 CancellationToken
//
// dev console 验证流程（settings 窗口 / pet 窗口任一 DevTools，withGlobalTauri 已启）：
//   await window.__TAURI__.core.invoke('set_openai_api_key', { key: 'sk-...' })
//   const un = await window.__TAURI__.event.listen('chat:test:delta', e => console.log(e.payload))
//   await window.__TAURI__.core.invoke('chat_send_test', { input: '你好' })
//   un()
//
// 测试 cancel：另开一个 DevTools tab（或在 chat_send_test 还在 await 时）：
//   await window.__TAURI__.core.invoke('cancel_test')
//
// 测试 AuthFailed：set 一个错的 key 再 chat_send_test，错误字符串前缀 "AuthFailed: ..."。
// 测试 Network：关网卡再 chat_send_test，错误字符串前缀 "Network: ..."。

import { invoke } from './ipc'

export const CHAT_TEST_DELTA_EVENT = 'chat:test:delta'

export function setOpenaiApiKey(key: string): Promise<void> {
  return invoke<void>('set_openai_api_key', { key })
}

export function getOpenaiApiKeySet(): Promise<boolean> {
  return invoke<boolean>('get_openai_api_key_set')
}

export function chatSendTest(input: string): Promise<string> {
  return invoke<string>('chat_send_test', { input })
}

export function cancelTest(): Promise<void> {
  return invoke<void>('cancel_test')
}
