// userProfile service（#37 2026-05-21 重设计）— Profile 字段（bio / 个性资料）的 KV 读写。
//
// 走通用 memory KV（services/memory.ts），不新建 Tauri command。
// avatar_path / nickname 走各自专用 service；本文件只管 bio。
//
// KV key 约定：
// - user:bio — 个性资料文本（一段话，<= 200 字符前端校验）

import { getMemory, setMemory } from './memory'

const USER_BIO_KEY = 'user:bio'

/** 读用户个性资料；不存在返 null。 */
export function getUserBio(): Promise<string | null> {
  return getMemory(USER_BIO_KEY)
}

/** 写入用户个性资料；空字符串与正常字符串都允许（用户清空意图）。 */
export function setUserBio(value: string): Promise<void> {
  return setMemory(USER_BIO_KEY, value)
}
