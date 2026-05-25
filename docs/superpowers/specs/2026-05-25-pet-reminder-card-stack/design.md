---
title: Pet Reminder UX — Single Card + Stacked Cards Model + Glance Direction
description: >
  废弃原 expanded/collapsed/single-bubble 状态机，改用更简单的单卡/叠卡模型；
  并引入 reminder 出现方向 → pet glance 方向联动，解决"reminder 在下方但 pet 固定抬头"的违和感。
updated: 2026-05-25
related:
  - ../../../src/components/PetReminderBubble.vue
  - ../../../src/composables/useReminderQueue.ts
  - ../../../src/composables/useReminderAnimation.ts
  - ../../../src/views/pet-reminder/PetReminderOverlayApp.vue
  - ../../../src/composables/usePetReaction.ts
  - ../../../src/services/vrm.ts
  - ../../../src-tauri/src/services/pet_overlay.rs
  - ../../../docs/decisions.md
---

# Pet Reminder UX — Single Card + Stacked Cards + Glance Direction

> **Status**: Design complete, awaiting approval before implementation.
> **Scope**: 前端 Vue 组件 + Rust overlay 位置事件 + VRM 动作系统。
> **Previous state**: 8 项结构修复已完成（P1-P8），见 commit `8e46047`。

---

## 1. 设计动机

原 `PetReminderBubble.vue` 采用 `expanded/collapsed/single-bubble` 三态状态机 + `<TransitionGroup>` enter/leave 动画：

- **过度复杂**：3 个状态 × 2 个方向 × badge 动画 = 组合爆炸，维护成本高。
- **对齐 Bug**：collapsed badge 的绝对定位与父容器 `left: 50%` 产生持续偏移问题。
- **Timer 泄漏**：collapsed 模式下不可见 bubble 的 auto-dismiss timer 仍运行，导致 queue count 静默递减。
- **Glance 违和**：reminder 可能出现在 pet **下方**，但 pet 永远执行固定 "nod"（抬头）动作，方向与弹窗位置不一致。

新模型通过以下原则简化：

1. **永远只显示一张卡**（单卡或叠卡的顶层卡），取消 collapsed/expanded 状态切换。
2. **叠卡用 ghost layer 视觉表达**，不做窗口级位移动画。
3. **Reminder 出现方向决定 pet glance 方向**（上方→抬头，下方→低头）。

---

## 2. 决策摘要（Brainstorm Q&A）

| # | 问题 | 决策 |
|---|---|---|
| 1 | `count === 1` 行为 | 单张独立 `ReminderCard`，无 badge，无状态切换动画 |
| 2 | `count > 1` 行为 | 卡片直接堆叠（ghost layers 在底层），仅 count badge 做轻量 pop/bump/scale 动画 |
| 3 | Auto-dismiss | **完全移除**（用户手动 complete/snooze/ignore） |
| 4 | Queue order | Newest-first：新提醒置顶，同 `reminderId` 去重时移动到顶部并刷新 payload |
| 5 | Top card 退出动画 | Slide right + fade out（A） |
| 6 | Command tray 打开时 | Reminder stack 整体 dim 到 40% opacity（A） |
| 7 | Glance direction | **新增**：reminder placement 方向（`above`/`below`）驱动 pet glance 方向 |

---

## 3. 单卡模型（`count === 1`）

### 3.1 视觉

- 仅渲染一张 `ReminderCard`。
- 无 count badge。
- 无 stack/ghost layer 效果。
- 卡片宽度固定（如 `280px`），高度自适应内容（标题 + 操作按钮）。

### 3.2 生命周期

1. `reminder:fired` → push 到 queue → emit `pet-reminder:active` → overlay 显示。
2. 用户点击「完成」→ remove from queue → 触发 exit animation → 若 queue 为空 → emit `pet-reminder:idle` → overlay hide。
3. 用户点击「稍后」→ snooze → 同完成流程（暂时从 queue 移除，snooze 后 scheduler 会再次 `fired`）。

---

## 4. 叠卡模型（`count > 1`）

### 4.1 视觉

```
┌──────────────────────────┐  ← top card（完整渲染，可交互）
│  💧 喝水                   │
│  每 30 分钟                │
│  [完成]  [稍后]            │
├──────────────────────────┤  ← ghost layer #2（仅边框/阴影/微缩，不渲染内容）
├──────────────────────────┤  ← ghost layer #3（同上）
└──────────────────────────┘
     ┌──┐
     │ 3│  ← count badge（右下角，pop animation on change）
     └──┘
```

- **Ghost layer**：底层卡片仅渲染外框（`border` + `box-shadow` + 微缩 `scale(0.96)` 逐层递减），不渲染文本/按钮，避免 DOM 膨胀。
- **Badge**：右下角圆形 badge，显示当前 queue 长度。count 变化时触发轻量 CSS keyframe（`scale(1) → scale(1.3) → scale(1)`，`200ms`）。
- **Z-index**：新卡（ newest ）在最上层。

### 4.2 父容器定位

