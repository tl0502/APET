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

// chrome bar ✕ 的关闭路径与 onResumeExit 完全一致（lib.rs:320 onboarding CloseRequested
// → app.exit(0)）。独立函数让模板/调用站点的语义更明确（用户主动放弃 vs 续接拒绝）。
async function onChromeClose() {
  try {
    await getCurrentWindow().close()
  } catch (e) {
    console.warn('[OnboardingApp] chrome close failed:', e)
  }
}
</script>

<template>
  <div class="onboarding-root">
    <!-- 自绘 chrome bar（#16 美化补丁 2026-05-22）：
         tauri.conf.json decorations:false → OS 标题栏关闭,本 bar 承担拖动 + 关闭。
         关闭 ✕ 调 getCurrentWindow().close() → lib.rs:320 拦截 onboarding CloseRequested
         → app.exit(0)（与原 OS X 按钮路径等价）。
         WorkspaceApp 同款 chrome 风格,但 onboarding 只需关闭按钮（resizable:false 无 min/max）。

         title span 显式重复 data-tauri-drag-region 作为防御:某些 Vue render
         边界下 attribute 继承在 webview 实测偶发失效（child 接收 mousedown 时
         查不到 drag attribute → startDragging 不触发）。冗余声明,无副作用。 -->
    <header class="onboarding-chrome" data-tauri-drag-region>
      <span
        class="onboarding-chrome__title"
        data-tauri-drag-region
      >灵魂宣誓 — AI 桌宠</span>
      <button
        class="aipet-chrome-btn aipet-chrome-btn--close"
        type="button"
        data-tauri-drag-region="false"
        title="关闭（退出应用）"
        aria-label="关闭"
        @click="onChromeClose"
      >✕</button>
    </header>

    <div class="onboarding-body">
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
    </div>
  </div>
</template>

<style scoped>
/* ============ root：chrome bar + body 垂直分层 ============
 * tauri.conf decorations:false 后窗体由前端自绘；onboarding-root 占满 webview。
 */
.onboarding-root {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background: var(--aipet-color-bg);
}

/* ============ chrome bar（自绘 40px 顶栏） ============
 * 整 header 设 data-tauri-drag-region；按钮 data-tauri-drag-region="false" 豁免。
 * Win11 风格 chrome 按钮复用全局 .aipet-chrome-btn（buttons.css）46×32，hover 红。
 * 1px hairline 底分 chrome 与 body；clean 桌面应用范式（WorkspaceApp 同款节奏）。
 */
.onboarding-chrome {
  flex: 0 0 40px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  background: var(--aipet-color-surface-soft);
  border-bottom: 1px solid var(--aipet-color-border-faint);
  user-select: none;
  cursor: default;
  z-index: 5;
}

.onboarding-chrome__title {
  padding-left: var(--aipet-space-4);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
  letter-spacing: 0.02em;
  /* 中间空白区域明确 drag 提示;按钮区不继承（aipet-chrome-btn 自身 cursor pointer） */
  cursor: move;
  flex: 1 1 auto;
}

/* ============ body：剩余空间承载 view ============
 * min-height: 0 是 flex column 内子项可缩容的标准技巧——避免 letter 卡片或大段
 * 内容撑爆 flex 容器把后续兄弟元素挤出可视区（lessons.md "grid item min-size"
 * 在 flex 上的等价问题）。
 */
.onboarding-body {
  flex: 1 1 auto;
  min-height: 0;
  position: relative;
}

/* ============ 续接模态：占满 body 居中 ============ */
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
