<script setup lang="ts">
// Nickname tab：M1 占位（issue #9）。
// 接 nickname_get_pet + nickname_get_user 仅展示当前值；编辑控件灰显，等 #15 启用。
import { onMounted, ref } from 'vue'
import { ElForm, ElFormItem, ElInput } from 'element-plus'
import { getPetNickname, getUserNickname } from '@/services/nickname'

const petNickname = ref('')
const userNickname = ref('')
const loading = ref(true)
const errorMsg = ref<string | null>(null)

onMounted(async () => {
  try {
    const [pet, user] = await Promise.all([getPetNickname(), getUserNickname()])
    petNickname.value = pet
    userNickname.value = user ?? ''
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <section class="panel">
    <h2 class="panel__title">昵称</h2>
    <p class="panel__hint">
      桌宠的称呼与你的称呼。编辑功能将在
      <code>#15 U.1/U.2 昵称设置 UI</code> 启用。
    </p>

    <p v-if="loading" class="panel__hint">加载中...</p>
    <p v-else-if="errorMsg" class="panel__error">读取失败：{{ errorMsg }}</p>
    <ElForm v-else class="placeholder-form" label-position="top" disabled>
      <ElFormItem label="桌宠昵称">
        <ElInput v-model="petNickname" />
      </ElFormItem>
      <ElFormItem label="你的昵称">
        <ElInput v-model="userNickname" placeholder="未设置" />
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
.panel__error {
  margin: 0;
  color: var(--aipet-color-danger);
  font-size: var(--aipet-font-size-sm);
}
.placeholder-form {
  max-width: 480px;
}
code {
  padding: 0 var(--aipet-space-1);
  border-radius: var(--aipet-radius-sm);
  background: var(--aipet-color-surface-raised);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-2);
}
</style>
