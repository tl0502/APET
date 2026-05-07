// 主题 store：三态 mode（auto / light / dark）+ matchMedia 跟随系统 + localStorage 持久化。
//
// 接入路线（ADR-017）：
// - 当前 (M1-D2)：仅前端 store + DOM toggle <html class="dark"> + localStorage。
// - #9 (M1-W2)：加 `storage` event listener 跨窗口同步（pet 窗 ↔ settings 窗 / 未来 onboarding）。
//   原理：同 origin 的两个 webview 共享 localStorage；窗口 A setItem 会触发窗口 B 的 storage event
//   （但不触发自己），所以仅靠 storage event 即可单向通知，无需 Tauri event。
// - 未来 (M3 G 模块)：localStorage 迁移到 SQLite settings 表（届时改走 Tauri event）。
//
// 不引 VueUse useDark：useDark 是布尔语义（dark/light），与本 store 三态 mode 不匹配；
// auto 模式下需要"切到 light/dark 仍记得用户偏好是 auto"，useDark 做不到。

import { defineStore } from 'pinia'

export type ThemeMode = 'auto' | 'light' | 'dark'

const STORAGE_KEY = 'aipet:theme-mode'
const MEDIA_QUERY = '(prefers-color-scheme: dark)'

function loadMode(): ThemeMode {
  const raw = localStorage.getItem(STORAGE_KEY)
  if (raw === 'light' || raw === 'dark' || raw === 'auto') return raw
  return 'auto'
}

function detectSystemDark(): boolean {
  return window.matchMedia(MEDIA_QUERY).matches
}

export const useThemeStore = defineStore('theme', {
  state: () => ({
    mode: loadMode() as ThemeMode,
    systemDark: detectSystemDark(),
    _initialized: false,
  }),
  getters: {
    isDark(state): boolean {
      return state.mode === 'dark' || (state.mode === 'auto' && state.systemDark)
    },
  },
  actions: {
    /** 启动调用：注册 matchMedia + storage listener + 立刻应用 DOM。仅生效一次，重复调用 no-op。 */
    init() {
      if (this._initialized) return
      this._initialized = true
      this.applyDom()
      const mql = window.matchMedia(MEDIA_QUERY)
      const handler = (e: MediaQueryListEvent) => {
        this.systemDark = e.matches
        this.applyDom()
      }
      // Safari < 14 用 addListener，现代浏览器（含 WebView2）用 addEventListener
      // 现代浏览器（含 WebView2）走 addEventListener；
      // Safari < 14 / 老 Edge 用废弃的 addListener，类型已 deprecated 但运行时还能用。
      if (typeof mql.addEventListener === 'function') {
        mql.addEventListener('change', handler)
      } else if (typeof mql.addListener === 'function') {
        mql.addListener(handler)
      }
      // #9 跨窗口同步：另一个 webview 改 localStorage 时本窗口收到 storage event。
      // 自身 setItem 不触发本事件（浏览器规范），所以不会与本窗 setMode 死循环。
      window.addEventListener('storage', (e: StorageEvent) => {
        if (e.key !== STORAGE_KEY) return
        const next = e.newValue
        if (next === 'auto' || next === 'light' || next === 'dark') {
          this.mode = next
          this.applyDom()
        }
      })
    },

    /** 用户切换：托盘菜单或 demo 调用。 */
    setMode(next: ThemeMode) {
      this.mode = next
      localStorage.setItem(STORAGE_KEY, next)
      this.applyDom()
    },

    /** 同步 <html class="dark"> 与 isDark 一致。Element Plus 暗色 css-vars 依赖此 class。 */
    applyDom() {
      document.documentElement.classList.toggle('dark', this.isDark)
    },
  },
})
