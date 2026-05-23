# workspace:rect 持久化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** workspace 主窗 OS rect（位置 + 尺寸 + monitor_id）跨重启持久化，复用 [src-tauri/src/services/window_state.rs](../../../src-tauri/src/services/window_state.rs) 的 pet/pomodoro pattern + 加 size 维度。

**Architecture:** 单文件 Rust 改动（`window_state.rs` 新增 `LastRect` struct / `WorkspaceSaveDebouncer` / `save_workspace_rect` / `apply_initial_workspace_rect`）+ `lib.rs` setup 钩子 + `WindowEvent::Moved` 加 workspace 分支 + 新增 `WindowEvent::Resized` 分支。零前端改动。

**Tech Stack:** Rust + Tauri 2.x + tokio + serde_json，无新依赖。回归用 cargo test（新增 1 例 serde 测试）+ 手动 e2e 5 例。

**关联文档:**
- Spec: [`docs/superpowers/specs/2026-05-22-workspace-rect-persistence-design.md`](../specs/2026-05-22-workspace-rect-persistence-design.md)
- Issue: [#34](https://github.com/tl0502/APET/issues/34)（已原地覆写）
- 同 pattern 参考：[`src-tauri/src/services/window_state.rs`](../../../src-tauri/src/services/window_state.rs) 现有 `LastPosition` / `SaveDebouncer` / `PomodoroSaveDebouncer` / `apply_initial_position` / `apply_initial_pomodoro_position`

---

## File Structure

| 文件 | 改动 |
|---|---|
| `src-tauri/src/services/window_state.rs` | 新增 `LastRect` struct / `CONFIG_KEY_WORKSPACE_RECT` const / `WORKSPACE_MIN_W` / `WORKSPACE_MIN_H` / `WORKSPACE_DEFAULT_W` / `WORKSPACE_DEFAULT_H` 常量 / `save_workspace_rect` / `load_workspace_rect` / `apply_initial_workspace_rect` / `fallback_workspace_default` / `WorkspaceSaveDebouncer` + tests |
| `src-tauri/src/lib.rs` | setup 段加 `apply_initial_workspace_rect` + `app.manage(WorkspaceSaveDebouncer::default())`；`WindowEvent::Moved` 分支加 workspace 处理；新增 `WindowEvent::Resized` 分支 |
| `docs/STATUS.md` | M2 W3 完成行追加 #34（10/10）+ current session / 下一步 字段同步 |

无前端改动。无 ADR 新增（属 ADR-021 P3 收尾）。

---

## Task 1: 新增 LastRect struct + 常量 + serde 单测

**Files:**
- Modify: `src-tauri/src/services/window_state.rs`

**Goal:** 加 struct 与常量，提交序列化双向测试。

- [ ] **Step 1: 在 LastPosition struct 之后新增 LastRect**

定位 `src-tauri/src/services/window_state.rs:75-80`（LastPosition 定义后），插入：

```rust
/// workspace 主窗 rect 持久化数据（位置 + 尺寸 + monitor_id）。
/// 与 LastPosition 并存：pet/pomodoro 用 LastPosition（size 固定），workspace 用 LastRect（resizable）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastRect {
    pub monitor_id: String,
    pub logical_x: f64,
    pub logical_y: f64,
    pub logical_w: f64,
    pub logical_h: f64,
}
```

- [ ] **Step 2: 在文件顶部 KV 常量段新增 workspace 常量**

定位 `src-tauri/src/services/window_state.rs:36-42`（现有 `CONFIG_KEY_PET_POSITION` 等常量段），在 `CONFIG_KEY_ALWAYS_ON_TOP` 行后插入：

```rust
/// `config` 表 key：workspace 主窗最后 rect（JSON 序列化 LastRect）
pub const CONFIG_KEY_WORKSPACE_RECT: &str = "window:workspace:last_rect";

/// workspace size 下限（tauri.conf.json minWidth / minHeight 同步）
const WORKSPACE_MIN_W: f64 = 800.0;
const WORKSPACE_MIN_H: f64 = 520.0;
/// workspace 首次启动 / fallback 默认尺寸（tauri.conf.json 同步）
const WORKSPACE_DEFAULT_W: f64 = 1100.0;
const WORKSPACE_DEFAULT_H: f64 = 720.0;
```

- [ ] **Step 3: 在测试模块加 1 例 roundtrip 单测**

定位 `src-tauri/src/services/window_state.rs` 文件末（应有 `#[cfg(test)] mod tests`；如无则新增）。在 tests 模块内追加：

```rust
#[test]
fn last_rect_serde_roundtrip() {
    let r = LastRect {
        monitor_id: "monitor-1920x1080-0-0".into(),
        logical_x: 100.5,
        logical_y: 80.25,
        logical_w: 1200.0,
        logical_h: 760.0,
    };
    let json = serde_json::to_string(&r).unwrap();
    let parsed: LastRect = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.monitor_id, "monitor-1920x1080-0-0");
    assert_eq!(parsed.logical_x, 100.5);
    assert_eq!(parsed.logical_y, 80.25);
    assert_eq!(parsed.logical_w, 1200.0);
    assert_eq!(parsed.logical_h, 760.0);
}
```

如果文件末没有 `#[cfg(test)] mod tests { ... }`，整体新增：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_rect_serde_roundtrip() {
        // ...上面的测试体...
    }
}
```

如果已有 tests 模块且已 `use super::*`，只追加 `#[test] fn last_rect_serde_roundtrip` 即可。

