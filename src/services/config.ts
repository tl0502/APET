// `config` 表 KV IPC wrapper（#30 磁吸 + 未来 settings 表 fallback 复用）。
//
// 与 `memory` 表区分（services/memory.ts）：
// - memory 表：用户偏好（"你叫什么名字" 等 LLM 推断）
// - config 表：运行时配置（窗口位置 / active_conversation_id / snap:constraints 等）
//
// 详 src-tauri/src/services/config.rs 头注释。

import { invoke } from './ipc'

/** 读 config 表 KV；不存在返 null。 */
export function getConfig(key: string): Promise<string | null> {
  return invoke<string | null>('config_get', { key })
}

/** UPSERT config 表 KV。 */
export function setConfig(key: string, value: string): Promise<void> {
  return invoke<void>('config_set', { key, value })
}

/** 删 config 表 KV；不存在 = no-op。 */
export function deleteConfig(key: string): Promise<void> {
  return invoke<void>('config_delete', { key })
}

/** B3 修复：原子 persist snap:constraints + 跨 webview 广播 snap:constraint-changed。
 *  Rust 端先 await config_set 完成（KV 写盘），再 emit 广播——保证抵达任一 webview 时 KV 已是新值。
 *  senderId 透传给前端 listener 自过滤（A4 修复）。
 *
 *  传入的 value 是序列化好的 JSON 字符串（与 setConfig('snap:constraints', value) 等价）。 */
export function snapPersistAndBroadcast(value: string, senderId: string): Promise<void> {
  return invoke<void>('snap_persist_and_broadcast', { value, senderId })
}
