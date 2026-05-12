<script setup lang="ts">
// ReminderIntentsView：Onboarding Step 4 — 选默认提醒模板（flows §1.2）。
//
// 关键设计（#21 issue body 拍板）：
// - **M1 只存 KV**：用户的选择写到 `onboarding:reminder_intents`（JSON 字符串）
// - **不实例化**：TaskService MVP 在 M2 启动期读 KV 批量建提醒（交接 Issue B / #22）
//   M1 不做假演示,避免与 M2 真番茄/真提醒打架
// - 默认勾选 water + sit_long（"主动陪伴"产品理念，让首启就有用）
// - "不需要"= 排他选项：勾上 → 其他自动取消；勾任意其他 → "不需要"自动取消
// - onMounted 读 KV 回填：支持"重做 onboarding"+ finalizeOnboarding 失败回退场景
//
// KV 三态约定（M1↔M2 契约）：
//   getMemory(key) 返 null  → 第一次没走过 onboarding（首启默认 water+sit_long）
//   value = "null"          → 用户明确"我不需要"（M2 不实例化任何提醒）
//   value = "[]"            → 用户走完但全空（中间态；M2 可忽略或重提示）
//   value = '["water",...]' → 正常勾选（M2 按 id 实例化）
//   M2 读取一律 `parsed = raw ? JSON.parse(raw) : undefined`,按上述四态分支。
//
// 存储位置：**memory 表**（不是 config 表）。调用链：
//   前端 setMemory → IPC `memory_set`（commands/memory.rs）
//   → 后端 services::preferences::set → INSERT/UPSERT memory 表
//   memory 表是"用户偏好 KV"（含 source='user_set' / 'inferred'）;config 表是"系统
//   运行时配置"（窗口位置 / 当前 active conv 等）。验证查
//   `select * from memory where key='onboarding:reminder_intents'`。
//
// intent id 与 M2 TaskService 约定对齐：
//   'water'       = 每 X 分钟提醒喝水
//   'sit_long'    = 每 X 分钟提醒起身
//   'focus_study' = 早晚提醒规划学习专注
// 扩选项时此处加一项 + M2 加 reminder 实例化映射。

import { computed, onMounted, ref } from 'vue'
import { ElButton, ElCheckbox, type CheckboxValueType } from 'element-plus'
import { getMemory, setMemory } from '@/services/memory'
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
/** onMounted 读 KV 回填期间禁交互,防止用户在闪烁中误点。 */
const initializing = ref(true)
const submitting = ref(false)
/** 交互禁用条件:KV 回填中 OR 正在提交。 */
const isLocked = computed(() => initializing.value || submitting.value)

const INTENT_IDS = new Set(INTENTS.map((i) => i.id))

onMounted(async () => {
  // KV 回填:支持"重做 onboarding"+ finalizeOnboarding 失败回退（组件重挂）。
  // 失败一律 fallback 构造函数默认值（water+sit_long）,不阻断流程。
  try {
    const raw = await getMemory(KV_KEY)
    if (raw !== null) {
      const parsed = JSON.parse(raw) as unknown
      if (parsed === null) {
        // "我不需要" sentinel
        noneChecked.value = true
        selected.value = new Set()
      } else if (Array.isArray(parsed)) {
        // 过滤未知 id,防御旧版本/脏数据(比如某次扩了 id 后又回退)
        const valid = parsed.filter(
          (id): id is string => typeof id === 'string' && INTENT_IDS.has(id),
        )
        noneChecked.value = false
        selected.value = new Set(valid)
      }
      // 其他非预期格式（字符串 / 数字 / 对象）：保持默认,不打断 UI
    }
  } catch (e) {
    console.warn('[ReminderIntentsView] hydrate from KV failed, fallback to default:', e)
  } finally {
    initializing.value = false
  }
})

const buttonLabel = computed(() => {
  if (noneChecked.value) return '好,跳过'
  if (selected.value.size === 0) return '什么都不选,继续'
  return '用这些'
})

function toggleIntent(id: string, checked: boolean) {
  if (isLocked.value) return
  if (checked) {
    selected.value.add(id)
    // 任选其一 → "不需要" 自动取消
    noneChecked.value = false
  } else {
    selected.value.delete(id)
  }
}

function toggleNone(checked: boolean) {
  if (isLocked.value) return
  noneChecked.value = checked
  if (checked) {
    // "不需要" 排他：其他全清
    selected.value.clear()
  }
}

async function onConfirm() {
  if (isLocked.value) return
  submitting.value = true
  try {
    // 三态写入（M1↔M2 KV 契约,见 file header）:
    //   "我不需要"   → "null"
    //   全空中间态   → "[]"
    //   正常勾选     → JSON 数组（按 INTENTS 声明顺序,稳定可 diff）
    let value: string
    if (noneChecked.value) {
      value = JSON.stringify(null) // "null"
    } else {
      const orderedIds = INTENTS.map((i) => i.id).filter((id) => selected.value.has(id))
      value = JSON.stringify(orderedIds)
    }
    await setMemory(KV_KEY, value)
    emit('done')
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    console.error('[ReminderIntentsView] setMemory failed:', e)
    toast.error(`保存失败：${msg}`, { duration: 5000 })
  } finally {
    // finally 复位:成功路径下组件即将卸载,复位 no-op;若上层日后接 KeepAlive,
    // 也能保证按钮不卡 disabled。
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

    <ul class="intent-list" aria-label="提醒类型多选">
      <li v-for="intent in INTENTS" :key="intent.id" class="intent-item">
        <ElCheckbox
          :model-value="selected.has(intent.id)"
          :disabled="isLocked"
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
          :disabled="isLocked"
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
        :disabled="isLocked"
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
