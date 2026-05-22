<script setup lang="ts">
// UserProfilePanel — 用户个人资料面板（#37 2026-05-21 重设计）。
//
// 设计：
// - 复用 NicknameForm（已含头像 cropper + 昵称编辑 + 校验 + 转场注入开关）
// - 追加 bio textarea（个性资料，<= 200 字符前端校验，走 userProfile service KV）
// - bio 与 nickname 各自独立保存（避免一起 commit 半成品）

import { onMounted, ref } from 'vue'
import { ElButton, ElInput } from 'element-plus'

import NicknameForm from '@/components/settings/NicknameForm.vue'
import { useToast } from '@/composables/useToast'
import { getUserBio, setUserBio } from '@/services/userProfile'

const toast = useToast()

const BIO_MAX = 200

const bioDraft = ref('')
const bioOriginal = ref('')
const bioLoading = ref(false)
const bioSaving = ref(false)

const bioChanged = () => bioDraft.value !== bioOriginal.value
const bioOverLimit = () => bioDraft.value.length > BIO_MAX

onMounted(async () => {
  bioLoading.value = true
  try {
    const v = await getUserBio()
    bioDraft.value = v ?? ''
    bioOriginal.value = v ?? ''
  } catch (e) {
    console.warn('[UserProfilePanel] getUserBio failed:', e)
  } finally {
    bioLoading.value = false
  }
})

async function onSaveBio() {
  if (bioOverLimit()) {
    toast.error(`个性资料不能超过 ${BIO_MAX} 字符`)
    return
  }
  bioSaving.value = true
  try {
    await setUserBio(bioDraft.value)
    bioOriginal.value = bioDraft.value
    toast.success('个性资料已保存')
  } catch (e) {
    toast.error(`保存失败：${e instanceof Error ? e.message : String(e)}`)
  } finally {
    bioSaving.value = false
  }
}
</script>

<template>
  <section class="panel panel--form">
    <h2 class="panel__title">个人资料</h2>
    <div class="panel__content">
      <p class="panel__hint">
        头像和昵称用于桃宝对你的称呼与显示；个性资料是可选的、对桃宝介绍你的几句话。
      </p>

      <!-- 复用 NicknameForm：头像上传 + 昵称编辑（含校验 + 转场开关） -->
      <NicknameForm />

      <!-- 个性资料：独立 section + 独立保存按钮 -->
      <div class="panel__section">
        <h3 class="panel__subtitle">个性资料</h3>
        <ElInput
          v-model="bioDraft"
          type="textarea"
          :rows="4"
          :disabled="bioLoading"
          :maxlength="BIO_MAX + 50"
          placeholder="简单几句话告诉桃宝你是谁、喜欢什么..."
          resize="vertical"
        />
        <p class="panel__hint">
          {{ bioDraft.length }} / {{ BIO_MAX }} 字符
          <span v-if="bioOverLimit()" class="panel__error">（已超出）</span>
        </p>
        <div class="panel__actions">
          <ElButton
            type="primary"
            :loading="bioSaving"
            :disabled="!bioChanged() || bioOverLimit() || bioLoading"
            @click="onSaveBio"
          >
            保存个性资料
          </ElButton>
        </div>
      </div>
    </div>
  </section>
</template>