- 使用 `.reminder-bubble-stack--collapsed` 条件 modifier：
  - `padding-top: 8px`（防止 badge 顶部被 crop）
  - `transform: translateX(calc(-50% + 2.5px))`（补偿 badge 左侧溢出，使整体视觉上居中于 pet 中心轴）
- 卡片自身尺寸不变，仅调整父容器定位。

### 4.3 入场/退场

- **新卡入场**：无 slide 动画，直接出现（或极短 `100ms` fade-in，避免突兀）。
- **Top card 退场**（完成/稍后）：`translateX(+40px) + opacity 0`，`200ms ease-out`。
- **Stack 整体**：无窗口级位移动画。overlay 大小由 `ResizeObserver` 动态调整（P6 已实现）。

---

## 5. Glance Direction（新增关键需求）

### 5.1 问题描述

当前链路：

```
reminder scheduler ──emit reminder:fired──→ pet window
                                              │
                                              ▼
                                    usePetReaction.playAction('nod')
                                              │
                                              ▼
                                    VRMRuntime.playNod() // 固定 +15° head.x
```

`playNod()` 固定执行 head bone X 轴 +15°（抬头），但 `pet_overlay.rs::compute_reminder_anchor` 会根据屏幕空间自动判断 reminder 出现在 pet **上方**（默认）或 **下方**（上方空间不足时 fallback）。

当 reminder 出现在 **下方** 时，pet 仍然"抬头"，产生严重违和感。

### 5.2 目标

**Reminder placement 方向决定 pet glance 方向**：

| Placement | Pet 动作 | 描述 |
|---|---|---|
| `above` | `glance_up` | 抬头看上方（与原 `nod` 同效果） |
| `below` | `glance_down` | 低头看下方（新动作） |

### 5.3 架构设计

#### 5.3.1 Rust 层 — `pet_overlay.rs`

`compute_reminder_anchor` 已天然知道 placement 方向：

```rust
let target_y_above = pet_y - h - GAP;
let placement = if target_y_above < mon_y + SCREEN_MARGIN {
    "below"   // fallback 到 pet 下方
} else {
    "above"   // 默认上方
};
```

**变更**：

1. `compute_reminder_anchor` 返回 `(f64, f64, &'static str)`（x, y, placement）。
2. `reposition_overlay` 在 `set_position` 成功后，emit `pet-reminder:placement` 事件给 **pet 窗口**：
   ```rust
   let _ = app.emit_to(PET_WINDOW_LABEL, "pet-reminder:placement", 
       json!({ "direction": placement }));
   ```
3. 仅在 `label == PET_REMINDER_OVERLAY_LABEL` 时 emit（command overlay 不涉及 glance）。
4. 在 `on_pet_settled` 中，若 reminder_has_content，reposition 后也会触发 placement emit。

#### 5.3.2 前端 — Pet 窗口

新增/修改文件：

**`src/composables/usePetGlance.ts`**（由 `usePetReaction.ts` 扩展/替换）

```typescript
// 维护当前 reminder placement 状态
const placement = ref<'above' | 'below'>('above')

// 监听 placement 更新（来自 Rust overlay 模块）
listen('pet-reminder:placement', (e) => {
  placement.value = e.payload.direction as 'above' | 'below'
})

// 监听 reminder fired，根据 placement 选择动作
listen(REMINDER_FIRED_EVENT, (e) => {
  // ...dedup logic...
  const actionId = placement.value === 'below' ? 'glance_down' : 'glance_up'
  runtime.playAction(actionId).catch(...)
})
```

**状态机**：

```
                    ┌─────────────────┐
   pet-reminder:    │  placement      │
   placement        │  ('above'/'below')│
   ────────────────→│                 │
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
   reminder:fired   │  dedup check    │
   ────────────────→│  → playAction   │
                    │  (glance_up/down)│
                    └─────────────────┘
```

- `placement` 是**持续性状态**，在 reminder overlay 每次 reposition 时更新。
- `reminder:fired` 触发时读取当前 `placement`，决定 glance 方向。
- 若 pet 正在移动中，overlay 被 hide，`placement` 保持最后一次有效值；settled 后更新。

#### 5.3.3 前端 — VRMRuntime

**`src/services/vrm.ts`**：

1. 扩展 `PetActionId`：
   ```typescript
   export type PetActionId =
     | 'glance_up' | 'glance_down'   // 新增，替代固定 nod
     | 'nod'                          // 保留兼容（行为 = glance_up）
     | 'head_pat' | 'surprised' ...   // placeholder
   ```

2. `playAction` 分支：
   ```typescript
   if (actionId === 'nod' || actionId === 'glance_up') {
     await this.playGlance(+1)   // +15°（抬头）
     return
   }
   if (actionId === 'glance_down') {
     await this.playGlance(-1)   // -15°（低头）
     return
   }
   ```

