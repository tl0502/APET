<script setup lang="ts">
// ProviderDrawer：添加 / 编辑 provider（cc-switch 风格右侧抽屉）。
//
// 模式：
// - mode='create'：drawer 打开 → form 全空 → 选 preset → 填字段 → 保存 → addProvider
// - mode='edit'：drawer 打开时 props.editingId 触发 watch → 拉 getProvider 含 api_key 回填 → 保存 → updateProvider partial
//
// Preset 标签（顶部一排 6 个）：点击 → 强制覆盖 base_url + model（保留 name + api_key）
// "测试连通"按钮：先要求保存（编辑模式）/ 后端校验 → testProvider(id) → toast preview
//   create 模式没 id，"测试连通"先 saveAndKeepOpen 拿到新 id 再测
//
// 偏离决策：
// - 删除按钮不放 drawer 内（卡片上有 ElPopconfirm 已够；drawer 专注编辑）
// - 不在 drawer 内做 radio 激活切换（卡片上做，单一事实源）
import { computed, ref, watch } from 'vue'
import {
  ElButton,
  ElDrawer,
  ElForm,
  ElFormItem,
  ElIcon,
  ElInput,
  ElOption,
  ElSelect,
  ElTag,
} from 'element-plus'
import { Refresh } from '@element-plus/icons-vue'
import { useToast } from '@/composables/useToast'
import {
  addProvider,
  getProvider,
  probeModels,
  testProvider,
  updateProvider,
} from '@/services/llm_providers'

interface Props {
  visible: boolean
  /** 'create' 全新加；'edit' 配合 editingId 回填。 */
  mode: 'create' | 'edit'
  /** edit 模式下要回填的 provider id；create 模式忽略。 */
  editingId?: string | null
}

const props = withDefaults(defineProps<Props>(), { editingId: null })
const emit = defineEmits<{
  'update:visible': [boolean]
  /** 保存成功后通知父组件刷列表；payload 是新建/更新的 provider id。 */
  saved: [string]
}>()

const toast = useToast()

interface Preset {
  id: string
  name: string
  baseUrl: string
  modelDefault: string
  modelOptions: string[]
}

const PRESETS: Preset[] = [
  {
    id: 'openai',
    name: 'OpenAI',
    baseUrl: 'https://api.openai.com/v1',
    modelDefault: 'gpt-4o-mini',
    modelOptions: ['gpt-4o-mini', 'gpt-4o', 'gpt-4-turbo'],
  },
  {
    id: 'deepseek',
    name: 'DeepSeek',
    baseUrl: 'https://api.deepseek.com',
    modelDefault: 'deepseek-chat',
    modelOptions: ['deepseek-chat', 'deepseek-reasoner'],
  },
  {
    id: 'moonshot',
    name: 'Moonshot',
    baseUrl: 'https://api.moonshot.cn/v1',
    modelDefault: 'moonshot-v1-8k',
    modelOptions: ['moonshot-v1-8k', 'moonshot-v1-32k', 'moonshot-v1-128k'],
  },
  {
    id: 'qwen',
    name: 'Qwen',
    baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    modelDefault: 'qwen-plus',
    modelOptions: ['qwen-plus', 'qwen-max', 'qwen-turbo'],
  },
  {
    id: 'ollama',
    name: 'Ollama',
    baseUrl: 'http://localhost:11434/v1',
    modelDefault: 'llama3.1',
    modelOptions: ['llama3.1', 'qwen2.5', 'phi3'],
  },
  { id: 'custom', name: '自定义', baseUrl: '', modelDefault: '', modelOptions: [] },
]

const form = ref({ name: '', apiKey: '', baseUrl: '', model: '' })
const apiKeyOriginal = ref('') // edit 模式回填的原始 key（partial update 不动 api_key 时省 IPC）
/** create 模式下首次"测试连通"会先保存拿 id；保存后此 ref 持有该 id，
 * 后续保存/测试都走 update + 该 id 路径，避免重复创建 + 抽屉残留状态错乱。 */
