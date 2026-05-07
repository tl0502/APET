<script setup lang="ts">
import { computed, ref } from 'vue'
import { ElButton } from 'element-plus'
import { useThemeStore } from '@/stores/theme'
import StandardDialog from '@/components/feedback/StandardDialog.vue'
import { useToast } from '@/composables/useToast'

const theme = useThemeStore()
const toast = useToast()

const dialogVisible = ref(false)
const dialogLoading = ref(false)

const colors = [
  ['primary', 'color-primary'],
  ['bg', 'color-bg'],
  ['surface', 'color-surface'],
  ['surface raised', 'color-surface-raised'],
  ['text 1', 'color-text-1'],
  ['text 2', 'color-text-2'],
  ['text 3', 'color-text-3'],
  ['border', 'color-border'],
  ['success', 'color-success'],
  ['warning', 'color-warning'],
  ['danger', 'color-danger'],
  ['overlay', 'color-overlay'],
]

const spacings = ['1', '2', '3', '4', '6', '8', '12', '16']
const fontSizes = ['xs', 'sm', 'base', 'lg', 'xl']
const radii = ['sm', 'base', 'lg', 'full']
const shadows = ['sm', 'base', 'lg', 'float']
const modes = ['auto', 'light', 'dark'] as const

const modeLabel = computed(() => `${theme.mode} / ${theme.isDark ? 'dark' : 'light'}`)
</script>

<template>
  <main class="tokens-preview">
    <header class="preview-header">
      <div>
        <p class="eyebrow">AIPET visual tokens</p>
        <h1>Token Preview</h1>
        <p class="summary">Current mode: {{ modeLabel }}</p>
      </div>
      <div class="mode-switcher">
        <button
          v-for="mode in modes"
          :key="mode"
          :class="['mode-button', { active: theme.mode === mode }]"
          type="button"
          @click="theme.setMode(mode)"
        >
          {{ mode }}
        </button>
      </div>
    </header>

    <section class="preview-section">
      <h2>Color</h2>
      <div class="color-grid">
        <div v-for="[label, token] in colors" :key="token" class="color-card">
          <div class="color-swatch" :style="{ background: `var(--aipet-${token})` }"></div>
          <span>{{ label }}</span>
          <code>--aipet-{{ token }}</code>
        </div>
      </div>
    </section>

    <section class="preview-section">
      <h2>Spacing</h2>
      <div class="stack-list">
        <div v-for="space in spacings" :key="space" class="space-row">
          <code>space-{{ space }}</code>
          <div class="space-bar" :style="{ width: `var(--aipet-space-${space})` }"></div>
        </div>
      </div>
    </section>

    <section class="preview-section">
      <h2>Typography</h2>
      <p
        v-for="size in fontSizes"
        :key="size"
        class="type-sample"
        :style="{ fontSize: `var(--aipet-font-size-${size})` }"
      >
        font-size-{{ size }}：默默 momo 正在检查主题 token。
      </p>
    </section>

    <section class="preview-section grid-two">
      <div>
        <h2>Radius</h2>
        <div class="shape-row">
          <div
            v-for="radius in radii"
            :key="radius"
            class="shape-card"
            :style="{ borderRadius: `var(--aipet-radius-${radius})` }"
          >
            {{ radius }}
          </div>
        </div>
      </div>
      <div>
        <h2>Shadow</h2>
        <div class="shape-row">
          <div
            v-for="shadow in shadows"
            :key="shadow"
            class="shape-card"
            :style="{ boxShadow: `var(--aipet-shadow-${shadow})` }"
          >
            {{ shadow }}
          </div>
        </div>
      </div>
    </section>

    <section class="preview-section">
      <h2>Motion</h2>
      <button class="motion-card" type="button">Hover me</button>
    </section>

    <section class="preview-section">
      <h2>Components</h2>
      <p class="section-hint">issue #8 三件公共件：Toast / StandardDialog / AppShell。</p>
      <div class="components-row">
        <ElButton @click="toast.success('保存成功')">Toast Success</ElButton>
        <ElButton
          @click="
            toast.error('请先填写 API Key', {
              action: { text: '去设置', handler: () => toast.info('跳到设置...') },
            })
          "
        >
          Toast Error w/ Action
        </ElButton>
        <ElButton @click="toast.info('信息提示')">Toast Info</ElButton>
        <ElButton @click="toast.warn('谨慎操作')">Toast Warn</ElButton>
        <ElButton type="primary" @click="dialogVisible = true">Open Dialog</ElButton>
        <ElButton @click="dialogLoading = !dialogLoading">
          Toggle Dialog Loading ({{ dialogLoading ? 'on' : 'off' }})
        </ElButton>
      </div>
      <StandardDialog
        v-model="dialogVisible"
        title="标准弹窗示例"
        :width="480"
        :loading="dialogLoading"
      >
        <p>正文：演示 ESC / 遮罩 / 关闭按钮三条关闭路径。</p>
        <p>切换 Loading 可看 spinner overlay 与 footer 屏蔽（防误关）。</p>
        <template #footer>
          <ElButton @click="dialogVisible = false">取消</ElButton>
          <ElButton type="primary" @click="dialogVisible = false">确定</ElButton>
        </template>
      </StandardDialog>
    </section>
  </main>
