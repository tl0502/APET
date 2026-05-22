<script setup lang="ts">
// PomodoroPanel：tasks 窗"番茄"tab 主面板（issue #28）。
//
// 布局（plan 决策 #10）：
//   ┌──────────────────────────────────────┐
//   │ 🎯 专注中           今日 3 × 25min  │
//   │                                       │
//   │       ⌒ ⌒ ⌒  18:42  ⌒ ⌒ ⌒          │
//   │                                       │
//   │   [暂停] [停止]   [高级 ▾]           │
//   ├ (高级折叠区) ──────────────────────  │
//   │ 预设 [25/5] [50/10]                  │
//   │ 专注时长 ──◯── 25 分钟               │
//   │ 休息时长 ──◯── 5 分钟                │
//   └──────────────────────────────────────┘
//
// 状态管理：
// - active = ref<ActiveSession | null>：当前 phase 真相源
// - remainingMs：listen 'pomodoro:tick' 更新；mount 时根据 active.phasePlannedEnd 初始化
// - 'pomodoro:state_changed' { phase: null } → active = null + remainingMs = 0
// - 'pomodoro:state_changed' { phase: ... } → 重新 getActivePomodoro
// - 'pomodoro:rest_ended' → 刷 todayStats（自动完成时计数 +1）
//
// 配置 draft（focusMinDraft / restMinDraft）：start 时传给 IPC；ElSlider 双绑。
// active 期 slider disabled（不能改运行中的时长）。
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { ElButton, ElCollapseTransition, ElMessage, ElProgress, ElSlider } from 'element-plus'
import { ArrowDown, Close, TopRight, VideoPause, VideoPlay } from '@element-plus/icons-vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  getActivePomodoro,
  getPomodoroTodayStats,
  pausePomodoro,
  resumePomodoro,
  startPomodoro,
  stopPomodoro,
} from '@/services/pomodoro'
import { showPomodoro } from '@/services/window'
import {
  DEFAULT_FOCUS_MIN,
  DEFAULT_REST_MIN,
  FOCUS_MIN_RANGE,
  POMODORO_FOCUS_ENDED_EVENT,
  POMODORO_PRESETS,
  POMODORO_REST_ENDED_EVENT,
  POMODORO_STATE_CHANGED_EVENT,
  POMODORO_TICK_EVENT,
  REST_MIN_RANGE,
  formatRemainingMs,
  getPhaseMeta,
  type ActiveSession,
  type PomodoroFocusEndedPayload,
  type PomodoroPreset,
  type PomodoroStateChangedPayload,
  type PomodoroTickPayload,
  type TodayStats,
} from '@/types/pomodoro'

const active = ref<ActiveSession | null>(null)
const remainingMs = ref<number>(0)
const todayStats = ref<TodayStats>({ completed: 0, cancelled: 0, totalFocusMin: 0 })
const busy = ref(false)
const advancedOpen = ref(false)

const focusMinDraft = ref<number>(DEFAULT_FOCUS_MIN)
const restMinDraft = ref<number>(DEFAULT_REST_MIN)

const phaseMeta = computed(() => getPhaseMeta(active.value?.phase ?? null))

const progressPct = computed(() => {
  if (!active.value) return 0
  const total =
    phaseMeta.value.isFocusLike
      ? active.value.focusMin * 60_000
      : active.value.restMin * 60_000
  if (total <= 0) return 0
  const elapsed = total - remainingMs.value
  const pct = (elapsed / total) * 100
  return Math.max(0, Math.min(100, pct))
})

const displayRemainingMs = computed(() => {
  if (active.value) return remainingMs.value
  // IDLE：预览模式，显示 focusMinDraft 时长
  return focusMinDraft.value * 60_000
})

const displayMode = computed(() => {
  if (!active.value) return `${focusMinDraft.value} 分钟专注`
  if (phaseMeta.value.isFocusLike) {
    return phaseMeta.value.isPaused ? '专注暂停' : '专注中'
  }
  return phaseMeta.value.isPaused ? '休息暂停' : '休息中'
})

const todaySummary = computed(() => {
  const avg =
    todayStats.value.completed > 0
      ? Math.round(todayStats.value.totalFocusMin / todayStats.value.completed)
      : 0
  return { completed: todayStats.value.completed, avg, cancelled: todayStats.value.cancelled }
})

async function refreshActive() {
  try {
    active.value = await getActivePomodoro()
    if (active.value) {
      // 用 phasePlannedEnd 推 remainingMs，避免下次 tick 前显示 00:00
      const pe = Date.parse(active.value.phasePlannedEnd)
      if (!Number.isNaN(pe)) {
        remainingMs.value = Math.max(0, pe - Date.now())
      }
    } else {
      remainingMs.value = 0
    }
  } catch (e) {
    console.error('[pomodoro-panel] getActivePomodoro failed:', e)
  }
}