const localSavedId = ref<string | null>(null)
const loading = ref(false)
const saving = ref(false)
const testing = ref(false)
const probing = ref(false)
/** 探测成功后持有的真实 model id 列表；优先于 preset.modelOptions 显示。
 * null = 还没探测过 / 切预设清掉，回退到 modelOptions = preset 硬编码。 */
const probedModels = ref<string[] | null>(null)

/** 当前操作真实使用的 provider id：edit 模式 = props.editingId；
 * create 模式首次保存前 = null，首次保存后 = localSavedId。 */
const effectiveId = computed<string | null>(() => localSavedId.value ?? props.editingId)

/** 当前 base_url 反推到哪个 preset；活化对应 tag。 */
const activePresetId = computed(() => {
  const matched = PRESETS.find((p) => p.id !== 'custom' && p.baseUrl === form.value.baseUrl.trim())
  return matched?.id ?? 'custom'
})

const modelOptions = computed(() => {
  if (probedModels.value !== null) return probedModels.value
  const matched = PRESETS.find((p) => p.id === activePresetId.value)
  return matched?.modelOptions ?? []
})

const drawerTitle = computed(() => (props.mode === 'edit' ? '编辑 Provider' : '添加 Provider'))

const canSave = computed(() => {
  if (saving.value || loading.value) return false
  if (form.value.name.trim().length === 0) return false
  if (form.value.baseUrl.trim().length === 0) return false
  if (form.value.model.trim().length === 0) return false
  // create 模式：必须填 api_key（后端允许空，但首条必须能用；用户体验上要求）
  if (props.mode === 'create' && form.value.apiKey.trim().length === 0) return false
  return true
})

// drawer visible 切到 true 时初始化：create 全空 / edit 拉详情
watch(
  () => props.visible,
  async (vis) => {
    if (!vis) return
    if (props.mode === 'edit' && props.editingId) {
      await loadEditing(props.editingId)
    } else {
      resetForm()
    }
  },
)

watch(
  () => props.editingId,
  async (id) => {
    if (props.visible && props.mode === 'edit' && id) {
      await loadEditing(id)
    }
  },
)

function resetForm() {
  form.value = { name: '', apiKey: '', baseUrl: '', model: '' }
  apiKeyOriginal.value = ''
  localSavedId.value = null
  probedModels.value = null
}

async function loadEditing(id: string) {
  loading.value = true
  try {
    const detail = await getProvider(id)
    form.value = {
      name: detail.name,
      apiKey: detail.apiKey,
      baseUrl: detail.baseUrl,
      model: detail.model,
    }
    apiKeyOriginal.value = detail.apiKey
    localSavedId.value = null // edit 模式走 props.editingId；clear 防上次 create 残留
    probedModels.value = null // 编辑切换时清探测结果，强制走 preset 默认或重新探测
  } catch (e) {
    toast.error(`加载 Provider 失败：${msgOf(e)}`)
    onClose()
  } finally {
    loading.value = false
  }
}

function applyPreset(preset: Preset) {
  // 切预设时清探测结果，避免 OpenAI 的列表错误地停留在切到 DeepSeek 之后
  probedModels.value = null
  if (preset.id === 'custom') {
    // 自定义：清 base_url + model 让用户手填；name 不动（用户已填的 name 是身份）
    form.value.baseUrl = ''
    form.value.model = ''
    return
  }
  // 强制覆盖 base_url + model（保留 name + api_key；plan 决策）
  form.value.baseUrl = preset.baseUrl
  form.value.model = preset.modelDefault
  // 若用户 name 还没填，顺手填 preset name 当默认
  if (form.value.name.trim().length === 0) {
    form.value.name = preset.name
  }
}

