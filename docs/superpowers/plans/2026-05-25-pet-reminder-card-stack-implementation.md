---
title: Pet Reminder Card-Stack + Glance Direction — Implementation Plan
description: >
  将 design spec 分解为 6 个独立 task，按依赖顺序排列，
  每个 task 含变更文件、验收标准、风险点。
updated: 2026-05-25
related:
  - ../specs/2026-05-25-pet-reminder-card-stack/design.md
  - ../../../docs/WORKFLOW.md
---

# Pet Reminder Card-Stack + Glance Direction — Implementation Plan

> **前提**: design spec `05fe2db` 已获用户批准。
> **总计**: 6 task，预计 1-2 session。
> **DoD**: 12 项功能验收 + `cargo test` 358 pass + `vitest` 293 pass。

---

## Task 1: VRM Glance Action 系统

**目标**: 让 pet 能根据方向执行抬头/低头动作，保留 `nod` 兼容。

**变更文件**:
- `src/services/vrm.ts`

**具体修改**:
1. 扩展 `PetActionId` union：新增 `'glance_up' | 'glance_down'`；`nod` 保留。
2. 将现有 `playNod()` 提取为 `private playGlance(sign: 1 | -1)`：
   - `peakDelta = sign * (15 * Math.PI) / 180`
   - 三角包络逻辑不变
   - 返回 `Promise<void>`
3. `playAction` 分支：
   - `'nod'` | `'glance_up'` → `playGlance(+1)`
   - `'glance_down'` → `playGlance(-1)`
4. 将 `_nodInProgress` 重命名为 `_glanceInProgress`（语义更清晰）。

**验收**:
- [ ] `playAction('glance_up')` 执行 head bone +15° 动画
- [ ] `playAction('glance_down')` 执行 head bone -15° 动画
- [ ] `playAction('nod')` 仍执行 +15°（兼容）
- [ ] 并发调用时 `_glanceInProgress` 阻断第二次（第 2 次静默 no-op）

**风险**: 无。纯内部重构，外部接口行为不变。

---

## Task 2: Rust Placement 事件发射

**目标**: `reposition_overlay` 完成后把 reminder 位置方向告知 pet 窗口。

**变更文件**:
- `src-tauri/src/services/pet_overlay.rs`
- `src-tauri/src/lib.rs`（检查是否需要 listener 注册）

**具体修改**:
1. `compute_reminder_anchor` 返回 `(f64, f64, &'static str)`：
   - 新增 `placement: &str`（`"above"` 或 `"below"`）。
2. `reposition_overlay` 在 `set_position` 成功后：
   ```rust
   if label == PET_REMINDER_OVERLAY_LABEL {
       let _ = app.emit_to(PET_WINDOW_LABEL, "pet-reminder:placement",
           json!({ "direction": placement }));
   }
   ```
3. `on_pet_settled` 中的 reposition 后也会触发 placement emit（因 settled 调 `reposition_overlay`）。
4. 更新 `pet_overlay.rs` 内单元测试：断言返回值包含正确的 placement 字符串。

**验收**:
- [ ] `compute_reminder_anchor` 上方空间足够时返回 `("above", ...)`
- [ ] `compute_reminder_anchor` 上方空间不足时返回 `("below", ...)`
- [ ] `reposition_overlay` 成功定位后 pet 窗口收到 `pet-reminder:placement` 事件
- [ ] Command overlay reposition 不 emit placement 事件
- [ ] `cargo test` 全绿（更新后的断言）

**风险**: `emit_to` 需确认 pet 窗口 label 正确。PET_WINDOW_LABEL 已导入，直接使用即可。

---

## Task 3: Pet 前端 Glance 监听层

**目标**: Pet 窗口接收 placement 事件并驱动正确 glance 动作。

**变更文件**:
- 新建 `src/composables/usePetGlance.ts`（由 `usePetReaction.ts` 升级/替换）
- 修改 `src/components/PetCanvas.vue`（替换 usePetReaction 为 usePetGlance）

**具体修改**:
1. 新建 `usePetGlance.ts`：
   - 内部 `placement = ref<'above' | 'below'>('above')`（默认上方，兼容原行为）。
   - 监听 `pet-reminder:placement` 更新 `placement`。
   - 监听 `REMINDER_FIRED_EVENT`，dedup 逻辑不变，但调用 `runtime.playAction(placement.value === 'below' ? 'glance_down' : 'glance_up')`。
2. `usePetReaction.ts` 删除（或保留为 usePetGlance 的薄 wrapper 以兼容）。
3. `PetCanvas.vue` 替换 `usePetReaction` 为 `usePetGlance`。

**验收**:
- [ ] Pet 窗口收到 `"above"` 时后续 fired 触发 `glance_up`
- [ ] Pet 窗口收到 `"below"` 时后续 fired 触发 `glance_down`
- [ ] `reminder:fired` 30s dedup 仍生效
- [ ] VRM 未加载时 fired 静默 no-op

**风险**: `emit_to` 的 payload schema 需与前端 listener 一致。建议用 `"direction"` 字段字符串值。

---

## Task 4: Reminder Queue 去 Auto-Dismiss

**目标**: 移除 auto-dismiss timer，保留 queue 核心逻辑。

**变更文件**:
- `src/composables/useReminderQueue.ts`
- `src/composables/useReminderAnimation.ts`

