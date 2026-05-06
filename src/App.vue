<script setup lang="ts">
// 角色窗主壳：保持透明（PRD §7.2 角色窗）。VRM 渲染由 PetCanvas 接管；物理交互 / 容器布局后续 task 接入。
// 主题（ADR-017）已在 main.ts 通过 useThemeStore().init() 启动；本组件不渲染任何控件。
import PetCanvas from '@/components/PetCanvas.vue'
</script>

<template>
  <div class="pet-shell">
    <PetCanvas />
  </div>
</template>

<style scoped>
.pet-shell {
  width: 100vw;
  height: 100vh;
  background: transparent;
}
</style>
