// usePetInteractionFeedback：消费 #40 InteractionRouter emit 的物理交互反馈。
//
// 范围（ADR-025 2a-lite 最少可见反馈，M2 KPI = AABB 触发率 ≥ 95%）：
// - pet:interaction_reacted：根据 action_id 触发 VRM 动作（'nod' 走真动效，其他 placeholder
//   走 shake 视觉补偿）+ template 气泡 + mood icon 闪烁
// - pet:protest_triggered：shake + mood='annoyed' + 气泡（强反馈）
// - pet:protest_reverted（5s 后由 Rust 自动 emit）：mood icon 清回 neutral
//
// 4 项最少可见反馈（issue 验收 ≥1 项消费即过）：
//   ✓ shake（CSS keyframe class，200ms）
//   ✓ nod（runtime.playAction('nod')）
//   ✓ mood icon 切换（mood ref → 父组件渲染 happy/annoyed/calm 图标 1-1.5s 闪烁）
//   ✓ 气泡反馈（template → bubble ref → 父组件渲染浮层文字 2s）

import { onBeforeUnmount, onMounted, ref, type Ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  INTERACTION_REACTED_EVENT,
  PROTEST_REVERTED_EVENT,
  PROTEST_TRIGGERED_EVENT,
  type InteractionReactedPayload,
  type ProtestPayload,
  type ProtestRevertPayload,
} from '@/services/interaction'
import type { PetActionId, VRMRuntime } from '@/services/vrm'

/** mood icon transient state；'neutral' 时不渲染图标。 */
export type FeedbackMood = 'neutral' | 'happy' | 'annoyed' | 'calm'

const MOOD_FLASH_MS = 1500
const BUBBLE_DURATION_MS = 2000
const SHAKE_DURATION_MS = 220

export interface UsePetInteractionFeedbackReturn {
  /** 当前 transient mood（5s 抗议态 / 1.5s 普通反馈）；父组件渲染图标。 */
  mood: Ref<FeedbackMood>
  /** 临时气泡文案（来自 reaction_table.template）；空字符串 = 不显示。 */
  bubble: Ref<string>
  /** 触发 .pet-stage shake CSS keyframe；父组件 class binding。 */
  shaking: Ref<boolean>
}

/**
 * 监听 InteractionRouter 3 个 emit 事件，把视觉反馈解耦到 ref，父组件按需消费。
 *
 * 调用方在 setup 阶段拿 runtime（VRMRuntime 实例）+ enabled fn；onboarding 等场景应传 false。
 */
