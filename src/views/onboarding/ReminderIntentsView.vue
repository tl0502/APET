<script setup lang="ts">
// ReminderIntentsView：Onboarding Step 4 — 选默认提醒模板（flows §1.2）。
//
// 关键设计（#21 issue body 拍板）：
// - **M1 只存 KV**：用户选中的 id 写到 `onboarding:reminder_intents`（JSON array string）
// - **不实例化**：TaskService MVP 在 M2 启动期读 KV 批量建提醒（交接 Issue B / #22）
//   M1 不做假演示,避免与 M2 真番茄/真提醒打架
// - 默认勾选 water + sit_long（"主动陪伴"产品理念，让首启就有用）
// - "不需要"= 排他选项：勾上 → 其他自动取消；勾任意其他 → "不需要"自动取消
// - "用这些"= setMemory + emit('done')；空选或"不需要"= 保存 [] 也算正常推进
//
// KV 格式：JSON.stringify(string[])
//   存储位置：**memory 表**（不是 config 表）—— setMemory IPC 走 preferences::set,
//   memory 表是"用户偏好 KV"（含 source='user_set' / 'inferred'）;config 表是"系统运行时配置"
//   （窗口位置 / 当前 active conv 等）。验证时记得 `select * from memory where key=...`
//   'water' = 每 X 分钟提醒喝水
//   'sit_long' = 每 X 分钟提醒起身
//   'focus_study' = 早晚提醒规划学习专注
// 三个 id 与 M2 TaskService 的 intent 名约定对齐;未来扩选项时此处加一项 + M2 加 reminder
// 实例化映射。

import { computed, ref } from 'vue'
import { ElButton, ElCheckbox, type CheckboxValueType } from 'element-plus'
import { setMemory } from '@/services/memory'
import { useToast } from '@/composables/useToast'

const emit = defineEmits<{ done: [] }>()
const toast = useToast()

interface ReminderIntent {
  id: string
  emoji: string
  label: string
  hint: string
}

const INTENTS: readonly ReminderIntent[] = [
  { id: 'water', emoji: '💧', label: '喝水', hint: '每隔一会儿提醒你抿一口' },
  { id: 'sit_long', emoji: '🪑', label: '久坐起身', hint: '太久没动就喊你起来走走' },
  { id: 'focus_study', emoji: '📚', label: '学习专注', hint: '早晚帮你规划今天要啃什么' },
] as const

/** KV key — 与 #21 issue body 字面对齐；M2 TaskService 启动期 read 同名 key。 */
const KV_KEY = 'onboarding:reminder_intents'

/** 默认勾选：water + sit_long（首启就让用户有用的轻提醒）。 */
const selected = ref<Set<string>>(new Set(['water', 'sit_long']))
/** "不需要"独立 ref：与 INTENTS 排他互斥。 */
const noneChecked = ref(false)
const submitting = ref(false)

const buttonLabel = computed(() => {
  if (noneChecked.value) return '好,跳过'
  if (selected.value.size === 0) return '什么都不选,继续'
  return '用这些'
})

function toggleIntent(id: string, checked: boolean) {
  if (submitting.value) return
  if (checked) {
    selected.value.add(id)
    // 任选其一 → "不需要" 自动取消
    noneChecked.value = false
  } else {
    selected.value.delete(id)
  }
}

function toggleNone(checked: boolean) {
  if (submitting.value) return
  noneChecked.value = checked
  if (checked) {
    // "不需要" 排他：其他全清
    selected.value.clear()
  }
}

async function onConfirm() {
  if (submitting.value) return
  submitting.value = true
  try {
    const ids = noneChecked.value ? [] : Array.from(selected.value)
    // 顺序按 INTENTS 声明,稳定（便于后续 diff / 文档）
    const orderedIds = INTENTS.map((i) => i.id).filter((id) => ids.includes(id))
    await setMemory(KV_KEY, JSON.stringify(orderedIds))
    emit('done')
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    console.error('[ReminderIntentsView] setMemory failed:', e)
    toast.error(`保存失败：${msg}`, { duration: 5000 })
    submitting.value = false
  }
}
</script>

<template>
  <section
    class="reminder-intents"
    role="dialog"
    aria-modal="true"
    aria-labelledby="reminder-title"
  >
    <h1 id="reminder-title" class="reminder-intents__title">要我帮你盯哪些事?</h1>
    <p class="reminder-intents__hint">
      选了我就轻轻提醒,不催不闹。以后随时能关。
    </p>

    <ul class="intent-list" role="group" aria-label="提醒类型多选">
      <li v-for="intent in INTENTS" :key="intent.id" class="intent-item">
        <ElCheckbox
          :model-value="selected.has(intent.id)"
          :disabled="submitting"
          @change="(v: CheckboxValueType) => toggleIntent(intent.id, Boolean(v))"
        >
          <span class="intent-item__emoji">{{ intent.emoji }}</span>
          <span class="intent-item__label">{{ intent.label }}</span>
          <span class="intent-item__hint">{{ intent.hint }}</span>
        </ElCheckbox>
      </li>
      <li class="intent-item intent-item--none">
        <ElCheckbox
          :model-value="noneChecked"
          :disabled="submitting"
          @change="(v: CheckboxValueType) => toggleNone(Boolean(v))"
        >
          <span class="intent-item__label">我不需要</span>
        </ElCheckbox>
      </li>
    </ul>

    <p class="reminder-intents__footer-hint">
      (提醒功能 M2 上线后生效;现在我先记下你的偏好。)
    </p>

    <div class="reminder-intents__actions">
      <ElButton
        type="primary"
        :disabled="submitting"
        :loading="submitting"
        @click="onConfirm"
      >
        {{ buttonLabel }}
      </ElButton>
    </div>
  </section>
</template>

<style scoped>
.reminder-intents {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  width: 100%;
  height: 100%;
  padding: var(--aipet-space-6) var(--aipet-space-8) var(--aipet-space-8);
  background: var(--aipet-color-bg);
  box-sizing: border-box;
  user-select: none;
}

.reminder-intents__title {
  margin: 0 0 var(--aipet-space-2);
  font-size: var(--aipet-font-size-xl);
  font-weight: 600;
  color: var(--aipet-color-text-1);
  text-align: center;
}

.reminder-intents__hint {
  margin: 0 0 var(--aipet-space-5);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
  text-align: center;
}

.intent-list {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
  margin: 0 0 var(--aipet-space-4);
  padding: 0;
  list-style: none;
}

.intent-item {
  padding: var(--aipet-space-3) var(--aipet-space-4);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
  background: var(--aipet-color-surface);
}

.intent-item--none {
  border-style: dashed;
  background: transparent;
}

.intent-item__emoji {
  margin-right: var(--aipet-space-2);
  font-size: var(--aipet-font-size-lg);
}

.intent-item__label {
  font-size: var(--aipet-font-size-base);
  color: var(--aipet-color-text-1);
  font-weight: 500;
}

.intent-item__hint {
  margin-left: var(--aipet-space-2);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
}

.reminder-intents__footer-hint {
  margin: var(--aipet-space-2) 0 var(--aipet-space-4);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
  text-align: center;
  font-style: italic;
}

.reminder-intents__actions {
  display: flex;
  justify-content: center;
  margin-top: auto;
}

/* ElCheckbox label 内嵌的 emoji + label + hint 横排齐顶 */
:deep(.el-checkbox__label) {
  display: inline-flex;
  align-items: baseline;
  gap: 2px;
}
</style>
