import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'
import path from 'node:path'

// 显式锁定绝对路径，绕过 Vite 在含中文目录（如 D:\Project\ai桌宠）下相对路径解析问题。
const projectRoot = fileURLToPath(new URL('.', import.meta.url))
const host = process.env.TAURI_DEV_HOST

export default defineConfig(({ command }) => ({
  plugins: [vue()],
  root: projectRoot,
  publicDir: path.resolve(projectRoot, 'public'),
  resolve: {
    alias: {
      '@': path.resolve(projectRoot, 'src'),
    },
  },
  clearScreen: false,
  server: {
    // 端口 1420（Tauri 2 默认）。本机 Windows HyperV TCP 排除范围 1423-1522 包含 1430，
    // 所以不能用 Tauri 1.x 旧默认 1430。HMR 同步从 1431 → 1421。
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    fs: {
      allow: [projectRoot],
    },
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  // production 构建时 strip console.log / debugger，避免 release 二进制污染控制台 + 暴露内部 metric 名。
  // dev 期保留 console 用于调试。
  esbuild: {
    drop: command === 'build' ? ['console', 'debugger'] : [],
  },
  build: {
    outDir: path.resolve(projectRoot, 'dist'),
    target: ['es2021', 'chrome105'],
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    // Multi-page（issue #9 / #14 / #16）：每个独立 Tauri 窗口对应一个 html 入口。
    // input 走绝对路径（同 root 的中文路径绕过原因），rollup 据此打多份 chunk。
    rollupOptions: {
      input: {
        main: path.resolve(projectRoot, 'index.html'),
        settings: path.resolve(projectRoot, 'settings.html'),
        chat: path.resolve(projectRoot, 'chat.html'),
        onboarding: path.resolve(projectRoot, 'onboarding.html'),
      },
    },
  },
}))