- [ ] **Step 4: 跑单测**

Run（cwd = `src-tauri`）:

```bash
cd src-tauri && cargo test last_rect_serde_roundtrip
```

Expected: `test last_rect_serde_roundtrip ... ok`

- [ ] **Step 5: 不 commit**

controller 会在所有 Rust 改动完成后统一 commit。

---

## Task 2: save / load helpers

**Files:**
- Modify: `src-tauri/src/services/window_state.rs`

**Goal:** 加 `save_workspace_rect`（写 KV） + `load_workspace_rect`（读 KV）+ `compute_rect_from_window`（从窗口取当前 rect）。

- [ ] **Step 1: 在 PomodoroSaveDebouncer impl 之前新增三个 helper 函数**

定位 `src-tauri/src/services/window_state.rs:390`（`/// 番茄独立窗位置防抖锁` 注释行）之前，插入：

```rust
// === #34 workspace rect 持久化 ===

/// 从 webview 当前 outer_position + outer_size + current_monitor 推导 LastRect。
pub fn compute_rect_from_window<R: Runtime>(
    window: &WebviewWindow<R>,
) -> Result<LastRect, String> {
    let physical_pos = window
        .outer_position()
        .map_err(|e| format!("outer_position: {e}"))?;
    let physical_size = window
        .outer_size()
        .map_err(|e| format!("outer_size: {e}"))?;
    let monitor = window
        .current_monitor()
        .map_err(|e| format!("current_monitor: {e}"))?
        .ok_or_else(|| "current_monitor returned None".to_string())?;
    let scale = monitor.scale_factor();
    let logical_pos = LogicalPosition::<f64>::from_physical(physical_pos, scale);
    let logical_size = tauri::LogicalSize::<f64>::from_physical(physical_size, scale);
    Ok(LastRect {
        monitor_id: monitor_id(&monitor),
        logical_x: logical_pos.x,
        logical_y: logical_pos.y,
        logical_w: logical_size.width,
        logical_h: logical_size.height,
    })
}

/// 把窗口当前 outer_position + outer_size 转 logical 写 KV。
pub async fn save_workspace_rect<R: Runtime>(
    window: &WebviewWindow<R>,
) -> Result<(), WindowStateError> {
    let rect = compute_rect_from_window(window)
        .map_err(|e| WindowStateError::Config(config::ConfigError::Database(e)))?;
    let serialized = serde_json::to_string(&rect)?;
    config::set(window.app_handle(), CONFIG_KEY_WORKSPACE_RECT, &serialized).await?;
    Ok(())
}

pub async fn load_workspace_rect<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<LastRect>, WindowStateError> {
    let raw = config::get(app, CONFIG_KEY_WORKSPACE_RECT).await?;
    match raw {
        None => Ok(None),
        Some(s) => match serde_json::from_str::<LastRect>(&s) {
            Ok(r) => Ok(Some(r)),
            // KV 损坏 → 视同空，启动期走 fallback default（不让单条坏数据阻断启动）
            Err(e) => {
                eprintln!("[window_state] load_workspace_rect parse failed (treat as empty): {e}");
                Ok(None)
            }
        },
    }
}
```

- [ ] **Step 2: 确认 `LogicalSize` 已 import**

检查 `src-tauri/src/services/window_state.rs:27-30` 的 use 段。当前是：

```rust
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Monitor, PhysicalPosition, Runtime,
    WebviewWindow,
};
```

`LogicalSize` 已 import，可直接用 `LogicalSize::<f64>::from_physical` 或全限定 `tauri::LogicalSize`。如果代码里写了 `tauri::LogicalSize::<f64>` 也 OK（编译器不挑）。若想用短名改成 `LogicalSize::<f64>::from_physical(physical_size, scale)`。

- [ ] **Step 3: cargo check**

Run:

```bash
cd src-tauri && cargo check
```

Expected: 0 error，0 warning（如有 unused import warning 自查），编译通过。

- [ ] **Step 4: 不 commit**

---

## Task 3: WorkspaceSaveDebouncer

**Files:**
- Modify: `src-tauri/src/services/window_state.rs`

**Goal:** 加 `WorkspaceSaveDebouncer` newtype（Default + schedule），与 `PomodoroSaveDebouncer` 同款。

