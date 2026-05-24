// MoodService 前端绑定（#41，模块 I.2/I.3）— mood / energy IPC + disabled_features KV。
//
// ## 锁定项（PRD §7.9）
// - mood / energy 全 transient 不持久（line 1073/1089）：前端不缓存到 KV，仅 polling
// - disabled_features 是用户偏好，KV `pet:disabled_features` 跨重启
//
// ## IPC 与 Rust commands::mood / commands::energy 同 schema：
//   mood_get → { mood: 'neutral' | 'happy' | 'focused' | 'sleepy' | 'cozy' | 'annoyed' }
//   energy_get → { value: 0-100 }
//   mood_get_disabled_features → ['mood_icon' | 'energy' | 'free_movement', ...]
//   mood_set_disabled_features({ features: [...] })

import { invoke } from './ipc'

/** Rust Mood enum 序列化为 lowercase（commands::mood / services::mood）。 */
export type Mood = 'neutral' | 'happy' | 'focused' | 'sleepy' | 'cozy' | 'annoyed'

export interface MoodSnapshot {
  mood: Mood
}

export interface EnergySnapshot {
  value: number
}

/** disabled_features 三个有效 key（前端 UI toggle 渲染顺序）。 */
export const DISABLEABLE_FEATURES = ['mood_icon', 'energy', 'free_movement'] as const
export type DisableableFeature = (typeof DISABLEABLE_FEATURES)[number]

export async function getMood(): Promise<Mood> {
  try {
    const snapshot = await invoke<MoodSnapshot>('mood_get')
    return snapshot.mood
  } catch (e) {
    console.warn('[mood] get failed, returning neutral:', e)
    return 'neutral'
  }
}

export async function getEnergy(): Promise<number> {
  try {
    const snapshot = await invoke<EnergySnapshot>('energy_get')
    return snapshot.value
  } catch (e) {
    console.warn('[mood] energy_get failed, returning 80:', e)
    return 80
  }
}

export async function getDisabledFeatures(): Promise<DisableableFeature[]> {
  try {
    const raw = await invoke<string[]>('mood_get_disabled_features')
    return raw.filter((v): v is DisableableFeature =>
      (DISABLEABLE_FEATURES as readonly string[]).includes(v),
    )
  } catch (e) {
    console.warn('[mood] get_disabled_features failed, returning empty:', e)
    return []
  }
}

export async function setDisabledFeatures(features: DisableableFeature[]): Promise<void> {
  await invoke<void>('mood_set_disabled_features', { features })
}

/** Mood emoji 表（前端 MoodIcon 渲染用）。lowercase key 与 Rust serialize 对齐。 */
export const MOOD_EMOJI: Record<Mood, string> = {
  neutral: '',
  happy: '😺',
  focused: '🎯',
  sleepy: '😴',
  cozy: '🌙',
  annoyed: '😾',
}

/** Mood 中文 label（hover tooltip 用）。 */
export const MOOD_LABEL: Record<Mood, string> = {
  neutral: '平静',
  happy: '开心',
  focused: '专注中',
  sleepy: '困倦',
  cozy: '夜晚',
  annoyed: '不悦',
}
