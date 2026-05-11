<script setup lang="ts">
// NicknameForm（M1，2026-05-09 重构后）：用户昵称编辑 + 转场注入开关。
//
// 范围：
// - 一栏"你的昵称"
// - 转场注入开关 checkbox（"昵称变更时通知 AI"，默认 ON）
// - 校验（前端立即提示，后端 commands/nickname.rs 也有 validate_nickname 兜底）：
//   * 1-16 字符（PRD §7.6.4 第 4 条；后端 cap 是 50，前端按 PRD 收紧到 16）
//   * 禁止控制字符 \x00-\x1F\x7F
//   * 禁止前后空白（trim 后 = 原值才允许保存）
//   * 连续 ≥ 3 个 emoji → warn（不阻断）
// - 保存逻辑：
//   * 仅变化的字段 setUser
//   * 后端 emit nickname:changed → store 自动同步
//   * 成功 toast.success；失败 toast.error 表单不重置便于重试
//   * 转场注入是后端在 set_user_nickname 内自动触发（active conversation 存在 + 开关 ON）
// - 即时生效：保存成功后 chat 窗口 / 后续 PetCanvas 都通过 store + listener 自动更新
//
// 已删除（2026-05-09）：
// - 桌宠昵称表单 + "恢复上一次"按钮（pet_nickname 机制移除）
// - 宠物名字源唯一化为 .soul.md persona.name，将由"灵魂编辑"功能维护
import { computed, onMounted, ref } from 'vue'
import { ElButton, ElCheckbox, ElForm, ElFormItem, ElInput } from 'element-plus'
import { useToast } from '@/composables/useToast'
import { useNicknameStore } from '@/stores/nickname'
import { getAnnounceUserChange, setAnnounceUserChange } from '@/services/nickname'

const toast = useToast()
const store = useNicknameStore()

const userDraft = ref('')
const errors = ref<{ user?: string }>({})
const saving = ref(false)
const announceEnabled = ref(true)
const announceLoaded = ref(false)

const NICKNAME_MIN = 1
const NICKNAME_MAX = 16
// PRD §7.6.4 第 4 条：禁止控制字符（U+0000 到 U+001F + U+007F DEL）
// eslint-disable-next-line no-control-regex
const CONTROL_CHAR_RE = /[\x00-\x1F\x7F]/
// emoji 简易识别：unicode emoji block 的常用区段；3 连续触发 warn（不阻断）
const EMOJI_RE = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u

onMounted(async () => {
  if (!store.loaded) await store.load()
  await store.ensureListener()
  resetDrafts()
  try {
    announceEnabled.value = await getAnnounceUserChange()
  } catch (e) {
    console.warn('[NicknameForm] getAnnounceUserChange failed:', e)
  } finally {
    announceLoaded.value = true
  }
})

function resetDrafts() {
  userDraft.value = store.user ?? ''
  errors.value = {}
}

function validate(name: string): string | null {
  if (name.length === 0) return null // 空字符串：调用方决定（user 留空 = 不修改）
  if (name !== name.trim()) return '前后不能有空格'
  const chars = [...name].length
  if (chars < NICKNAME_MIN || chars > NICKNAME_MAX) return `长度需 ${NICKNAME_MIN}-${NICKNAME_MAX} 字符`
  if (CONTROL_CHAR_RE.test(name)) return '不能含控制字符'
  return null
}

function countConsecutiveEmoji(s: string): number {
  let max = 0
  let cur = 0
  for (const ch of s) {
    if (EMOJI_RE.test(ch)) {
      cur += 1
      if (cur > max) max = cur
    } else {
      cur = 0
    }
  }
  return max
}

const userError = computed(() => {
  const v = userDraft.value
  if (v.length === 0) return null // 留空 = 不修改 user 昵称（保留旧值）
  return validate(v)
})

const userChanged = computed(
  () => userDraft.value !== '' && userDraft.value !== (store.user ?? ''),
)
const canSave = computed(() => {
  if (saving.value) return false
  if (userError.value) return false
  return userChanged.value
})

async function onSave() {
  if (!canSave.value) return
  saving.value = true
  errors.value = {}

  if (countConsecutiveEmoji(userDraft.value) >= 3) {
    toast.warn('你的昵称含 3+ 连续 emoji，可能影响显示')
  }

  try {
    await store.setUser(userDraft.value)
    saving.value = false
    toast.success('昵称已保存')
    resetDrafts()
  } catch (e) {
    saving.value = false
    errors.value.user = msgOf(e)
    toast.error('昵称保存失败，请检查输入')
  }
}

async function onAnnounceToggle(value: boolean | string | number) {
  const enabled = Boolean(value)
  // optimistic UI：先更新本地，失败再回滚
  const prev = announceEnabled.value
  announceEnabled.value = enabled
  try {
    await setAnnounceUserChange(enabled)
  } catch (e) {
    announceEnabled.value = prev
    toast.error(`保存开关失败：${msgOf(e)}`)
  }
}

function msgOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}
</script>

<template>
  <section class="panel">
    <h2 class="panel__title">昵称</h2>
    <p class="panel__hint">
      改完会立即同步到对话窗口。留空 = 保留当前值。
    </p>

    <ElForm class="nickname-form" label-position="top" :disabled="saving">
      <ElFormItem
        label="你的昵称"
        :error="userError ?? errors.user"
      >
        <ElInput
          v-model="userDraft"
          :maxlength="NICKNAME_MAX"
          show-word-limit
          :placeholder="store.user ?? 'TA 怎么称呼你'"
        />
      </ElFormItem>

      <ElFormItem>
        <ElButton type="primary" :loading="saving" :disabled="!canSave" @click="onSave">
          保存
        </ElButton>
      </ElFormItem>

      <ElFormItem class="nickname-form__announce">
        <ElCheckbox
          :model-value="announceEnabled"
          :disabled="!announceLoaded"
          @change="onAnnounceToggle"
        >
          昵称变更时通知 AI
        </ElCheckbox>
        <p class="panel__hint nickname-form__announce-hint">
          开启后，改昵称会在当前对话里插入一条系统消息提醒 AI 切换称呼，避免历史记录里的旧称呼污染后续回复。
        </p>
      </ElFormItem>
    </ElForm>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
}
.panel__title {
  margin: 0;
  font-size: var(--aipet-font-size-lg);
  font-weight: 600;
  color: var(--aipet-color-text-1);
}
.panel__hint {
  margin: 0;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
  line-height: var(--aipet-line-height-base);
}
.nickname-form {
  max-width: 480px;
}
.nickname-form__announce :deep(.el-form-item__content) {
  flex-direction: column;
  align-items: flex-start;
  gap: var(--aipet-space-1);
}
.nickname-form__announce-hint {
  margin: 0;
}
</style>