- [ ] **Step 1: 在 PomodoroSaveDebouncer impl 之后追加 WorkspaceSaveDebouncer**

定位 `src-tauri/src/services/window_state.rs:413`（PomodoroSaveDebouncer impl 末 `}` 之后），在文件末或 tests 模块之前插入：

```rust
/// workspace 主窗 rect 防抖锁（独立 slot，避免与 pet/pomodoro 串扰）。
/// Moved + Resized 共用同一 debouncer：每次 schedule 都 abort 上次 → 拖动 + resize 期间停手 200ms 后落盘一次。
#[derive(Default)]
pub struct WorkspaceSaveDebouncer {
    pending: Mutex<Option<JoinHandle<()>>>,
}

impl WorkspaceSaveDebouncer {
    pub fn schedule<R: Runtime>(&self, window: WebviewWindow<R>) {
        let mut slot = match self.pending.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if let Some(prev) = slot.take() {
            prev.abort();
        }
        let handle = tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(SAVE_DEBOUNCE_MS)).await;
            if let Err(e) = save_workspace_rect(&window).await {
                eprintln!("[window_state] save_workspace_rect failed: {e}");
            }
        });
        *slot = Some(handle);
    }
}
```

- [ ] **Step 2: cargo check**

Run:

```bash
cd src-tauri && cargo check
```

Expected: 编译通过。

- [ ] **Step 3: 不 commit**

---

## Task 4: apply_initial_workspace_rect + fallback

**Files:**
- Modify: `src-tauri/src/services/window_state.rs`

**Goal:** 加 `apply_initial_workspace_rect`（启动期同步 block_on 读 KV + clamp + set_size + set_position）+ `fallback_workspace_default`。

- [ ] **Step 1: 在 WorkspaceSaveDebouncer 之前（或 helpers 段之后）插入 fallback + apply 函数**

定位 §Task 2 插入的 `load_workspace_rect` 之后，继续追加：

```rust
/// workspace fallback default：主屏 center + 默认 1100×720。
/// 主屏不存在（极端 headless 场景）→ None，由调用方走 tauri.conf center:true 兜底。
fn fallback_workspace_default<R: Runtime>(
    app: &AppHandle<R>,
) -> Option<(f64, f64, f64, f64)> {
    let primary = app.primary_monitor().ok().flatten()?;
    let scale = primary.scale_factor();
    let origin = primary.position().to_logical::<f64>(scale);
    let size = primary.size().to_logical::<f64>(scale);
    let w = WORKSPACE_DEFAULT_W;
    let h = WORKSPACE_DEFAULT_H;
    let x = origin.x + (size.width - w) / 2.0;
    let y = origin.y + (size.height - h) / 2.0;
    Some((x, y, w, h))
}

/// 启动期调用：读 last_rect → 找 monitor 是否还在 → set_size + set_position 还原 / fallback。
/// workspace `visible: false`，setup 期 set_size + set_position 不视觉抖动。
pub fn apply_initial_workspace_rect<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = app
        .get_webview_window(WORKSPACE_WINDOW_LABEL)
        .ok_or_else(|| format!("workspace window '{WORKSPACE_WINDOW_LABEL}' not found"))?;

    let last = tauri::async_runtime::block_on(load_workspace_rect(app))
        .map_err(|e| format!("load workspace rect: {e}"))?;

    let monitors = window
        .available_monitors()
        .map_err(|e| format!("available_monitors: {e}"))?;

    let (logical_x, logical_y, w, h) = match last {
        Some(r) => match monitors.iter().find(|m| monitor_id(m) == r.monitor_id) {
            Some(monitor) => {
                let w = r.logical_w.max(WORKSPACE_MIN_W);
                let h = r.logical_h.max(WORKSPACE_MIN_H);
                let (x, y) = clamp_into_monitor(monitor, w, h, r.logical_x, r.logical_y);
                (x, y, w, h)
            }
            // monitor 不在场 → fallback 主屏 center
            None => fallback_workspace_default(app)
                .ok_or_else(|| "no primary monitor for fallback".to_string())?,
        },
        // KV 空 / 损坏 → fallback 主屏 center
        None => fallback_workspace_default(app)
            .ok_or_else(|| "no primary monitor for fallback".to_string())?,
    };

    window
        .set_size(LogicalSize::new(w, h))
        .map_err(|e| format!("set_size: {e}"))?;
    window
        .set_position(LogicalPosition::new(logical_x, logical_y))
        .map_err(|e| format!("set_position: {e}"))?;
    Ok(())
}
```

- [ ] **Step 2: 确认 WORKSPACE_WINDOW_LABEL 已 import**

`src-tauri/src/services/window_state.rs:33-34`（use 段下方）当前已有：