async function refreshStats() {
  try {
    todayStats.value = await getPomodoroTodayStats()
  } catch (e) {
    console.error('[pomodoro-panel] getPomodoroTodayStats failed:', e)
  }
}

function applyPreset(p: PomodoroPreset) {
  focusMinDraft.value = p.focusMin
  restMinDraft.value = p.restMin
}

async function onStart() {
  busy.value = true
  try {
    await startPomodoro({ focusMin: focusMinDraft.value, restMin: restMinDraft.value })
    advancedOpen.value = false
    // state_changed listener 会刷新 active
  } catch (e) {
    ElMessage.error(`启动番茄失败：${formatErr(e)}`)
  } finally {
    busy.value = false
  }
}

async function onPauseOrResume() {
  if (!active.value) return
  busy.value = true
  try {
    if (phaseMeta.value.isPaused) {
      await resumePomodoro()
    } else {
      await pausePomodoro()
    }
  } catch (e) {
    ElMessage.error(`操作失败：${formatErr(e)}`)
  } finally {
    busy.value = false
  }
}

async function onStop() {
  if (!active.value) return
  const wasFocusLike = phaseMeta.value.isFocusLike
  busy.value = true
  try {
    const result = await stopPomodoro()
    if (result.status === 'cancelled' && wasFocusLike) {
      // plan 决策 #11：toast 直接放弃，无 modal
      ElMessage({
        type: 'warning',
        duration: 4000,
        message: `本次专注 ${result.focusMinActual.toFixed(1)} 分钟，未达 30%，记为放弃`,
      })
    } else if (result.status === 'completed' && wasFocusLike) {
      ElMessage.success(`专注完成 ${result.focusMinActual.toFixed(1)} 分钟 🎯`)
    } else {
      ElMessage.success('番茄结束 ✓')
    }
    await refreshStats()
  } catch (e) {
    ElMessage.error(`停止失败：${formatErr(e)}`)
  } finally {
    busy.value = false
  }
}

function formatErr(e: unknown): string {
  if (e instanceof Error) return e.message
  return String(e)
}

/** #28 follow-up：唤起独立番茄窗（紧凑 Pomotroid 型 widget）。
 * tasks tab 大面板 + 独立窗双入口并存，相互独立，复用同一 service 层数据 / 事件流。 */
async function onOpenStandalone() {
  try {
    await showPomodoro()
  } catch (e) {
    ElMessage.error(`打开独立窗失败：${formatErr(e)}`)
  }
}

// === listeners ===
let unlistenTick: UnlistenFn | null = null
let unlistenStateChanged: UnlistenFn | null = null
let unlistenFocusEnded: UnlistenFn | null = null
let unlistenRestEnded: UnlistenFn | null = null

onMounted(async () => {
  // 先注册 listener 再 refresh：避免 mount 间隔后端 emit 的 tick / state_changed 漏掉
  // （review BUG-5）。listen 是 async 订阅，await 完成保证 subscribe 已生效。
  unlistenTick = await listen<PomodoroTickPayload>(POMODORO_TICK_EVENT, (e) => {
    if (!e.payload) return
    // tick 来时 active 可能尚未刷新（state_changed 顺序）；先用 tick payload 短路
    if (!active.value || active.value.sessionId !== e.payload.sessionId) {
      void refreshActive()
    } else if (active.value.phase !== e.payload.phase) {
      // FOCUS → REST 自动转换：phase 变了但 sessionId 同。重新拉以拿到新的 phasePlannedEnd 等
      void refreshActive()
    }
    remainingMs.value = e.payload.remainingMs
  })

  unlistenStateChanged = await listen<PomodoroStateChangedPayload>(
    POMODORO_STATE_CHANGED_EVENT,
    (e) => {
      const phase = e.payload?.phase ?? null
      if (phase === null) {
        active.value = null
        remainingMs.value = 0
      } else {
        void refreshActive()
      }
    },
  )

  // focus_ended { completed: false, interruptedBy }：硬提醒打断时 toast
  unlistenFocusEnded = await listen<PomodoroFocusEndedPayload>(
    POMODORO_FOCUS_ENDED_EVENT,
    (e) => {
      if (!e.payload) return
      if (e.payload.interruptedBy) {
        ElMessage({
          type: 'warning',
          duration: 5000,
          message: '硬提醒打断了番茄专注（记为放弃）',
        })
        void refreshStats()
      } else if (e.payload.completed) {
        // 自动 FOCUS→REST，无需 toast（用户在 UI 上看 phase 变化即可）
      }
    },
  )

  unlistenRestEnded = await listen(POMODORO_REST_ENDED_EVENT, () => {
    // 自动 REST→IDLE：刷统计（completed 计数 +1）
    void refreshStats()
  })

  // listener 全部就位后再拉取初始状态
  await Promise.all([refreshActive(), refreshStats()])
})

