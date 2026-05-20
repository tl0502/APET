<script setup lang="ts">
// PomodoroApp：番茄独立窗 root（#28 follow-up）。
//
// 设计参考：Pomotroid / Fomodoro / Pomodor / Athenify / FocusBox：
// - 紧凑模式：360×480 锁定窗，240×240 大环 + 56px hero 倒计时 + 2 主按钮 + 齿轮 popover
// - 全屏模式：setFullscreen(true) + 480 大环 + 144px hero + 极简控件（仅圈 + 字 + 1-2 按钮）
//   触发：header 齿轮旁 ⛶ toggle；退出：Esc 键 / 再点 ⛶
// - 倒计时显示采用「ElProgress + 绝对定位 overlay」结构（ElProgress 的 default slot 在
//   `:show-text="false"` 时仍会渲染 `.el-progress__text`，CSS 隐藏 __text 会一并隐藏 slot；
//   独立 overlay 避开 EP 内部条件）
//
// 与 tasks tab 的 PomodoroPanel.vue 完全独立：tasks tab 是大面板（嵌 800×600），独立窗
// 是紧凑桌面 widget；两者复用 service 层（IPC + events + types），不复用组件。
//
// 5 个 listener：
// - pomodoro:tick → remainingMs 实时更新
// - pomodoro:state_changed → active 切换 + AOT phase-driven 同步（修订 #3）
// - pomodoro:focus_ended → 硬提醒打断 toast
// - pomodoro:rest_ended → 刷统计
//
// hide 时的「计时继续在后台」用户提示走 OS 系统通知（lib.rs CloseRequested 分支发），
// 不在前端 listen——hide 后 webview 内 toast 用户看不到（review BUG-18）。

import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  ElButton,
  ElMessage,
  ElPopover,
  ElProgress,
  ElSlider,
} from 'element-plus'
import {
  Close,
  FullScreen,
  Setting,
  VideoPause,
  VideoPlay,
} from '@element-plus/icons-vue'
import { getCurrentWindow, LogicalPosition } from '@tauri-apps/api/window'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import AppShell from '@/components/layouts/AppShell.vue'
import SnapGhost from '@/components/SnapGhost.vue'
import { useSnapWindow } from '@/composables/useSnapWindow'
import { useFocusAOT } from '@/composables/useFocusAOT'
import { constraintStore } from '@/lib/snap/constraintStore'
import { persistAndBroadcastConstraints } from '@/lib/snap/persistence'
import type { Rect } from '@/lib/snap/types'
import {
  getActivePomodoro,
  getPomodoroTodayStats,
  pausePomodoro,
  resumePomodoro,
  startPomodoro,
  stopPomodoro,
} from '@/services/pomodoro'
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
  type PomodoroPhase,
  type PomodoroPreset,
  type PomodoroStateChangedPayload,
  type PomodoroTickPayload,
  type TodayStats,
} from '@/types/pomodoro'

// === 状态 ===
const active = ref<ActiveSession | null>(null)
const remainingMs = ref<number>(0)
const todayStats = ref<TodayStats>({ completed: 0, cancelled: 0, totalFocusMin: 0 })
const busy = ref(false)
const popoverOpen = ref(false)
const isFullscreen = ref(false)

const focusMinDraft = ref<number>(DEFAULT_FOCUS_MIN)
const restMinDraft = ref<number>(DEFAULT_REST_MIN)

// === 磁吸接入（#30 follow-up E）===
// pomodoro 作为 secondary 参与磁吸：紧凑模式正常吸附；全屏模式由 toggleFullscreen 主动
// detach + visible=false 让其他窗忽略本窗。reactive ref 仅在紧凑模式渲染消费。
const {
  isPreviewAnchor: pomoIsPreviewAnchor,
  previewEdgeFor: pomoPreviewEdge,
  previewIntensityFor: pomoPreviewIntensity,
  isFieldAnchor: pomoIsFieldAnchor,
  fieldIntensityFor: pomoFieldIntensity,
  selfLean: pomoSelfLean,
  syncRustSnap: pomoSyncRustSnap,
  broadcastSelfRect: pomoBroadcastSelfRect,
} = useSnapWindow('pomodoro')