async function onSave({ keepOpen = false } = {}): Promise<string | null> {
  if (!canSave.value) return null
  saving.value = true
  try {
    const id = effectiveId.value
    if (id) {
      // Update path：edit 模式 + create 模式首次保存后的所有后续保存都走这里
      const update: Record<string, string> = {
        name: form.value.name.trim(),
        baseUrl: form.value.baseUrl.trim(),
        model: form.value.model.trim(),
      }
      if (form.value.apiKey !== apiKeyOriginal.value) {
        update.apiKey = form.value.apiKey.trim()
      }
      await updateProvider(id, update)
      apiKeyOriginal.value = form.value.apiKey // 同步 baseline；下次保存按 partial 路径
      toast.success(props.mode === 'edit' ? '已保存修改' : `已更新 ${form.value.name.trim()}`)
      emit('saved', id)
      if (!keepOpen) onClose()
      return id
    } else {
      // Initial create：只在 create 模式 + 未保存过时走这里
      const newId = await addProvider({
        name: form.value.name.trim(),
        apiKey: form.value.apiKey.trim(),
        baseUrl: form.value.baseUrl.trim(),
        model: form.value.model.trim(),
      })
      localSavedId.value = newId
      apiKeyOriginal.value = form.value.apiKey // 同步 baseline
      toast.success(`已添加 ${form.value.name.trim()}`)
      emit('saved', newId)
      if (!keepOpen) onClose()
      return newId
    }
  } catch (e) {
    toast.error(`保存失败：${msgOf(e)}`)
    return null
  } finally {
    saving.value = false
  }
}

async function onTest() {
  if (testing.value) return
  // 解析 testId：优先用已有的 effectiveId（edit 模式 props.editingId / create 模式已保存的 localSavedId）
  // 都没有 → create 模式首次测试，先 keepOpen 保存拿 id 再测（不关抽屉）
  let testId = effectiveId.value
  if (!testId) {
    if (!canSave.value) {
      toast.warn('请先填完必填字段')
      return
    }
    const newId = await onSave({ keepOpen: true })
    if (!newId) return
    testId = newId
  }

  testing.value = true
  try {
    const reply = await testProvider(testId)
    const preview = reply.length > 40 ? `${reply.slice(0, 40)}…` : reply
    toast.success(`连通成功：${preview}`, { duration: 5000 })
  } catch (e) {
    const m = msgOf(e)
    const kind = m.split(':')[0]
    const hint =
      kind === 'AuthFailed'
        ? 'API Key 错误或已失效'
        : kind === 'Network'
          ? '网络不通（端点不可达）'
          : kind === 'BadRequest'
            ? '请求被拒绝（model 名 / API Key 缺失 / base_url 路径错）'
            : kind === 'ParseError'
              ? '响应解析失败（base_url 可能不是 OpenAI 兼容协议）'
              : kind === 'RateLimit'
                ? '请求过于频繁，稍后再试'
                : kind
    toast.error(`连通失败：${hint}`, { duration: 6000 })
  } finally {
    testing.value = false
  }
}

function onClose() {
  emit('update:visible', false)
}

/** onProbeModels 错误 kind → 中文提示。BadRequest / ParseError 文案与 onTest 略有差异
 * （那边强调 model / api_key / base_url；这里强调端点是否支持 /models）。 */
function probeHintForKind(kind: string): string {
  switch (kind) {
    case 'AuthFailed':
      return 'API Key 错误或已失效'
    case 'Network':
      return '网络不通（端点不可达）'
    case 'RateLimit':
      return '请求过于频繁，稍后再试'
    case 'BadRequest':
      return '端点不支持 /v1/models（可能旧版 Ollama / 自定义端点）'
    case 'ParseError':
      return '响应格式不识别（base_url 可能不是 OpenAI 兼容协议）'
    default:
      return kind
  }
}

