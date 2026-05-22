<script setup lang="ts">
// PopupSidebar（#37 2026-05-21 重设计）：popup 左 240 列。
//
// 三段（spec §4.2）：
// 1) User identity card（固定，整块点击 → 切到 profile）
// 2) 搜索框（固定，Phase 1 客户端 nav 项过滤）
// 3) Nav 列表（滚动，3 分组扁平结构）
//
// 复用 useNicknameStore / useAvatarsStore（与磁吸 chat 窗同源）。

import { computed, onMounted, ref } from 'vue'

import { useUserPopupStore, type PopupNavId } from '@/stores/userPopup'
import { useNicknameStore } from '@/stores/nickname'
import { useAvatarsStore } from '@/stores/avatars'

const popup = useUserPopupStore()
const nickname = useNicknameStore()
const avatars = useAvatarsStore()

const searchQuery = ref('')

interface NavItemDef {
  id: PopupNavId
  label: string
  icon: string
  badge?: string
  disabled?: boolean
}

interface NavGroupDef {
  title: string
  items: NavItemDef[]
}

const NAV_GROUPS: NavGroupDef[] = [
  {
    title: '个人',
    items: [
      { id: 'profile', label: '个人资料', icon: '👤' },
      { id: 'account', label: '账户', icon: '🔑', badge: '登录后', disabled: true },
    ],
  },
  {
    title: '应用',
    items: [
      { id: 'privacy', label: '数据与隐私', icon: '🔒', badge: 'M3+', disabled: true },
      { id: 'notifications', label: '通知', icon: '🔔', badge: 'M3+', disabled: true },
    ],
  },
  {
    title: '支持',
    items: [
      { id: 'help', label: '帮助', icon: '❓' },
      { id: 'about', label: '关于', icon: 'ⓘ' },
    ],
  },
]

// 客户端 nav 项过滤：模糊匹配 label
const filteredGroups = computed<NavGroupDef[]>(() => {
  const q = searchQuery.value.trim().toLowerCase()
  if (!q) return NAV_GROUPS
  return NAV_GROUPS.map((g) => ({
    ...g,
    items: g.items.filter((i) => i.label.toLowerCase().includes(q)),
  })).filter((g) => g.items.length > 0)
})

function onIdentityClick() {
  popup.setNav('profile')
}

function onNavClick(item: NavItemDef) {
  if (item.disabled) return
  popup.setNav(item.id)
}

onMounted(async () => {
  // 拉用户昵称 + avatar（popup 复用 store；store 自带 loaded 守卫，多次调安全）
  await Promise.all([nickname.load(), avatars.load()])
  await Promise.all([nickname.ensureListener(), avatars.ensureListener()])
})
</script>

<template>
  <aside class="popup-sidebar" aria-label="用户设置导航">
    <!-- 1) User identity card -->
    <button
      class="popup-sidebar__identity"
      type="button"
      aria-label="编辑个人资料"
      @click="onIdentityClick"
    >
      <div class="popup-sidebar__identity-avatar">
        <img
          v-if="avatars.userAvatarUrl"
          :src="avatars.userAvatarUrl"
          alt=""
          class="popup-sidebar__identity-img"
        />
        <span v-else>{{ nickname.user?.[0] ?? '你' }}</span>
      </div>
      <div class="popup-sidebar__identity-info">
        <div class="popup-sidebar__identity-name">
          {{ nickname.user ?? '未设置昵称' }}
        </div>
        <div class="popup-sidebar__identity-edit">编辑资料</div>
      </div>
    </button>

    <!-- 2) 搜索 -->
    <div class="popup-sidebar__search">
      <span class="popup-sidebar__search-icon" aria-hidden="true">🔍</span>
      <input
        v-model="searchQuery"
        type="text"
        class="popup-sidebar__search-input"
        placeholder="搜索设置..."
        aria-label="搜索设置"
      />
    </div>

    <!-- 3) Nav 分组列表 -->
    <nav class="popup-sidebar__nav" aria-label="设置分组">
      <div
        v-for="group in filteredGroups"
        :key="group.title"
        class="popup-sidebar__nav-group"
      >
        <div class="popup-sidebar__nav-group-title">{{ group.title }}</div>
        <button
          v-for="item in group.items"
          :key="item.id"
          type="button"
          class="popup-sidebar__nav-item"
          :class="{
            'popup-sidebar__nav-item--active':
              !item.disabled && popup.activeNav === item.id,
            'popup-sidebar__nav-item--disabled': item.disabled,
          }"
          :disabled="item.disabled"
          :aria-current="popup.activeNav === item.id ? 'page' : undefined"
          @click="onNavClick(item)"
        >
          <span class="popup-sidebar__nav-item-icon">{{ item.icon }}</span>
          <span class="popup-sidebar__nav-item-label">{{ item.label }}</span>
          <span v-if="item.badge" class="popup-sidebar__nav-item-badge">
            {{ item.badge }}
          </span>
        </button>
      </div>
    </nav>
  </aside>
