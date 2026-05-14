<script setup lang="ts">
// VrmAvatarExporter（#26 增强）：实时预览 + 表情/缩放/俯仰调整 + 一键截图。
//
// 改造原因：v1 隐藏 canvas + 一击导出无法预览效果，用户无法调整角度。v2 做完整 UI：
// - 256×256 可见 canvas 持续渲染（RAF 循环走 VRMRuntime startLoop）
// - 右侧控制：6 个表情 radio + 缩放 slider + 上下 slider（垂直平移 lookAt 中心）
// - 调参实时反映到 runtime（setExpression / setCameraZoom / setCameraPan）
// - 截图按钮：复用 captureSnapshot（保留当前 expression / 镜头状态）
// - VRM 加载在 onMounted 自动起；持续到 unmount 才 destroy（便利于多次调参导出）
//
// B1 修复：settings 是 hide-not-close + ElTabPane v-show → exporter 一旦 mount 永远跑 RAF。
// 通过 inject 父级 activeTab + document.visibilityState 联合判断：tab 切走 / 窗口隐藏时
// pauseLoop，回来时 resumeLoop。VRM 资源保留不重 load（切换体验流畅）。
//
// 设计取舍：
// - 表情用 VRM 标准 emotion 6 选 1（VRM 1.0 spec）；模型未烘焙某些表情时 setValue 静默
//   no-op，UI 不报错（用户切换试用；首次启动 console 列出可用 expression 名方便排查）
// - zoom 滑块 0.5-2.0：覆盖头部特写到半身全景；步长 0.05 给足够精度
// - "上下" 是 lookAt 中心垂直平移（camera + target 同时偏移）, 不是相机俯仰角；UI 文案
//   仅"上下"避免误导（issue agent 提示：camera + target 同 delta = 平移不是 pitch）
// - preserveDrawingBuffer:true：截图可靠（同 v1）
// - H1 修复：renderer 有 setPixelRatio(devicePixelRatio)，setSize(512,512) 在 HiDPI 实际产生
//   512*dpr 的 PNG（爆 5MB）。截图前临时 setPixelRatio(1)，截图后恢复
import { computed, inject, onBeforeUnmount, onMounted, ref, watch, type Ref } from 'vue'
import {
  ElButton,
  ElIcon,
  ElRadio,
  ElRadioGroup,
  ElSlider,
} from 'element-plus'
import { Picture } from '@element-plus/icons-vue'
import { useToast } from '@/composables/useToast'
import { applyPersonaAvatar, removePersonaAvatar } from '@/services/avatars'
import { VRMRuntime } from '@/services/vrm'

interface Props {
  personaId: string | null
}
const props = defineProps<Props>()

const toast = useToast()

const PREVIEW_SIZE = 256
const SNAPSHOT_SIZE = 512 // 保持落盘高清，未来 retina 大头像也够
const MODEL_URL = '/avatar/avatar.vrm'

type EmotionName = 'neutral' | 'happy' | 'angry' | 'sad' | 'relaxed' | 'surprised'
const EMOTION_OPTIONS: { value: EmotionName; label: string }[] = [
  { value: 'neutral', label: '平静' },
  { value: 'happy', label: '开心' },
  { value: 'relaxed', label: '放松' },
  { value: 'surprised', label: '惊讶' },
  { value: 'sad', label: '失落' },
  { value: 'angry', label: '生气' },
]

const canvasRef = ref<HTMLCanvasElement | null>(null)
const loading = ref(true)
const loadError = ref<string | null>(null)
const exporting = ref(false)
const lastUrl = ref<string | null>(null)

// 调参状态（v-model 双向绑）
const emotion = ref<EmotionName>('neutral')
const zoom = ref(1) // 0.5 拉近 ~ 2 拉远
const panY = ref(0) // -0.3 下移 ~ 0.3 上移（lookAt 中心垂直平移）

