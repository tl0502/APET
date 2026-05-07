// useToast：项目统一 Toast facade（issue #8）。
// - 包 EP ElMessage，注入 customClass='aipet-toast' 走 components.css 的 token 化样式
// - 4 方法：success / error / info / warn（→ EP 'warning'）
// - action 参数：可选行动按钮，点击触发 handler 并自动关闭 toast
import { h } from 'vue'
import { ElMessage } from 'element-plus'
import type { MessageHandler } from 'element-plus'

export interface ToastAction {
  text: string
  handler: () => void
}

export interface ToastOptions {
  duration?: number
  action?: ToastAction
}

type ToastType = 'success' | 'error' | 'info' | 'warning'

const DEFAULT_DURATION = 3000

function show(type: ToastType, message: string, options?: ToastOptions): MessageHandler {
  // action 与 instance 通过闭包绑定：先 let 占位，VNode click 触发时 instance 已就绪。
  let instance: MessageHandler | null = null
  const action = options?.action

  const messageNode = action
    ? h('span', { class: 'aipet-toast__content' }, [
        h('span', { class: 'aipet-toast__text' }, message),
        h(
          'button',
          {
            class: 'aipet-toast__action',
            type: 'button',
            onClick: () => {
              action.handler()
              instance?.close()
            },
          },
          action.text,
        ),
      ])
    : message

  instance = ElMessage({
    type,
    message: messageNode,
    customClass: 'aipet-toast',
    duration: options?.duration ?? DEFAULT_DURATION,
  })
  return instance
}

export function useToast() {
  return {
    success: (message: string, options?: ToastOptions) => show('success', message, options),
    error: (message: string, options?: ToastOptions) => show('error', message, options),
    info: (message: string, options?: ToastOptions) => show('info', message, options),
    warn: (message: string, options?: ToastOptions) => show('warning', message, options),
  }
}
