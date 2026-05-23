// #29 TodoService 前端类型契约。
// 与后端 services/todo.rs 同步（serde camelCase 序列化）。

export type TodoStatus = 'open' | 'done' | 'cancelled'
export type TodoPriority = 'low' | 'normal' | 'high'

export interface Todo {
  id: string
  title: string
  status: TodoStatus
  /** RFC3339 UTC；null 表示无到期时刻。 */
  dueAt: string | null
  /** 联动 reminder 的 id；due_at 非空时由后端同 tx 内创建并回填。 */
  reminderId: string | null
  /** 分数排序；前端只读不改（拖拽通过 todo_reorder 走后端）。 */
  orderIndex: number
  priority: TodoPriority
  createdAt: string
  updatedAt: string
}

export interface TodoCreateInput {
  title: string
  /** RFC3339 UTC；非空时后端同 tx 内联动创建 once reminder。 */
  dueAt?: string
  priority?: TodoPriority
}

/**
 * due_at 三态:
 * - 字段省略 (undefined) → keep 不改
 * - { kind: 'set', value: '...' } → 设置具体时刻 (RFC3339 UTC)
 * - { kind: 'clear' }             → 清空到 null
 *
 * 与后端 DueAtChange 枚举对齐（serde tag='kind', content='value'）。
 */
export type DueAtChange =
  | { kind: 'set'; value: string }
  | { kind: 'clear' }

export interface TodoUpdateInput {
  title?: string
  /** 'done' 走 todo_complete（complete-once 语义，spec §4.2）。 */
  status?: 'open' | 'cancelled'
  dueAt?: DueAtChange
  priority?: TodoPriority
}
