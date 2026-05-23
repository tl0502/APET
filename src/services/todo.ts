// #29 TodoService 前端 IPC wrapper。
// 与 src-tauri/src/commands/todo.rs 同步；6 命令。
// camelCase 字段名是 serde rename + Tauri 自动转换的产物，前端使用 camelCase。

import { invoke } from './ipc'
import type { Todo, TodoCreateInput, TodoUpdateInput } from '@/types/todo'

export function createTodo(input: TodoCreateInput): Promise<Todo> {
  return invoke<Todo>('todo_create', { input })
}

export function listTodos(): Promise<Todo[]> {
  return invoke<Todo[]>('todo_list')
}

export function updateTodo(id: string, input: TodoUpdateInput): Promise<Todo> {
  return invoke<Todo>('todo_update', { id, input })
}

export function completeTodo(id: string): Promise<Todo> {
  return invoke<Todo>('todo_complete', { id })
}

/** M3+ 才实现；当前后端返 BreakdownNotImplemented。 */
export function breakdownTodo(id: string): Promise<string[]> {
  return invoke<string[]>('todo_breakdown', { id })
}

/** afterId=null 表示拖到最前；否则插到 afterId 行之后（midpoint 分数序）。 */
export function reorderTodo(id: string, afterId: string | null): Promise<Todo> {
  return invoke<Todo>('todo_reorder', { id, afterId })
}
