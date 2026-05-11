import { invoke } from './ipc'

/** 读 user_nickname；NULL 返回 null（调用方决定 UI 文案）。 */
export function getUserNickname(): Promise<string | null> {
  return invoke<string | null>('nickname_get_user')
}

/** 设 user_nickname。后端在转场开关 ON 时会向 active conversation 注入 system 转场消息。 */
export function setUserNickname(name: string): Promise<void> {
  return invoke<void>('nickname_set_user', { name })
}

/** 读"昵称变更时通知 AI"开关；缺省视为 true（默认 ON）。 */
export function getAnnounceUserChange(): Promise<boolean> {
  return invoke<boolean>('nickname_get_announce_user_change')
}

/** 写"昵称变更时通知 AI"开关。 */
export function setAnnounceUserChange(enabled: boolean): Promise<void> {
  return invoke<void>('nickname_set_announce_user_change', { enabled })
}
