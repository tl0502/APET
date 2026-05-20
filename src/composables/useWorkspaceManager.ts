// useWorkspaceManager：从 Vue inject 拿 WorkspaceManager 实例（#35 ADR-021 P1）
//
// 注入约定：WorkspaceApp.vue 在 onMounted 前 `provide(WORKSPACE_MANAGER_KEY, mgr)`；
// 子组件（WorkspaceShell / ActivityBar / 面板 SFC）通过本 composable 拿引用，避免：
// - 业务 SFC 自己 new WorkspaceManager（破坏单实例）
// - 业务 SFC 直接 import dockview API（破坏 ADR-021 分层）
//
// 守卫：早于 provide 调用时抛错（开发期暴露 setup 顺序错误，prod 不静默）。

import { type InjectionKey, inject } from 'vue'

import type { WorkspaceManager } from '@/lib/workspace/manager'

export const WORKSPACE_MANAGER_KEY: InjectionKey<WorkspaceManager> = Symbol('workspaceManager')

export function useWorkspaceManager(): WorkspaceManager {
  const mgr = inject(WORKSPACE_MANAGER_KEY)
  if (!mgr) {
    throw new Error(
      '[useWorkspaceManager] WorkspaceManager not provided — ensure WorkspaceApp.vue provides it before child setup',
    )
  }
  return mgr
}