export function usePetInteractionFeedback(
  runtime: VRMRuntime,
  isEnabled: () => boolean = () => true,
): UsePetInteractionFeedbackReturn {
  const mood = ref<FeedbackMood>('neutral')
  const bubble = ref<string>('')
  const shaking = ref<boolean>(false)

  let unlistenReacted: UnlistenFn | null = null
  let unlistenProtest: UnlistenFn | null = null
  let unlistenReverted: UnlistenFn | null = null

  let moodTimer: number | null = null
  let bubbleTimer: number | null = null
  let shakeTimer: number | null = null
  /** 抗议态 = true 时，普通 reaction 不覆盖 mood（5s 锁定 annoyed）。 */
  let protestActive = false

  function flashMood(next: FeedbackMood, duration = MOOD_FLASH_MS) {
    if (moodTimer !== null) window.clearTimeout(moodTimer)
    mood.value = next
    if (next === 'neutral') {
      moodTimer = null
      return
    }
    moodTimer = window.setTimeout(() => {
      moodTimer = null
      mood.value = 'neutral'
    }, duration) as unknown as number
  }

  function flashBubble(text: string) {
    if (bubbleTimer !== null) window.clearTimeout(bubbleTimer)
    bubble.value = text
    bubbleTimer = window.setTimeout(() => {
      bubbleTimer = null
      bubble.value = ''
    }, BUBBLE_DURATION_MS) as unknown as number
  }

  function triggerShake() {
    if (shakeTimer !== null) {
      window.clearTimeout(shakeTimer)
      shaking.value = false
      // 短暂中断让 CSS keyframe 重置；下一 frame 再开（避免连续 shake 看起来卡）
      void requestAnimationFrame(() => {
        shaking.value = true
        shakeTimer = window.setTimeout(() => {
          shakeTimer = null
          shaking.value = false
        }, SHAKE_DURATION_MS) as unknown as number
      })
      return
    }
    shaking.value = true
    shakeTimer = window.setTimeout(() => {
      shakeTimer = null
      shaking.value = false
    }, SHAKE_DURATION_MS) as unknown as number
  }

  function playVrmAction(actionId: string) {
    // 仅 nod 在 VRMRuntime 内有真实现（#29）；其他 actionId 走 dev warn no-op
    // —— 由 shake / mood / bubble 三件套兜底视觉反馈（ADR-025 2a-lite）。
    runtime
      .playAction(actionId as PetActionId)
      .catch((e) => console.warn('[interaction-feedback] playAction failed:', e))
  }

  function handleReacted(payload: InteractionReactedPayload) {
    if (!isEnabled()) return
    // VRM 动作：nod 真动效，其他 actionId 由 VRMRuntime placeholder 警告（不抛错）。
    playVrmAction(payload.actionId)

    // 视觉补偿：除了 nod / fall_asleep 外，所有 action 都跟一次 shake 让用户感知"触发了"。
    // nod 自带视觉幅度；fall_asleep 是"懒动"，不该 shake。
    if (payload.actionId !== 'nod' && payload.actionId !== 'fall_asleep') {
      triggerShake()
    }

    // mood icon：抗议态 5s 内不被普通 reaction 抢走；其他态按 mood_change flash。
    if (!protestActive && payload.moodChange) {
      const next = payload.moodChange as FeedbackMood
      if (next === 'happy' || next === 'annoyed' || next === 'calm') {
        flashMood(next)
      }
    }

    // 气泡文案：template 非空时 flash 2s。
    if (payload.template) {
      flashBubble(payload.template)
    }
  }

  function handleProtest(payload: ProtestPayload) {
    if (!isEnabled()) return
    protestActive = true
    triggerShake()
    flashMood('annoyed', payload.revertAfterMs)
    if (payload.template) {
      flashBubble(payload.template)
    }
    // playAction('protest') 是 placeholder（#23 接 reaction_table 时填）；
    // 不调用以免每次 dev 期 console.warn 刷屏。protest 视觉由 shake + mood + bubble 三件套承载。
  }

  function handleProtestReverted(_payload: ProtestRevertPayload) {
    if (!isEnabled()) return
    protestActive = false
    // mood 已被 flashMood 内部计时器在 5s 后清回 neutral；这里仅清 protestActive 锁。
    // 若用户在 5s 内又触发了 happy 等普通 reaction —— 由本路径解锁后下次 reaction 才生效（合理）。
  }

  onMounted(async () => {
    if (!isEnabled()) return
    try {
      unlistenReacted = await listen<InteractionReactedPayload>(
        INTERACTION_REACTED_EVENT,
        (e) => {
          if (e.payload) handleReacted(e.payload)
        },
      )
    } catch (e) {
      console.warn('[interaction-feedback] listen reacted failed:', e)
    }
    try {
      unlistenProtest = await listen<ProtestPayload>(
        PROTEST_TRIGGERED_EVENT,
        (e) => {
          if (e.payload) handleProtest(e.payload)
        },
      )
    } catch (e) {
      console.warn('[interaction-feedback] listen protest failed:', e)
    }
    try {
      unlistenReverted = await listen<ProtestRevertPayload>(
        PROTEST_REVERTED_EVENT,
        (e) => {
          if (e.payload) handleProtestReverted(e.payload)
        },
      )
    } catch (e) {
      console.warn('[interaction-feedback] listen revert failed:', e)
    }
  })

  onBeforeUnmount(() => {
    unlistenReacted?.()
    unlistenProtest?.()
    unlistenReverted?.()
    if (moodTimer !== null) window.clearTimeout(moodTimer)
    if (bubbleTimer !== null) window.clearTimeout(bubbleTimer)
    if (shakeTimer !== null) window.clearTimeout(shakeTimer)
  })

  return { mood, bubble, shaking }
}
