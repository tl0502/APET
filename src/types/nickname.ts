/** NicknameService IPC 事件 payload（架构 §711 'nickname:changed'）。
 *
 * M1（2026-05-09 起）：
 * - which 仅 'user'。pet 改名机制已移除（宠物名字源唯一化为 .soul.md persona.name，
 *   相关变更走 'persona:activated' 事件）。
 * - value 永远是非空 string（emit 仅发生在 set_user_nickname 写库成功后；
 *   validate_nickname 拒空白 + IPC 入口无"清空"通道）。
 *   "首次未设置"是 DB NULL 初值，由 getUserNickname 返 null 表达，与此 payload 无关。
 *
 * 2026-05-10 收窄：移除 'pet' 字面与 null 死路径——后端类型与此对齐
 * （NicknameChangedPayload.value: String）。将来若需要"清空昵称"feature，开专门 IPC，
 * 别复用本 payload 重新加 null。
 */
export interface NicknameChangedPayload {
  which: 'user'
  value: string
}