const pomoSnapPreviewClass = computed(() => {
  const cls: Record<string, boolean> = {
    'snap-preview': pomoIsPreviewAnchor.value,
    'snap-field-anchor': pomoIsFieldAnchor.value,
  }
  if (pomoIsPreviewAnchor.value && pomoPreviewEdge.value) {
    cls[`snap-preview--edge-${pomoPreviewEdge.value}`] = true
  }
  return cls
})
const pomoSnapPreviewStyle = computed(() => ({
  '--snap-preview-intensity': String(pomoPreviewIntensity.value),
  '--snap-field-intensity': String(pomoFieldIntensity.value),
}))
const pomoLeanStyle = computed(() => {
  const lean = pomoSelfLean.value
  if (!lean) return {}
  return { transform: `translate(${lean.dx.toFixed(2)}px, ${lean.dy.toFixed(2)}px)` }
})

const phaseMeta = computed(() => getPhaseMeta(active.value?.phase ?? null))

const progressPct = computed(() => {
  if (!active.value) return 0
  const total = phaseMeta.value.isFocusLike
    ? active.value.focusMin * 60_000
    : active.value.restMin * 60_000
  if (total <= 0) return 0
  const elapsed = total - remainingMs.value
  const pct = (elapsed / total) * 100
  return Math.max(0, Math.min(100, pct))
})

const displayRemainingMs = computed(() => {
  if (active.value) return remainingMs.value
  return focusMinDraft.value * 60_000
})

const displayCaption = computed(() =>
  active.value ? phaseMeta.value.label : '准备开始',
)

const todaySummary = computed(() => {
  const avg =
    todayStats.value.completed > 0
      ? Math.round(todayStats.value.totalFocusMin / todayStats.value.completed)
      : 0
  return { completed: todayStats.value.completed, avg, cancelled: todayStats.value.cancelled }
})

// === phase-driven AOT（修订 #3 + #30 follow-up H 整合） ===
// FOCUS / PAUSED_F 期保持 topmost（番茄专注模式不被任何窗盖住，优先级高于 focus）。
// 其他 phase 走 useFocusAOT 的 focus-driven 逻辑（被点中升 topmost，失焦降回）。
//
// 整合后单一权威：所有 setAlwaysOnTop 都经 useFocusAOT.resync → applyAOT 幂等 cache，
// 避免之前 applyAOT(phase) 与 toggleFullscreen 的 setAlwaysOnTop(false) 并发写引发抖动。
function shouldAOTForPhase(phase: PomodoroPhase | null): boolean {
  return phase === 'FOCUS' || phase === 'PAUSED_F'
}

// useFocusAOT 调用：shouldKeepTopmost 让 phase 强制 topmost 优先于 focus；
// fullscreen 期也返 false（全屏窗 AOT 无意义且可能与 setFullscreen 冲突）。
const focusAOT = useFocusAOT({
  shouldKeepTopmost: () => !isFullscreen.value && shouldAOTForPhase(active.value?.phase ?? null),
})

// caller 接口：phase / fullscreen 变化时调一次让 useFocusAOT 重算综合 AOT 状态。
async function applyAOT(_phase: PomodoroPhase | null) {
  await focusAOT.resync()
}

