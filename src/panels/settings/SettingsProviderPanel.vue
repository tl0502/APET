<script setup lang="ts">
// ProviderPanel：LLM Provider 主页（cc-switch 风格）。
//
// 范围：
// - 顶部：标题 + "+ 添加"按钮
// - 列表：ProviderCard × N（empty state 引导添加第一个）
// - drawer 控制：mode='create' 添加 / mode='edit' 编辑（带 editingId）
// - 激活切换：点卡片 / radio → activateProvider → 刷新列表
// - 删除：卡片 ElPopconfirm → deleteProvider → 刷新列表（激活的后端拦截）
//
// 多 provider 的"启用哪个"语义：单 active；ChatService.build_provider 真消费 active 那一份
// （services/chat/service.rs:300-310）。Migrate 启动期：旧 #12 `llm:openai:*` 三键 → "默认 OpenAI"。
import { computed, onMounted, ref } from 'vue'
import { ElButton, ElIcon } from 'element-plus'
import { Plus } from '@element-plus/icons-vue'
import ProviderCard from '@/components/settings/ProviderCard.vue'
import ProviderDrawer from '@/components/settings/ProviderDrawer.vue'
import { useToast } from '@/composables/useToast'
import {
  activateProvider,
  deleteProvider,
  listProviders,
} from '@/services/llm_providers'
import type { ProviderListItem } from '@/types/llm_providers'

const toast = useToast()

const providers = ref<ProviderListItem[]>([])
const loading = ref(true)
const mutating = ref(false) // 激活/删除中禁用所有交互

const drawerVisible = ref(false)
const drawerMode = ref<'create' | 'edit'>('create')
const drawerEditingId = ref<string | null>(null)

const activeId = computed(() => providers.value.find((p) => p.isActive)?.id ?? null)
const isEmpty = computed(() => !loading.value && providers.value.length === 0)

onMounted(async () => {
  await refresh()
})

async function refresh() {
  loading.value = true
  try {
    providers.value = await listProviders()
  } catch (e) {
    toast.error(`加载 Provider 列表失败：${msgOf(e)}`)
  } finally {
    loading.value = false
  }
}

function openCreate() {
  drawerMode.value = 'create'
  drawerEditingId.value = null
  drawerVisible.value = true
}

function openEdit(id: string) {
  drawerMode.value = 'edit'
  drawerEditingId.value = id
  drawerVisible.value = true
}

async function onActivate(id: string) {
  if (mutating.value) return
  mutating.value = true
  try {
    await activateProvider(id)
    await refresh()
  } catch (e) {
    toast.error(`激活失败：${msgOf(e)}`)
  } finally {
    mutating.value = false
  }
}

async function onDelete(id: string) {
  if (mutating.value) return
  mutating.value = true
  try {
    await deleteProvider(id)
    toast.success('已删除')
    await refresh()
  } catch (e) {
    const m = msgOf(e)
    if (m.includes('不能删除当前激活')) {
      toast.warn('请先切换到其他 Provider 再删除')
    } else {
      toast.error(`删除失败：${m}`)
    }
  } finally {
    mutating.value = false
  }
}

async function onSaved(_id: string) {
  await refresh()
}

function msgOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}
</script>

<template>
  <section class="panel">
    <div class="panel__head">
      <div class="panel__title-area">
        <h2 class="panel__title">LLM Provider</h2>
        <p class="panel__hint">多 Provider 实例；选择一个为当前生效。改完即时影响对话。</p>
      </div>
      <ElButton type="primary" :disabled="loading" @click="openCreate">
        <ElIcon class="el-icon--left"><Plus /></ElIcon>
        添加
      </ElButton>
    </div>

    <div v-if="loading" class="panel__loading">加载中…</div>

    <div v-else-if="isEmpty" class="panel__empty">
      <p>还没有 Provider，点击右上角「添加」开始第一个 ~</p>
    </div>

    <div v-else class="provider-list">
      <ProviderCard
        v-for="p in providers"
        :key="p.id"
        :provider="p"
        :active-id="activeId"
        :disabled="mutating"
        @activate="onActivate"
        @edit="openEdit"
        @delete="onDelete"
      />
    </div>

    <ProviderDrawer
      v-model:visible="drawerVisible"
      :mode="drawerMode"
      :editing-id="drawerEditingId"
      @saved="onSaved"
    />
  </section>
</template>

<style scoped>
.panel__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--aipet-space-3);
}

.panel__title-area {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-1);
}

.panel__loading,
.panel__empty {
  padding: var(--aipet-space-6) var(--aipet-space-4);
  text-align: center;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
  border: 1px dashed var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
}

.provider-list {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}
</style>
