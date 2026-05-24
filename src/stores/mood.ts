// mood store（#41）— polling mood/energy + disabled_features KV state。
//
// ## 设计
// - 1s polling 拉 mood + energy（Rust 端 transient state，cost 极低）
// - disabled_features 是 KV 持久态，store 加载一次 + setter 调 IPC 写回
// - polling 在 startPolling() 显式启动，stopPolling() 停（pet 窗 / workspace 窗各自管）
// - 不在 store 内做"看不见 = 不 polling"优化：窗口 hide 时 polling 仍跑（mood 状态机仍在 tick），
//   省的是 IPC 来回 ~5ms × 1Hz = 0.5% 单核，不值得在 store 层加 visibilitychange 监听复杂度
//
// ## 与 #40 InteractionRouter 的关系
// - #40 emit pet:interaction_reacted 含 mood_delta（transient hint），由 #41 Rust mood::apply_delta 真消费
// - polling 拉 mood_get 是真值（transient + base 合并后）；前端无需独立监听 emit 来切 mood UI
// - usePetInteractionFeedback 现仍消费 emit 做 shake + bubble（动作即时反馈），与 mood 显示职责分离

import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import {
  DISABLEABLE_FEATURES,
  type DisableableFeature,
  type Mood,
  getDisabledFeatures,
  getEnergy,
  getMood,
  setDisabledFeatures,
} from '@/services/mood'

const POLL_INTERVAL_MS = 1000

export const useMoodStore = defineStore('mood', () => {
  const mood = ref<Mood>('neutral')
  const energy = ref<number>(80)
  const disabledFeatures = ref<DisableableFeature[]>([])
  const loaded = ref(false)

  let pollTimer: ReturnType<typeof setInterval> | null = null
  let pollRefCount = 0

  // === getters ===

  const isMoodIconEnabled = computed(() => !disabledFeatures.value.includes('mood_icon'))
  const isEnergyEnabled = computed(() => !disabledFeatures.value.includes('energy'))
  const isFreeMovementEnabled = computed(
    () => !disabledFeatures.value.includes('free_movement'),
  )

  // === actions ===

  async function refresh() {
    const [m, e] = await Promise.all([getMood(), getEnergy()])
    mood.value = m
    energy.value = e
  }

  async function loadDisabledFeatures() {
    disabledFeatures.value = await getDisabledFeatures()
    loaded.value = true
  }

  /** 引用计数式 polling：多窗 / 多组件可独立 start/stop。 */
  function startPolling() {
    pollRefCount++
    if (pollTimer !== null) return
    void refresh()
    pollTimer = setInterval(() => {
      void refresh()
    }, POLL_INTERVAL_MS)
  }

  function stopPolling() {
    if (pollRefCount > 0) pollRefCount--
    if (pollRefCount > 0) return
    if (pollTimer !== null) {
      clearInterval(pollTimer)
      pollTimer = null
    }
  }

  async function setFeatureDisabled(feature: DisableableFeature, disabled: boolean) {
    const set = new Set(disabledFeatures.value)
    if (disabled) {
      set.add(feature)
    } else {
      set.delete(feature)
    }
    const next = Array.from(set).filter((v): v is DisableableFeature =>
      (DISABLEABLE_FEATURES as readonly string[]).includes(v),
    )
    // 乐观更新 UI，失败回滚
    const prev = disabledFeatures.value
    disabledFeatures.value = next
    try {
      await setDisabledFeatures(next)
    } catch (e) {
      console.warn('[mood] setDisabledFeatures failed, rolling back:', e)
      disabledFeatures.value = prev
      throw e
    }
  }

  return {
    // state
    mood,
    energy,
    disabledFeatures,
    loaded,
    // getters
    isMoodIconEnabled,
    isEnergyEnabled,
    isFreeMovementEnabled,
    // actions
    refresh,
    loadDisabledFeatures,
    startPolling,
    stopPolling,
    setFeatureDisabled,
  }
})
