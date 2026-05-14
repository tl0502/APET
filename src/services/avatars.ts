// 头像 IPC service（#25 用户上传 + #26 VRM 导出）。
// 落盘路径由后端返回；前端通过 memory_set 写到 KV，与现有偏好层统一。
// 路径变更后 emit 'avatar:changed' 让所有窗口（chat 头像 / MessageBubble）热刷新。
import { emit } from '@tauri-apps/api/event'
import { invoke } from './ipc'
import { deleteMemory, setMemory } from './memory'

/** KV key 约定（与后端 services/avatars.rs 注释保持一致）。 */
export const USER_AVATAR_KEY = 'user:avatar_path'
export const personaAvatarKey = (personaId: string) => `persona:${personaId}:avatar_path`

/** 全窗口头像热刷新 event；任何 set/clear 完成后由本 service 统一 emit。 */
export const AVATAR_CHANGED_EVENT = 'avatar:changed'

export interface AvatarChangedPayload {
  /** 哪个 KV 的头像变了；persona 时附带 personaId 让监听方判断是否影响自己。 */
  kind: 'user' | 'persona'
  /** 当 kind='persona' 时为该人格 id；'user' 时为 undefined。 */
  personaId?: string
  /** 新路径；null 表示已清除（fallback 到占位）。 */
  path: string | null
}

/** 复制本地 PNG/JPG 到 app data。后端做扩展名 + magic byte + size 校验。 */
async function setUserAvatarFile(srcPath: string): Promise<string> {
  return invoke<string>('user_avatar_set', { srcPath })
}

/** 删盘上 user.* 头像文件。返被删个数（一般 0 或 1）。 */
async function clearUserAvatarFile(): Promise<number> {
  return invoke<number>('user_avatar_clear')
}

/** 读源文件并返 `data:image/...;base64,...`，给 cropper 喂图（#25 裁剪流）。 */
export async function readImageToDataUrl(srcPath: string): Promise<string> {
  return invoke<string>('avatar_read_to_data_url', { srcPath })
}

/** 裁剪后 PNG dataURL 落盘 user.png（#25 裁剪流）。 */
async function saveUserAvatarFromDataUrl(dataUrl: string): Promise<string> {
  return invoke<string>('user_avatar_save_data_url', { dataUrl })
}

/** 把 data URL（PNG only）写到 <app_config>/avatars/persona-<id>.png。 */
async function savePersonaAvatarFile(personaId: string, dataUrl: string): Promise<string> {
  return invoke<string>('persona_avatar_save', { personaId, dataUrl })
}

/** 删 persona-<id>.png；不存在返 false。 */
async function clearPersonaAvatarFile(personaId: string): Promise<boolean> {
  return invoke<boolean>('persona_avatar_clear', { personaId })
}

/**
 * 用户头像（裁剪流）：crop 后 PNG dataURL → 落盘 → 写 KV → emit。
 * 任一步骤失败抛出，UI 层 toast 即可；落盘失败下 KV 不会被改写。
 */
export async function applyUserAvatarFromDataUrl(dataUrl: string): Promise<string> {
  const path = await saveUserAvatarFromDataUrl(dataUrl)
  await setMemory(USER_AVATAR_KEY, path)
  await emit(AVATAR_CHANGED_EVENT, { kind: 'user', path } satisfies AvatarChangedPayload)
  return path
}

/**
 * 用户头像（直接复制流）：不裁剪，源文件直接 copy 到 app data。
 * 保留作向后兼容路径；UI 主流量走 applyUserAvatarFromDataUrl + 裁剪 Modal。
 */
export async function applyUserAvatar(srcPath: string): Promise<string> {
  const path = await setUserAvatarFile(srcPath)
  await setMemory(USER_AVATAR_KEY, path)
  await emit(AVATAR_CHANGED_EVENT, { kind: 'user', path } satisfies AvatarChangedPayload)
  return path
}

/** 用户头像清空：删盘 → 删 KV → emit。 */
export async function removeUserAvatar(): Promise<void> {
  await clearUserAvatarFile()
  await deleteMemory(USER_AVATAR_KEY)
  await emit(AVATAR_CHANGED_EVENT, { kind: 'user', path: null } satisfies AvatarChangedPayload)
}

/** Persona 头像保存：data URL → 落盘 → 写 KV → emit。 */
export async function applyPersonaAvatar(personaId: string, dataUrl: string): Promise<string> {
  const path = await savePersonaAvatarFile(personaId, dataUrl)
  await setMemory(personaAvatarKey(personaId), path)
  await emit(AVATAR_CHANGED_EVENT, {
    kind: 'persona',
    personaId,
    path,
  } satisfies AvatarChangedPayload)
  return path
}

/** Persona 头像清空：删盘 → 删 KV → emit。 */
export async function removePersonaAvatar(personaId: string): Promise<void> {
  await clearPersonaAvatarFile(personaId)
  await deleteMemory(personaAvatarKey(personaId))
  await emit(AVATAR_CHANGED_EVENT, {
    kind: 'persona',
    personaId,
    path: null,
  } satisfies AvatarChangedPayload)
}
