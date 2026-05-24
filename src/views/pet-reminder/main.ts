// pet-reminder overlay 入口（2026-05-24 pet UI 重构第二轮）。
// 透明窗，不加 Element Plus / Pinia / 主题 store —— PetReminderBubble 是纯 button + CSS 变量
// 实现，主题切换由 tokens.css `[data-theme]` selector 直接生效（其他窗 setMode 通过 storage
// 事件同步 documentElement，dark mode 暂不跨 overlay 窗，M3+ 看需求再补 theme listener）。
import { createApp } from 'vue'
import '@/styles/tokens.css'
import '@/styles/components.css'
import PetReminderOverlayApp from './PetReminderOverlayApp.vue'

const app = createApp(PetReminderOverlayApp)
app.mount('#app')
