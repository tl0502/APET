<script setup lang="ts">
// OnboardingApp：onboarding 窗口的 root（#16 起，#21 step router 扩展，ADR-019 续接）。
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
// 3. ADR-019 进度持久化：每次 advance 前 saveStep KV `onboarding:current_step`；
//    onMounted 时检测 KV 存在 → 弹"继续 / 重来 / 退出"模态。
//
// dev mode（浏览器直访 onboarding.html）：所有 IPC 抛错 → console.warn 不阻断；
// 子 view 自身的 IPC 失败也由各自 toast 处理，本 router 只负责调度。

import { onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ElButton } from 'element-plus'
import { useToast } from '@/composables/useToast'
import { loadOnboardingStep, saveOnboardingStep } from '@/services/onboarding'
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

/** 续接模态展示用 step 标题（不包括 'completed' / 'soul-pledge'，后者直接 NotGranted 走重头）。 */
const STEP_DISPLAY_NAMES: Record<OnboardingStep, string> = {
  'soul-pledge': '灵魂宣誓',
  'persona-picker': '选择人格',
  'shortcut-confirm': '确认快捷键',
  'reminder-intents': '提醒偏好',
  'summon-invite': '准备完成',
  completed: '完成',
}

const RESUMABLE_STEPS: ReadonlySet<OnboardingStep> = new Set([
  'persona-picker',
  'shortcut-confirm',
  'reminder-intents',
  'summon-invite',
])

const toast = useToast()
const currentStep = ref<OnboardingStep>('soul-pledge')
const finalizing = ref(false)
/** 启动期 load KV 中；期间不渲染任何 view,避免 SoulPledgeView 闪现然后切到续接模态。 */
const initializing = ref(true)
/** 非 null = 显示续接模态；null = 正常 view 流程。 */
const resumePrompt = ref<{ step: OnboardingStep } | null>(null)

onMounted(async () => {
  try {
    const saved = await loadOnboardingStep()
    if (saved && isResumableStep(saved)) {
      resumePrompt.value = { step: saved }
    }
    // else：KV 不存在（首次 / 已完成 / 已重置）或 saved='soul-pledge'（语义同首次,走重头）
    //   → currentStep 保持默认 'soul-pledge'
  } catch (e) {
    console.warn('[OnboardingApp] loadOnboardingStep failed, fallback to fresh start:', e)
  } finally {
    initializing.value = false
  }
})

function isResumableStep(s: string): s is OnboardingStep {
  return RESUMABLE_STEPS.has(s as OnboardingStep)
}

async function advanceStep() {
  const next = nextStepOf(currentStep.value)
  if (!next) return
  // KV 写"下一步要恢复到哪";completed 不写（onboarding_complete 会 clear KV）
  if (next !== 'completed') {
    try {
      await saveOnboardingStep(next)
    } catch (e) {
      console.warn('[OnboardingApp] saveOnboardingStep failed (non-fatal):', e)
    }
  }
  if (next === 'completed') {
    currentStep.value = 'completed'
    await finalizeOnboarding()
  } else {
    currentStep.value = next
  }
}

function nextStepOf(s: OnboardingStep): OnboardingStep | null {
  switch (s) {
    case 'soul-pledge':
      return 'persona-picker'
    case 'persona-picker':
      return 'shortcut-confirm'
    case 'shortcut-confirm':
      return 'reminder-intents'
    case 'reminder-intents':
      // Step 5（番茄演示）跳过 → 直接到 Step 6
      return 'summon-invite'
    case 'summon-invite':
      return 'completed'
    case 'completed':
      console.warn('[OnboardingApp] advanceStep called in completed state')
      return null
  }
}