```rust
use crate::services::window_actions::{CHAT_WINDOW_LABEL, PET_WINDOW_LABEL, POMODORO_WINDOW_LABEL};
```

加上 `WORKSPACE_WINDOW_LABEL`：

**Old:**
```rust
use crate::services::window_actions::{CHAT_WINDOW_LABEL, PET_WINDOW_LABEL, POMODORO_WINDOW_LABEL};
```

**New:**
```rust
use crate::services::window_actions::{
    CHAT_WINDOW_LABEL, PET_WINDOW_LABEL, POMODORO_WINDOW_LABEL, WORKSPACE_WINDOW_LABEL,
};
```

- [ ] **Step 3: cargo check**

Run:

```bash
cd src-tauri && cargo check
```

Expected: 编译通过。

- [ ] **Step 4: cargo check lib-only（lesson §4）**

Run:

```bash
cd src-tauri && cargo check --bins
```

Expected: 编译通过（确认非 test 路径也能编译，避免 tokio macros feature 陷阱重演）。

- [ ] **Step 5: 不 commit**

---

## Task 5: lib.rs setup 钩子

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Goal:** setup 段加 `apply_initial_workspace_rect` 调用 + `app.manage(WorkspaceSaveDebouncer::default())`。

- [ ] **Step 1: 改 use 段引入 WorkspaceSaveDebouncer**

定位 `src-tauri/src/lib.rs:13`：

**Old:**
```rust
use services::window_state::{PomodoroSaveDebouncer, SaveDebouncer};
```

**New:**
```rust
use services::window_state::{PomodoroSaveDebouncer, SaveDebouncer, WorkspaceSaveDebouncer};
```

- [ ] **Step 2: 在 setup 段加 apply + manage**

定位 `src-tauri/src/lib.rs:244`（`app.manage(PomodoroSaveDebouncer::default());` 行之后），插入：

```rust
            // #34 workspace 主窗 rect（位置 + 尺寸）持久化：setup 阶段 visible:false 状态下还原（无视觉抖动）。
            // 首启 KV 空 / 损坏 / 拔屏导致 monitor 不在 → 静默 fallback 主屏 center + 默认 1100×720。
            if let Err(e) =
                crate::services::window_state::apply_initial_workspace_rect(app.handle())
            {
                eprintln!("[setup] apply_initial_workspace_rect failed: {e}");
            }
            app.manage(WorkspaceSaveDebouncer::default());
```

完整片段（含上下文）：

```rust
            if let Err(e) = crate::services::window_state::apply_initial_pomodoro_position(
                app.handle(),
                360.0,
                480.0,
            ) {
                eprintln!("[setup] apply_initial_pomodoro_position failed: {e}");
            }
            app.manage(PomodoroSaveDebouncer::default());
            // ↓ 新增
            if let Err(e) =
                crate::services::window_state::apply_initial_workspace_rect(app.handle())
            {
                eprintln!("[setup] apply_initial_workspace_rect failed: {e}");
            }
            app.manage(WorkspaceSaveDebouncer::default());
            // ↑ 新增
            // #30 follow-up I：磁吸 solver state。
            app.manage(crate::services::snap::SnapState::default());
```

- [ ] **Step 3: cargo check**

Run:

```bash
cd src-tauri && cargo check
```

Expected: 编译通过。

- [ ] **Step 4: 不 commit**

---

## Task 6: lib.rs Moved 分支加 workspace

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Goal:** `WindowEvent::Moved` 分支处理 workspace label，schedule WorkspaceSaveDebouncer。

- [ ] **Step 1: 改 Moved 分支**

定位 `src-tauri/src/lib.rs:393-405`（`WindowEvent::Moved(_) =>` 块），在 `else if label == POMODORO_WINDOW_LABEL` 后追加 workspace 分支：

**Old:**
```rust
                tauri::WindowEvent::Moved(_) => {
                    let app = window.app_handle();
                    if label == PET_WINDOW_LABEL {
                        if let Some(pet) = app.get_webview_window(PET_WINDOW_LABEL) {
                            let debouncer = app.state::<SaveDebouncer>();
                            debouncer.schedule(pet);
                        }
                    } else if label == POMODORO_WINDOW_LABEL {
                        if let Some(pom) = app.get_webview_window(POMODORO_WINDOW_LABEL) {
                            let debouncer = app.state::<PomodoroSaveDebouncer>();
                            debouncer.schedule(pom);
                        }
                    }
                    // #30 follow-up I：所有窗 Moved 都触发 snap solver（fast-path 内部判定）。
```

