<script setup lang="ts">
// BrandBar (#33 phase B-redo)：最左 60px 列。
//
// 结构（从上到下）：
// 1) pet 头像 40×40（avatar store；点击 → setCategoryAndItem('creation', 'SettingsPersona')）
// 2) hairline 分隔
// 3) 4 类别 icon 按钮（chat / task / creation / config）
// 4) flex spacer
// 5) ❓ 帮助按钮（点击弹简易 about 模态 / 跳 docs；M3+ 实装真实弹层）
//
// active 视觉：左侧 2px primary 竖条 + icon 色 primary（与原 VSCode ActivityBar 同款，但去 IDE 感）

import { ref } from 'vue'
import { ElIcon, ElTooltip } from 'element-plus'
import { QuestionFilled } from '@element-plus/icons-vue'

import { useAvatarsStore } from '@/stores/avatars'
import { useWorkspaceLayoutStore, type CategoryId } from '@/stores/workspaceLayout'

const layout = useWorkspaceLayoutStore()
const avatars = useAvatarsStore()

const avatarFailed = ref(false)

function handleCategoryClick(id: CategoryId) {
  layout.setCategory(id)
}

function handleAvatarClick() {
  // 头像 = 桃宝身份入口；点击直接跳到「创作 → 人格」
  layout.setCategoryAndItem('creation', 'SettingsPersona')
}

function handleHelp() {
  // M3+ 实装 about 模态 / docs 跳转
  console.info('[BrandBar] help button clicked (placeholder)')
}
</script>

<template>
  <nav class="brand-bar" aria-label="桃宝工作台导航">
    <!-- 顶部：pet 头像 -->
    <div class="brand-bar__top">
      <ElTooltip content="桃宝（点击进入人格）" placement="right" :show-after="500">
        <button
          class="brand-bar__avatar"
          :class="{
            'brand-bar__avatar--active':
              layout.currentCategory === 'creation' && layout.currentItem === 'SettingsPersona',
          }"
          aria-label="桃宝"
          @click="handleAvatarClick"
        >
          <img
            v-if="avatars.personaAvatarUrl && !avatarFailed"
            :src="avatars.personaAvatarUrl"
            alt=""
            class="brand-bar__avatar-img"
            @error="avatarFailed = true"
          />
          <img v-else src="/avatar/momo-avatar.svg" alt="" class="brand-bar__avatar-img" />
        </button>
      </ElTooltip>
    </div>

    <div class="brand-bar__divider" />

    <!-- 中段：4 类别 -->
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
            @click="handleCategoryClick(cat.id)"
          >
            <ElIcon :size="20">
              <component :is="cat.icon" />
            </ElIcon>
          </button>
        </ElTooltip>
      </li>
    </ul>

    <div class="brand-bar__spacer" data-tauri-drag-region />

    <!-- 底部：辅助 -->
    <ul class="brand-bar__list brand-bar__list--bottom">
      <li class="brand-bar__item">
        <ElTooltip content="关于 / 帮助" placement="right" :show-after="500">
          <button
            class="brand-bar__btn brand-bar__btn--ghost"
            aria-label="关于 / 帮助"
            @click="handleHelp"
          >
            <ElIcon :size="18"><QuestionFilled /></ElIcon>
          </button>
        </ElTooltip>
      </li>
    </ul>
  </nav>
</template>

<style scoped>
.brand-bar {
  flex: 0 0 60px;
  width: 60px;
  height: 100%;
  background: var(--aipet-color-surface);
  border-right: 1px solid var(--aipet-color-border-faint);
  display: flex;
  flex-direction: column;
  align-items: stretch;
  /* 顶部 32px 让位给 invisible drag-bar，避免与 pet 头像点击冲突 */
  padding: calc(32px + var(--aipet-space-2)) 0 var(--aipet-space-3);
  position: relative;
  z-index: 2;
}

.brand-bar__top {
  display: flex;
  justify-content: center;
  padding: var(--aipet-space-1) 0 var(--aipet-space-2);
}

.brand-bar__avatar {
  position: relative;
  /* spec §3.2 z-index 协议：brand-bar 按钮(6) > drag-bar(5)，避免被顶部 32px invisible drag-bar 拦截点击 */
  z-index: 6;
  width: 40px;
  height: 40px;
  border-radius: 50%;
  overflow: hidden;
  background: var(--aipet-color-bg);
  border: 1px solid var(--aipet-color-border);
  padding: 0;
  cursor: pointer;
  transition: transform 0.6s var(--aipet-ease-emphasized),
    border-color 120ms ease, box-shadow 120ms ease;
}

.brand-bar__avatar:hover {
  transform: rotate(4deg) scale(1.04);
  border-color: var(--aipet-color-primary);
}

.brand-bar__avatar--active {
  border-color: var(--aipet-color-primary);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--aipet-color-primary) 25%, transparent);
}

.brand-bar__avatar-img {
  width: 100%;
  height: 100%;
  display: block;
}

.brand-bar__divider {
  height: 1px;
  margin: 0 var(--aipet-space-3);
  background: var(--aipet-color-border-faint);
}

.brand-bar__list {
  list-style: none;
  margin: 0;
  padding: var(--aipet-space-2) 0 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.brand-bar__list--bottom {
  padding: 0;
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
  /* spec §3.2 z-index 协议：brand-bar 按钮(6) > drag-bar(5) */
  z-index: 6;
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
}

.brand-bar__btn--ghost {
  color: var(--aipet-color-text-3);
  width: 36px;
  height: 36px;
}
</style>
