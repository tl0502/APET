<script setup lang="ts">
// Phase A 占位：让 `workspace_show` IPC 能看到非空 webview，验收"窗口可唤起"。
// Phase C 会替换为完整 AppShell standalone + WorkspaceShell（含 ActivityBar + dockview 主区）。
</script>

<template>
  <div class="placeholder">
    <h1>工作台</h1>
    <p>Phase A 占位 — Phase C 实施完整 workspace shell（dockview + ActivityBar + 命令面板）。</p>
  </div>
</template>

<style scoped>
.placeholder {
  position: fixed;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  background: #f7f7f8;
  color: #1f1f1f;
  font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
}
.placeholder h1 {
  font-size: 28px;
  margin: 0;
}
.placeholder p {
  margin: 0;
  color: #6e6e76;
  font-size: 14px;
}
</style>
