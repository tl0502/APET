// usePetInteractionFeedback：消费 #40 InteractionRouter emit 的物理交互"动作即时反馈"。
//
// ## 范围调整（#41 完成后）
// - 仅负责 shake + bubble + nod 这类"用户触发了某事 → 立即反馈"的视觉反馈
// - **mood 显示已移到** MoodIcon.vue（1s polling mood_get，6 mood 真值 + base/transient 合并）
// - 不再返 mood ref，不再处理 PROTEST_REVERTED 事件清状态（mood 由 Rust transient 自然过期）
//
// ## 4 项最少可见反馈中本 composable 承担的 3 项（ADR-025 2a-lite，验收 ≥1 项即过）：
//   ✓ shake（CSS keyframe class，200ms）
//   ✓ nod（runtime.playAction('nod')；其他 actionId 占位走 shake 兜底）
//   ✓ 气泡反馈（template → bubble ref → 父组件渲染浮层文字 2s）

import { onBeforeUnmount, onMounted, ref, type Ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  INTERACTION_REACTED_EVENT,
  PROTEST_TRIGGERED_EVENT,
  type InteractionReactedPayload,
  type ProtestPayload,
} from '@/services/interaction'
import type { PetActionId, VRMRuntime } from '@/services/vrm'

const BUBBLE_DURATION_MS = 2000
const SHAKE_DURATION_MS = 220

export interface UsePetInteractionFeedbackReturn {
  /** 临时气泡文案（来自 reaction_table.template）；空字符串 = 不显示。 */
  bubble: Ref<string>
  /** 触发 .pet-stage shake CSS keyframe；父组件 class binding。 */
  shaking: Ref<boolean>
}

/**
 * 监听 InteractionRouter 2 个 emit 事件（reacted + protest），把视觉反馈解耦到 ref。
 * mood 显示由独立 MoodIcon.vue 负责（polling mood_get），与本 composable 解耦。
 *
 * 调用方在 setup 阶段拿 runtime（VRMRuntime 实例）+ enabled fn；onboarding 等场景应传 false。
 */
export function usePetInteractionFeedback(
  runtime: VRMRuntime,
  isEnabled: () => boolean = () => true,
): UsePetInteractionFeedbackReturn {
  const bubble = ref<string>('')
  const shaking = ref<boolean>(false)

  let unlistenReacted: UnlistenFn | null = null
  let unlistenProtest: UnlistenFn | null = null

  let bubbleTimer: number | null = null
  let shakeTimer: number | null = null

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
    runtime
      .playAction(actionId as PetActionId)
      .catch((e) => console.warn('[interaction-feedback] playAction failed:', e))
  }

  function handleReacted(payload: InteractionReactedPayload) {
    if (!isEnabled()) return
    playVrmAction(payload.actionId)

    // 视觉补偿：nod / fall_asleep 自带视觉，其他 actionId 跟一次 shake 让用户感知"触发了"。
    if (payload.actionId !== 'nod' && payload.actionId !== 'fall_asleep') {
      triggerShake()
    }
    if (payload.template) {
      flashBubble(payload.template)
    }
  }

  function handleProtest(payload: ProtestPayload) {
    if (!isEnabled()) return
    triggerShake()
    if (payload.template) {
      flashBubble(payload.template)
    }
    // mood='annoyed' 由 #41 Rust 端 record_drag_count 内 mood::apply_delta 已处理；
    // MoodIcon polling 会拉到 annoyed transient（5s 内）+ 自动过期 → 不再前端兜底清状态。
    // playAction('protest') 是 placeholder；由 shake + bubble + mood 三件套承载视觉。
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
  })

  onBeforeUnmount(() => {
    unlistenReacted?.()
    unlistenProtest?.()
    if (bubbleTimer !== null) window.clearTimeout(bubbleTimer)
    if (shakeTimer !== null) window.clearTimeout(shakeTimer)
  })

  return { bubble, shaking }
}
