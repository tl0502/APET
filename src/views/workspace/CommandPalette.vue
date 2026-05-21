<script setup lang="ts">
// CommandPalette：Ctrl+P 命令面板（#35 ADR-021 P1 Phase D）
//
// 设计（ADR-021 D7）：
// - ElDialog + ElInput + 手写 fuzzyFilter 结果列表（fuse.js 6KB gzip 不值，自家 fuzzyMatch ~40 行够）
// - reactive 可见性 = `useContextKey<boolean>('paletteVisible')`，open/close 由 mgr 控制
// - 键盘交互：Esc 关 / Enter 执行第一项 / ↑↓ 移动选中 / 输入更新过滤
// - input.focus on open（autofocus + visibility watcher 兜底）
// - listCommands(true) → 自动按 when DSL 过滤掉禁用命令（避免显示后点不动）
//
// MVP 简化：
// - 不做命令分类（VSCode 是 @ 分组前缀；MVP 单列表足够）
// - 不做最近使用历史（MVP 命令量小没必要）
// - 高亮匹配字符跳过（MVP 简化；P2 优化时用 matchResult.indices 渲染）

import { computed, nextTick, onBeforeUnmount, onMounted, ref, useTemplateRef, watch } from 'vue'
import { ElDialog, ElInput } from 'element-plus'

import { fuzzyFilter } from '@/lib/workspace/fuzzyMatch'
import type { Command } from '@/lib/workspace/types'
import { useContextKey } from '@/composables/useContextKey'
import { useWorkspaceManager } from '@/composables/useWorkspaceManager'

const mgr = useWorkspaceManager()

// 可见性桥接：mgr.setContextKey('paletteVisible', true/false) → ref 自动跟随
const paletteVisible = useContextKey<boolean>('paletteVisible')
const visible = computed({
  get: () => paletteVisible.value === true,
  set: (v) => mgr.setContextKey('paletteVisible', v),
})

const query = ref('')
const activeIdx = ref(0)
const inputRef = useTemplateRef<InstanceType<typeof ElInput>>('inputRef')

/**
 * commandsVersion：单调 bump 触发 `filtered` computed 重跑 listCommands。
 * review P1 修复 (F-3.4)：动态 registerCommand/unregisterCommand 时，已打开的 palette
 * 必须能反映新增/删除。listCommands 返回数组快照不 reactive，所以走 onCommandsChanged 订阅
 * + version bump 的标准模式（同 ActivityBar 的 panelsVersion）。
 */
const commandsVersion = ref(0)

const filtered = computed(() => {
  void commandsVersion.value // 显式依赖 → commands 变化时重跑
  const all = mgr.listCommands(true) // when=false 的命令过滤掉
  return fuzzyFilter(query.value, all, (c) => c.title)
})

let unsubCommandsChanged: (() => void) | null = null

onMounted(() => {
  unsubCommandsChanged = mgr.onCommandsChanged(() => {
    commandsVersion.value++
  })
})

onBeforeUnmount(() => {
  unsubCommandsChanged?.()
})

// 选中行索引自适应：filtered 收窄到 activeIdx 之外时回到 0
watch(filtered, (list) => {
  if (activeIdx.value >= list.length) activeIdx.value = 0
})

// 打开时清 query + 重置 activeIdx + 焦点入 input（nextTick 等 dialog DOM 挂上）
watch(visible, async (open) => {
  if (open) {
    query.value = ''
    activeIdx.value = 0
    await nextTick()
    inputRef.value?.focus()
  }
})

function close() {
  visible.value = false
}

async function execute(cmd: Command) {
  close()
  try {
    await mgr.executeCommand(cmd.id)
  } catch (e) {
    console.warn('[CommandPalette] executeCommand failed:', cmd.id, e)
  }
}

function onKeydown(e: KeyboardEvent) {
  switch (e.key) {
    case 'Escape':
      e.preventDefault()
      close()
      break
    case 'Enter': {
      e.preventDefault()
      const cmd = filtered.value[activeIdx.value]
      if (cmd) void execute(cmd)
      break
    }
    case 'ArrowDown':
      e.preventDefault()
      if (filtered.value.length === 0) return
      activeIdx.value = (activeIdx.value + 1) % filtered.value.length
      break
    case 'ArrowUp':
      e.preventDefault()
      if (filtered.value.length === 0) return
      activeIdx.value =
        (activeIdx.value - 1 + filtered.value.length) % filtered.value.length
      break
  }
}
</script>

