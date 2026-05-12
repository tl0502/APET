// LivingPetService IPC binding（#21 收尾 L1）。
//
// 仅 1 个 IPC：cancelWander —— 用户拖动 / 唤起 chat / 点击桌宠前调用，立即取消正在
// 进行的 wander tween。tween 在 select! 内监听 cancellation token，立即退出保留当前
// 位置（capture current state，不 snap 到段终点）。
//
// 设计点：
// - 调用方应 fire-and-forget（不 await），避免 startDragging 等用户输入路径被 IPC
//   往返延迟影响。但仍返 Promise<void> 兼容旁路 catch；调用方自行决定是否 await。
// - 后端 no-op 兼容：无 wander 进行中 → cancel 是 noop，无副作用。

import { invoke } from './ipc'

export function cancelWander(): Promise<void> {
  return invoke<void>('living_pet_cancel_wander')
}