onBeforeUnmount(() => {
  unlistenTick?.()
  unlistenStateChanged?.()
  unlistenFocusEnded?.()
  unlistenRestEnded?.()
})
</script>

<template>
  <div class="panel pomodoro-panel">
    <header class="pomodoro-header">
      <div class="pomodoro-header__phase">
        <span class="pomodoro-header__emoji">{{ phaseMeta.emoji }}</span>
        <span class="pomodoro-header__label">{{ displayMode }}</span>
      </div>
      <div class="pomodoro-header__stats">
        <ElButton
          class="pomodoro-header__open"
          text
          size="small"
          :icon="TopRight"
          title="番茄是高频快速操作，独立窗常显更好用"
          @click="onOpenStandalone"
        >
          独立窗口
        </ElButton>
        <span class="pomodoro-header__count">
          今日 <strong>{{ todaySummary.completed }}</strong>
          <template v-if="todaySummary.completed > 0">
            × {{ todaySummary.avg }}min
          </template>
        </span>
        <span v-if="todaySummary.cancelled > 0" class="pomodoro-header__cancelled">
          放弃 {{ todaySummary.cancelled }}
        </span>
      </div>
    </header>

    <main class="pomodoro-main">
      <div class="pomodoro-ring">
        <ElProgress
          type="circle"
          :percentage="progressPct"
          :width="220"
          :stroke-width="8"
          :color="phaseMeta.color"
          :show-text="false"
        />
        <div class="pomodoro-countdown">
          <span class="pomodoro-countdown__time">{{
            formatRemainingMs(displayRemainingMs)
          }}</span>
          <span class="pomodoro-countdown__caption">
            {{ active ? phaseMeta.label : '准备开始' }}
          </span>
        </div>
      </div>
    </main>

    <footer class="pomodoro-footer" data-no-drag>
      <ElButton
        v-if="!phaseMeta.active"
        type="primary"
        size="large"
        :loading="busy"
        :icon="VideoPlay"
        @click="onStart"
      >
        开始
      </ElButton>
      <ElButton
        v-else
        size="large"
        :type="phaseMeta.isPaused ? 'primary' : 'default'"
        :loading="busy"
        :icon="phaseMeta.isPaused ? VideoPlay : VideoPause"
        @click="onPauseOrResume"
      >
        {{ phaseMeta.isPaused ? '继续' : '暂停' }}
      </ElButton>
      <ElButton
        v-if="phaseMeta.active"
        size="large"
        :loading="busy"
        :icon="Close"
        @click="onStop"
      >
        停止
      </ElButton>
      <ElButton
        size="large"
        text
        :icon="ArrowDown"
        :class="{ 'pomodoro-footer__advanced--open': advancedOpen }"
        @click="advancedOpen = !advancedOpen"
      >
        高级
      </ElButton>
    </footer>

    <ElCollapseTransition>
      <section v-show="advancedOpen" class="pomodoro-advanced">
        <div class="pomodoro-advanced__presets">
          <span class="pomodoro-advanced__title">预设</span>
          <div class="pomodoro-advanced__preset-row">
            <ElButton
              v-for="p in POMODORO_PRESETS"
              :key="p.id"
              size="small"
              :disabled="busy || phaseMeta.active"
              @click="applyPreset(p)"
            >
              {{ p.label }}
            </ElButton>
          </div>
        </div>

        <div class="pomodoro-advanced__field">
          <div class="pomodoro-advanced__label">
            <span>专注时长</span>
            <span class="pomodoro-advanced__value">{{ focusMinDraft }} 分钟</span>
          </div>
          <ElSlider
            v-model="focusMinDraft"
            :min="FOCUS_MIN_RANGE.min"
            :max="FOCUS_MIN_RANGE.max"
            :step="FOCUS_MIN_RANGE.step"
            :disabled="busy || phaseMeta.active"
            :marks="{
              [DEFAULT_FOCUS_MIN]: '25',
              50: '50',
              90: '90',
            }"
          />
        </div>

        <div class="pomodoro-advanced__field">
          <div class="pomodoro-advanced__label">
            <span>休息时长</span>
            <span class="pomodoro-advanced__value">{{ restMinDraft }} 分钟</span>
          </div>
          <ElSlider
            v-model="restMinDraft"
            :min="REST_MIN_RANGE.min"
            :max="REST_MIN_RANGE.max"
            :step="REST_MIN_RANGE.step"
            :disabled="busy || phaseMeta.active"
            :marks="{
              [DEFAULT_REST_MIN]: '5',
              10: '10',
              30: '30',
            }"
          />
        </div>

        <p class="pomodoro-advanced__hint">
          自定义后下次启动会自动记忆。专注期间无法调整。
        </p>
      </section>
    </ElCollapseTransition>
  </div>
