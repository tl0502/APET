import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import './styles/tokens.css'
import './styles/element-overrides.css'
import './styles/components.css'
import './styles/main.css'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'

import App from './App.vue'
import { useThemeStore } from '@/stores/theme'

const appComponent =
  import.meta.env.DEV && new URLSearchParams(window.location.search).get('view') === 'tokens'
    ? (await import('@/views/_dev/TokensPreview.vue')).default
    : App

const app = createApp(appComponent)
app.use(createPinia())
app.use(ElementPlus, { locale: zhCn })

// pinia 装好后立刻 init 主题：先 DOM 后 mount，避免亮 → 暗闪烁。
useThemeStore().init()

app.mount('#app')
