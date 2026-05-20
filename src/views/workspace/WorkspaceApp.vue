<script setup lang="ts">
// WorkspaceApp：workspace 独立窗口 root（#35 ADR-021 P1 Phase C）
//
// 职责（按 onMounted 时序）：
// 1) new WorkspaceManager({ persistence: KvWorkspacePersistence }) + provide WORKSPACE_MANAGER_KEY
// 2) registerPanel × 3（MVP placeholder：WorkspaceChat / WorkspaceLibrary / WorkspaceSettings）
// 3) registerCommand × 5（reveal × 3 + close active + togglePalette 占位）
// 4) WorkspaceShell @ready → loadLayoutFromKv（成功 → layout 还原；失败 / 无 KV → openPanel 3 个 default）
// 5) loadLastActiveFromKv → revealPanel(savedId) 还原"上次最后一个 panel"
// 6) emit_visibility_changed listener：WebView2 hide 时 DOM visibilitychange 不触发（lib.rs 后端
//    手动 emit），监听后 saveLayout 落盘（避免崩溃丢失最近 layout 变更）
//
// onBeforeUnmount：再 saveLayout + saveLastActive 一次（"关 = hide" 不触发，但极少数 quit 路径触发）

import { onBeforeUnmount, onMounted, provide, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import { ChatLineRound, Files, Setting } from '@element-plus/icons-vue'

import AppShell from '@/components/layouts/AppShell.vue'
import WorkspaceShell from './WorkspaceShell.vue'
import ActivityBar from './ActivityBar.vue'
import CommandPalette from './CommandPalette.vue'
import PlaceholderPanel from './panels/PlaceholderPanel.vue'

import { WORKSPACE_MANAGER_KEY } from '@/composables/useWorkspaceManager'
import { WorkspaceManager } from '@/lib/workspace/manager'
import { KvWorkspacePersistence } from '@/lib/workspace/persistence'
import type { PanelDescriptor } from '@/lib/workspace/types'
import { getConfig, setConfig } from '@/services/config'
import { hideWorkspace } from '@/services/window'

// === 单实例 WorkspaceManager（全窗生命周期）===
const mgr = new WorkspaceManager({
  persistence: new KvWorkspacePersistence({
    getKv: (key) => getConfig(key),
    setKv: (key, value) => setConfig(key, value),
  }),
})
provide(WORKSPACE_MANAGER_KEY, mgr)

const ready = ref(false)
const unlistenFns: UnlistenFn[] = []

// === Panel descriptors（M2 阶段 3 个占位；P2 #33 替换为真业务 panel）===
const PANELS: PanelDescriptor[] = [
  {
    id: 'WorkspaceChat',
    title: '对话',
    component: PlaceholderPanel,
    category: 'chat',
    mountStrategy: 'always', // 对话场景保 form state；spike #32 验证 dockview 'always' renderer = 内置 keep-alive
    defaultLocation: 'main',
    icon: ChatLineRound,
  },
  {
    id: 'WorkspaceLibrary',
    title: '资源库',
    component: PlaceholderPanel,
    category: 'creation',
    mountStrategy: 'lazy',
    defaultLocation: 'main.right',
    icon: Files,
  },
  {
    id: 'WorkspaceSettings',
    title: '设置',
    component: PlaceholderPanel,
    category: 'config',
    mountStrategy: 'lazy',
    defaultLocation: 'main.right',
    icon: Setting,
  },
]

function registerDefaults() {
  for (const desc of PANELS) {
    try {
      mgr.registerPanel(desc)
    } catch (e) {
      console.warn('[WorkspaceApp] registerPanel failed:', desc.id, e)
    }
  }
  // 5 命令（Phase D 命令面板会消费这些；Phase C 阶段 ActivityBar 也已能 revealPanel 复用同样语义）
  for (const id of ['WorkspaceChat', 'WorkspaceLibrary', 'WorkspaceSettings']) {
    try {
      mgr.registerCommand({
        id: `panel.reveal.${id}`,
        title: `打开 ${PANELS.find((p) => p.id === id)?.title ?? id}`,
        handler: () => {
          mgr.revealPanel(id)
        },
      })
    } catch (e) {
      console.warn('[WorkspaceApp] registerCommand failed:', id, e)
    }
  }
  try {
    mgr.registerCommand({
      id: 'workspace.closeActive',
      title: '关闭当前 panel',
      handler: () => {
        const active = mgr.getActivePanel()
        if (active) void mgr.closePanel(active)
      },
    })
  } catch (e) {
    console.warn('[WorkspaceApp] register closeActive failed:', e)
  }
  try {
    mgr.registerCommand({
      id: 'workspace.togglePalette',
      title: '命令面板（Ctrl+P）',
      handler: () => {
        const cur = mgr.getContextKey('paletteVisible') === true
        mgr.setContextKey('paletteVisible', !cur)
      },
    })
  } catch (e) {
    console.warn('[WorkspaceApp] register togglePalette failed:', e)
  }
}

async function restoreLayout() {
  // KV 有 → 走还原；失败 / 无 KV → openPanel 3 个 default
  try {
    await mgr.loadLayoutFromKv()
  } catch (e) {
    console.warn('[WorkspaceApp] loadLayoutFromKv failed:', e)
  }
  // 判定是否还原成功：还原后 isPanelOpen 至少一个为 true（KV 损坏 self-heal 后 KV 为空，
  // openPanels = ∅）
  const restored = PANELS.some((p) => mgr.isPanelOpen(p.id))
  if (!restored) {
    // default：开 3 个 panel；defaultLocation 决定位置（main / main.right / main.right）
    try {
      mgr.openPanel('WorkspaceChat', { tone: 'chat' })
      mgr.openPanel('WorkspaceLibrary', { tone: 'library' })
      mgr.openPanel('WorkspaceSettings', { tone: 'settings' })
    } catch (e) {
      console.warn('[WorkspaceApp] default openPanel failed:', e)
    }
  }
  // 上次最后 active panel（若已 open 则切到它；不存在则保持 default = 最后开的那个）
  try {
    const last = await mgr.loadLastActiveFromKv()
    if (last && mgr.isPanelOpen(last)) mgr.revealPanel(last)
  } catch (e) {
    console.warn('[WorkspaceApp] loadLastActiveFromKv failed:', e)
  }
}

function onShellReady() {
  ready.value = true
  void restoreLayout()
}

async function saveSnapshot() {
  try {
    await mgr.saveLayoutToKv()
  } catch (e) {
    console.warn('[WorkspaceApp] saveLayoutToKv failed:', e)
  }
  try {
    await mgr.saveLastActiveToKv()
  } catch (e) {
    console.warn('[WorkspaceApp] saveLastActiveToKv failed:', e)
  }
}

async function onClose() {
  // 关 = hide（lib.rs CloseRequested 拦截 + 联判，本路径是用户点 ✕ 走 IPC）
  // 先 saveSnapshot 再 hide，避免极少数 hide 立即销毁 webview 路径丢失 layout
  await saveSnapshot()
  try {
    await hideWorkspace()
  } catch (e) {
    console.warn('[WorkspaceApp] hideWorkspace failed:', e)
  }
}

function onGlobalKeydown(e: KeyboardEvent) {
  // Ctrl+P (Cmd+P) → 命令面板（仅 workspace 窗内生效；与浏览器"打印"冲突由 preventDefault 处理）
  // 不挂 Ctrl+Shift+P：MVP 阶段单触发够用，避免与系统/IDE 撞
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'p') {
    e.preventDefault()
    try {
      void mgr.executeCommand('workspace.togglePalette')
    } catch (err) {
      console.warn('[WorkspaceApp] toggle palette via Ctrl+P failed:', err)
    }
    return
  }

  if (e.key !== 'Escape') return
  // Esc 弹窗 / input 聚焦时让组件 cancel，不一律 hide
  if (document.querySelector('.el-message-box, .el-dialog__wrapper, .el-overlay')) return
  const active = document.activeElement
  if (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement) return
  void onClose()
}

