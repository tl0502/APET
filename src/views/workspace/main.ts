// workspace 窗口入口（#33 phase B-redo 重写 / #37 P3 补 workspace.css 全局 reset）
//
// CSS 加载顺序与其他窗一致：
//   EP → dark css-vars → tokens → element-overrides → components → panel → buttons → workspace.css
// （workspace.css 必须最后 import，让 html/body/#app reset + box-sizing 兜底生效）
//
// Phase B-redo 改动 vs Phase B：
// - 砍 dockview-vue + PlaceholderPanel
// - 砍 app.component(...) 全局注册（DetailColumn 直接 import panel SFC）
// - WorkspaceApp 内部用 useWorkspaceLayoutStore（Pinia provide 由 createPinia 全局完成）
//
// #37 P3 修：
// - 新增 workspace.css 全局 reset（之前 workspace 是唯一没有全局 reset 的窗，导致外层滚动）
// - 全局 box-sizing: border-box（防 Tauri frameless 1px overflow，已知 issue tauri#7506）

import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import '@/styles/tokens.css'
import '@/styles/element-overrides.css'
import '@/styles/components.css'
import '@/styles/panel.css'
import '@/styles/buttons.css'
import '@/styles/workspace.css'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'

import WorkspaceApp from './WorkspaceApp.vue'
import { useThemeStore } from '@/stores/theme'

const app = createApp(WorkspaceApp)
app.use(createPinia())
app.use(ElementPlus, { locale: zhCn })

useThemeStore().init()

app.mount('#app')
