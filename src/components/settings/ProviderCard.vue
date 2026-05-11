<script setup lang="ts">
// ProviderCard：单条 provider 卡片（cc-switch 风格）。
//
// 视觉：
// - 左：圆形 logo（首字母 + 主题色块；按 name 第一个字符）
// - 中：name (主) + base_url (副，ellipsis) + model (副，灰)
// - 右：radio（视觉上当 active=checked 显示）+ "当前生效" tag（active 时露） + 编辑/删除按钮
//
// 交互：
// - 整行点击 → 激活该项（emit 'activate'）；流式中可禁用
// - 编辑按钮 → emit 'edit'（父组件打开 drawer）
// - 删除按钮 → ElPopconfirm 二次确认 → emit 'delete'
//   后端会拦激活的（CannotDeleteActive），前端 disabled 删除按钮额外保险
import { computed } from 'vue'
import { ElButton, ElIcon, ElPopconfirm, ElRadio, ElTag } from 'element-plus'
import { Delete, Edit } from '@element-plus/icons-vue'
import type { ProviderListItem } from '@/types/llm_providers'

interface Props {
  provider: ProviderListItem
  /** 当前激活 id（用于 ElRadio :model-value 让整行受控）。 */
  activeId: string | null
  /** 流式中或保存中等场景禁用所有交互。 */
  disabled?: boolean
}

const props = withDefaults(defineProps<Props>(), { disabled: false })
const emit = defineEmits<{ activate: [string]; edit: [string]; delete: [string] }>()

const initial = computed(() => {
  const ch = [...(props.provider.name || '?')][0] ?? '?'
  return ch.toUpperCase()
})

// 用 name 哈希到 6 个色调中的一个，让多 provider 视觉可区分
const COLOR_HUES = [262, 200, 30, 130, 340, 50]
const hue = computed(() => {
  let sum = 0
  for (const ch of props.provider.name) {
    sum = (sum + ch.charCodeAt(0)) % 1000
  }
  return COLOR_HUES[sum % COLOR_HUES.length]
})

const logoStyle = computed(() => ({
  background: `hsl(${hue.value}, 70%, 55%)`,
}))

function onCardClick() {
  if (props.disabled) return
  if (props.provider.isActive) return
  emit('activate', props.provider.id)
}

function onEdit(e: Event) {
  e.stopPropagation()
  if (props.disabled) return
  emit('edit', props.provider.id)
}

function onDeleteConfirm() {
  emit('delete', props.provider.id)
}
</script>

<template>
  <div
    class="provider-card"
    :class="{
      'provider-card--active': provider.isActive,
      'provider-card--disabled': disabled,
    }"
    @click="onCardClick"
  >
    <ElRadio
      :model-value="activeId ?? undefined"
      :value="provider.id"
      :disabled="disabled"
      class="provider-card__radio"
      @click.stop
      @change="onCardClick"
    >
      <template #default><span /></template>
    </ElRadio>

    <div class="provider-card__logo" :style="logoStyle">{{ initial }}</div>

    <div class="provider-card__main">
      <div class="provider-card__row">
        <span class="provider-card__name">{{ provider.name }}</span>
        <ElTag v-if="provider.isActive" size="small" type="success" effect="light">当前生效</ElTag>
        <ElTag v-if="!provider.hasApiKey" size="small" type="warning" effect="plain">缺 API Key</ElTag>
      </div>
      <div class="provider-card__url" :title="provider.baseUrl">{{ provider.baseUrl }}</div>
      <div class="provider-card__model">{{ provider.model }}</div>
    </div>

    <div class="provider-card__actions" @click.stop>
      <ElButton link :disabled="disabled" :icon="Edit" @click="onEdit" />
      <ElPopconfirm
        title="删除此 Provider？"
        confirm-button-text="删除"
        cancel-button-text="取消"
        confirm-button-type="danger"
        :disabled="disabled || provider.isActive"
        @confirm="onDeleteConfirm"
      >
        <template #reference>
          <ElButton
            link
            :disabled="disabled || provider.isActive"
            :title="provider.isActive ? '激活的不能删除，请先切换' : ''"
          >
            <ElIcon><Delete /></ElIcon>
          </ElButton>
        </template>
      </ElPopconfirm>
    </div>
  </div>
</template>

<style scoped>
.provider-card {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-3);
  padding: var(--aipet-space-3);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
  background: var(--aipet-color-surface);
  cursor: pointer;
  transition: border-color var(--aipet-duration-fast) var(--aipet-ease-standard),
    background var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.provider-card:hover {
  border-color: var(--aipet-color-primary);
}

.provider-card--active {
  border-color: var(--aipet-color-primary);
  background: var(--aipet-color-surface-raised);
  cursor: default;
}

.provider-card--disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.provider-card__radio {
  flex: 0 0 auto;
  margin: 0;
  /* 隐藏默认 label 占位，仅保留 radio 圆点 */
  height: 18px;
}
.provider-card__radio :deep(.el-radio__label) {
  display: none;
}

.provider-card__logo {
  flex: 0 0 auto;
  width: 40px;
  height: 40px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  font-size: var(--aipet-font-size-lg);
  font-weight: 700;
  user-select: none;
}

.provider-card__main {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.provider-card__row {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
}

.provider-card__name {
  font-size: var(--aipet-font-size-base);
  font-weight: 600;
  color: var(--aipet-color-text-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.provider-card__url {
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
  font-family: var(--aipet-font-family-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.provider-card__model {
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}

.provider-card__actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: var(--aipet-space-1);
}
</style>
