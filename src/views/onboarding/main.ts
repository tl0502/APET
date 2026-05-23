// onboarding 窗口入口（issue #16）。
// CSS 加载顺序与 chat/settings 一致（EP → dark → tokens → overrides → components → onboarding.css）；
// 不复用 main.css（pet 窗的透明 + overflow:hidden 规则不适用于此独立窗口）。
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import '@/styles/tokens.css'
import '@/styles/element-overrides.css'
import '@/styles/components.css'
import '@/styles/buttons.css'
import '@/styles/onboarding.css'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'

import OnboardingApp from './OnboardingApp.vue'
import { useThemeStore } from '@/stores/theme'

const app = createApp(OnboardingApp)
app.use(createPinia())
app.use(ElementPlus, { locale: zhCn })

// 与 pet/settings/chat 一样在 mount 之前 init 主题，避免亮 → 暗闪烁；store 内部 storage event
// listener 让本窗口收到他窗 setMode 后的同步。
useThemeStore().init()

app.mount('#app')
