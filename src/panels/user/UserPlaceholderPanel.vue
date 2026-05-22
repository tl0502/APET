<script setup lang="ts">
// UserPlaceholderPanel（#37 2026-05-21 重设计）— 3 个 disabled nav 项共用的占位 panel。
//
// 通过 kind prop 区分文案。store 守卫已阻止 setNav 切到这些，理论永不会被渲染；
// 但模板上 v-show 保留作为防御性兜底，避免未来误改 store 时静默坏掉。

import { computed } from 'vue'

type PlaceholderKind = 'account' | 'privacy' | 'notifications'

interface PlaceholderCopy {
  title: string
  hint: string
  status: string
}

const COPY: Record<PlaceholderKind, PlaceholderCopy> = {
  account: {
    title: '账户',
    hint: '账号信息 / 登录方式管理 / 密码 / 安全中心 / 邮箱手机绑定 / 二步验证 / 设备管理。',
    status: '账户系统将随登录系统一同上线（M3+）。',
  },
  privacy: {
    title: '数据与隐私',
    hint: '数据导出 / 清除 / 同步、隐私权限、模型访问范围。',
    status: 'M3+ 开发中。',
  },
  notifications: {
    title: '通知',
    hint: '全局通知开关、桌面通知样式、声音偏好。',
    status: 'M3+ 开发中。',
  },
}

const props = defineProps<{ kind: PlaceholderKind }>()

const copy = computed(() => COPY[props.kind])
</script>

<template>
  <section class="panel panel--form">
    <h2 class="panel__title">{{ copy.title }}</h2>
    <div class="panel__content">
      <div class="placeholder">
        <p class="panel__hint">{{ copy.hint }}</p>
        <p class="placeholder__status">{{ copy.status }}</p>
      </div>
    </div>
  </section>
</template>

<style scoped>
.placeholder {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
  padding: var(--aipet-space-6);
  background: var(--aipet-color-surface);
  border: 1px dashed var(--aipet-color-border);
  border-radius: var(--aipet-radius-card);
  align-items: center;
  text-align: center;
}
.placeholder__status {
  margin: 0;
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
}
</style>