</template>

<style scoped>
.pomodoro-panel {
  padding: var(--aipet-space-6) var(--aipet-space-2);
  align-items: stretch;
}

/* === Header === */
.pomodoro-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--aipet-space-3);
  padding: 0 var(--aipet-space-2);
}

.pomodoro-header__phase {
  display: inline-flex;
  align-items: center;
  gap: var(--aipet-space-2);
}

.pomodoro-header__emoji {
  font-size: 20px;
  line-height: 1;
}

.pomodoro-header__label {
  font-size: 14px;
  font-weight: 600;
  color: var(--aipet-color-text-1);
}

.pomodoro-header__stats {
  display: inline-flex;
  align-items: center;
  gap: var(--aipet-space-3);
  font-size: 12px;
  color: var(--aipet-color-text-3);
}

/* 「独立窗口 ↗」按钮（#28 follow-up 双入口；click → showPomodoro IPC） */
.pomodoro-header__open {
  color: var(--aipet-color-text-3);
  font-size: 12px;
  padding: 2px 6px;
  height: auto;
}

.pomodoro-header__open:hover:not(.is-disabled) {
  color: var(--aipet-color-primary);
}

.pomodoro-header__count strong {
  color: var(--aipet-color-text-1);
  font-weight: 600;
  font-size: 13px;
}

.pomodoro-header__cancelled {
  color: color-mix(in srgb, var(--aipet-color-primary) 40%, var(--aipet-color-text-3));
}

/* === Main: 大环 + 倒计时 === */
.pomodoro-main {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--aipet-space-4) 0;
}

/* ElProgress + 绝对定位 overlay 组合（fix #28 倒计时不显示 bug）：
 * EP 在 `:show-text="false"` 且 default slot 存在时仍渲染 `.el-progress__text`，
 * 旧版 CSS `:deep(.el-progress__text) { display:none }` 把整个 __text 隐藏 →
 * 连带 slot 内的倒计时数字一起消失。改用独立 overlay 完全脱离 EP slot 路径。 */
.pomodoro-ring {
  position: relative;
  width: 220px;
  height: 220px;
}

.pomodoro-countdown {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  pointer-events: none;
}

.pomodoro-countdown__time {
  font-size: 44px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--aipet-color-text-1);
  letter-spacing: -0.02em;
  line-height: 1.1;
}

.pomodoro-countdown__caption {
  font-size: 12px;
  color: var(--aipet-color-text-3);
}

/* === Footer: 按钮区 === */
.pomodoro-footer {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--aipet-space-2);
  padding: 0 var(--aipet-space-2);
}

.pomodoro-footer :deep(.el-button--large) {
  min-width: 92px;
}

.pomodoro-footer__advanced--open :deep(.el-icon) {
  transform: rotate(180deg);
  transition: transform var(--aipet-duration-fast) var(--aipet-ease-standard);
}

/* === Advanced 折叠区 === */
.pomodoro-advanced {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
  margin-top: var(--aipet-space-2);
  padding: var(--aipet-space-4);
  background: var(--aipet-color-surface);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-lg);
}

.pomodoro-advanced__presets {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}

.pomodoro-advanced__title {
  font-size: 12px;
  font-weight: 600;
  color: var(--aipet-color-text-2);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.pomodoro-advanced__preset-row {
  display: flex;
  gap: var(--aipet-space-2);
  flex-wrap: wrap;
}

.pomodoro-advanced__field {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}

.pomodoro-advanced__label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 13px;
  color: var(--aipet-color-text-2);
}

.pomodoro-advanced__value {
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--aipet-color-text-1);
}

.pomodoro-advanced__hint {
  margin: 0;
  font-size: 11px;
  color: var(--aipet-color-text-3);
  line-height: 1.5;
}

/* === ElProgress 主体覆写：让进度环更柔和 === */
.pomodoro-main :deep(.el-progress-circle__track) {
  stroke: color-mix(in srgb, var(--aipet-color-border) 70%, transparent);
}
</style>
