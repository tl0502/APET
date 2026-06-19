<script setup lang="ts">
import { computed } from 'vue'
import { ElTag } from 'element-plus'
import type { PersonaListItem } from '@/types/persona'

const props = defineProps<{
  personas: PersonaListItem[]
  selectedId: string | null
  loading: boolean
}>()

const emit = defineEmits<{
  select: [id: string]
}>()

const sortedPersonas = computed(() => [
  ...props.personas.filter((persona) => persona.is_active),
  ...props.personas.filter((persona) => !persona.is_active),
])
</script>

<template>
  <section class="persona-card-stage" aria-label="角色卡舞台">
    <p v-if="props.loading" class="persona-card-stage__state">加载中...</p>
    <p v-else-if="sortedPersonas.length === 0" class="persona-card-stage__state">
      还没有可用人格
    </p>

    <div v-else class="persona-card-stage__grid">
      <button
        v-for="persona in sortedPersonas"
        :key="persona.id"
        type="button"
        class="persona-card"
        :class="{ 'persona-card--selected': persona.id === props.selectedId }"
        :aria-pressed="persona.id === props.selectedId"
        @click="emit('select', persona.id)"
      >
        <span class="persona-card__label">角色卡</span>
        <strong class="persona-card__name">{{ persona.name }}</strong>
        <span class="persona-card__id">{{ persona.id }}</span>

        <span class="persona-card__meta">
          <ElTag size="small">{{ persona.source }}</ElTag>
          <ElTag size="small">v{{ persona.version }}</ElTag>
          <ElTag v-if="persona.is_active" size="small" type="success">active</ElTag>
        </span>
      </button>
    </div>
  </section>
</template>

<style scoped>
.persona-card-stage {
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow: auto;
}

.persona-card-stage__grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: var(--aipet-space-4);
  align-content: start;
}

.persona-card-stage__state {
  display: grid;
  min-height: 220px;
  margin: 0;
  place-items: center;
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: var(--aipet-radius-card);
  color: var(--aipet-color-text-3);
}

.persona-card {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  min-width: 0;
  min-height: 220px;
  padding: var(--aipet-space-4);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: var(--aipet-radius-card);
  background: var(--aipet-color-surface);
  color: inherit;
  text-align: left;
  cursor: pointer;
  transition:
    border-color 140ms ease,
    background 140ms ease,
    box-shadow 140ms ease,
    transform 140ms ease;
}

.persona-card:hover,
.persona-card:focus-visible {
  border-color: var(--aipet-color-border-strong);
  outline: none;
  transform: translateY(-1px);
}

.persona-card--selected {
  border-color: var(--aipet-color-primary);
  background: color-mix(in srgb, var(--aipet-color-primary) 7%, var(--aipet-color-surface));
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--aipet-color-primary) 28%, transparent);
}

.persona-card__label {
  width: fit-content;
  padding: 2px var(--aipet-space-2);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: var(--aipet-radius-sm);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}

.persona-card__name {
  margin-top: var(--aipet-space-6);
  font-size: var(--aipet-font-size-xl);
  color: var(--aipet-color-text-1);
}

.persona-card__id {
  margin-top: var(--aipet-space-1);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}

.persona-card__meta {
  display: flex;
  flex-wrap: wrap;
  gap: var(--aipet-space-2);
  margin-top: var(--aipet-space-5);
}
</style>