</template>

<style scoped>
.popup-sidebar {
  background: var(--aipet-color-surface-soft);
  border-right: 1px solid var(--aipet-color-border-faint);
  display: flex;
  flex-direction: column;
  min-height: 0;
}

/* 1) identity card */
.popup-sidebar__identity {
  flex: 0 0 auto;
  margin: var(--aipet-space-4) var(--aipet-space-3) var(--aipet-space-3);
  padding: var(--aipet-space-3);
  display: flex;
  align-items: center;
  gap: var(--aipet-space-3);
  background: var(--aipet-color-bg);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: 12px;
  cursor: pointer;
  text-align: left;
  transition:
    border-color 120ms ease,
    box-shadow 120ms ease;
}

.popup-sidebar__identity:hover {
  border-color: var(--aipet-color-border);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
}

.popup-sidebar__identity:focus-visible {
  outline: none;
  border-color: var(--aipet-color-primary);
  box-shadow: var(--aipet-ring-focus);
}

.popup-sidebar__identity-avatar {
  width: 44px;
  height: 44px;
  flex: 0 0 auto;
  border-radius: 50%;
  overflow: hidden;
  background: var(--aipet-color-surface);
  color: var(--aipet-color-text-2);
  font-size: 16px;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--aipet-color-border);
}

.popup-sidebar__identity-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.popup-sidebar__identity-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.popup-sidebar__identity-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--aipet-color-text-1);
  line-height: 1.3;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.popup-sidebar__identity-edit {
  font-size: 12px;
  color: var(--aipet-color-text-3);
  line-height: 1.3;
  transition: color 120ms ease;
}

.popup-sidebar__identity:hover .popup-sidebar__identity-edit {
  color: var(--aipet-color-primary);
}

/* 2) 搜索 */
.popup-sidebar__search {
  flex: 0 0 auto;
  position: relative;
  margin: 0 var(--aipet-space-3) var(--aipet-space-3);
}

.popup-sidebar__search-icon {
  position: absolute;
  left: 12px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 12px;
  color: var(--aipet-color-text-3);
  pointer-events: none;
}

.popup-sidebar__search-input {
  width: 100%;
  height: 32px;
  padding: 0 12px 0 32px;
  background: var(--aipet-color-bg);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: 16px;
  font-size: 13px;
  color: var(--aipet-color-text-1);
  outline: none;
  box-sizing: border-box;
  font-family: inherit;
  transition: border-color 120ms ease, box-shadow 120ms ease;
}

.popup-sidebar__search-input:focus {
  border-color: var(--aipet-color-primary);
  box-shadow: var(--aipet-ring-focus);
}

/* 3) Nav */
.popup-sidebar__nav {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: var(--aipet-space-1) var(--aipet-space-2) var(--aipet-space-4);
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
  min-height: 0;
}

.popup-sidebar__nav-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.popup-sidebar__nav-group-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--aipet-color-text-3);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  padding: var(--aipet-space-1) var(--aipet-space-3) var(--aipet-space-2);
  user-select: none;
}

.popup-sidebar__nav-item {
  position: relative;
  width: 100%;
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
  padding: 7px var(--aipet-space-3);
  border-radius: 6px;
  font-size: 14px;
  color: var(--aipet-color-text-2);
  background: transparent;
  border: none;
  cursor: pointer;
  min-height: 32px;
  text-align: left;
  font-family: inherit;
  transition: background-color 100ms ease, color 100ms ease;
}

.popup-sidebar__nav-item:hover {
  background: color-mix(in srgb, var(--aipet-color-text-1) 5%, transparent);
  color: var(--aipet-color-text-1);
}

.popup-sidebar__nav-item:focus-visible {
  outline: none;
  box-shadow: var(--aipet-ring-focus);
}

.popup-sidebar__nav-item--active {
  background: color-mix(in srgb, var(--aipet-color-primary) 12%, transparent);
  color: var(--aipet-color-primary);
  font-weight: 500;
}

.popup-sidebar__nav-item--active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 8px;
  bottom: 8px;
  width: 2px;
  border-radius: 2px;
  background: var(--aipet-color-primary);
}

.popup-sidebar__nav-item--disabled {
  color: var(--aipet-color-text-3);
  cursor: not-allowed;
  opacity: 0.5;
}

.popup-sidebar__nav-item--disabled:hover {
  background: transparent;
  color: var(--aipet-color-text-3);
}

.popup-sidebar__nav-item-icon {
  font-size: 15px;
  width: 18px;
  text-align: center;
  flex: 0 0 auto;
}

.popup-sidebar__nav-item-label {
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.popup-sidebar__nav-item-badge {
  font-size: 10px;
  color: var(--aipet-color-text-3);
  background: color-mix(in srgb, var(--aipet-color-text-1) 5%, transparent);
  padding: 1px 6px;
  border-radius: 4px;
  flex: 0 0 auto;
}
</style>
