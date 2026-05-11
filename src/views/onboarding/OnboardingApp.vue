<script setup lang="ts">
// OnboardingApp：onboarding 窗口的 root（#16 起，#21 step router 扩展）。
//
// 当前接入 step（按 flows §1.2 顺序）：
// - soul-pledge（#16 Step 1）：灵魂宣誓 + grant consent
// - persona-picker（#21 Step 2）：3 内置人格选择
// - shortcut-confirm（#21 Step 3）：chat 全局快捷键确认 + 占用探测
// - completed：调 invoke('onboarding_complete') 切窗到 pet + 广播 onboarding:step-done
//
// 设计点：
// 1. 子 view 完成自己的业务（写 DB / activate persona / set shortcut）后 emit('done'),
//    由本 router 决定下一步
// 2. 切窗 IPC（onboarding_complete）集中在本 router 调，子 view 不感知"我是最后一步吗"
// 3. 未来 Step 4-6 接入时：扩 OnboardingStep 联合类型 + 在 advanceStep 加分支即可
// 4. 进度持久化（KV `onboarding:current_step`）+ 续接 弹窗在 #21 后续 commit 加（ADR-019）
//
// dev mode（浏览器直访 onboarding.html）：onboarding_complete IPC 抛错 → console.warn 不阻断；
// 子 view 自身的 IPC 失败也由各自 toast 处理，本 router 只负责调度。
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useToast } from '@/composables/useToast'
import SoulPledgeView from './SoulPledgeView.vue'
import PersonaPickerView from './PersonaPickerView.vue'
import ShortcutConfirmView from './ShortcutConfirmView.vue'

type OnboardingStep = 'soul-pledge' | 'persona-picker' | 'shortcut-confirm' | 'completed'

const toast = useToast()
const currentStep = ref<OnboardingStep>('soul-pledge')
const finalizing = ref(false)

async function advanceStep() {
  switch (currentStep.value) {
    case 'soul-pledge':
      currentStep.value = 'persona-picker'
      break
    case 'persona-picker':
      currentStep.value = 'shortcut-confirm'
      break
    case 'shortcut-confirm':
      // M1 阶段 Step 3 是最后一个已实现 step；Step 4-6 接入后挪到对应 step done
      currentStep.value = 'completed'
      await finalizeOnboarding()
      break
    case 'completed':
      // 防御：completed 状态被重复触发（理论不该发生）
      console.warn('[OnboardingApp] advanceStep called in completed state')
      break
  }
}

async function finalizeOnboarding() {
  if (finalizing.value) return
  finalizing.value = true
  try {
    // onboarding_complete：后端 hide onboarding + show pet + emit 'onboarding:step-done'
    await invoke('onboarding_complete')
    // 成功后本窗口被 hide；视图不会再被看到
  } catch (e) {
    console.error('[OnboardingApp] onboarding_complete failed:', e)
    toast.warn(
      '快捷键已保存,但窗口切换失败,请重启应用。错误：' +
        (e instanceof Error ? e.message : String(e)),
      { duration: 8000 },
    )
    // 切回上一 step（shortcut-confirm），让用户能再次触发"用这个"重试 onboarding_complete
    currentStep.value = 'shortcut-confirm'
    finalizing.value = false
  }
}
</script>

<template>
  <SoulPledgeView v-if="currentStep === 'soul-pledge'" @done="advanceStep" />
  <PersonaPickerView v-else-if="currentStep === 'persona-picker'" @done="advanceStep" />
  <ShortcutConfirmView v-else-if="currentStep === 'shortcut-confirm'" @done="advanceStep" />
</template>

<style scoped></style>