async function onProbeModels() {
  if (probing.value) return
  const baseUrl = form.value.baseUrl.trim()
  if (baseUrl.length === 0) {
    toast.warn('先填 Base URL')
    return
  }
  probing.value = true
  try {
    const list = await probeModels(baseUrl, form.value.apiKey.trim())
    probedModels.value = list
    if (list.length === 0) {
      toast.warn('端点返回空列表（可能此 provider 不暴露 /v1/models）')
      return
    }
    toast.success(`探测到 ${list.length} 个模型`)
    // B5：仅在 model 还没填时自动选首项；用户已选的不动也不警告——
    // 探测列表可能不全（OpenRouter 100+ models 截断、Ollama 仅返本地装的几个），
    // 用户可能知道一个 list 里没列出的合法 id。
    if (form.value.model.trim().length === 0) {
      form.value.model = list[0]
    }
  } catch (e) {
    const m = msgOf(e)
    const kind = m.split(':')[0]
    toast.error(`探测失败：${probeHintForKind(kind)}`, { duration: 6000 })
  } finally {
    probing.value = false
  }
}

function msgOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}
</script>

<template>
  <ElDrawer
    :model-value="visible"
    :title="drawerTitle"
    direction="rtl"
    size="380px"
    :close-on-click-modal="!saving && !testing"
    @update:model-value="onClose"
  >
    <div class="drawer-body">
      <section class="presets-section">
        <div class="presets-section__label">预设</div>
        <div class="presets">
          <ElTag
            v-for="preset in PRESETS"
            :key="preset.id"
            :type="activePresetId === preset.id ? 'primary' : 'info'"
            :effect="activePresetId === preset.id ? 'dark' : 'light'"
            class="preset-tag"
            @click="applyPreset(preset)"
          >
            {{ preset.name }}
          </ElTag>
        </div>
      </section>

      <ElForm class="provider-form" label-position="top" :disabled="loading">
        <ElFormItem label="名称">
          <ElInput
            v-model="form.name"
            :maxlength="50"
            show-word-limit
            placeholder="如：DeepSeek 工作"
          />
        </ElFormItem>

        <ElFormItem label="API Key">
          <ElInput
            v-model="form.apiKey"
            type="password"
            show-password
            :placeholder="mode === 'edit' ? '留空 = 不修改（已保存）' : '仅本地保存，不上传'"
          />
        </ElFormItem>

        <ElFormItem label="Base URL">
          <ElInput v-model="form.baseUrl" placeholder="https://api.example.com/v1" />
        </ElFormItem>

        <ElFormItem label="Model">
          <div class="model-field">
            <ElSelect
              v-model="form.model"
              placeholder="输入或选择 model id"
              filterable
              allow-create
              default-first-option
              class="model-select"
            >
              <ElOption v-for="m in modelOptions" :key="m" :label="m" :value="m" />
            </ElSelect>
            <ElButton
              link
              :loading="probing"
              :disabled="loading || saving || testing"
              :title="'探测 ' + (form.baseUrl.trim() || '<base_url>') + '/models'"
              class="model-probe-btn"
              @click="onProbeModels"
            >
              <ElIcon><Refresh /></ElIcon>
            </ElButton>
          </div>
        </ElFormItem>
      </ElForm>
    </div>

    <template #footer>
      <div class="drawer-footer">
        <ElButton :loading="testing" :disabled="loading || saving" @click="onTest">
          测试连通
        </ElButton>
        <ElButton type="primary" :loading="saving" :disabled="!canSave" @click="() => onSave()">
          保存
        </ElButton>
      </div>
    </template>
  </ElDrawer>
</template>

<style scoped>
.drawer-body {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
}

.presets-section {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}

.presets-section__label {
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
}

.presets {
  display: flex;
  flex-wrap: wrap;
  gap: var(--aipet-space-2);
}

.preset-tag {
  cursor: pointer;
  user-select: none;
  transition: opacity var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.preset-tag:hover {
  opacity: 0.85;
}

.provider-form {
  width: 100%;
}

.model-field {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
  width: 100%;
}

.model-select {
  flex: 1 1 auto;
}

.model-probe-btn {
  flex: 0 0 auto;
}

.drawer-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--aipet-space-2);
}
</style>
