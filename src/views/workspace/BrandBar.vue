<script setup lang="ts">
// BrandBar (#37 2026-05-21 重设计 P3)：左 60px 列。
//
// 与上一版差异：
// - 删 brand-bar__top（桃宝头像搬到 topbar）
// - 删 brand-bar__divider（无 avatar 不需要分隔）
// - 删顶部 32px 让位 padding（topbar 接管 drag）
// - 底部 help 按钮替换为用户头像 32×32（点击呼出 userPopup）

import { onMounted } from 'vue'
import { ElIcon, ElTooltip } from 'element-plus'

import { useWorkspaceLayoutStore, type CategoryId } from '@/stores/workspaceLayout'
import { useUserPopupStore } from '@/stores/userPopup'
import { useNicknameStore } from '@/stores/nickname'
import { useAvatarsStore } from '@/stores/avatars'

const layout = useWorkspaceLayoutStore()
const popup = useUserPopupStore()
const nickname = useNicknameStore()
const avatars = useAvatarsStore()

function handleCategoryClick(id: CategoryId) {
  layout.setCategory(id)
}

function handleUserClick() {
  popup.open()
}

onMounted(async () => {
  await Promise.all([nickname.load(), avatars.load()])
  await Promise.all([nickname.ensureListener(), avatars.ensureListener()])
})
</script>

<template>
  <nav class="brand-bar" aria-label="工作台导航">
    <!-- 4 类别 -->
    <ul class="brand-bar__list">
      <li
        v-for="cat in layout.brandBarItems"
        :key="cat.id"
        class="brand-bar__item"
      >
        <ElTooltip :content="String(cat.title)" placement="right" :show-after="500">
          <button
            class="brand-bar__btn"
            :class="{ 'brand-bar__btn--active': layout.currentCategory === cat.id }"
            :aria-pressed="layout.currentCategory === cat.id"
            :aria-label="String(cat.title)"
            type="button"
            @click="handleCategoryClick(cat.id)"
          >
            <ElIcon :size="20">
              <component :is="cat.icon" />
            </ElIcon>
          </button>
        </ElTooltip>
      </li>
    </ul>

    <div class="brand-bar__spacer" />

    <!-- 底部：用户头像（替代原 help 按钮） -->
    <div class="brand-bar__user">
      <ElTooltip
        :content="`用户：${nickname.user ?? '未设置昵称'}`"
        placement="right"
        :show-after="500"
      >
        <button
          class="brand-bar__user-btn"
          type="button"
          aria-label="打开用户设置"
          @click="handleUserClick"
        >
          <img
            v-if="avatars.userAvatarUrl"
            :src="avatars.userAvatarUrl"
            alt=""
            class="brand-bar__user-img"
          />
          <span v-else class="brand-bar__user-fallback">
            {{ nickname.user?.[0] ?? '你' }}
          </span>
        </button>
      </ElTooltip>
    </div>
  </nav>
</template>

<style scoped>
.brand-bar {
  width: 60px;
  height: 100%;
  background: var(--aipet-color-surface-soft);
  border-right: 1px solid var(--aipet-color-border-faint);
  display: flex;
  flex-direction: column;
  align-items: stretch;
  padding: var(--aipet-space-2) 0 var(--aipet-space-3);
  position: relative;
  z-index: 2;
}

.brand-bar__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.brand-bar__spacer {
  flex: 1 1 auto;
}

.brand-bar__item {
  display: flex;
  justify-content: center;
}

.brand-bar__btn {
  position: relative;
  z-index: 3;
  width: 44px;
  height: 44px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--aipet-color-text-3);
  cursor: pointer;
  border-radius: 8px;
  transition: color 120ms ease, background-color 120ms ease;
  padding: 0;
}

.brand-bar__btn:hover {
  color: var(--aipet-color-text-1);
  background: color-mix(in srgb, var(--aipet-color-text-1) 6%, transparent);
}

.brand-bar__btn:focus-visible {
  outline: none;
  box-shadow: var(--aipet-ring-focus);
}

.brand-bar__btn:active {
  background: color-mix(in srgb, var(--aipet-color-text-1) 12%, transparent);
}

.brand-bar__btn--active {
  color: var(--aipet-color-primary);
  background: color-mix(in srgb, var(--aipet-color-primary) 8%, transparent);
}

.brand-bar__btn--active::before {
  content: '';
  position: absolute;
  left: -8px;
  top: 10px;
  bottom: 10px;
  width: 2px;
  border-radius: 2px;
  background: var(--aipet-color-primary);
  animation: brand-bar-active-in 200ms var(--aipet-ease-emphasized);
}

@keyframes brand-bar-active-in {
  from {
    transform: scaleY(0);
    opacity: 0;
  }
  to {
    transform: scaleY(1);
    opacity: 1;
  }
}

/* 底部用户头像 */
.brand-bar__user {
  display: flex;
  justify-content: center;
  padding-top: var(--aipet-space-2);
}

.brand-bar__user-btn {
  width: 32px;
  height: 32px;
  padding: 0;
  border: 2px solid var(--aipet-color-primary);
  border-radius: 50%;
  background: var(--aipet-color-bg);
  cursor: pointer;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 120ms ease, box-shadow 120ms ease;
}

.brand-bar__user-btn:hover {
  transform: scale(1.06);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--aipet-color-primary) 20%, transparent);
}

.brand-bar__user-btn:focus-visible {
  outline: none;
  box-shadow: var(--aipet-ring-focus);
}

.brand-bar__user-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.brand-bar__user-fallback {
  color: var(--aipet-color-text-2);
  font-size: 12px;
  font-weight: 600;
}
</style>
