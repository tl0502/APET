// workspace 窗入口（#35 ADR-021 P1，Phase A 占位）
// Phase A 仅装最简 placeholder 让窗口可加载；Phase C 会替换为完整 WorkspaceApp + WorkspaceShell。
import { createApp } from 'vue'

import WorkspaceApp from './WorkspaceApp.vue'

createApp(WorkspaceApp).mount('#app')
