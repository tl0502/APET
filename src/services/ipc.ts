import { invoke as tauriInvoke, type InvokeArgs } from '@tauri-apps/api/core'
import { IpcError } from '@/types/ipc'

/** 统一 IPC wrapper：失败抛 IpcError 带命令名；service 层通过此函数调用 Rust。 */
export async function invoke<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args)
  } catch (cause) {
    throw new IpcError(cmd, cause)
  }
}
