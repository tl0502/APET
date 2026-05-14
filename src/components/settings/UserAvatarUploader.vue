<script setup lang="ts">
// UserAvatarUploader（#25 + 裁剪流）：dialog 选图 → 后端读 bytes 返 dataURL → 弹 cropper modal
// → 用户裁剪 → 输出 512×512 PNG dataURL → 后端落盘 user.png。
//
// 改自最初的"直接复制"版本，加入业界通用的"保存前裁剪"环节。
// - 圆形预览 + 1:1 aspect + 缩放/拖动
// - 失败 toast.error，UI 状态不变；中途取消（dialog 取消 / cropper 取消）回到选图前状态
import { onMounted, ref, watch } from 'vue'
import { ElButton, ElIcon } from 'element-plus'
import { Upload } from '@element-plus/icons-vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useToast } from '@/composables/useToast'
import { useAvatarsStore } from '@/stores/avatars'
import {
  applyUserAvatarFromDataUrl,
  readImageToDataUrl,
  removeUserAvatar,
} from '@/services/avatars'
import ImageCropperModal from './ImageCropperModal.vue'

const toast = useToast()
const avatarsStore = useAvatarsStore()
const uploading = ref(false)
const cropperOpen = ref(false)
const cropperSrc = ref('')
// onCropConfirm 把 confirmed 翻 true,让 watch(cropperOpen) 区分"用户确认关闭"和"X/ESC 关闭"
// 后者要复位 uploading + cropperSrc, 否则按钮永久 loading（H4 修复）
let confirmed = false

const ready = ref(false)
onMounted(async () => {
  if (!avatarsStore.loaded) await avatarsStore.load()
  await avatarsStore.ensureListener()
  ready.value = true
})

// H4 修复：ElDialog 的 X / ESC / 点 modal 外 都只发 update:open，不触发我们自定义的 cancel event。
// 监听 cropperOpen 翻 false 时复位状态（确认路径 onCropConfirm 已自己处理，靠 confirmed flag 区分）。
watch(cropperOpen, (open) => {
  if (!open) {
    if (!confirmed) {
      // 取消路径：用户 X/ESC/backdrop 关闭 dialog
      uploading.value = false
      cropperSrc.value = ''
    }
    // 不论确认/取消，都复位 confirmed flag 给下次使用
    confirmed = false
  }
})

async function onPick() {
  if (uploading.value) return
  uploading.value = true
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: '图片（PNG / JPG）', extensions: ['png', 'jpg', 'jpeg'] }],
    })
    if (!selected) {
      uploading.value = false
      return
    }
    const srcPath = typeof selected === 'string' ? selected : null
    if (!srcPath) {
      uploading.value = false
      return
    }
    // 后端读 bytes 返 dataURL 给 cropper 使用（绕过 asset scope 限制）
    const dataUrl = await readImageToDataUrl(srcPath)
    cropperSrc.value = dataUrl
    cropperOpen.value = true
    // uploading 等 cropper 完成后再 clear
  } catch (e) {
    console.error('[UserAvatarUploader] read source failed:', e)
    toast.error(`读取图片失败：${e instanceof Error ? e.message : String(e)}`)
    uploading.value = false
  }
}

async function onCropConfirm(dataUrl: string) {
  confirmed = true // 让 watch(cropperOpen) 知道是确认路径，不复位 uploading
  try {
    const finalPath = await applyUserAvatarFromDataUrl(dataUrl)
    toast.success(`头像已保存：${finalPath}`)
  } catch (e) {
    console.error('[UserAvatarUploader] save cropped failed:', e)
    toast.error(`保存失败：${e instanceof Error ? e.message : String(e)}`)
  } finally {
    uploading.value = false
    cropperSrc.value = ''
  }
}

function onCropCancel() {
  // 用户点 cropper 内"取消"按钮的路径。X/ESC/backdrop 走 watch(cropperOpen) 处理，
  // 故此函数仅在 cropper 自己的 cancel 按钮被点时跑。两条路径结果一致。
  uploading.value = false
  cropperSrc.value = ''
}

async function onClear() {
  if (uploading.value) return
  try {
    await removeUserAvatar()
    toast.success('已清除自定义头像，回退到昵称首字符占位')
  } catch (e) {
    toast.error(`清除失败：${e instanceof Error ? e.message : String(e)}`)
  }
}
</script>

<template>
  <section class="user-avatar">
    <h3 class="user-avatar__title">头像</h3>
    <p class="user-avatar__hint">
      支持本地 PNG / JPG（≤ 5MB）。选择后可裁剪缩放再保存；移动/删除原文件不影响头像。
    </p>

    <div class="user-avatar__row">
      <div class="user-avatar__preview">
        <img
          v-if="avatarsStore.userAvatarUrl"
          :src="avatarsStore.userAvatarUrl"
          alt="当前头像"
          class="user-avatar__preview-img"
        />
        <span v-else class="user-avatar__preview-empty">未设置</span>
      </div>
      <div class="user-avatar__actions">
        <ElButton type="primary" :loading="uploading" :disabled="!ready" @click="onPick">
          <ElIcon><Upload /></ElIcon>
          <span style="margin-left: 4px">{{
            avatarsStore.userAvatarUrl ? '更换头像' : '选择本地图片'
          }}</span>
        </ElButton>
        <ElButton :disabled="uploading || !avatarsStore.userAvatarUrl" @click="onClear">
          移除
        </ElButton>
      </div>
    </div>

    <ImageCropperModal
      v-model:open="cropperOpen"
      :src="cropperSrc"
      @confirm="onCropConfirm"
      @cancel="onCropCancel"
    />
  </section>
</template>

<style scoped>
.user-avatar {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
  margin-top: var(--aipet-space-4);
  padding-top: var(--aipet-space-4);
  border-top: 1px solid var(--aipet-color-border);
}
.user-avatar__title {
  margin: 0;
  font-size: var(--aipet-font-size-base);
  font-weight: 600;
  color: var(--aipet-color-text-2);
}
.user-avatar__hint {
  margin: 0;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
  line-height: var(--aipet-line-height-base);
}
.user-avatar__row {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-4);
}
.user-avatar__preview {
  flex: 0 0 auto;
  width: 64px;
  height: 64px;
  border-radius: 50%;
  border: 1px solid var(--aipet-color-border);
  background: var(--aipet-color-surface-soft);
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
}
.user-avatar__preview-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.user-avatar__preview-empty {
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}
.user-avatar__actions {
  display: flex;
  gap: var(--aipet-space-2);
}
</style>
