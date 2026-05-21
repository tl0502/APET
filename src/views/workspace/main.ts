// workspace 窗口入口（#35 ADR-021 P1 Phase C）
//
// CSS 加载顺序与 tasks/settings/chat 一致：
//   EP → dark css-vars → tokens → element-overrides → components → dockview
// + dockview-vue 的 CSS（最后，避免被 components.css 覆盖关键 dock 内部 token）
//
// Phase C 改动 vs Phase A：
// - 加 Pinia + ElementPlus（与其他窗一致）
// - 加 dockview-vue CSS + `app.component()` 注册 3 个 placeholder panel（spike 坑 2 落地）
// - 加 useThemeStore 防主题闪烁

import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import '@/styles/tokens.css'
import '@/styles/element-overrides.css'
import '@/styles/components.css'
import 'dockview-vue/dist/styles/dockview.css'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'

import WorkspaceApp from './WorkspaceApp.vue'
import PlaceholderPanel from './panels/PlaceholderPanel.vue'
// #33 phase B：5 settings panel 全局注册（panel.id ↔ Vue component name 一对一）
import SettingsThemePanel from '@/panels/settings/SettingsThemePanel.vue'
import SettingsProviderPanel from '@/panels/settings/SettingsProviderPanel.vue'
import SettingsPersonaPanel from '@/panels/settings/SettingsPersonaPanel.vue'
import SettingsNicknamePanel from '@/panels/settings/SettingsNicknamePanel.vue'
import SettingsAboutPanel from '@/panels/settings/SettingsAboutPanel.vue'
import { useThemeStore } from '@/stores/theme'

const app = createApp(WorkspaceApp)
app.use(createPinia())
app.use(ElementPlus, { locale: zhCn })

// spike #32 坑 2：dockview-vue 6.x 通过 `findComponent(parent, name)` 查 Vue 全局 component 注册表，
// 不是 5.x 的 named slot。Vue component 名 = PanelDescriptor.id（PanelRegistry.register 内部 regex 校验 PascalCase）。
app.component('WorkspaceChat', PlaceholderPanel) // phase D 替换为 ChatHubPanel
app.component('SettingsTheme', SettingsThemePanel)
app.component('SettingsProvider', SettingsProviderPanel)
app.component('SettingsPersona', SettingsPersonaPanel)
app.component('SettingsNickname', SettingsNicknamePanel)
app.component('SettingsAbout', SettingsAboutPanel)

useThemeStore().init()

app.mount('#app')
