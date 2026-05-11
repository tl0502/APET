<script setup lang="ts">
// OnboardingApp：onboarding 窗口的 root（issue #16）。
//
// 当前仅渲染 SoulPledgeView（Step 1）。#17 Onboarding 状态机来时把这里扩成 step router：
// 监听 `onboarding:step-done` 事件 → 切换到 PersonaPicker（Step 2）→ ShortcutConfirm（Step 3）...
// 至 Step 6 完成后 emit `onboarding:all-done` → Rust 端 hide onboarding 窗 + 写 onboarded=true。
import SoulPledgeView from './SoulPledgeView.vue'
</script>

<template>
  <SoulPledgeView />
</template>

<style scoped></style>