// B1 修复：tab 切走 / 窗口隐藏时停 RAF
// inject 拿父级 activeTab；fallback 给 default 防独立测试 mount 时崩
const activeTabRef = inject<Ref<string>>('settings-active-tab', ref('persona'))
const isTabActive = computed(() => activeTabRef.value === 'persona')
const isPageVisible = ref(!document.hidden)
function onVisChange() {
  isPageVisible.value = !document.hidden
}
const shouldRun = computed(() => isTabActive.value && isPageVisible.value)

let runtime: VRMRuntime | null = null

onMounted(async () => {
  if (!canvasRef.value) return
  document.addEventListener('visibilitychange', onVisChange)

  runtime = new VRMRuntime()
  try {
    runtime.init(canvasRef.value, 'half', { preserveDrawingBuffer: true })
    await runtime.loadModel(MODEL_URL)
    // 首次应用初始状态
    runtime.setExpression(emotion.value, 1)
    runtime.setCameraZoom(zoom.value)
    runtime.setCameraPan(0, panY.value)
    // load 完成后立即按当前 shouldRun 决定是否 pause（处理"tab 没激活时挂载"边角）
    if (!shouldRun.value) runtime.pauseLoop()
    loading.value = false
  } catch (e) {
    console.error('[VrmAvatarExporter] init/load failed:', e)
    loadError.value = e instanceof Error ? e.message : String(e)
    loading.value = false
  }
})

onBeforeUnmount(() => {
  document.removeEventListener('visibilitychange', onVisChange)
  runtime?.destroy()
  runtime = null
})

// B1：tab 切换 / 窗口可见性变化 → pause/resume RAF（保留 VRM 资源不卸载）
watch(shouldRun, (run) => {
  if (!runtime) return
  if (run) runtime.resumeLoop()
  else runtime.pauseLoop()
})

// 调参 → 立即应用到 runtime（RAF 下一帧反映；pause 期间也写值，resume 后即生效）
watch(emotion, (v) => {
  runtime?.setExpression(v, 1)
})
watch(zoom, (v) => {
  runtime?.setCameraZoom(v)
})
watch(panY, (v) => {
  runtime?.setCameraPan(0, v)
})

async function onExport() {
  if (exporting.value || !runtime) return
  const personaId = props.personaId
  if (!personaId) {
    toast.error('未检测到当前激活人格')
    return
  }
  exporting.value = true
  try {
    // 截图前确保 expression + camera 是最新值（watch 是异步，万一未触发）
    runtime.setExpression(emotion.value, 1)
    runtime.setCameraZoom(zoom.value)
    runtime.setCameraPan(0, panY.value)
    // 等一帧让 watch 落到 runtime
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))

    // captureSnapshot 内部 resize 到 SNAPSHOT_SIZE，处理 DPR + 恢复（见 vrm.ts 注释）
    const dataUrl = runtime.captureSnapshot(SNAPSHOT_SIZE)

    const finalPath = await applyPersonaAvatar(personaId, dataUrl)
    lastUrl.value = dataUrl
    toast.success(`头像已导出，落盘于 ${finalPath}`)
  } catch (e) {
    console.error('[VrmAvatarExporter] export failed:', e)
    toast.error(`导出失败：${e instanceof Error ? e.message : String(e)}`)
  } finally {
    exporting.value = false
  }
}

async function onClear() {
  if (exporting.value) return
  const personaId = props.personaId
  if (!personaId) return
  try {
    await removePersonaAvatar(personaId)
    lastUrl.value = null
    toast.success('已清除自定义头像，回退到 momo 占位')
  } catch (e) {
    toast.error(`清除失败：${e instanceof Error ? e.message : String(e)}`)
  }
}

function onReset() {
  emotion.value = 'neutral'
  zoom.value = 1
  panY.value = 0
}
</script>

