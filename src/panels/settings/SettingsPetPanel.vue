<script setup lang="ts">
// 桌宠运行行为面板（#41）— mood_icon / energy / free_movement 3 独立 toggle + energy 实时显示。
//
// ## 设计
// - 每项 toggle 调 mood store.setFeatureDisabled（IPC + 失败回滚）
// - energy 值用 mood store.energy（1s polling）；若 energy 关闭仍正常 polling 但 UI 不显示
// - free_movement 关闭后 Rust LivingPet scheduler 立即生效（每次 wake 查 KV）
// - mood_icon 关闭后 PetCanvas 内 MoodIcon v-if 立即隐藏（store.isMoodIconEnabled 计算属性）

import { onMounted } from 'vue'
import { ElSwitch } from 'element-plus'
import { useMoodStore } from '@/stores/mood'
import type { DisableableFeature } from '@/services/mood'
import { MOOD_EMOJI, MOOD_LABEL } from '@/services/mood'
import { useToast } from '@/composables/useToast'

const store = useMoodStore()
const toast = useToast()

onMounted(async () => {
  await store.loadDisabledFeatures()
  store.startPolling()
})

async function onToggle(feature: DisableableFeature, enabled: boolean) {
  try {
    await store.setFeatureDisabled(feature, !enabled)
  } catch (e) {
    toast.error(`保存失败：${e instanceof Error ? e.message : String(e)}`)
  }
}
</script>

<template>
  <section class="panel panel--form">
    <h2 class="panel__title">桌宠运行行为</h2>
    <div class="panel__content">
      <p class="panel__hint">
        关闭对应功能可让桌宠更安静（关心提醒、专注模式等其他能力不受影响）。设置跨重启持久。
      </p>

      <!-- 当前 mood / energy 实时显示（用户对照"为什么我桌宠现在是这个表情"） -->
      <div class="panel__section panel__section--readonly">
        <h3 class="panel__subtitle">当前状态</h3>
        <dl class="status-grid">
          <div class="status-row">
            <dt>心情</dt>
            <dd>
              <span class="status-emoji">{{ MOOD_EMOJI[store.mood] || '😐' }}</span>
              <span class="status-label">{{ MOOD_LABEL[store.mood] }}</span>
            </dd>
          </div>
          <div class="status-row">
            <dt>精力</dt>
            <dd>
              <span class="energy-bar" :title="`${store.energy} / 100`">
                <span class="energy-bar__fill" :style="{ width: `${store.energy}%` }" />
              </span>
              <span class="status-label">{{ store.energy }} / 100</span>
            </dd>
          </div>
        </dl>
        <p class="panel__caption">
          心情与精力都是临时状态，不跨重启保存；启动时精力恢复到 80，5 分钟没人理就会慢慢下降。
        </p>
      </div>

      <div class="panel__section">
        <h3 class="panel__subtitle">显示</h3>

        <div class="toggle-row">
          <div class="toggle-row__main">
            <div class="toggle-row__title">心情图标</div>
            <div class="toggle-row__hint">关闭后桌宠左上角不再显示心情 emoji</div>
          </div>
          <ElSwitch
            :model-value="store.isMoodIconEnabled"
            :disabled="!store.loaded"
            @update:model-value="(v) => onToggle('mood_icon', Boolean(v))"
          />
        </div>

        <div class="toggle-row">
          <div class="toggle-row__main">
            <div class="toggle-row__title">精力显示</div>
            <div class="toggle-row__hint">
              关闭后本面板不再显示精力进度条（精力本身仍在后台运作，自由活动仍受其影响）
            </div>
          </div>
          <ElSwitch
            :model-value="store.isEnergyEnabled"
            :disabled="!store.loaded"
            @update:model-value="(v) => onToggle('energy', Boolean(v))"
          />
        </div>
      </div>

      <div class="panel__section">
        <h3 class="panel__subtitle">行为</h3>

        <div class="toggle-row">
          <div class="toggle-row__main">
            <div class="toggle-row__title">自由活动</div>
            <div class="toggle-row__hint">
              关闭后桌宠保持原地不动（5-15 分钟随机走动行为被禁用）
            </div>
          </div>
          <ElSwitch
            :model-value="store.isFreeMovementEnabled"
            :disabled="!store.loaded"
            @update:model-value="(v) => onToggle('free_movement', Boolean(v))"
          />
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.panel__section--readonly {
  background: var(--aipet-color-surface-soft);
  border-radius: var(--aipet-radius-base);
  padding: var(--aipet-space-3) var(--aipet-space-4);
}

.status-grid {
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}

.status-row {
  display: grid;
  grid-template-columns: 64px 1fr;
  align-items: center;
  gap: var(--aipet-space-3);
}

.status-row dt {
  font-size: 13px;
  color: var(--aipet-color-text-2);
  margin: 0;
}

.status-row dd {
  margin: 0;
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
}

.status-emoji {
  font-size: 20px;
  line-height: 1;
}

.status-label {
  font-size: 13px;
  color: var(--aipet-color-text-1);
}

.energy-bar {
  display: inline-block;
  width: 160px;
  height: 8px;
  background: var(--aipet-color-border-faint);
  border-radius: 4px;
  overflow: hidden;
}

.energy-bar__fill {
  display: block;
  height: 100%;
  background: var(--aipet-color-primary);
  transition: width 240ms var(--aipet-ease-standard);
}

.panel__caption {
  margin: var(--aipet-space-2) 0 0;
  font-size: 12px;
  color: var(--aipet-color-text-3);
  line-height: 1.5;
}

.toggle-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--aipet-space-3);
  padding: var(--aipet-space-3) 0;
  border-bottom: 1px solid var(--aipet-color-border-faint);
}

.toggle-row:last-child {
  border-bottom: none;
}

.toggle-row__main {
  flex: 1 1 auto;
  min-width: 0;
}

.toggle-row__title {
  font-size: 14px;
  color: var(--aipet-color-text-1);
  font-weight: 500;
}

.toggle-row__hint {
  margin-top: 2px;
  font-size: 12px;
  color: var(--aipet-color-text-3);
  line-height: 1.5;
}
</style>
