import { invoke } from './ipc'

/** 读 pet_nickname；NULL 时 fallback 到 active persona.name，再无则兜底 '默默'。 */
export function getPetNickname(): Promise<string> {
  return invoke<string>('nickname_get_pet')
}

/** 读 user_nickname；NULL 返回 null（调用方决定 UI 文案）。 */
export function getUserNickname(): Promise<string | null> {
  return invoke<string | null>('nickname_get_user')
}

/** 设 pet_nickname；自动把当前值搬到 pet_nickname_previous。 */
export function setPetNickname(name: string): Promise<void> {
  return invoke<void>('nickname_set_pet', { name })
}

/** 设 user_nickname。 */
export function setUserNickname(name: string): Promise<void> {
  return invoke<void>('nickname_set_user', { name })
}

/** pet_nickname 与 pet_nickname_previous 原子 swap；previous 为 NULL 时报错。 */
export function restorePetNickname(): Promise<string | null> {
  return invoke<string | null>('nickname_restore_pet')
}