**New:**
```rust
                tauri::WindowEvent::Moved(_) => {
                    let app = window.app_handle();
                    if label == PET_WINDOW_LABEL {
                        if let Some(pet) = app.get_webview_window(PET_WINDOW_LABEL) {
                            let debouncer = app.state::<SaveDebouncer>();
                            debouncer.schedule(pet);
                        }
                    } else if label == POMODORO_WINDOW_LABEL {
                        if let Some(pom) = app.get_webview_window(POMODORO_WINDOW_LABEL) {
                            let debouncer = app.state::<PomodoroSaveDebouncer>();
                            debouncer.schedule(pom);
                        }
                    } else if label == WORKSPACE_WINDOW_LABEL {
                        // #34 workspace 主窗位置持久化：Moved 触发 debouncer（与 Resized 共用）
                        if let Some(ws) = app.get_webview_window(WORKSPACE_WINDOW_LABEL) {
                            let debouncer = app.state::<WorkspaceSaveDebouncer>();
                            debouncer.schedule(ws);
                        }
                    }
                    // #30 follow-up I：所有窗 Moved 都触发 snap solver（fast-path 内部判定）。
```

- [ ] **Step 2: cargo check**

Run:

```bash
cd src-tauri && cargo check
```

Expected: 编译通过。

- [ ] **Step 3: 不 commit**

---

## Task 7: lib.rs 新增 Resized 分支

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Goal:** 新增 `WindowEvent::Resized(_)` 分支，workspace 触发同一 debouncer（pet/pomodoro 不 resize 所以忽略）。

- [ ] **Step 1: 在 Moved 分支之后插入 Resized 分支**

定位 §Task 6 改后的 `WindowEvent::Moved(_)` 块完整结束位置（`crate::services::snap::on_window_moved(app, label);` 行之后的 `}`）。在 Moved 块 `}` 与下一个 WindowEvent arm 之间插入：

```rust
                tauri::WindowEvent::Resized(_) => {
                    // #34 workspace 主窗尺寸持久化：仅 workspace resizable，其他窗 size 固定不需处理
                    if label == WORKSPACE_WINDOW_LABEL {
                        let app = window.app_handle();
                        if let Some(ws) = app.get_webview_window(WORKSPACE_WINDOW_LABEL) {
                            let debouncer = app.state::<WorkspaceSaveDebouncer>();
                            debouncer.schedule(ws);
                        }
                    }
                }
```

完整片段（含上下文）：

```rust
                tauri::WindowEvent::Moved(_) => {
                    // ... §Task 6 改后内容 ...
                    crate::services::snap::on_window_moved(app, label);
                }
                // ↓ 新增
                tauri::WindowEvent::Resized(_) => {
                    if label == WORKSPACE_WINDOW_LABEL {
                        let app = window.app_handle();
                        if let Some(ws) = app.get_webview_window(WORKSPACE_WINDOW_LABEL) {
                            let debouncer = app.state::<WorkspaceSaveDebouncer>();
                            debouncer.schedule(ws);
                        }
                    }
                }
                // ↑ 新增
                // 下一个 WindowEvent arm（如有）...
```

- [ ] **Step 2: cargo check**

Run:

```bash
cd src-tauri && cargo check
```

Expected: 编译通过，无 unreachable arm 警告。

- [ ] **Step 3: cargo test 全套**

Run:

```bash
cd src-tauri && cargo test
```

Expected: 所有测试通过，含 §Task 1 新增的 `last_rect_serde_roundtrip`。

- [ ] **Step 4: 不 commit**

---

## Task 8: 手动 e2e 5 例

**Files:** 无 — 这是 dev 环境手动验证 checkpoint。

**Goal:** 按 spec §11.2 验证 5 例端到端行为。

- [ ] **Step 1: 起 dev 环境**

Run（cwd = 项目根 `d:/Project/temp/4`）:

```bash
pnpm tauri:dev
```

Expected: 应用启动，pet 桌宠窗显示；workspace 主窗按 IPC 显示（Ctrl+Alt+W 唤起，或托盘菜单 / 双击）。

- [ ] **Step 2: 例 1 — 拖位置持久化**

操作：
1. Ctrl+Alt+W 显示 workspace 主窗
2. 拖窗到屏幕左上角（如 50, 50）
3. 关闭主窗（close button = hide）
4. 完全退出应用（托盘退出）
5. 重新 `pnpm tauri:dev`
6. Ctrl+Alt+W 再显示 workspace

Expected: workspace 回到左上角（50, 50 附近），不是 tauri.conf 默认 center。

- [ ] **Step 3: 例 2 — 改尺寸持久化**

操作：
1. workspace 当前位置不动，拖角 resize 到 1400×900
2. 关 + 退出 + 重启 + 显示

Expected: workspace 尺寸恢复 1400×900。

- [ ] **Step 4: 例 3 — 拖 + resize 合并**

操作：
1. workspace 同时改位置（拖到中间）+ 改尺寸（如 1200×800）
2. 关 + 退出 + 重启 + 显示

Expected: 两者都恢复。

- [ ] **Step 5: 例 4 — 拔副屏 fallback（如有副屏）**

