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
