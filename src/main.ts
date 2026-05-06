import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'

import App from './App.vue'
import './styles/main.css'
import { useThemeStore } from '@/stores/theme'

const app = createApp(App)
app.use(createPinia())
app.use(ElementPlus, { locale: zhCn })

// pinia 装好后立刻 init 主题：先 DOM 后 mount，避免亮 → 暗闪烁。
useThemeStore().init()

app.mount('#app')