**具体修改**:
1. `useReminderQueue.ts`：
   - 删除所有 `autoDismissTimer` / `setAutoDismissTimer` / `clearAutoDismissTimer` 相关代码。
   - `displayItems` computed 保留（用于单卡/叠卡的渲染）。
   - `complete()` 和 `snooze()` 逻辑不变（从 queue 移除）。
   - `pauseNonVisibleTimers()` 删除（无 timer 可暂停）。
2. `useReminderAnimation.ts`：
   - 删除 `transitionReason` 相关状态机代码。
   - 保留 `badgePop` / `triggerBadgePop()` / `isBadgePopping`（count badge 动画）。
   - 可考虑合并入 `useReminderQueue`（减少文件数），但保持独立亦可。

**验收**:
- [ ] Reminder 加入 queue 后不会自动消失
- [ ] 用户点击「完成」→ 从 queue 移除 + top card exit animation
- [ ] 用户点击「稍后」→ 从 queue 移除，snooze 后 scheduler 重新 fired
- [ ] 同 `reminderId` 去重：移动到 top + 刷新 payload

**风险**: 确认没有其他地方依赖 auto-dismiss（如 Rust 侧或测试）。检查 `useReminderQueue` 的导出接口是否变化。

---

## Task 5: 单卡 + 叠卡组件重写

**目标**: `PetReminderBubble.vue` 从 TransitionGroup 状态机改为单卡/叠卡模板。

**变更文件**:
- `src/components/PetReminderBubble.vue`
- `src/views/pet-reminder/PetReminderOverlayApp.vue`

**具体修改**:
1. `PetReminderBubble.vue`：
   - 移除 `<TransitionGroup>` 及其 `enter`/`leave` CSS。
   - 模板改为：
     ```vue
     <div class="reminder-bubble-stack" :class="{ 'reminder-bubble-stack--collapsed': count > 1 }">
       <!-- ghost layers (count > 1 时渲染 count-1 层) -->
       <div v-for="i in ghostCount" :key="`ghost-${i}`" class="reminder-card--ghost" />
       <!-- top card -->
       <div class="reminder-card" :class="{ 'reminder-card--exiting': isExiting }">
         ... content ...
         <div v-if="count > 1" class="reminder-badge" :class="{ 'reminder-badge--pop': isBadgePopping }">
           {{ count }}
         </div>
       </div>
     </div>
     ```
   - ghost layer CSS：`border` + `box-shadow` + `scale` 递减（每层 `scale(0.96)` 相对上一层）。
   - badge pop：CSS `@keyframes badgePop { 0% { scale: 1 } 50% { scale: 1.3 } 100% { scale: 1 } }`，`200ms`。
   - exit animation：`translateX(40px) + opacity(0)`，`200ms ease-out`。
   - 保持 `stackEl` expose（`tgRef` 改为 `stackRef`），供父 overlay 的 `ResizeObserver` 使用。
2. `PetReminderOverlayApp.vue`：
   - 适配新组件接口（props / emits 变化）。
   - `ResizeObserver` 仍监听 `stackEl`。
   - command tray open 时 opacity dim 到 40%（保留现有逻辑）。

**验收**:
- [ ] `count === 1`：单卡，无 badge，无 ghost layer
- [ ] `count === 2`：1 ghost layer + badge `2`
- [ ] `count === 3`：2 ghost layers + badge `3`
- [ ] Badge count 变化时触发 pop animation
- [ ] Top card 完成/稍后时 slide right + fade out
- [ ] 父容器 `.reminder-bubble-stack--collapsed` modifier 正确应用（居中 + padding 防 crop）
- [ ] `ResizeObserver` 仍能读取 stack 高度并调用 `setSize`

**风险**: ghost layer 的 `scale` 递减叠加时视觉可能过于密集，需手动调参。

---

## Task 6: 集成测试 + 边界回归

**目标**: 确保各 task 组合后无 regression，边界情况通过。

**执行内容**:
1. `cargo test`：确认 `pet_overlay.rs` 测试通过（placement 断言更新后）。
2. `vitest`：
   - `useReminderQueue` 无 auto-dismiss 测试更新。
   - `usePetGlance`（或 `usePetReaction`）dedup 测试通过。
3. 手动 e2e：
   - Pet 正常位置（屏幕中部）→ fired → reminder 在上方 → pet 抬头。
   - Pet 拖到屏幕顶部 → fired → reminder 在下方 → pet 低头。
   - 连续 fired 2 次（同 id，间隔 < 30s）→ 仅 1 次 glance。
   - `count` 从 1→2→3→2→1 的完整视觉流转。

**验收**:
- [ ] `cargo test` 358 pass
- [ ] `vitest` 293 pass
- [ ] 手动 e2e 4 例全绿

---

## 依赖图

```
Task 1 (VRM glance) ──┐
                       ├──→ Task 3 (PetGlance) ──→ Task 6 (Integration)
Task 2 (Rust placement)┘
                       ↑
Task 4 (Queue no-dismiss) ──→ Task 5 (Component rewrite) ──→ Task 6
```

- Task 1 和 Task 2 独立，可并行。
- Task 3 依赖 Task 1 + Task 2。
- Task 4 和 Task 5 可并行（组件模板不依赖 queue 内部 timer 移除）。
- Task 6 依赖全部前置 task。

---

## 回滚策略

- 若任一 task 失败，可独立 revert 该 task commit，不影响其他 task（文件变更无重叠）。
- 最危险的是 Task 5（组件重写），建议保留旧文件备份（git 历史即可），必要时 `git checkout` 回退。