async function finalizeOnboarding() {
  if (finalizing.value) return
  finalizing.value = true
  try {
    // onboarding_complete：后端 clear KV + hide onboarding + show pet + emit 'onboarding:step-done'
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

// ───── 续接模态 ─────

function onResumeContinue() {
  if (!resumePrompt.value) return
  currentStep.value = resumePrompt.value.step
  resumePrompt.value = null
}

async function onResumeRestart() {
  // 写 KV='soul-pledge';不动 consent.granted（ADR-019：合规标记不被 UX 流程 reset）。
  //
  // 为什么写 KV 而非 clear KV：原实现 clear KV 后,如果用户在 SoulPledge 关窗,
  // 下次启动 consent.granted=true + KV 不存在 → 启动期错认为"已完成 onboarding"
  // 直接进 pet 主态 → SoulPledge 被跳过(实测 bug)。
  // 写 KV='soul-pledge' 后,启动期看 KV 存在 → 仍开 onboarding;前端 onMounted
  // 读到 'soul-pledge'(不在 RESUMABLE_STEPS)→ 不弹模态,正常显示 Step 1 重头。
  try {
    await saveOnboardingStep('soul-pledge')
  } catch (e) {
    console.warn('[OnboardingApp] saveOnboardingStep on restart failed (non-fatal):', e)
  }
  currentStep.value = 'soul-pledge'
  resumePrompt.value = null
}

async function onResumeExit() {
  // onboarding 窗口的 close 事件已被 lib.rs::on_window_event 绑定到 app.exit(0)（#16）
  // 所以 getCurrentWindow().close() = 进程退出。dev 模式（浏览器直访）下走 window.close()
  // 等价路径或抛错（前端兜底打 log 即可）。
  try {
    await getCurrentWindow().close()
  } catch (e) {
    console.warn('[OnboardingApp] window.close failed:', e)
  }
}
</script>

<template>
  <!-- 续接模态：onMounted 检测到 KV 存在 → 显示 -->
  <div
    v-if="resumePrompt"
    class="resume-overlay"
    role="dialog"
    aria-modal="true"
    aria-labelledby="resume-title"
  >
    <div class="resume-card">
      <h2 id="resume-title" class="resume-card__title">欢迎回来 👋</h2>
      <p class="resume-card__hint">
        上次我们在
        <strong>{{ STEP_DISPLAY_NAMES[resumePrompt.step] }}</strong>
        这步停下了。
      </p>
      <p class="resume-card__hint resume-card__hint--small">
        想接着走完，还是从头开始?
      </p>
      <div class="resume-card__actions">
        <ElButton type="primary" @click="onResumeContinue">继续</ElButton>
        <ElButton @click="onResumeRestart">重来</ElButton>
        <ElButton text @click="onResumeExit">退出</ElButton>
      </div>
    </div>
  </div>
  <!-- 启动期等 KV load 完;期间不渲染 view,避免短暂闪现 -->
  <template v-else-if="!initializing">
    <SoulPledgeView v-if="currentStep === 'soul-pledge'" @done="advanceStep" />
    <PersonaPickerView v-else-if="currentStep === 'persona-picker'" @done="advanceStep" />
    <ShortcutConfirmView v-else-if="currentStep === 'shortcut-confirm'" @done="advanceStep" />
    <ReminderIntentsView v-else-if="currentStep === 'reminder-intents'" @done="advanceStep" />
    <SummonInviteView v-else-if="currentStep === 'summon-invite'" @done="advanceStep" />
  </template>
</template>

<style scoped>
.resume-overlay {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  padding: var(--aipet-space-6);
  background: var(--aipet-color-bg);
  box-sizing: border-box;
  user-select: none;
}

.resume-card {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  width: 100%;
  max-width: 360px;
  padding: var(--aipet-space-6) var(--aipet-space-8);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-lg);
  background: var(--aipet-color-surface);
  box-shadow: var(--aipet-shadow-lg);
}

.resume-card__title {
  margin: 0 0 var(--aipet-space-3);
  font-size: var(--aipet-font-size-2xl);
  font-weight: 600;
  color: var(--aipet-color-text-1);
  text-align: center;
  line-height: var(--aipet-line-height-display);
  letter-spacing: -0.01em;
}

.resume-card__hint {
  margin: 0 0 var(--aipet-space-2);
  font-size: var(--aipet-font-size-base);
  color: var(--aipet-color-text-2);
  text-align: center;
  line-height: 1.5;
}

.resume-card__hint--small {
  margin-bottom: var(--aipet-space-5);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
}

.resume-card__actions {
  display: flex;
  justify-content: center;
  gap: var(--aipet-space-3);
}
</style>