操作：
1. 接入副屏（HDMI / USB-C / 远程桌面副屏均可）
2. workspace 拖到副屏上
3. 关 + 退出
4. 拔副屏
5. 重启 + 显示

Expected: workspace 出现在主屏 center 1100×720（fallback default），不消失也不在虚空中。

如无副屏可跳过；可在例 5 间接验证 fallback 路径。

- [ ] **Step 6: 例 5 — size 越下限自愈**

操作：
1. 退出应用
2. 手动改 KV 把 width 设小（用 SQL 或 dev 工具）：

```bash
# 找 KV DB 位置（Windows 一般在 %APPDATA%\com.aipet.app\aipet.db）
# 用 sqlite3 命令直接改：
sqlite3 "%APPDATA%/com.aipet.app/aipet.db" "UPDATE config SET value = '{\"monitor_id\":\"x\",\"logical_x\":100,\"logical_y\":100,\"logical_w\":100,\"logical_h\":100}' WHERE key = 'window:workspace:last_rect'"
```

3. 重启 + 显示

Expected: workspace 启动时 clamp 到 min 800×520（不是 100×100）。位置因 monitor_id 'x' 不在场，走 fallback 主屏 center。

- [ ] **Step 7: 报告巡检结论**

向 controller 报告 5 例每一例的实测结果（通过 / 失败 + 失败现象）。任何一例失败回到 §Task 1-7 调代码。

---

## Task 9: Commit Rust 改动

**Files:**
- Stage: `src-tauri/src/services/window_state.rs` + `src-tauri/src/lib.rs`

- [ ] **Step 1: 检查改动范围**

Run:

```bash
git diff --stat src-tauri/src/services/window_state.rs src-tauri/src/lib.rs
```

Expected: 2 files changed，window_state.rs ~120 insertions，lib.rs ~25 insertions。

- [ ] **Step 2: 自检完整 diff**

Run:

```bash
git diff src-tauri/src/services/window_state.rs src-tauri/src/lib.rs | head -100
```

Expected: 改动如 §Task 1-7 计划，无其他文件改动。

- [ ] **Step 3: stage + commit**

```bash
git add src-tauri/src/services/window_state.rs src-tauri/src/lib.rs
git commit -m "feat: #34 workspace 主窗 rect 跨重启持久化

新增（src-tauri/src/services/window_state.rs）：
- LastRect struct（monitor_id + logical x/y/w/h），与 LastPosition 并存
- CONFIG_KEY_WORKSPACE_RECT = 'window:workspace:last_rect'
- WORKSPACE_MIN_W/H (800/520) + WORKSPACE_DEFAULT_W/H (1100/720) 与 tauri.conf 同步
- save_workspace_rect / load_workspace_rect / compute_rect_from_window helpers
- WorkspaceSaveDebouncer (200ms，Moved + Resized 共用)
- apply_initial_workspace_rect（setup 同步 block_on，clamp size 到 min，clamp 位置到 monitor 内安全边距）
- fallback_workspace_default（主屏 center + 默认 1100×720）

lib.rs：
- setup 段调 apply_initial_workspace_rect + manage WorkspaceSaveDebouncer
- WindowEvent::Moved 加 workspace 分支
- 新增 WindowEvent::Resized 分支（仅 workspace 触发）

回归：
- cargo test last_rect_serde_roundtrip pass
- cargo check / cargo check --bins 通过
- 手动 e2e 5 例（拖位置 / 改尺寸 / 拖+resize 合并 / 拔屏 fallback / size 越下限自愈）全绿

Closes #34"
```

Expected: 1 commit，2 files changed。

---

## Task 10: STATUS.md 同步

**Files:**
- Modify: `docs/STATUS.md`

**Goal:** 把当前 session 切到 #34 后状态，M2 进度 9/9 → 10/10，追加 #34 完成行。

- [ ] **Step 1: 改「当前 milestone」字段**

定位 `docs/STATUS.md:22`：

**Old:**
```markdown
- **当前 milestone**：M2 W3 进行中（9/9 落地 ✅；待办 + 物理交互待办）
```

**New:**
```markdown
- **当前 milestone**：M2 W3 进行中（10/10 落地 ✅；待办 + 物理交互待办）
```

- [ ] **Step 2: 改「当前 session 在做」字段**

定位 `docs/STATUS.md:23`：

**Old:**
```markdown
- **当前 session 在做**：[#38](https://github.com/tl0502/APET/issues/38) dark mode token 阶梯改造 — tokens.css 单文件 patch（light 背峰式 3+1 + dark 保守型 4 色阶 总跨 28 + dark border #333→#3d 衍生 fix + border-faint 6%→8%/10% + dark bubble-assistant 跟 L2）— 1 commit `d4dff7d`，293/293 vitest pass，4 大窗 × 2 主题手动 e2e 全绿
```

