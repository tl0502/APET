// 窗口角色（ADR-020 #30 follow-up D *Updated 2026-05-19*）。
//
// 主体（primary）/ 子体（secondary）二元模型。当前仅 pet 是 primary，其他窗（chat /
// future settings / tasks 等）都是 secondary。primary 与 secondary 在磁吸语义上不对称：
//
// - primary 拖动：若 primary 自身是 anchor（有 dependents），走 group-drag 平移整族；
//   若 primary 无 dependents，走 primary-attract 反向吸引附近 secondary。
// - secondary 拖动：走 source 模式，首帧 detachAll 立即脱钩。
// - primary 永远不能是任何窗的 source（commit 路径强制 secondary → primary）。
//
// 不变量：
// - I3：constraint.sourceId 永远不是 primary（commit 路径 + cleanupDirtyPrimaryOutbound 保证）
//
// 抽离原因：原 PRIMARY_LABEL = 'pet' 散布在 useSnapWindow 的 mode 判定 / cleanup helper /
// onPointerDown sanity check 三处。M3 多 primary 配置 UI 时只需改本文件，调用方逻辑不动。

/** primary 窗 label 集合。常量 — 编译期确定，dev/prod 一致。
 *  M3 follow-up：改成 ref<Set<string>> + KV 持久化 + Settings UI 配置时，
 *  isPrimary 改成 reactive，调用方需 `computed(() => isPrimary(label))`。 */
const PRIMARY_LABELS: ReadonlySet<string> = new Set(['pet'])

/** 主 primary（用于诊断日志 / 兜底 cleanup helper 取单个 primary 时）。
 *  M3 多 primary 后此 helper 语义模糊 — 调用方应迁移到 isPrimary(label)。 */
export const PRIMARY_LABEL = 'pet'

/** 判定一个窗 label 是否为 primary 角色。常量集合查询，O(1)。 */
export function isPrimary(label: string): boolean {
  return PRIMARY_LABELS.has(label)
}

/** 返回所有 primary label（数组）。当前仅 ['pet']。 */
export function primaryLabels(): readonly string[] {
  return Array.from(PRIMARY_LABELS)
}
