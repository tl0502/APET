/** IPC 错误：包装 tauri invoke 抛出的原始错误，带命令名上下文。 */
export class IpcError extends Error {
  public readonly command: string
  public readonly cause: unknown

  constructor(command: string, cause: unknown) {
    const msg = cause instanceof Error ? cause.message : String(cause)
    super(`IPC ${command} failed: ${msg}`)
    this.name = 'IpcError'
    this.command = command
    this.cause = cause
  }
}