<template>
  <section class="vrm-exporter">
    <h3 class="vrm-exporter__title">人格头像</h3>
    <p class="vrm-exporter__hint">
      从当前 VRM 自动生成头像。调整表情和镜头到满意角度再点导出。
    </p>

    <div class="vrm-exporter__main">
      <div class="vrm-exporter__stage">
        <canvas
          ref="canvasRef"
          class="vrm-exporter__canvas"
          :width="PREVIEW_SIZE"
          :height="PREVIEW_SIZE"
        ></canvas>
        <div v-if="loading" class="vrm-exporter__stage-overlay">加载 VRM 中…</div>
        <div v-else-if="loadError" class="vrm-exporter__stage-overlay vrm-exporter__stage-overlay--error">
          VRM 加载失败：{{ loadError }}
        </div>
      </div>

      <div class="vrm-exporter__controls">
        <div class="vrm-exporter__field">
          <label class="vrm-exporter__label">表情</label>
          <ElRadioGroup v-model="emotion" :disabled="loading || !!loadError">
            <ElRadio v-for="opt in EMOTION_OPTIONS" :key="opt.value" :value="opt.value">
              {{ opt.label }}
            </ElRadio>
          </ElRadioGroup>
        </div>

        <div class="vrm-exporter__field">
          <label class="vrm-exporter__label">
            缩放
            <span class="vrm-exporter__value">{{ zoom.toFixed(2) }}×</span>
          </label>
          <ElSlider
            v-model="zoom"
            :min="0.5"
            :max="2.0"
            :step="0.05"
            :disabled="loading || !!loadError"
          />
        </div>

        <div class="vrm-exporter__field">
          <label class="vrm-exporter__label">
            上下
            <span class="vrm-exporter__value">{{ panY > 0 ? '+' : '' }}{{ panY.toFixed(2) }}m</span>
          </label>
          <ElSlider
            v-model="panY"
            :min="-0.3"
            :max="0.3"
            :step="0.02"
            :disabled="loading || !!loadError"
          />
        </div>

        <div class="vrm-exporter__actions">
          <ElButton
            type="primary"
            :loading="exporting"
            :disabled="!personaId || loading || !!loadError"
            @click="onExport"
          >
            <ElIcon><Picture /></ElIcon>
            <span style="margin-left: 4px">{{ lastUrl ? '重新导出' : '导出头像' }}</span>
          </ElButton>
          <ElButton :disabled="exporting || loading" @click="onReset">还原</ElButton>
          <ElButton :disabled="exporting || !personaId" @click="onClear">清除</ElButton>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.vrm-exporter {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
  margin-top: var(--aipet-space-4);
  padding-top: var(--aipet-space-4);
  border-top: 1px solid var(--aipet-color-border);
}
.vrm-exporter__title {
  margin: 0;
  font-size: var(--aipet-font-size-base);
  font-weight: 600;
  color: var(--aipet-color-text-2);
}
.vrm-exporter__hint {
  margin: 0;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
  line-height: var(--aipet-line-height-base);
}

.vrm-exporter__main {
  display: flex;
  gap: var(--aipet-space-4);
  align-items: flex-start;
}

/* canvas stage:固定 PREVIEW_SIZE 边长;border + 圆角 + 浅底,让 alpha 透明背景下角色清晰可见 */
.vrm-exporter__stage {
  position: relative;
  flex: 0 0 auto;
  width: 256px;
  height: 256px;
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
  background: var(--aipet-color-surface-soft);
  overflow: hidden;
}
.vrm-exporter__canvas {
  display: block;
  width: 256px;
  height: 256px;
}
.vrm-exporter__stage-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
  background: var(--aipet-color-surface);
  text-align: center;
  padding: var(--aipet-space-2);
}
.vrm-exporter__stage-overlay--error {
  color: var(--aipet-color-danger);
}

/* 右侧控制面板:flex 1 撑开,字段间距均匀 */
.vrm-exporter__controls {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
}
.vrm-exporter__field {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-1);
}
.vrm-exporter__label {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
}
.vrm-exporter__value {
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
  font-variant-numeric: tabular-nums;
}
.vrm-exporter__actions {
  display: flex;
  gap: var(--aipet-space-2);
  margin-top: var(--aipet-space-2);
}

/* 表情 radio:横向 wrap,密集排布 */
.vrm-exporter__field :deep(.el-radio-group) {
  display: flex;
  flex-wrap: wrap;
  gap: var(--aipet-space-2);
}
.vrm-exporter__field :deep(.el-radio) {
  margin-right: 0;
}
</style>
