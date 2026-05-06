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
  font-size: 12px;
  color: #555;
  background: rgba(255, 255, 255, 0.9);
  padding: 4px 12px;
  border-radius: 6px;
  text-align: center;
  pointer-events: none;
  white-space: nowrap;
}

.hint-error {
  background: rgba(255, 220, 220, 0.95);
  color: #722;
  font-size: 10px;
  max-width: 290px;
  white-space: normal;
  line-height: 1.4;
}

.hint-error code {
  background: rgba(0, 0, 0, 0.07);
  padding: 1px 4px;
  border-radius: 3px;
  font-family: ui-monospace, monospace;
}
</style>
