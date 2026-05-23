<script setup lang="ts">
import { computed, ref, onMounted, onBeforeUnmount } from 'vue'
import { Calendar as VCalendar } from 'v-calendar'
import 'v-calendar/style.css'
import type { Todo } from '@/types/todo'

interface Props {
  todos: Todo[]
}

const props = defineProps<Props>()

// dark mode 检测：项目用 :root.dark class（非 prefers-color-scheme media query）
const isDark = ref(false)
let mo: MutationObserver | null = null

function syncDark() {
  isDark.value = document.documentElement.classList.contains('dark')
}

onMounted(() => {
  syncDark()
  mo = new MutationObserver(syncDark)
  mo.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
})

onBeforeUnmount(() => {
  mo?.disconnect()
  mo = null
})

function priorityDotColor(p: string): string {
  switch (p) {
    case 'high': return 'orange'
    case 'low': return 'gray'
    default: return 'blue'
  }
}

const attributes = computed(() => {
  return props.todos
    .filter(t => t.dueAt && t.status === 'open')
    .map(t => ({
      key: t.id,
      dot: { color: priorityDotColor(t.priority) },
      dates: [new Date(t.dueAt as string)],
      popover: { label: t.title },
    }))
})
</script>

<template>
  <div class="todo-calendar">
    <VCalendar :attributes="attributes" :is-dark="isDark" expanded />
  </div>
</template>

<style scoped>
.todo-calendar {
  padding: 12px;
  background: var(--aipet-surface-1);
  border-radius: 6px;
}
:deep(.vc-container) {
  border: none;
  background: transparent;
}
</style>
