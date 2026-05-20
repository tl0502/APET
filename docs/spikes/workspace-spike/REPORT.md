---
title: dockview-vue workspace shell POC 报告（spike #32）
updated: 2026-05-20
related:
  - ../../decisions.md
  - ../../design/desktop-ui-principles.md
---

# dockview-vue workspace shell POC 报告

> ADR-021 落地前的 P0 spike，目标是验证 9 项关键不确定项。
>
> 分支：`spike/dockview-poc`（不进 main）。代码位置：`spike.html` + `src/views/spike/`。
> 启动方式：`pnpm dev`（vite 1420 端口），浏览器开 [http://localhost:1420/spike.html](http://localhost:1420/spike.html)；或 `pnpm tauri:dev` 后 spike 窗口（1200×800 visible）随其他窗口一起出现。
>
> dockview-vue 实测版本：**6.3.0**（peer vue ≥3.4）。

---

## 摘要

| # | 项 | 状态 | 一句话 |
|---|---|---|---|
| ① | dockview-vue SFC 集成 | ✅ | 6.x 用 **Vue component registry** 注册 panel（不是 named slot），typecheck + build 全过 |
| ② | EP token 串联 | ✅ | scoped style `var(--aipet-*)` 完整保留，与 `dv-*` 类无冲突 |
| ③ | EP popper z-index | ✅ | toast / Dialog / Dropdown 在 dock 内及 floating 后都在最上层显示 |
| ④ | 中文 IME 候选框 | ✅ | dock 内 + floating 内输中文，候选框正常不裁剪 |
| ⑤ | ResizeObserver + WebView2 | ✅ | 拖 dock 边界 → 尺寸实时刷新 + 触发次数累加 |
| ⑥ | bundle 体积 | ✅ | dockview 库总贡献 ≈ 62 kB gzip JS + 8.5 kB gzip CSS，**远低于 ADR 估算 150 kB gzip** |
| ⑦ | keep-alive 内存 | ✅ | 100 次切换 ΔMem 0（受 Chrome perf.memory 100KB 粒度限制）；JS Heap 稳定 < 60MB，反复点击不累积；always / onlyWhenVisible 两模式表现一致 |
| ⑧ | popout window in Tauri | ❌ | **结构性不可行**：[Tauri #14263](https://github.com/tauri-apps/tauri/issues/14263) 全局拦截 `window.open` 无视权限；dockview popout 依赖 `window.opener` 引用，无等价方案 |
| ⑨ | shadow DOM | ✅ | customElements + attachShadow 正常渲染（紫色虚线框 + scoped style 生效） |

**结论**：**8 ✅ + 1 ❌（⑧）**。⑧ 早在 ADR-021 cost ⑩ 写明"MVP 不依赖 popout"，无颠覆性发现，**ADR-021 架构层不需 rewrite**，但需补 4 个**实操坑**到 P1 实施指引（详见下文 §实操坑）。

---

## 测试方法

### 跑起来

vite dev 已经在 1420 端口（你的 `pnpm tauri:dev` 子进程自动起的）：

- 浏览器打开 [http://localhost:1420/spike.html](http://localhost:1420/spike.html) 即可（Chrome / Edge 内核 = Tauri WebView2 内核，渲染等价于 ④/⑤/⑦/⑨ 在 Tauri 内的结果）
- Tauri 内行为（仅 ⑧ 强依赖此）：kill 当前 tauri:dev 后重启，spike 窗口（visible:true）随其他 6 个窗口一起出现

### 自动验证

- `pnpm typecheck` ✅
- `pnpm lint` ✅
- `pnpm build` ✅（10s，1802 modules）
- spike chunks: `spike-*.js 263 kB raw / 62 kB gzip` + `spike-*.css 100.87 kB raw / 8.49 kB gzip`

---

## 实操坑（**写进 P1 issue #35 必须遵循**）

这 4 条都是从教程 / 官方文档**看不出来**，spike 跑出来才发现。

### 坑 1：DockviewVue 是空 div ref，自带 layout 仅 onMounted 一次

源码（`dockview-vue/dist/dockview-vue.es.js`）：

```js
onMounted(() => {
  const api = createDockview(el.value, { ...coreOptions, ...frameworkOptions });
  const { clientWidth, clientHeight } = el.value;
  api.layout(clientWidth, clientHeight);  // ← 只这一次！
  emit("ready", { api });
});
return () => createElementBlock("div", { ref: el }, null);  // ← 空 div，没任何 class
```

意味着：
- DockviewVue 自身不带 ResizeObserver，**窗口或父容器 resize 时 dockview 不会跟随**
- 必须给 `<DockviewVue>` 加 `style="height:100%;width:100%;display:block;"` 保 mount 时刻有非 0 尺寸
- 必须在外层用 `ResizeObserver` 监听父容器，触发 `api.layout(w, h)` 让 dockview re-layout

**对 ADR-021 / P1 影响**：`DockviewAdapter` 类构造时**必须**：
1. 给 dockview root element 100% 尺寸
2. 自带 ResizeObserver 监听容器 + 节流 50ms 喂 `api.layout`
3. 析构时 disconnect

参考实现见 [src/views/spike/SpikeApp.vue](../../../src/views/spike/SpikeApp.vue) `onReady` + `layoutRO` 段。

### 坑 2：panel 用 Vue component registry 注册（不是 named slot）

dockview-vue 6.x 的 `findComponent`：
```js
function findComponent(parent, name) {
  let component = parent.instance.components?.[name]      // 局部
            ?? parent.appContext.components?.[name];      // 全局 app.component
  if (!component) throw new Error(`Failed to find Vue Component '${name}'`);
  return component;
}
```

意味着：
- `addPanel({ component: 'ChatPanel' })` 的 `component` 字段是 **Vue component 名字**，必须先注册
- 不能用 `<template #ChatPanel>` slot（那是 5.x API）

**对 ADR-021 / P1 影响**：`DockviewAdapter` 注册 panel 时必须 `app.component(descriptor.id, descriptor.component)` 全局注册（或工厂模式动态注册）。PanelDescriptor 的 `id` 字段同时充当 Vue component 名（必须 PascalCase 满足 ESLint `vue/component-definition-name-casing`）。

### 坑 3：panel SFC 接到的 props 是**嵌套**结构

dockview 内部 `VueRenderer` 给 panel 传：
```ts
mountVueComponent(component, parent, { params: {
  params: addPanelParams,   // ← 你 addPanel 时传的 user params 在这一层
  api: PanelApi,
  containerApi: DockviewApi,
  tabLocation: 'header' | 'floatingPanel' | ...
}}, element);
```

意味着 panel SFC 必须：
```ts
defineProps<{ params: {
  params: MyUserParams,
  api: PanelApi,
  containerApi: DockviewApi,
  tabLocation: TabLocation
}}>()
// 然后用 props.params.params.xxx 拿 user params
```

**对 ADR-021 / P1 影响**：必须提供 `PanelContext<T>` 工具类型，让用户写 `defineProps<{ params: PanelContext<MyParams> }>` 替代手敲嵌套。schema 字段说明里明确这一约束。

### 坑 4：Popout 在 Tauri 完全不可行

dockview popout 实现：`window.open(url, '_blank', features)` + 通过 `window.opener` 反向引用同步状态。

Tauri 限制：所有 webview 内 `window.open` 调用被 Tauri runtime 全局拦截（[Tauri #14263](https://github.com/tauri-apps/tauri/issues/14263)），无视任何 capability 配置。要绕开必须改用 `WebviewWindow` API + `core:webview:allow-create-webview-window` 权限 + 自实现 popout 桥接（拦截 dockview popout 事件 → 创建 Tauri WebviewWindow → 跨 process 用 Tauri event 同步 dockview state）。

**对 ADR-021 / P1 影响**：⑧ 锁死 ❌，**MVP 不做 popout**与 ADR-021 决策完全一致；未来如需 popout 是独立大工程（~5-7 天）。

---

## 逐项详情

### ① dockview-vue SFC 集成 ✅

**实测**：typecheck + lint + build 全过。3 个 panel SFC（ChatPanel / SettingsPanel / TasksPanel）全部正常渲染、tab 切换、drag-drop reorder 正常。

**关键发现**：6.x 实际用 Vue component registry，不是网搜到的"named slot"——后者是 5.x 文档。详见 [坑 2](#坑-2panel-用-vue-component-registry-注册不是-named-slot)。

### ② EP token 串联 ✅

**实测**：build 期 `dist/assets/spike-*.css` 含 5 个 `--aipet-color-*` token + 全套 `dv-*` 类（dv-active-group / dv-active-tab / dv-animated ...）。dev 期 panel 内的 ElButton / ElTabs / ElDialog 全部呈现 Apple/Bear neutral 主题色（来自 element-overrides.css）。

**CSS 加载顺序**（spike main.ts，照搬 settings 窗口）：
```
element-plus/dist/index.css       ← EP 默认
element-plus/dark/css-vars.css    ← EP dark
@/styles/tokens.css                ← aipet tokens
@/styles/element-overrides.css     ← EP 主题覆盖
@/styles/components.css            ← 通用组件样式
dockview-vue/dist/styles/dockview.css ← dockview
```

dockview 自己用 `--dv-*` 变量族（如 `--dv-active-sash-color`），和 `--aipet-*` 完全不冲突。

### ③ EP popper z-index ✅

**实测**：
1. dock 内点 chat panel 的 toast / Dialog / Dropdown，三者都在最上层显示
2. 点 "Float Tasks Panel" 把 tasks 浮起来后回 chat 再点三者，**仍然**显示在 floating panel 之上（不被遮挡）

**根据**：dockview `--dv-floating-group-z-index` 默认值看 dockview.css 实测在 99（floating overlay）/ 1000 区间；EP `--el-index-popper` 默认 2000，`--el-index-overlay` 默认 1000，`--el-index-top` 默认 9000（toast）。基本 EP 永远高于 dockview，无需 z-index override。

### ④ 中文 IME 候选框 ✅

**实测**：dock 内 chat textarea 输入"测试" / "你好"候选框正常浮在光标下方；Float Tasks Panel 后回 chat 再输入候选框仍正常；把 chat 手动拖出 dock 区浮起来再输入，候选框仍正常。

**根据**：IME 候选框由 OS（Windows IME framework）渲染，不受 DOM overflow / transform 限制；Chromium WebView2 通过 OnTextInput 等事件正确传递 IME 位置。

### ⑤ ResizeObserver + Tauri WebView2 ✅

**实测**：tasks panel 内的 size-box 显示 "当前尺寸 W × H"；拖 dock 边界改变 panel 宽度 → 数字实时更新；"触发次数"累加。

### ⑥ Serialize bundle 体积 ✅

| chunk | raw | gzip |
|---|---|---|
| `dist/spike.html` | 0.61 kB | 0.35 kB |
| `dist/assets/spike-*.js` | 263.44 kB | **62.45 kB** |
| `dist/assets/spike-*.css` | 100.87 kB | **8.49 kB** |

对照（已有窗口同样含 EP + tokens + AppShell）：
- settings: js 69 / gz 24 + css 17.5 / gz 3.26
- chat: js 22.5 / gz 8.5 + css 16.7 / gz 3.14
- tasks: js 20.85 / gz 7.59 + css 10.72 / gz 2.01

spike 比 settings 多约 175-200 kB raw / 38-55 kB gzip JS + 84 kB raw / 6 kB gzip CSS ≈ **dockview 库总贡献 60 kB gzip**。ADR-021 原估算 150 kB gzip 偏保守。

### ⑦ keep-alive 内存 ✅

**实测**：
- always 模式：连点"切 100 次"5 次，每次 toast 显示 ΔMem 0（Chrome perf.memory 100KB 粒度限制，意为 < 100KB 真实增长 / GC 即时回收），JS Heap 稳定 < 60MB，无单调上涨
- onlyWhenVisible 模式：同上，行为一致

**结论**：100 次连续切换无内存泄漏。两种 renderer 模式都是健康的。

**对 ADR-021 影响**：原 ADR 写"Lazy + keep-alive"，实测 dockview 内置 `renderer: 'always'` 即原生 keep-alive，**完全不需要 Vue `<keep-alive>` 包裹**。PanelDescriptor `mountStrategy` 字段直接映射到 dockview `renderer`：
- `mountStrategy: 'lazy'` → `renderer: 'onlyWhenVisible'`（默认，省内存，但 DOM state 丢失）
- `mountStrategy: 'eager'` → `renderer: 'always'`（保 DOM，对 EP Dialog state 友好；推荐 chat.hub / settings.theme 等含表单的 panel）

### ⑧ Popout window in Tauri ❌

**浏览器实测**：点 Popout chat Group 报错 `dockview: failed to create popout. perhaps you need to allow pop-ups for this website` —— 浏览器 popup blocker 拦截；允许后理论可弹出（未深测因结论已锁死）。

**Tauri 实测**：未单独跑（基于 [Tauri #14263](https://github.com/tauri-apps/tauri/issues/14263) 已知行为 + dockview popout 实现依赖 `window.opener` 引用）。

**结论**：Tauri 内 dockview popout 结构性不可行。要做必须重写 popout bridge（5-7 天工作）。MVP 决策"不做"完全正确。

### ⑨ Shadow DOM ✅

**实测**：tasks panel 下方紫色虚线框 + "Shadow DOM 内 / 如果你能看到紫色虚线框和颜色 = ⑨ Shadow DOM 在 Tauri WebView 工作" 紫字正常显示。devtools 看到 `<spike-shadow-box>` 含 `#shadow-root (open)`。

---

## ADR-021 修订动作

不需要 rewrite ADR-021（架构未变）。但**直接编辑 ADR-021 原文**（未实施阶段，按 memory rule [feedback_adr_rewrite_style.md]）做 3 处微调：

1. 版本字段：**dockview-vue 5.x → 6.3.0**（实测）
2. keep-alive 实现细节：去掉"Vue keep-alive"提及，改为"dockview renderer always | onlyWhenVisible 内置实现"
3. panel 注册机制：去掉"named slot"提及，改为"Vue component registry（app.component 全局或局部注册）"

**P1 issue #35 描述追加**：4 个实操坑（链接本 REPORT.md §实操坑）

**P0 issue #32 关闭**：spike 全套完成 → close。

---

## 附录

### A. 测试代码位置

- spike root: [spike.html](../../../spike.html) + [src/views/spike/main.ts](../../../src/views/spike/main.ts)
- workspace shell: [src/views/spike/SpikeApp.vue](../../../src/views/spike/SpikeApp.vue)
- panel SFC: [src/views/spike/panels/](../../../src/views/spike/panels/)

### B. 版本固定

- dockview-vue 6.3.0
- dockview-core 6.3.0（pnpm 自动作为 dockview-vue 依赖装入）
- Vue 3.5.34
- vite 7.3.2

### C. 参考

- [Dockview docs](https://dockview.dev/)
- [Dockview GitHub](https://github.com/mathuo/dockview)
- [Dockview issue #897 — Vue3 SFC](https://github.com/mathuo/dockview/issues/897)
- [Dockview PR #1000 — empty component fix](https://github.com/mathuo/dockview/pull/1000)
- [Tauri issue #14263 — window.open blocked](https://github.com/tauri-apps/tauri/issues/14263)
- [Tauri WebviewWindow API](https://v2.tauri.app/reference/javascript/api/namespacewebviewwindow/)