onMounted(async () => {
  registerDefaults()

  window.addEventListener('keydown', onGlobalKeydown)

  // emit_visibility_changed：lib.rs 在 show/hide 时 emit `window:visibility-changed`
  // payload = { label, visible }。hide 之前 save snapshot 一次。
  try {
    const un = await listen<{ label: string; visible: boolean }>(
      'window:visibility-changed',
      async (event) => {
        if (event.payload.label === 'workspace' && event.payload.visible === false) {
          await saveSnapshot()
        }
      },
    )
    unlistenFns.push(un)
  } catch (e) {
    console.warn('[WorkspaceApp] listen visibility-changed failed:', e)
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onGlobalKeydown)
  unlistenFns.forEach((u) => u())
  // unmount 时也 save 一次（极少数 quit 路径走这）
  void saveSnapshot()
})
</script>

<template>
  <AppShell variant="standalone">
    <template #header>
      <span class="workspace-shell-title" data-tauri-drag-region>工作台</span>
      <span class="aipet-shell__header-spacer" data-tauri-drag-region />
      <button
        class="aipet-shell__close"
        title="关闭（进托盘）"
        aria-label="关闭"
        data-tauri-drag-region="false"
        @click="onClose"
      >✕</button>
    </template>
    <WorkspaceShell @ready="onShellReady">
      <template #activity>
        <ActivityBar v-if="ready" />
      </template>
    </WorkspaceShell>
    <CommandPalette v-if="ready" />
  </AppShell>
</template>

<style scoped>
.workspace-shell-title {
  font-size: var(--aipet-font-size-base);
  font-weight: 500;
  color: var(--aipet-color-text-1);
  padding: 0 var(--aipet-space-3);
}
</style>
