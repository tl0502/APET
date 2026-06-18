import { defineStore } from 'pinia'
import { reactive, shallowRef } from 'vue'

import {
  SAFETY_SCOPES,
  getSafetyPolicy,
  setSafetyScope,
  type SafetyPolicySnapshot,
  type SafetyScope,
} from '@/services/safety'

function emptySnapshot(): SafetyPolicySnapshot {
  return {
    prefixInjection: false,
    userInput: false,
    streamToken: false,
    finalOutput: false,
  }
}

export const useSafetyStore = defineStore('safety', () => {
  const scopes = reactive<SafetyPolicySnapshot>(emptySnapshot())
  const savingScopes = reactive<SafetyPolicySnapshot>(emptySnapshot())
  const loaded = shallowRef(false)

  function applySnapshot(snapshot: SafetyPolicySnapshot) {
    for (const scope of SAFETY_SCOPES) {
      scopes[scope] = snapshot[scope] === true
    }
  }

  async function load() {
    applySnapshot(await getSafetyPolicy())
    loaded.value = true
  }

  async function setScope(scope: SafetyScope, enabled: boolean) {
    const previous = scopes[scope]
    scopes[scope] = enabled
    savingScopes[scope] = true
    try {
      await setSafetyScope(scope, enabled)
    } catch (e) {
      scopes[scope] = previous
      throw e
    } finally {
      savingScopes[scope] = false
    }
  }

  return {
    scopes,
    savingScopes,
    loaded,
    load,
    setScope,
  }
})
