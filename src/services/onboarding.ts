// OnboardingService IPC binding（#21, ADR-019）。
//
// 2 个 commands：
// - saveOnboardingStep(step)：advanceStep 前写 KV `onboarding:current_step`
// - loadOnboardingStep()：启动 OnboardingApp.vue onMounted 读续接状态
//
// 「重来」按钮改用 saveOnboardingStep('soul-pledge')（写而非清 KV），原 resetOnboarding
// IPC 已删:实测发现清 KV 后 consent.granted=true 状态会让启动期错跳过 SoulPledge。
// 详 ADR-019 Updated 2026-05-12 + OnboardingApp.vue::onResumeRestart 注释。
//
// onboarding_complete 仍在 src/services/window.ts（核心动作是切窗）。本 service 只管进度。

import { invoke } from './ipc'

export function saveOnboardingStep(step: string): Promise<void> {
  return invoke<void>('onboarding_save_step', { step })
}

export function loadOnboardingStep(): Promise<string | null> {
  return invoke<string | null>('onboarding_load_step')
}
