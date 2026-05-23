<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { ElInput, ElButton, ElButtonGroup, ElDialog, ElMessage, ElIcon } from 'element-plus'
import { Search, Calendar, List, Plus, Refresh } from '@element-plus/icons-vue'
import { storeToRefs } from 'pinia'
import { useWorkspaceLayoutStore } from '@/stores/workspaceLayout'
import type { Todo, TodoCreateInput, TodoUpdateInput, TodoPriority } from '@/types/todo'
import {
  listTodos,
  createTodo,
  updateTodo,
  completeTodo,
  reorderTodo,
} from '@/services/todo'
import TodoList from '@/components/tasks/TodoList.vue'
import TodoCalendar from '@/components/tasks/TodoCalendar.vue'
import TodoForm from '@/components/tasks/TodoForm.vue'
import TodoBatchBar from '@/components/tasks/TodoBatchBar.vue'

const layout = useWorkspaceLayoutStore()
const { todoView } = storeToRefs(layout)

const todos = ref<Todo[]>([])
const loading = ref(false)
const searchQuery = ref('')
const showAll = ref(false)
const selectedIds = ref<Set<string>>(new Set())
const formOpen = ref(false)
const editingTodo = ref<Todo | null>(null)

async function refresh() {
  loading.value = true
  try {
    todos.value = await listTodos()
  } catch (e: unknown) {
    ElMessage.error(`加载待办失败：${e}`)
  } finally {
    loading.value = false
  }
}

const filtered = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  let list = todos.value
  if (!showAll.value) list = list.filter(t => t.status === 'open')
  if (q) list = list.filter(t => t.title.toLowerCase().includes(q))
  return list
})

const canDrag = computed(
  () => !searchQuery.value.trim() && selectedIds.value.size === 0 && !showAll.value
)

function toggleSelect(id: string, checked: boolean) {
  if (checked) selectedIds.value.add(id)
  else selectedIds.value.delete(id)
  selectedIds.value = new Set(selectedIds.value)
}

function clearSelection() {
  selectedIds.value = new Set()
}

function openCreate() {
  editingTodo.value = null
  formOpen.value = true
}

function openEdit(todo: Todo) {
  editingTodo.value = todo
  formOpen.value = true
}

async function onFormSubmit(input: TodoCreateInput | TodoUpdateInput) {
  try {
    if (editingTodo.value) {
      await updateTodo(editingTodo.value.id, input as TodoUpdateInput)
    } else {
      await createTodo(input as TodoCreateInput)
    }
    formOpen.value = false
    await refresh()
  } catch (e: unknown) {
    ElMessage.error(`保存失败：${e}`)
  }
}

async function onComplete(id: string) {
  try {
    await completeTodo(id)
    await refresh()
  } catch (e: unknown) {
    ElMessage.error(`完成失败：${e}`)
  }
}

async function onCancel(id: string) {
  try {
    await updateTodo(id, { status: 'cancelled' })
    await refresh()
  } catch (e: unknown) {
    ElMessage.error(`取消失败：${e}`)
  }
}

async function onReorder(movedId: string, afterId: string | null) {
  try {
    await reorderTodo(movedId, afterId)
    await refresh()
  } catch (e: unknown) {
    ElMessage.error(`排序失败：${e}`)
  }
}

async function batchComplete() {
  const ids = Array.from(selectedIds.value)
  const results = await Promise.allSettled(ids.map(id => completeTodo(id)))
  const fails = results.filter(r => r.status === 'rejected').length
  if (fails > 0) ElMessage.warning(`${fails} 个未能完成`)
  clearSelection()
  await refresh()
}

async function batchCancel() {
  const ids = Array.from(selectedIds.value)
  const results = await Promise.allSettled(
    ids.map(id => updateTodo(id, { status: 'cancelled' }))
  )
  const fails = results.filter(r => r.status === 'rejected').length
  if (fails > 0) ElMessage.warning(`${fails} 个未能取消`)
  clearSelection()
  await refresh()
}

async function batchSetPriority(p: TodoPriority) {
  const ids = Array.from(selectedIds.value)
  const results = await Promise.allSettled(
    ids.map(id => updateTodo(id, { priority: p }))
  )
  const fails = results.filter(r => r.status === 'rejected').length
  if (fails > 0) ElMessage.warning(`${fails} 个未能改优先级`)
  clearSelection()
  await refresh()
}

async function switchView(v: 'list' | 'calendar') {
  await layout.setTodoView(v)
}

onMounted(refresh)
</script>

<template>
  <section class="panel panel--list tasks-todo-panel">
    <header class="panel__header tasks-todo-panel__header">
      <div class="tasks-todo-panel__header-row1">
        <h2 class="panel__title">待办</h2>
        <div class="tasks-todo-panel__actions">
          <ElButtonGroup>
            <ElButton
              :type="todoView === 'list' ? 'primary' : 'default'"
              size="small"
              @click="switchView('list')"
            >
              <ElIcon><List /></ElIcon>
            </ElButton>
            <ElButton
              :type="todoView === 'calendar' ? 'primary' : 'default'"
              size="small"
              @click="switchView('calendar')"
            >
              <ElIcon><Calendar /></ElIcon>
            </ElButton>
          </ElButtonGroup>
          <ElButton size="small" :loading="loading" @click="refresh">
            <ElIcon><Refresh /></ElIcon>
          </ElButton>
          <ElButton size="small" type="primary" @click="openCreate">
            <ElIcon><Plus /></ElIcon>新建
          </ElButton>
        </div>
      </div>
      <div class="tasks-todo-panel__header-row2">
        <ElInput
          v-model="searchQuery"
          placeholder="搜索待办..."
          clearable
          :prefix-icon="Search"
        />
        <ElButton size="small" :type="showAll ? 'primary' : 'default'" @click="showAll = !showAll">
          {{ showAll ? '只看进行中' : '显示全部' }}
        </ElButton>
      </div>
    </header>

    <TodoBatchBar
      :count="selectedIds.size"
      @complete-all="batchComplete"
      @cancel-all="batchCancel"
      @set-priority="batchSetPriority"
      @clear-selection="clearSelection"
    />

    <div class="panel__body tasks-todo-panel__body">
      <TodoList
        v-if="todoView === 'list'"
        :todos="filtered"
        :selected-ids="selectedIds"
        :search-query="searchQuery"
        :enable-drag="canDrag"
        @complete="onComplete"
        @cancel="onCancel"
        @edit="openEdit"
        @toggle-select="toggleSelect"
        @reorder="onReorder"
      />
      <TodoCalendar v-else :todos="todos" />
    </div>

    <ElDialog v-model="formOpen" :title="editingTodo ? '编辑待办' : '新建待办'" width="480px">
      <TodoForm
        :todo="editingTodo"
        :open="formOpen"
        @submit="onFormSubmit"
        @cancel="formOpen = false"
      />
    </ElDialog>
  </section>
</template>

<style scoped>
.tasks-todo-panel__header {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.tasks-todo-panel__header-row1 {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.tasks-todo-panel__header-row2 {
  display: flex;
  gap: 8px;
  align-items: center;
}
.tasks-todo-panel__actions {
  display: flex;
  gap: 4px;
}
.tasks-todo-panel__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 12px;
}
</style>
