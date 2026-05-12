<script setup lang="ts">
// OnboardingApp：onboarding 窗口的 root（#16 起，#21 step router 扩展）。
//
// 当前接入 step（按 flows §1.2 顺序）：
// - soul-pledge（#16 Step 1）：灵魂宣誓 + grant consent
// - persona-picker（#21 Step 2）：3 内置人格选择
// - shortcut-confirm（#21 Step 3）：chat 全局快捷键确认 + 占用探测
// - reminder-intents（#21 Step 4）：提醒模板多选,写 KV onboarding:reminder_intents
// - Step 5（番茄演示）：完全跳过（issue #21 拍板：M2 真番茄上线前不假演示）
// - summon-invite（#21 Step 6）：显示 chat 快捷键 + "开始陪伴" 按钮 → 调 finalize
// - completed：调 invoke('onboarding_complete') 切窗到 pet + 广播 onboarding:step-done
//
// 设计点：
// 1. 子 view 完成自己的业务（写 DB / activate persona / set shortcut / set KV）后 emit('done'),
//    由本 router 决定下一步
// 2. 切窗 IPC（onboarding_complete）集中在本 router 调，子 view 不感知"我是最后一步吗"
// 3. 进度持久化（KV `onboarding:current_step`）+ 续接 弹窗在 #21 后续 commit 加（ADR-019）
//
// dev mode（浏览器直访 onboarding.html）：onboarding_complete IPC 抛错 → console.warn 不阻断；
// 子 view 自身的 IPC 失败也由各自 toast 处理，本 router 只负责调度。
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useToast } from '@/composables/useToast'
import SoulPledgeView from './SoulPledgeView.vue'
import PersonaPickerView from './PersonaPickerView.vue'
import ShortcutConfirmView from './ShortcutConfirmView.vue'
import ReminderIntentsView from './ReminderIntentsView.vue'
import SummonInviteView from './SummonInviteView.vue'

type OnboardingStep =
  | 'soul-pledge'
  | 'persona-picker'
  | 'shortcut-confirm'
  | 'reminder-intents'
  | 'summon-invite'
  | 'completed'

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
      currentStep.value = 'reminder-intents'
      break
    case 'reminder-intents':
      // Step 5（番茄演示）跳过 → 直接到 Step 6
      currentStep.value = 'summon-invite'
      break
    case 'summon-invite':
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
      '准备就绪,但窗口切换失败,请重启应用。错误：' +
        (e instanceof Error ? e.message : String(e)),
      { duration: 8000 },
    )
    // 切回 summon-invite，让用户能再次点"开始陪伴"重试 onboarding_complete
    currentStep.value = 'summon-invite'
    finalizing.value = false
  }
}
</script>

<template>
  <SoulPledgeView v-if="currentStep === 'soul-pledge'" @done="advanceStep" />
  <PersonaPickerView v-else-if="currentStep === 'persona-picker'" @done="advanceStep" />
  <ShortcutConfirmView v-else-if="currentStep === 'shortcut-confirm'" @done="advanceStep" />
  <ReminderIntentsView v-else-if="currentStep === 'reminder-intents'" @done="advanceStep" />
  <SummonInviteView v-else-if="currentStep === 'summon-invite'" @done="advanceStep" />
</template>

<style scoped></style>
