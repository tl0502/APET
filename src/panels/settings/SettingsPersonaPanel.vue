<script setup lang="ts">
import { onBeforeUnmount, onMounted, shallowRef } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import PersonaWorkshopPanel from '@/components/persona-workshop/PersonaWorkshopPanel.vue'
import VrmAvatarExporter from '@/components/settings/VrmAvatarExporter.vue'
import { getActivePersona } from '@/services/persona'

const props = withDefaults(defineProps<{ isActive?: boolean }>(), { isActive: true })

const activePersonaId = shallowRef<string | null>(null)
let unlistenActivated: UnlistenFn | null = null

async function refreshActivePersonaId() {
  try {
    activePersonaId.value = (await getActivePersona()).id
  } catch (e) {
    console.warn('[SettingsPersonaPanel] refresh active persona failed:', e)
  }
}

onMounted(async () => {
  await refreshActivePersonaId()
  try {
    unlistenActivated = await listen('persona:activated', () => {
      void refreshActivePersonaId()
    })
  } catch (e) {
    console.warn('[SettingsPersonaPanel] listen persona:activated failed:', e)
  }
})

onBeforeUnmount(() => {
  unlistenActivated?.()
  unlistenActivated = null
})
</script>

<template>
  <section class="settings-persona">
    <PersonaWorkshopPanel class="settings-persona__workshop" :is-active="props.isActive" />

    <div class="settings-persona__avatar-export">
      <VrmAvatarExporter :persona-id="activePersonaId" :is-active="props.isActive" />
    </div>
  </section>
</template>

<style scoped>
.settings-persona {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-5);
  min-height: 0;
}

.settings-persona__workshop {
  min-height: 620px;
}

.settings-persona__avatar-export {
  flex: 0 0 auto;
  padding-top: var(--aipet-space-5);
  border-top: 1px solid var(--aipet-color-border-faint);
}
</style>
