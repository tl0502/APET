<script setup lang="ts">
// SummonInviteView：Onboarding Step 6 — 第一次唤起对话引导（flows §1.2 Step 6）。
//
// 设计简化：
// - flows §1.2 字面要求 "显示快捷键浮窗 3s + 监听快捷键 5s + 触发后弹 chat" —— 跨窗口耦合
//   高、易出 race。本 view 改为 onboarding 内一个完成感页面：显示当前 chat 快捷键 +
//   "开始陪伴" 按钮 → emit('done') → router 调 onboarding_complete 切到 pet
// - 用户切到 pet 后,真正按快捷键召唤是已有 #11 全局快捷键的事,本 view 不再做
//   "首次按键监听 / 5s 倒计时" —— 那些是 M2 范围（首启引导 toast 在 pet 端）
// - 没 API Key 走 #20 preset 引导是 ChatPanel 自身的事,与本 view 无关
//
// 显示的快捷键来源：getChatShortcut()。Step 3 已确保启动期 register 成功（或用户已设过新值）。
// 极端 fallback：getChatShortcut 返 null（启动期失败 + 用户未在 Step 3 改键）→ 文案改成
// "右键我也能打开菜单",不假装快捷键存在。

import { onMounted, ref } from 'vue'
import { ElButton } from 'element-plus'
import { getChatShortcut } from '@/services/shortcut'

const emit = defineEmits<{ done: [] }>()

const shortcut = ref<string | null>(null)
const submitting = ref(false)

onMounted(async () => {
  try {
    shortcut.value = await getChatShortcut()
  } catch (e) {
    console.warn('[SummonInviteView] getChatShortcut failed:', e)
    shortcut.value = null
  }
})

function onStart() {
  if (submitting.value) return
  submitting.value = true
  emit('done')
}
</script>

<template>
  <section
    class="summon-invite"
    role="dialog"
    aria-modal="true"
    aria-labelledby="summon-title"
  >
    <h1 id="summon-title" class="summon-invite__title">我们准备好啦 ✨</h1>
    <p class="summon-invite__hint">
      想找我聊天的时候,随时按这个组合,我就跳出来听你说。
    </p>

    <div v-if="shortcut" class="summon-invite__shortcut">
      <kbd class="summon-invite__kbd">{{ shortcut }}</kbd>
    </div>
    <div v-else class="summon-invite__shortcut summon-invite__shortcut--fallback">
      <p>(快捷键还没设好,可以右键我打开菜单)</p>
    </div>

    <p class="summon-invite__footer-hint">设置里能随时改键、调人格、关提醒。</p>

    <div class="summon-invite__actions">
      <ElButton
        type="primary"
        :loading="submitting"
        :disabled="submitting"
        @click="onStart"
      >
        开始陪伴
      </ElButton>
    </div>
  </section>
</template>

<style scoped>
.summon-invite {
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

.summon-invite__title {
  margin: 0 0 var(--aipet-space-2);
  font-size: var(--aipet-font-size-xl);
  font-weight: 600;
  color: var(--aipet-color-text-1);
  text-align: center;
}

.summon-invite__hint {
  margin: 0 0 var(--aipet-space-6);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
  text-align: center;
}

.summon-invite__shortcut {
  display: flex;
  justify-content: center;
  margin: var(--aipet-space-4) 0 var(--aipet-space-6);
}

.summon-invite__kbd {
  display: inline-block;
  padding: var(--aipet-space-3) var(--aipet-space-5);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
  background: var(--aipet-color-surface);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-lg);
  color: var(--aipet-color-text-1);
  letter-spacing: 0.5px;
  box-shadow: 0 2px 0 var(--aipet-color-border);
}

.summon-invite__shortcut--fallback p {
  margin: 0;
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
  font-style: italic;
}

.summon-invite__footer-hint {
  margin: var(--aipet-space-2) 0 var(--aipet-space-4);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
  text-align: center;
}

.summon-invite__actions {
  display: flex;
  justify-content: center;
  margin-top: auto;
}
</style>
