// tasks 窗口入口（#22）。
// CSS 加载顺序与 src/views/settings/main.ts 一致：
//   EP → dark css-vars → tokens → element-overrides → components → tasks.css
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import '@/styles/tokens.css'
import '@/styles/element-overrides.css'
import '@/styles/components.css'
import '@/styles/tasks.css'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'

import TasksApp from './TasksApp.vue'
import { useThemeStore } from '@/stores/theme'

const app = createApp(TasksApp)
app.use(createPinia())
app.use(ElementPlus, { locale: zhCn })

// 与 settings / chat / onboarding 窗一样，mount 前 init 主题避免亮暗闪烁
useThemeStore().init()

app.mount('#app')