**New（commit hash 由 Task 9 实际产生后回填，写到 spec/plan/STATUS 时统一称为 `<rect-commit>` 占位）:**
```markdown
- **当前 session 在做**：[#34](https://github.com/tl0502/APET/issues/34) workspace 主窗 rect 持久化（ADR-021 P3 收尾）— window_state.rs 新增 LastRect + WorkspaceSaveDebouncer + apply_initial_workspace_rect + lib.rs Moved/Resized 钩子 — 1 commit `<rect-commit>`，cargo test 通过，手动 e2e 5 例全绿
```

- [ ] **Step 3: 改 M2 W3-W4 段标题**

定位 `docs/STATUS.md:39`：

**Old:**
```markdown
### M2 W3-W4（任务三件套 + 物理交互 + 磁吸 + 人格工坊 + workspace 壳）— 进行中（9/9 完成 ✅）
```

**New:**
```markdown
### M2 W3-W4（任务三件套 + 物理交互 + 磁吸 + 人格工坊 + workspace 壳）— 进行中（10/10 完成 ✅）
```

- [ ] **Step 4: 在 #38 完成行之后追加 #34 完成行**

定位 `docs/STATUS.md:60` 之后（最后一个 ✅ 行；现在是 #38）。追加：

```markdown
- ✅ [#34](https://github.com/tl0502/APET/issues/34) ADR-021 P3 收尾：workspace 主窗 rect 跨重启持久化 — window_state.rs 新增 LastRect + WorkspaceSaveDebouncer + apply_initial_workspace_rect 复用 pet/pomodoro pattern + lib.rs Moved/Resized 钩子；min 800×520 自愈 / 拔屏 fallback 主屏 center；1 commit `<rect-commit>`，cargo test pass，手动 e2e 5 例全绿
```

- [ ] **Step 5: 不动 frontmatter updated（同日 2026-05-22）**

---

## Task 11: 回填 Task 9 实际 commit hash

**Files:**
- Modify: `docs/STATUS.md`

**Goal:** Task 10 写的 `<rect-commit>` 占位用 Task 9 实际 commit hash 替换。

- [ ] **Step 1: 取 commit hash**

Run:

```bash
git log -1 --pretty=format:'%h %s' src-tauri/src/services/window_state.rs
```

Expected: 看到形如 `abc1234 feat: #34 workspace 主窗 rect 跨重启持久化`。短 hash 取前 7 位。

- [ ] **Step 2: 把两处 `<rect-commit>` 替换为实际 hash**

定位 `docs/STATUS.md`（两处含 `<rect-commit>` 字面占位）。

用实际短 hash（举例 `a1b2c3d`）替换：

**Old（两处都改）:**
```markdown
1 commit `<rect-commit>`
```

**New:**
```markdown
1 commit `a1b2c3d`
```

---

## Task 12: Commit STATUS 同步

**Files:**
- Stage: `docs/STATUS.md`

- [ ] **Step 1: 检查改动**

Run:

```bash
git diff docs/STATUS.md
```

Expected: 4 处 hunk — L22 9/9→10/10 / L23 session 字段切到 #34 / L39 段标题 9/9→10/10 / 追加 #34 完成行。

- [ ] **Step 2: stage + commit**

```bash
git add docs/STATUS.md
git commit -m "docs: #34 STATUS 同步 — M2 W3 10/10"
```

Expected: 1 commit，1 file changed。

---

## Task 13: 关闭 issue #34

**Files:** 无 — GitHub 操作。

- [ ] **Step 1: 取 Task 9 commit hash 用于 closing comment**

参考 Task 11 取的短 hash。

- [ ] **Step 2: 写 closing comment 并关闭**

Run（用 HEREDOC 防换行错乱；用 Task 9 实际 commit hash 替换 `<rect-commit>`）:

```bash
gh issue close 34 --comment "$(cat <<'EOF'
## 落地总结

ADR-021 P3 收尾。`src-tauri/src/services/window_state.rs` 单文件新增 + `lib.rs` 钩子 setup/Moved/Resized。2 commit（feat + docs sync）+ spec/plan。

### 实施

- **`window_state.rs`** 新增：
  - `LastRect` struct（monitor_id + logical x/y/w/h），与现有 `LastPosition` 并存（pet/pomodoro size 固定走 LastPosition；workspace resizable 走 LastRect）
  - `CONFIG_KEY_WORKSPACE_RECT = "window:workspace:last_rect"`
  - 常量 `WORKSPACE_MIN_W/H` (800/520) + `WORKSPACE_DEFAULT_W/H` (1100/720)（与 tauri.conf.json 同步）
  - helpers：`compute_rect_from_window` / `save_workspace_rect` / `load_workspace_rect`
  - `WorkspaceSaveDebouncer`（200ms，Moved + Resized 共用 → 拖动 + resize 停手 200ms 后落盘一次）
  - `apply_initial_workspace_rect`（setup 同步 block_on 读 KV；KV 空 / 损坏 / monitor 不在场 → fallback 主屏 center 1100×720；size 自动 clamp 到 min；位置 clamp 到 monitor 内 16px 安全边距）

- **`lib.rs`** setup 段加 `apply_initial_workspace_rect` + `manage(WorkspaceSaveDebouncer)`；`WindowEvent::Moved` 加 workspace 分支；**新增** `WindowEvent::Resized` 分支（仅 workspace 触发，pet/pomodoro 不 resize）

### 设计决策

- **LastRect 与 LastPosition 并存** 而非扩展 LastPosition 加 `Option<w/h>`：pet/pomodoro 拿 Option<> 是冗余字段。两 struct 各自语义清晰（"where" vs "where + size"）。
- **Moved 与 Resized 共用同一 debouncer**：每次 schedule 重置计时器 → 用户拖动 + resize 期间只在停手后落盘一次，避免 N 次 IPC + DB 写入。
- **size min 锁底 / max 不锁**：min 由 tauri.conf 已约束 + 启动期自愈；max 交给 OS / monitor 边界处理。

### 巡检

\`cargo test last_rect_serde_roundtrip\`：pass。
\`cargo check\` / \`cargo check --bins\`：通过（lesson §4 lib-only 路径验证）。

\`pnpm tauri:dev\` 手动 e2e 5 例：
- ✅ 拖位置 → 关 + 重启 → 位置恢复
- ✅ 改尺寸 → 关 + 重启 → 尺寸恢复
- ✅ 拖 + resize → 关 + 重启 → 两者都恢复（debouncer 合并写一次）
- ✅ 拔副屏 → 重启 → fallback 主屏 center 不消失
- ✅ size 越下限自愈 → 启动 clamp 到 min 800×520

### Commit

- \`<rect-commit>\` feat: #34 workspace 主窗 rect 跨重启持久化
- docs: #34 STATUS 同步 — M2 W3 10/10

### 文档

- spec：\`docs/superpowers/specs/2026-05-22-workspace-rect-persistence-design.md\`
- plan：\`docs/superpowers/plans/2026-05-22-workspace-rect-persistence-implementation.md\`
- STATUS.md：M2 W3 10/10 ✅
- **不新增 ADR**（属 ADR-021 P3 收尾，无新设计决策；ADR-021 在 #37 已 Updated）

### 抛光部分

#34 body 中标记的「桌面级动效抛光」未做：可选项，无明确验收线，留作后续手感调优时按需做。
EOF
)" 2>&1 | tail -5
```

Expected: gh CLI 输出 `Closed tl0502/APET#34`。

- [ ] **Step 3: 验证已关**

Run:

```bash
gh issue view 34 --json state -q .state
```

Expected: `CLOSED`

---

## 自检清单（Plan 完成判据）

落地后逐项核对：

- [ ] `src-tauri/src/services/window_state.rs` 新增 `LastRect` + 4 个 helper + `WorkspaceSaveDebouncer` + `apply_initial_workspace_rect`
- [ ] `src-tauri/src/services/window_state.rs` 测试 `last_rect_serde_roundtrip` 已加并 pass
- [ ] `src-tauri/src/lib.rs` setup 调 `apply_initial_workspace_rect` + manage debouncer
- [ ] `src-tauri/src/lib.rs` Moved 分支处理 workspace + 新增 Resized 分支
- [ ] `cargo test` / `cargo check` / `cargo check --bins` 全通过
- [ ] 5 例手动 e2e 全通过
- [ ] STATUS.md M2 W3 10/10 + 追加 #34 完成行（含 commit hash 已回填）
- [ ] `gh issue view 34` state = CLOSED
- [ ] 共 2 个 commit：rect 实施 + STATUS 同步

---

## 风险与回退

- **风险 1**：手动 e2e 例 4（拔副屏 fallback）实测发现 fallback 走的不是主屏 center → 检查 `fallback_workspace_default` 计算（origin + (size - w/h) / 2.0），可能是 origin 处理错（多屏 origin 是 monitor 在虚拟桌面的位置，不是 0,0）。
- **风险 2**：cargo check 通过但 `pnpm tauri:dev` 启动炸 → 大概率是 `WindowEvent::Resized` 模式匹配语法错（如 arm 顺序问题），回查 §Task 7。
- **风险 3**：workspace 启动后第一次显示位置/尺寸不对 → setup 时机问题，确认 `apply_initial_workspace_rect` 在 `app.manage` 之前，且 `WORKSPACE_WINDOW_LABEL` 窗口已由 tauri runtime 创建（webview2 启动期是同步的，window 在 setup callback 触发时已存在）。

回退：`git revert <rect-commit>` 一条命令回到改前状态；STATUS.md 改动可手动反 patch（影响面 4 行）。