</template>

<style scoped>
.tokens-preview {
  height: 100vh;
  overflow-y: auto;
  padding: var(--aipet-space-8);
  color: var(--aipet-color-text-1);
  background: var(--aipet-color-bg);
  font-family: var(--aipet-font-family-base);
  line-height: var(--aipet-line-height-base);
}

.preview-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--aipet-space-4);
  margin-bottom: var(--aipet-space-8);
}

.eyebrow,
.summary {
  margin: 0;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
}

h1,
h2 {
  margin: 0;
  line-height: var(--aipet-line-height-tight);
}

h1 {
  margin-top: var(--aipet-space-1);
  font-size: 32px;
}

h2 {
  margin-bottom: var(--aipet-space-4);
  font-size: var(--aipet-font-size-xl);
}

.preview-section {
  margin-bottom: var(--aipet-space-8);
  padding: var(--aipet-space-6);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-lg);
  background: var(--aipet-color-surface);
  box-shadow: var(--aipet-shadow-sm);
}

.mode-switcher,
.color-grid,
.shape-row {
  display: flex;
  flex-wrap: wrap;
  gap: var(--aipet-space-3);
}

.mode-button,
.motion-card {
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-full);
  background: var(--aipet-color-surface-raised);
  color: var(--aipet-color-text-1);
  font: inherit;
  cursor: pointer;
  transition:
    transform var(--aipet-duration-fast) var(--aipet-ease-standard),
    background var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.mode-button {
  padding: var(--aipet-space-2) var(--aipet-space-4);
}

.mode-button.active {
  background: var(--aipet-color-primary);
  color: var(--aipet-color-surface-raised);
}

.color-card {
  width: 160px;
  padding: var(--aipet-space-3);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
  background: var(--aipet-color-surface-raised);
}

.color-swatch {
  height: 56px;
  margin-bottom: var(--aipet-space-2);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
}

.color-card span,
.color-card code,
.space-row code {
  display: block;
}

code {
  color: var(--aipet-color-text-3);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
}

.stack-list {
  display: grid;
  gap: var(--aipet-space-2);
}

.space-row {
  display: grid;
  grid-template-columns: 96px 1fr;
  align-items: center;
  gap: var(--aipet-space-3);
}

.space-bar {
  height: var(--aipet-space-3);
  border-radius: var(--aipet-radius-full);
  background: var(--aipet-color-primary);
}

.type-sample {
  margin: 0 0 var(--aipet-space-2);
}

.grid-two {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--aipet-space-6);
}

.shape-card {
  display: grid;
  width: 92px;
  height: 64px;
  place-items: center;
  border: 1px solid var(--aipet-color-border);
  background: var(--aipet-color-surface-raised);
  color: var(--aipet-color-text-2);
  font-size: var(--aipet-font-size-sm);
}

.motion-card {
  padding: var(--aipet-space-3) var(--aipet-space-6);
}

.motion-card:hover,
.mode-button:hover {
  transform: translateY(-1px);
  background: var(--aipet-color-primary);
  color: var(--aipet-color-surface-raised);
}

.section-hint {
  margin: 0 0 var(--aipet-space-3);
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
}

.components-row {
  display: flex;
  flex-wrap: wrap;
  gap: var(--aipet-space-2);
}
</style>
