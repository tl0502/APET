// ConsentService IPC binding（issue #16，ADR-008 灵魂宣誓 v1.0）。
//
// 4 个 commands：
// - getConsent(): 读单行 consent 当前状态
// - grantConsent(method, version): "我懂了"路径写入；version 必须先调
//     getCurrentVersion 拿（避免前端硬编码 1 在 v2 上线时 stale）
// - checkVersion(): 启动期路由判定（Match | NeedReconsent | NotGranted）
// - getCurrentVersion(): 拿后端 CURRENT_CONSENT_VERSION 常量
//
// 视图层 SoulPledgeView 留 #16b（与 #17 Onboarding 状态机配合）。
//
// dev console 验证：
//   await window.__TAURI__.core.invoke('consent_get')
//   await window.__TAURI__.core.invoke('consent_check_version')
//   const v = await window.__TAURI__.core.invoke('consent_get_current_version')
//   await window.__TAURI__.core.invoke('consent_grant', { method: 'soul_pledge', version: v })

import { invoke } from './ipc'
import type { ConsentMethod, ConsentRecord, ConsentStatus } from '@/types/consent'

export function getConsent(): Promise<ConsentRecord> {
  return invoke<ConsentRecord>('consent_get')
}

export function grantConsent(method: ConsentMethod, version: number): Promise<void> {
  return invoke<void>('consent_grant', { method, version })
}

export function checkConsentVersion(): Promise<ConsentStatus> {
  return invoke<ConsentStatus>('consent_check_version')
}

export function getCurrentConsentVersion(): Promise<number> {
  return invoke<number>('consent_get_current_version')
}

/**
 * C6 修复：先 fetch 后端当前版本再 grant，避免前端硬编码 1 在 v2 上线时 stale 被后端拒。
 * 视图层（#16b SoulPledgeView）应优先使用本函数；只有对版本号有完全控制的场景才用 raw `grantConsent`。
 */
export async function grantConsentSafe(method: ConsentMethod): Promise<void> {
  const version = await getCurrentConsentVersion()
  await grantConsent(method, version)
}
