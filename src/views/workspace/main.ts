// workspace 窗口入口（#33 phase B-redo 重写：三栏 Desktop App Shell）
//
// CSS 加载顺序与其他窗一致：
//   EP → dark css-vars → tokens → element-overrides → components
// （dockview-vue 已砍，少 ~103KB CSS）
//
// Phase B-redo 改动 vs Phase B：
// - 砍 dockview-vue + PlaceholderPanel
// - 砍 app.component(...) 全局注册（DetailColumn 直接 import panel SFC）
// - WorkspaceApp 内部用 useWorkspaceLayoutStore（Pinia provide 由 createPinia 全局完成）

import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import '@/styles/tokens.css'
import '@/styles/element-overrides.css'
import '@/styles/components.css'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'

import WorkspaceApp from './WorkspaceApp.vue'
import { useThemeStore } from '@/stores/theme'

const app = createApp(WorkspaceApp)
app.use(createPinia())
app.use(ElementPlus, { locale: zhCn })

useThemeStore().init()

app.mount('#app')
