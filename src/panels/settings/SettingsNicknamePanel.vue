<script setup lang="ts">
// Nickname tab：U.1/U.2 昵称编辑（issue #15）。
// 之前是 #9 的占位（只展示当前值 + disabled 表单）；现接入完整 NicknameForm。
import NicknameForm from '@/components/settings/NicknameForm.vue'
</script>

<template>
  <NicknameForm />
</template>