<template>
  <ElDialog
    v-model="visible"
    :show-close="false"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    width="520"
    align-center
    append-to-body
    class="command-palette"
  >
    <div class="command-palette__inner" @keydown="onKeydown">
      <ElInput
        ref="inputRef"
        v-model="query"
        placeholder="输入命令名搜索…"
        size="large"
        class="command-palette__input"
        :prefix-icon="undefined"
      />
      <div class="command-palette__list" role="listbox" aria-label="命令列表">
        <button
          v-for="(cmd, idx) in filtered"
          :key="cmd.id"
          class="command-palette__item"
          :class="{ 'command-palette__item--active': idx === activeIdx }"
          role="option"
          :aria-selected="idx === activeIdx"
          @click="execute(cmd)"
          @mouseenter="activeIdx = idx"
        >
          <span class="command-palette__title">{{ cmd.title }}</span>
          <span class="command-palette__id">{{ cmd.id }}</span>
        </button>
        <div v-if="filtered.length === 0" class="command-palette__empty">
          没找到匹配命令
        </div>
      </div>
      <div class="command-palette__hint">
        <span><kbd>↑</kbd><kbd>↓</kbd> 切换</span>
        <span><kbd>Enter</kbd> 执行</span>
        <span><kbd>Esc</kbd> 关闭</span>
      </div>
    </div>
  </ElDialog>
</template>

<style scoped>
/* === Dialog 本体外观（去 title bar、加圆角与浮起阴影） === */
.command-palette :deep(.el-dialog) {
  padding: 0;
  border-radius: var(--aipet-radius-window, 14px);
  overflow: hidden;
  box-shadow: var(--aipet-shadow-float);
  background: var(--aipet-color-bg);
}

.command-palette :deep(.el-dialog__header) {
  display: none;
}

.command-palette :deep(.el-dialog__body) {
  padding: 0;
}

.command-palette__inner {
  display: flex;
  flex-direction: column;
  background: var(--aipet-color-bg);
}

/* === input === */
.command-palette__input :deep(.el-input__wrapper) {
  border-radius: 0;
  box-shadow: none;
  border-bottom: 1px solid var(--aipet-color-border-faint);
  padding: var(--aipet-space-3) var(--aipet-space-4);
  background: transparent;
}

.command-palette__input :deep(.el-input__wrapper.is-focus),
.command-palette__input :deep(.el-input__wrapper:hover) {
  box-shadow: none;
  border-bottom-color: var(--aipet-color-primary);
}

.command-palette__input :deep(.el-input__inner) {
  font-size: 15px;
  color: var(--aipet-color-text-1);
}

/* === 列表 === */
.command-palette__list {
  max-height: 360px;
  overflow-y: auto;
  padding: var(--aipet-space-1) 0;
}

.command-palette__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--aipet-space-3);
  width: 100%;
  padding: var(--aipet-space-2) var(--aipet-space-4);
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--aipet-color-text-1);
  font-size: 14px;
  text-align: left;
  transition: background-color 80ms ease;
}

.command-palette__item--active {
  background: color-mix(in srgb, var(--aipet-color-primary) 10%, transparent);
}

.command-palette__item:active {
  background: color-mix(in srgb, var(--aipet-color-primary) 18%, transparent);
}

.command-palette__title {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command-palette__id {
  flex: 0 0 auto;
  font-size: 11px;
  color: var(--aipet-color-text-3);
  font-family: var(--aipet-font-family-mono, ui-monospace, monospace);
}

.command-palette__empty {
  padding: var(--aipet-space-4) var(--aipet-space-4) var(--aipet-space-5);
  color: var(--aipet-color-text-3);
  font-size: 13px;
  text-align: center;
}

/* === hint === */
.command-palette__hint {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-4);
  padding: var(--aipet-space-2) var(--aipet-space-4);
  border-top: 1px solid var(--aipet-color-border-faint);
  color: var(--aipet-color-text-3);
  font-size: 11px;
  background: var(--aipet-color-surface);
}

.command-palette__hint kbd {
  display: inline-block;
  padding: 1px 5px;
  margin-right: 4px;
  border-radius: 3px;
  border: 1px solid var(--aipet-color-border);
  background: var(--aipet-color-bg);
  font-family: var(--aipet-font-family-mono, ui-monospace, monospace);
  font-size: 10px;
  line-height: 1.2;
  color: var(--aipet-color-text-2);
}
</style>