// === 全屏 toggle ===
// #30 follow-up E：全屏期间 pomodoro rect = 整屏，参与磁吸会让其他窗在屏幕任何位置都
// 匹配 pomodoro projection overlap，必须主动从 snap 网络脱开。
//
// 时序（进入）：保存进入前 rect 快照 → 断开 pomodoro 出向 constraint（含 persist + 广播）
//   → emit visible=false 让其他窗的 findCandidates 跳过本窗 → 关 AOT → setFullscreen(true)
// 时序（退出）：setFullscreen(false) → setPosition 复位到快照 rect → emit visible=true
//   重广播 → 恢复 phase-driven AOT。**复位不重建 constraint**——用户期望"复位 = 视觉还原，
//   不自动重连"，避免全屏前后吸附状态不一致带来的惊讶。
let preFullscreenRect: Rect | null = null

async function readSelfRect(): Promise<Rect> {
  const w = getCurrentWindow()
  const pos = await w.outerPosition()
  const size = await w.outerSize()
  const sf = await w.scaleFactor()
  return {
    x: Math.round(pos.x / sf),
    y: Math.round(pos.y / sf),
    w: Math.round(size.width / sf),
    h: Math.round(size.height / sf),
  }
}

async function broadcastVisibility(visible: boolean, rect: Rect): Promise<void> {
  // P6 修复 (review 2)：走 composable 暴露的 broadcastSelfRect，统一 payload schema
  // （含 visualInset 字段，与 useSnapWindow 内部其他 emit 路径一致），避免 contract 分叉。
  try {
    await pomoBroadcastSelfRect(rect, visible)
  } catch (e) {
    console.warn('[pomodoro-app] broadcast visibility failed:', e)
  }
}

/** 进入全屏前断开 pomodoro 自身出向 constraint（仅删 source=pomodoro 的边）。
 *  不删入向（M3 多窗时其他窗若依附 pomodoro，全屏不应强拽它们）。
 *
 *  P1 修复 (review 2)：persist+broadcast 后必须本地 syncRustSnap。本 webview emit 的
 *  constraint-changed 会被 A4 senderId 自过滤跳过自己 → 自己 webview 永远不会
 *  loadPersistedConstraints + syncRustSnap，要等 pet webview 收到 broadcast → 跨 webview
 *  reload → 才间接修正 Rust SnapState。这中间窗口内 pet 拖动会让 Rust BFS 仍看到
 *  pomodoro 出向，mark internal_until[pomodoro] + setPosition（fullscreen 被 OS 忽略，
 *  但 guard 100ms 内会吞掉 pomodoro 合法 Moved）。M3 多 primary 启用后 BFS 真会拽。 */
async function detachForFullscreen(): Promise<void> {
  const removed = constraintStore.removeAllInvolving('pomodoro')
  if (removed.length > 0) {
    try {
      await persistAndBroadcastConstraints('pomodoro')
      await pomoSyncRustSnap()
    } catch (e) {
      console.warn('[pomodoro-app] persist after fullscreen detach failed:', e)
    }
  }
}

