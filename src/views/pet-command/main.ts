// pet-command overlay 入口。同 pet-reminder 风格（透明窗，无 EP/Pinia/主题 store）。
import { createApp } from 'vue'
import '@/styles/tokens.css'
import '@/styles/components.css'
import PetCommandOverlayApp from './PetCommandOverlayApp.vue'

const app = createApp(PetCommandOverlayApp)
app.mount('#app')