3. 提取通用方法 `playGlance(sign: 1 | -1)`：
   - 参数 `sign` 控制方向：`+1` = 向"看上方"方向旋转，`-1` = 向"看下方"方向旋转。
   - 复用现有 `playNod` 的三角包络逻辑，仅 `peakDelta` 的符号由 `sign` 决定。
   - `_nodInProgress` 标志复用为 `_glanceInProgress`（或保留原名，语义扩展为"任何 glance 动作进行中"）。

4. **并发保护**：`_glanceInProgress` 原子标志防止多次 reminder fired 导致 interleaved RAF。

#### 5.3.4 回退策略

- 若 `pet-reminder:placement` 事件因 race 未到达（如 overlay 模块尚未初始化），`placement` 默认值 `'above'` 保证与原行为一致。
- `nod` actionId 保留并映射到 `glance_up`，确保任何外部调用 `playAction('nod')` 的代码（如 `usePetInteractionFeedback`）行为不变。

---

## 6. 文件变更清单

| 文件 | 变更类型 | 说明 |
|---|---|---|
| `src/components/PetReminderBubble.vue` | 重写 | 移除 TransitionGroup 状态机；改为单卡/叠卡模板；ghost layer CSS；count badge pop animation |
| `src/composables/useReminderQueue.ts` | 修改 | 移除 auto-dismiss timer 逻辑；保留 queue 数据结构 + IPC listen + complete/snooze/remove |
| `src/composables/useReminderAnimation.ts` | 修改/合并 | 移除 transition reason 状态机；保留 badge-pop 动画逻辑（可合并入 `useReminderQueue` 或独立） |
| `src/views/pet-reminder/PetReminderOverlayApp.vue` | 修改 | 适配新组件接口；ResizeObserver 保留 |
| `src/composables/usePetReaction.ts` | 重命名/扩展 → `usePetGlance.ts` | 监听 `pet-reminder:placement` + `reminder:fired`；按方向调用 playAction |
| `src/services/vrm.ts` | 修改 | 新增 `glance_up`/`glance_down` action；提取 `playGlance(sign)` 通用方法 |
| `src-tauri/src/services/pet_overlay.rs` | 修改 | `compute_reminder_anchor` 返回 placement；`reposition_overlay` emit `pet-reminder:placement` |
| `src-tauri/src/lib.rs` | 可能修改 | 确认 `pet-reminder:placement` 事件无需额外 listener 注册（`emit_to` 直接投递） |
| `src/types/reminder.ts` | 可能新增 | 若需要前端类型，可新增 `PetReminderPlacementPayload` 接口（或内联） |

---

## 7. 验收标准

### 7.1 功能验收

- [ ] `count === 1`：仅显示单张卡片，无 badge，无 ghost layer。
- [ ] `count === 2`：顶层卡可交互，底层一张 ghost layer（仅边框+阴影），右下角 badge 显示 `2`。
- [ ] `count === 3`：两层 ghost layer（每层 `scale` 递减 `0.04`），badge 显示 `3`。
- [ ] Badge count 从 `2→3` 时触发 pop animation（`scale 1 → 1.3 → 1`，`200ms`）。
- [ ] 点击「完成」：top card 执行 slide right + fade out；下一层 ghost layer 升为 top card（无入场动画，直接显现）。
- [ ] 点击「稍后」：同完成流程，reminder 从 queue 移除，scheduler 会在 snooze 后重新 `fired`。
- [ ] Command tray 打开时：reminder stack 整体 `opacity: 0.4`。
- [ ] **Glance up**：reminder 出现在 pet 上方时，pet 执行抬头动作（head bone +15°）。
- [ ] **Glance down**：reminder 出现在 pet 下方时，pet 执行低头动作（head bone -15°）。
- [ ] 连续多个 reminder fired 时，`_glanceInProgress` 保护生效，不触发 interleaved RAF。
- [ ] Pet 移动 settled 后，reminder reposition 到下方时，后续 fired 自动切换为 `glance_down`。

### 7.2 测试验收

- [ ] `cargo test` 全绿（`pet_overlay.rs` 单元测试更新 placement 断言）。
- [ ] `vitest` 全绿（`useReminderQueue` / `usePetGlance` 逻辑测试）。
- [ ] 手动 e2e：pet 拖到屏幕顶部（强制 reminder fallback 到下方）→ fired → 确认 pet 低头。

---

## 8. 边界情况

| 场景 | 行为 |
|---|---|
| Pet 移动中 + reminder fired | overlay hide 中，pet 执行 glance 动作（方向按上次有效 placement）；settled 后 overlay reposition 到新位置 |
| Reminder 在下方时 badge pop | badge 位置在右下角，不受下方 placement 影响 |
| `count` 从 1→2（首张变叠卡） | 原单卡保持为 top card，新增 ghost layer 在下方，badge 从隐藏到显示 `2`（pop） |
| `count` 从 2→1（最后一张完成） | 退场动画后无 ghost layer，badge 消失，恢复单卡形态 |
| 同 `reminderId` 重复 fired | Queue 中去重：移动到 top，刷新 payload，不触发额外 glance（dedup 阈值 30s 内） |
| VRM 未加载完成时 fired | `playAction` 静默 no-op（现有行为保留） |
