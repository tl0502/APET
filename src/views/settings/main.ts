// settings 窗口入口（issue #9）。
// CSS 加载顺序与 src/main.ts 一致（EP → dark → tokens → overrides → components → settings.css），
// 但替换 main.css 为 settings.css —— pet 窗的透明背景规则不适用于此独立窗口。
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import '@/styles/tokens.css'
import '@/styles/element-overrides.css'
import '@/styles/components.css'
import '@/styles/settings.css'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'

import SettingsApp from './SettingsApp.vue'
import { useThemeStore } from '@/stores/theme'

const app = createApp(SettingsApp)
app.use(createPinia())
app.use(ElementPlus, { locale: zhCn })

// 与 pet 窗一样在 mount 之前 init 主题，避免亮 → 暗闪烁；store 内部 storage event listener
// 让本窗口收到 pet 窗 setMode 后的同步。
useThemeStore().init()

app.mount('#app')
