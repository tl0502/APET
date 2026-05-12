// OnboardingService IPC binding（#21, ADR-019）。
//
// 3 个 commands：
// - saveOnboardingStep(step)：advanceStep 前写 KV `onboarding:current_step`
// - loadOnboardingStep()：启动 OnboardingApp.vue onMounted 读续接状态
// - resetOnboarding()：「重来」按钮调；clear KV，不动 consent.granted
//
// onboarding_complete 仍在 src/services/window.ts（核心动作是切窗）。本 service 只管进度。

import { invoke } from './ipc'

export function saveOnboardingStep(step: string): Promise<void> {
  return invoke<void>('onboarding_save_step', { step })
}

export function loadOnboardingStep(): Promise<string | null> {
  return invoke<string | null>('onboarding_load_step')
}

export function resetOnboarding(): Promise<void> {
  return invoke<void>('onboarding_reset')
}
