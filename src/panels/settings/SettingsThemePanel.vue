<script setup lang="ts">
// 外观 tab（M1：主题三选一；#24：增「角色窗 → 视角」半身/全身）。
// 主题：三选一 radio 直接驱动 useThemeStore.setMode；store 内 storage event listener 让 pet 窗同步。
// 视角：onMounted 读 KV 初始化；onChange 调 setPetViewPreset（后端 原子写 KV + setSize + clamp 位置 + emit 事件）。
// dev 模式下露出 token 预览页地址（pet 窗 ?view=tokens 路由），便于设计核对。
import { onMounted, ref } from 'vue'
import { ElRadio, ElRadioGroup } from 'element-plus'
import { useThemeStore } from '@/stores/theme'
import type { ThemeMode } from '@/stores/theme'
import { getPetViewPreset, setPetViewPreset } from '@/services/window'
import type { PetViewPreset } from '@/services/window'
import { useToast } from '@/composables/useToast'

const theme = useThemeStore()
const isDev = import.meta.env.DEV
const toast = useToast()

const viewPreset = ref<PetViewPreset>('half')
const viewSwitching = ref(false)

onMounted(async () => {
  // 读 KV 当前值（无值后端默认返 'half'）；失败留 'half' 默认值不阻断 tab 渲染。
  try {
    viewPreset.value = await getPetViewPreset()
  } catch (e) {
    console.warn('[ThemePanel] getPetViewPreset failed, fallback half:', e)
  }
})

function onThemeChange(value: ThemeMode | string | number | boolean | undefined) {
  if (value === 'auto' || value === 'light' || value === 'dark') {
    theme.setMode(value)
  }
}

async function onViewChange(value: PetViewPreset | string | number | boolean | undefined) {
  if (value !== 'half' && value !== 'full') return
  // 乐观更新 UI，失败回滚 + toast。setPetViewPreset 后端做 setSize + clamp 位置 + emit。
  const prev = viewPreset.value
  viewPreset.value = value
  viewSwitching.value = true
  try {
    await setPetViewPreset(value)
  } catch (e) {
    viewPreset.value = prev
    toast.error(`切换视角失败：${msgOf(e)}`)
  } finally {
    viewSwitching.value = false
  }
}

function msgOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}
</script>

<template>
  <section class="panel panel--form">
    <h2 class="panel__title">外观</h2>
    <div class="panel__content">
      <p class="panel__hint">
        切换会同步到桌宠窗口（与未来的 onboarding / hub 窗口）；选择持久化到本地。
      </p>

      <div class="panel__section">
        <h3 class="panel__subtitle">主题</h3>
        <ElRadioGroup :model-value="theme.mode" @change="onThemeChange">
          <ElRadio value="auto">跟随系统（当前：{{ theme.systemDark ? '暗色' : '亮色' }}）</ElRadio>
          <ElRadio value="light">亮色</ElRadio>
          <ElRadio value="dark">暗色</ElRadio>
        </ElRadioGroup>
      </div>

      <div class="panel__section">
        <h3 class="panel__subtitle">角色窗 · 视角</h3>
        <p class="panel__hint">
          切换会即时改变 pet 窗口尺寸与相机取景；位置自动按新尺寸 clamp 到屏内安全边距。
        </p>
        <ElRadioGroup
          :model-value="viewPreset"
          :disabled="viewSwitching"
          @change="onViewChange"
        >
          <ElRadio value="half">半身（320×320，胸口以上）</ElRadio>
          <ElRadio value="full">全身（320×512，1:1.6）</ElRadio>
        </ElRadioGroup>
      </div>

      <div v-if="isDev" class="panel__dev">
        <h3 class="panel__subtitle">开发工具</h3>
        <p class="panel__hint">
          在浏览器或新 webview 中访问
          <code>http://localhost:1420/?view=tokens</code>
          可查看 token 视觉对照页（仅 dev 模式可用）。
        </p>
      </div>
    </div>
  </section>
</template>

<style scoped>
.panel__dev {
  margin-top: var(--aipet-space-2);
  padding: var(--aipet-space-3) var(--aipet-space-4);
  border: 1px dashed var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
  background: var(--aipet-color-surface);
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}
code {
  padding: 0 var(--aipet-space-1);
  border-radius: var(--aipet-radius-sm);
  background: var(--aipet-color-surface-raised);
  color: var(--aipet-color-text-2);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
}
</style>
