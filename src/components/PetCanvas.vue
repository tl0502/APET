<script setup lang="ts">
// PetCanvas：320×320 透明角色窗主画面（PRD §7.2 角色窗）。
// M1 spike 阶段仅渲染 VRM；hitbox 上报 / 拖动 / IPC 推到后续 task（A.6 / N 模块）。
import { ref } from 'vue'
import { useVRMModel } from '@/composables/useVRMModel'

const canvasRef = ref<HTMLCanvasElement | null>(null)

// public/avatar/avatar.vrm 由 Vite static serve；用户私有，.gitignore 已忽略。
const MODEL_URL = '/avatar/avatar.vrm'

const { isLoaded, errorMessage } = useVRMModel(canvasRef, MODEL_URL)
</script>

<template>
  <div class="pet-stage">
    <canvas ref="canvasRef" class="pet-canvas" width="320" height="320"></canvas>
    <div v-if="!isLoaded && !errorMessage" class="hint">Loading VRM…</div>
    <div v-else-if="errorMessage" class="hint hint-error">
      VRM 加载失败：{{ errorMessage }}<br />
      请把一个 .vrm 文件放在 <code>public/avatar/avatar.vrm</code>
    </div>
  </div>
</template>

<style scoped>
.pet-stage {
  position: relative;
  width: 320px;
  height: 320px;
}

.pet-canvas {
  display: block;
  width: 320px;
  height: 320px;
}

.hint {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
  background: var(--aipet-color-surface);
  padding: var(--aipet-space-1) var(--aipet-space-3);
  border-radius: var(--aipet-radius-base);
  box-shadow: var(--aipet-shadow-sm);
  text-align: center;
  pointer-events: none;
  white-space: nowrap;
}

.hint-error {
  background: var(--aipet-color-error-surface);
  color: var(--aipet-color-danger);
  font-size: var(--aipet-font-size-xs);
  max-width: 290px;
  white-space: normal;
  line-height: var(--aipet-line-height-base);
}

.hint-error code {
  background: var(--aipet-color-code-bg);
  padding: 1px var(--aipet-space-1);
  border-radius: var(--aipet-radius-sm);
  font-family: var(--aipet-font-family-mono);
}
</style>