async function toggleFullscreen() {
  const next = !isFullscreen.value
  try {
    const w = getCurrentWindow()
    if (next) {
      // 1. 快照进入前 rect（退出时用此复位）
      try {
        preFullscreenRect = await readSelfRect()
      } catch (e) {
        console.warn('[pomodoro-app] snapshot pre-fullscreen rect failed:', e)
        preFullscreenRect = null
      }
      // 2. 断开 snap constraint（在 setFullscreen 之前，避免中间帧别窗看到 pomodoro 已变巨型 rect 还带 constraint）
      await detachForFullscreen()
      // 3. 标记 visible=false 让其他窗 findCandidates 跳过
      if (preFullscreenRect) {
        await broadcastVisibility(false, preFullscreenRect)
      }
      // 4. 关 AOT（setFullscreen 与 AOT 某些 WM 互斥）
      // 先翻 isFullscreen 标志再 resync，让 shouldKeepTopmost 返 false
      // → useFocusAOT 综合判断后 setAlwaysOnTop(false)，同步 lastAOT cache（避免后续判 noop）
      isFullscreen.value = true
      await focusAOT.resync()
      // 5. setFullscreen
      await w.setFullscreen(true)
    } else {
      // 退出：与进入对称的反向序列
      // P7 修复 (review 2)：先翻 isFullscreen=false 让 useFocusAOT.shouldKeepTopmost 回到
      // phase-driven，避免 setFullscreen(false) 触发 onFocusChanged 时 shouldKeepTopmost 还读
      // 旧的 isFullscreen=true（短窗口期 ~10ms 的 AOT 错位）。
      isFullscreen.value = false
      // 1. setFullscreen(false) 让窗回到原 size
      await w.setFullscreen(false)
      // 2. 复位 OS rect
      if (preFullscreenRect) {
        try {
          await w.setPosition(new LogicalPosition(preFullscreenRect.x, preFullscreenRect.y))
        } catch (e) {
          console.warn('[pomodoro-app] restore pre-fullscreen position failed:', e)
        }
        // 3. 标记 visible=true 重新广播
        await broadcastVisibility(true, preFullscreenRect)
        preFullscreenRect = null
      } else {
        // 异常路径：进入时没拿到 rect 快照，至少把 visible 还原成 true
        try {
          const cur = await readSelfRect()
          await broadcastVisibility(true, cur)
        } catch (e) {
          console.warn('[pomodoro-app] fallback visibility restore failed:', e)
        }
      }
      // 4. resync 让 useFocusAOT 综合 phase + focus 重设 AOT
      await focusAOT.resync()
    }
  } catch (e) {
    console.warn('[pomodoro-app] setFullscreen failed:', e)
    ElMessage.error('全屏切换失败')
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && isFullscreen.value) {
    e.preventDefault()
    void toggleFullscreen()
  }
}

// === IPC refresh ===
async function refreshActive() {
  try {
    active.value = await getActivePomodoro()
    if (active.value) {
      const pe = Date.parse(active.value.phasePlannedEnd)
      if (!Number.isNaN(pe)) {
        remainingMs.value = Math.max(0, pe - Date.now())
      }
      focusMinDraft.value = active.value.focusMin
      restMinDraft.value = active.value.restMin
    } else {
      remainingMs.value = 0
    }
  } catch (e) {
    console.error('[pomodoro-app] getActivePomodoro failed:', e)
  }
}

async function refreshStats() {
  try {
    todayStats.value = await getPomodoroTodayStats()
  } catch (e) {
    console.error('[pomodoro-app] getPomodoroTodayStats failed:', e)
  }
}

function applyPreset(p: PomodoroPreset) {
  focusMinDraft.value = p.focusMin
  restMinDraft.value = p.restMin
}

// === 主操作 ===
async function onStart() {
  busy.value = true
  try {
    await startPomodoro({ focusMin: focusMinDraft.value, restMin: restMinDraft.value })
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
      ElMessage({
        type: 'warning',
        duration: 4000,
        showClose: true,
        message: `本次专注 ${result.focusMinActual.toFixed(1)} 分钟，未达 30%，记为放弃`,
      })
    } else if (result.status === 'completed' && wasFocusLike) {
      ElMessage.success({
        duration: 4000,
        showClose: true,
        message: `专注完成 ${result.focusMinActual.toFixed(1)} 分钟 🎯`,
      })
    } else {
      ElMessage.success({ duration: 3000, showClose: true, message: '番茄结束 ✓' })
    }
    await refreshStats()
  } catch (e) {
    ElMessage.error(`停止失败：${formatErr(e)}`)
  } finally {
    busy.value = false
  }
}

async function onClose() {
  // decorations:false 后无 OS 关闭按钮 → 自定义 ✕ 走 window.close() 触发 CloseRequested,
  // lib.rs 拦截 prevent_close → hide + 首次 OS 通知，与 Alt+F4 路径一致。
  //
  // 全屏期点 ✕ 先退出全屏（review BUG-7）：避免下次 show 时窗仍 fullscreen 而前端 ref
  // 也保留为 fullscreen，造成"打开 = 全屏"的突兀体验。Alt+F4 全屏的极少场景不处理。
  try {
    if (isFullscreen.value) {
      await toggleFullscreen()
    }
    await getCurrentWindow().close()
  } catch (e) {
    console.warn('[pomodoro-app] close failed:', e)
  }
}

