import { invoke } from './ipc'

export const SAFETY_SCOPES = [
  'prefixInjection',
  'userInput',
  'streamToken',
  'finalOutput',
] as const

export type SafetyScope = (typeof SAFETY_SCOPES)[number]

export type SafetyPolicySnapshot = Record<SafetyScope, boolean>

export function getSafetyPolicy(): Promise<SafetyPolicySnapshot> {
  return invoke<SafetyPolicySnapshot>('safety_policy_get')
}

export function setSafetyScope(scope: SafetyScope, enabled: boolean): Promise<void> {
  return invoke<void>('safety_policy_set', { scope, enabled })
}