function formatErr(e: unknown): string {
  if (e instanceof Error) return e.message
  return String(e)
}

// === listeners ===
let unlistenTick: UnlistenFn | null = null
let unlistenStateChanged: UnlistenFn | null = null
let unlistenFocusEnded: UnlistenFn | null = null
let unlistenRestEnded: UnlistenFn | null = null

onMounted(async () => {
  // 先注册 listener 再 refresh：避免 mount 间隔（refresh→AOT→listen）中后端 emit 的
  // state_changed / tick 事件被漏掉，造成 phase/color 短暂错位（review BUG-5）。
  // listen 是 async 订阅，await 完后保证 subscribe 已生效。
  unlistenTick = await listen<PomodoroTickPayload>(POMODORO_TICK_EVENT, (e) => {
    if (!e.payload) return
    if (!active.value || active.value.sessionId !== e.payload.sessionId) {
      void refreshActive()
    } else if (active.value.phase !== e.payload.phase) {
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
      void applyAOT(phase)
    },
  )

  unlistenFocusEnded = await listen<PomodoroFocusEndedPayload>(
    POMODORO_FOCUS_ENDED_EVENT,
    (e) => {
      if (!e.payload) return
      if (e.payload.interruptedBy) {
        ElMessage({
          type: 'warning',
          duration: 5000,
          showClose: true,
          message: '硬提醒打断了番茄专注（记为放弃）',
        })
        void refreshStats()
      }
    },
  )

  unlistenRestEnded = await listen(POMODORO_REST_ENDED_EVENT, () => {
    void refreshStats()
  })

  // listener 全部就位后再拉取初始状态 + 应用 AOT
  await Promise.all([refreshActive(), refreshStats()])
  await applyAOT(active.value?.phase ?? null)

  window.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
  unlistenTick?.()
  unlistenStateChanged?.()
  unlistenFocusEnded?.()
  unlistenRestEnded?.()
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <!-- 全屏模式：极简覆盖层（隐藏 header/footer，仅圈 + 字 + 1-2 按钮） -->
  <div v-if="isFullscreen" class="pomo-fs" :data-phase="phaseMeta.isFocusLike ? 'focus' : 'rest'">
    <div class="pomo-fs__ring">
      <ElProgress
        type="circle"
        :percentage="progressPct"
        :width="480"
        :stroke-width="6"
        :color="phaseMeta.color"
        :show-text="false"
      />
      <div class="pomo-fs__overlay">
        <span class="pomo-fs__time">{{ formatRemainingMs(displayRemainingMs) }}</span>
        <span class="pomo-fs__caption">
          <span class="pomo-fs__emoji">{{ phaseMeta.emoji }}</span>
          {{ displayCaption }}
        </span>
      </div>
    </div>

    <div class="pomo-fs__actions">
      <ElButton
        v-if="!phaseMeta.active"
        type="primary"
        size="large"
        :loading="busy"
        :icon="VideoPlay"
        class="pomo-fs__primary"
        @click="onStart"
      >
        开始
      </ElButton>
      <template v-else>
        <ElButton
          size="large"
          :type="phaseMeta.isPaused ? 'primary' : 'default'"
          :loading="busy"
          :icon="phaseMeta.isPaused ? VideoPlay : VideoPause"
          @click="onPauseOrResume"
        >
          {{ phaseMeta.isPaused ? '继续' : '暂停' }}
        </ElButton>
        <ElButton size="large" :loading="busy" :icon="Close" @click="onStop">停止</ElButton>
      </template>
    </div>

    <button
      type="button"
      class="pomo-fs__exit"
      title="退出全屏 (Esc)"
      @click="toggleFullscreen"
    >
      <ElIcon><FullScreen /></ElIcon>
    </button>

    <div class="pomo-fs__hint">按 <kbd>Esc</kbd> 退出全屏</div>
  </div>

  <!-- 紧凑模式：标准 360×480 widget，包 .window-root 挂 snap lean / preview class / SnapGhost
       与 ChatApp 同模型。全屏模式 v-if 分支不参与磁吸（toggleFullscreen 已主动 detach + visible=false）。 -->
  <div v-else class="window-root" :style="pomoLeanStyle">
    <SnapGhost source-label="pomodoro" />
    <AppShell
      variant="standalone"
      :class="pomoSnapPreviewClass"
      :style="pomoSnapPreviewStyle"
    >
    <template #header>
      <span class="pomo-shell__title" data-tauri-drag-region>番茄</span>
      <ElButton
        class="pomo-shell__btn"
        text
        :icon="FullScreen"
        title="全屏专注"
        @click="toggleFullscreen"
      />
      <ElPopover
        v-model:visible="popoverOpen"
        placement="bottom-end"
        :width="260"
        trigger="click"
        popper-class="pomo-popover"
        :show-arrow="true"
      >
        <template #reference>
          <ElButton
            class="pomo-shell__btn"
            text
            :icon="Setting"
            :disabled="phaseMeta.active"
            :title="phaseMeta.active ? '运行中无法调整时长' : '高级设置'"
          />
        </template>

        <div class="pomo-popover__inner">
          <div class="pomo-popover__group">
            <span class="pomo-popover__label">预设</span>
            <div class="pomo-popover__presets">
              <ElButton
                v-for="p in POMODORO_PRESETS"
                :key="p.id"
                size="small"
                @click="applyPreset(p)"
              >
                {{ p.label }}
              </ElButton>
            </div>
          </div>

          <div class="pomo-popover__group">
            <div class="pomo-popover__row">
              <span class="pomo-popover__label">专注时长</span>
              <span class="pomo-popover__value">{{ focusMinDraft }} 分钟</span>
            </div>
            <ElSlider
              v-model="focusMinDraft"
              :min="FOCUS_MIN_RANGE.min"
              :max="FOCUS_MIN_RANGE.max"
              :step="FOCUS_MIN_RANGE.step"
            />
          </div>

          <div class="pomo-popover__group">
            <div class="pomo-popover__row">
              <span class="pomo-popover__label">休息时长</span>
              <span class="pomo-popover__value">{{ restMinDraft }} 分钟</span>
            </div>
            <ElSlider
              v-model="restMinDraft"
              :min="REST_MIN_RANGE.min"
              :max="REST_MIN_RANGE.max"
              :step="REST_MIN_RANGE.step"
            />
          </div>

          <p class="pomo-popover__hint">运行中无法调整；下次启动生效。</p>
        </div>
      </ElPopover>
      <ElButton
        class="pomo-shell__btn pomo-shell__btn--close"
        text
        :icon="Close"
        title="关闭（计时继续在后台运行）"
        @click="onClose"
      />
    </template>

    <div class="pomo-window">
      <main class="pomo-main">
        <div class="pomo-ring">
          <ElProgress
            type="circle"
            :percentage="progressPct"
            :width="240"
            :stroke-width="8"
            :color="phaseMeta.color"
            :show-text="false"
          />
          <div class="pomo-ring__overlay">
            <span class="pomo-ring__time">{{ formatRemainingMs(displayRemainingMs) }}</span>
            <span class="pomo-ring__caption">
              <span class="pomo-ring__emoji">{{ phaseMeta.emoji }}</span>
              {{ displayCaption }}
            </span>
          </div>
        </div>
      </main>

      <section class="pomo-actions" data-no-drag>
        <ElButton
          v-if="!phaseMeta.active"
          type="primary"
          size="large"
          :loading="busy"
          :icon="VideoPlay"
          class="pomo-actions__primary"
          @click="onStart"
        >
          开始
        </ElButton>
        <template v-else>
          <ElButton
            size="large"
            :type="phaseMeta.isPaused ? 'primary' : 'default'"
            :loading="busy"
            :icon="phaseMeta.isPaused ? VideoPlay : VideoPause"
            class="pomo-actions__btn"
            @click="onPauseOrResume"
          >
            {{ phaseMeta.isPaused ? '继续' : '暂停' }}
          </ElButton>
          <ElButton
            size="large"
            :loading="busy"
            :icon="Close"
            class="pomo-actions__btn"
            @click="onStop"
          >
            停止
          </ElButton>
        </template>
      </section>

      <footer class="pomo-stats">
        <span class="pomo-stats__main">
          今日 <strong>{{ todaySummary.completed }}</strong>
          <template v-if="todaySummary.completed > 0">
            × {{ todaySummary.avg }} min
          </template>
        </span>
        <span v-if="todaySummary.cancelled > 0" class="pomo-stats__cancelled">
          · 放弃 {{ todaySummary.cancelled }}
        </span>
      </footer>
    </div>
    </AppShell>
  </div>
</template>

<style scoped>
/* ============================================================
 * 紧凑模式（360×480）
 * ============================================================ */

/* === window-root：磁吸 wrapper（lean transform / SnapGhost / preview-class anchor） ===
   pomodoro 是 opaque 窗（transparent:false / decorations:false），与 chat 透明窗不同：
   - 无圆角阴影 → 不需要 chat 那种 12px padding 让 box-shadow 露出
   - 整窗即 AppShell 全占 → width/height 100%
   - lean transform 用 chat 同款 160ms ease 过渡，磁吸吸引时细微贴附 ≤3px */
.window-root {
  width: 100%;
  height: 100%;
  background: transparent;
  transition: transform 160ms var(--aipet-ease-standard);
}

.pomo-shell__title {
  flex: 1 1 auto;
  font-size: var(--aipet-font-size-base);
  font-weight: 600;
  color: var(--aipet-color-text-1);
  cursor: default;
}

.pomo-shell__btn {
  color: var(--aipet-color-text-3);
}

.pomo-shell__btn:hover:not(.is-disabled) {
  color: var(--aipet-color-text-1);
}

/* === 主体 window === */
.pomo-window {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: var(--aipet-space-4);
  height: 100%;
  padding: var(--aipet-space-4) var(--aipet-space-5) var(--aipet-space-5);
}

/* === 圆环 + overlay（修倒计时显示 bug：脱离 EP default slot） === */
.pomo-main {
  flex: 1 1 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  padding-top: var(--aipet-space-2);
}

.pomo-ring {
  position: relative;
  width: 240px;
  height: 240px;
}

.pomo-ring__overlay {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  pointer-events: none;
}

.pomo-ring__time {
  font-size: 56px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--aipet-color-text-1);
  letter-spacing: -0.02em;
  line-height: 1.05;
}

.pomo-ring__caption {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 13px;
  color: var(--aipet-color-text-2);
}

.pomo-ring__emoji {
  font-size: 16px;
  line-height: 1;
}

.pomo-main :deep(.el-progress-circle__track) {
  stroke: color-mix(in srgb, var(--aipet-color-border) 70%, transparent);
}

/* === 主操作按钮区 === */
.pomo-actions {
  flex: 0 0 auto;
  display: flex;
  justify-content: center;
  gap: var(--aipet-space-3);
}

.pomo-actions__primary {
  min-width: 180px;
}

.pomo-actions__btn {
  min-width: 96px;
}

/* === 底部统计 === */
.pomo-stats {
  flex: 0 0 auto;
  display: flex;
  justify-content: center;
  align-items: baseline;
  gap: 4px;
  font-size: 12px;
  color: var(--aipet-color-text-3);
}

.pomo-stats__main strong {
  color: var(--aipet-color-text-1);
  font-weight: 600;
  font-size: 13px;
  margin: 0 2px;
}

.pomo-stats__cancelled {
  color: color-mix(in srgb, var(--aipet-color-primary) 40%, var(--aipet-color-text-3));
}

/* ============================================================
 * 全屏模式：极简覆盖层
 * ============================================================ */

.pomo-fs {
  position: fixed;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: clamp(var(--aipet-space-6), 4vh, var(--aipet-space-8));
  background: var(--aipet-color-bg);
  background-image:
    radial-gradient(
      circle at 50% 30%,
      color-mix(in srgb, var(--aipet-color-primary) 8%, transparent) 0,
      transparent 60%
    ),
    radial-gradient(
      circle at 1px 1px,
      color-mix(in srgb, var(--aipet-color-text-3) 18%, transparent) 1px,
      transparent 1px
    );
  background-size: auto, 28px 28px;
  color: var(--aipet-color-text-1);
  user-select: none;
  z-index: 9999;
}

.pomo-fs[data-phase='rest'] {
  background-image:
    radial-gradient(
      circle at 50% 30%,
      color-mix(in srgb, #22c55e 10%, transparent) 0,
      transparent 60%
    ),
    radial-gradient(
      circle at 1px 1px,
      color-mix(in srgb, var(--aipet-color-text-3) 18%, transparent) 1px,
      transparent 1px
    );
  background-size: auto, 28px 28px;
}

.pomo-fs__ring {
  position: relative;
  width: clamp(360px, 56vmin, 520px);
  aspect-ratio: 1;
}

.pomo-fs__ring :deep(.el-progress--circle) {
  width: 100% !important;
  height: 100% !important;
}

.pomo-fs__ring :deep(.el-progress--circle svg) {
  width: 100%;
  height: 100%;
}

.pomo-fs__ring :deep(.el-progress-circle__track) {
  stroke: color-mix(in srgb, var(--aipet-color-border) 60%, transparent);
}

.pomo-fs__overlay {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  pointer-events: none;
}

.pomo-fs__time {
  font-size: clamp(96px, 18vmin, 160px);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--aipet-color-text-1);
  letter-spacing: -0.03em;
  line-height: 1;
}

.pomo-fs__caption {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: clamp(16px, 2.4vmin, 22px);
  color: var(--aipet-color-text-2);
}

.pomo-fs__emoji {
  font-size: clamp(20px, 2.6vmin, 26px);
  line-height: 1;
}

.pomo-fs__actions {
  display: flex;
  gap: var(--aipet-space-4);
}

.pomo-fs__actions :deep(.el-button) {
  min-width: 140px;
  height: 48px;
  font-size: 16px;
}

.pomo-fs__primary {
  min-width: 200px !important;
}

.pomo-fs__exit {
  position: fixed;
  top: 16px;
  right: 16px;
  width: 36px;
  height: 36px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--aipet-color-surface) 80%, transparent);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
  color: var(--aipet-color-text-2);
  cursor: pointer;
  font-size: 16px;
  transition: color var(--aipet-duration-fast) var(--aipet-ease-standard),
    background-color var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.pomo-fs__exit:hover {
  color: var(--aipet-color-text-1);
  background: var(--aipet-color-surface);
}

.pomo-fs__hint {
  position: fixed;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%);
  font-size: 12px;
  color: var(--aipet-color-text-3);
  letter-spacing: 0.02em;
}

.pomo-fs__hint kbd {
  display: inline-block;
  padding: 1px 6px;
  margin: 0 2px;
  border: 1px solid var(--aipet-color-border);
  border-radius: 4px;
  background: var(--aipet-color-surface);
  color: var(--aipet-color-text-2);
  font-family: var(--aipet-font-family-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
  font-size: 11px;
  line-height: 1.4;
}
</style>
