---
title: workspace:rect 主窗位置 + 尺寸持久化设计
updated: 2026-05-22
related:
  - ../../../decisions.md
  - ../../../STATUS.md
  - ../../../../src-tauri/src/services/window_state.rs
  - ../../../../src-tauri/tauri.conf.json
---

# workspace:rect 持久化 设计文档

> 对应 issue [#34](https://github.com/tl0502/APET/issues/34)（已原地覆写）。承接 [#33](https://github.com/tl0502/APET/issues/33) + [#37](https://github.com/tl0502/APET/issues/37) workspace 重设计后，补齐主窗 rect 跨重启持久化能力。

## 1. 背景

#34 原计划的 layout 持久化大多已在 #33 phase B-redo 通过 `workspace:current_category` / `workspace:item_per_category` / `workspace:master_width` 三个细粒度 KV 等价完成（[src/stores/workspaceLayout.ts](../../../../src/stores/workspaceLayout.ts)）。

唯一**未做**的 P3 真实工作：workspace 主窗的 **OS 级 rect**（屏幕位置 + 窗口尺寸）跨重启持久化。当前现象：用户拖动 workspace 主窗 / 改窗口大小 → 关闭主窗（hide）→ 重启应用 → 主窗回到 tauri.conf.json 默认（1100×720 居中），用户折腾失效。

pet (#10) / pomodoro (#28 follow-up) 已有等价机制（[window_state.rs](../../../../src-tauri/src/services/window_state.rs) `window:pet:last_position` / `window:pomodoro:last_position`），workspace 复用同 pattern + 加 size 维度即可。

## 2. 设计目标

- workspace 主窗 OS rect（x / y / width / height + monitor_id）跨重启持久化
- 拔屏后启动不消失 → fallback 主屏 center + 默认 1100×720
- 用户改窗口尺寸超出 min 800×520（tauri.conf.json 现有约束）时启动 clamp 回 min
- max size 不 clamp（OS / monitor 自处理）
- 0.5d 体量，单文件 Rust 改动

## 3. 不在范围内

- 桌面级动效抛光（#34 body 标可选，无验收线，留作以后手感调优时按需做）
- `workspace:last_visible` 启动恢复可见性（当前 `visible: false` + IPC show 已够用；user 无明确诉求）
- 前端任何改动（持久化是 OS 窗口属性层面，与 Vue 组件无关）
- pet / pomodoro `apply_initial_position` 行为变动
- workspace 透明度 / decorations / always-on-top 等其他窗属性
- 自动化截图 / e2e 测试（手动验证即可，单人项目）

## 4. 架构

**单文件 Rust patch + 一个 lib.rs 钩子点**：

- `src-tauri/src/services/window_state.rs` 新增：`LastRect` struct / `CONFIG_KEY_WORKSPACE_RECT` const / `save_workspace_rect()` / `apply_initial_workspace_rect()` / `WorkspaceSaveDebouncer`
- `src-tauri/src/lib.rs` setup 阶段调 `apply_initial_workspace_rect`；`WindowEvent::Moved` / `WindowEvent::Resized` 分支加 workspace 处理（schedule debouncer）；`commands::window` 暴露 `save_workspace_rect` IPC（与 pet/pomodoro 同款，便于 IPC 主动触发，比如 view 切换后强制落盘 — workspace 暂无对应场景，但保留对称）

无前端代码改动，无 Pinia 改动，无新单测以外的测试基建。

## 5. 数据模型

**新增 `LastRect` struct**（与现有 `LastPosition` 并存，不复用）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastRect {
    pub monitor_id: String,
    pub logical_x: f64,
    pub logical_y: f64,
    pub logical_w: f64,
    pub logical_h: f64,
}
```

**决策理由**：

- pet / pomodoro size 固定（view_preset 切换 / 设计上不 resize），不需要 size 字段；扩展 LastPosition 为 `Option<f64>` 会让 2/3 消费者拿到无意义字段
- LastPosition / LastRect 各自语义清晰（"where" vs "where + size"）
- 旧 KV 不冲突：`window:pet:last_position` 仍用 LastPosition；`window:workspace:last_rect` 是新 key，新数据

## 6. KV 与常量

```rust
/// `config` 表 key：workspace 主窗最后 rect（JSON 序列化 LastRect）
pub const CONFIG_KEY_WORKSPACE_RECT: &str = "window:workspace:last_rect";
```

放在 `window_state.rs` 顶部，与 pet/pomodoro 同款命名空间 `window:*`。

## 7. 写路径

### 7.1 触发点

`lib.rs` `WindowEvent::Moved(_)` 和 `WindowEvent::Resized(_)` 两个分支：

```rust
tauri::WindowEvent::Moved(_) => {
    // ... 现有 pet/pomodoro 分支 ...
    if label == WORKSPACE_WINDOW_LABEL {
        if let Some(ws) = app.get_webview_window(WORKSPACE_WINDOW_LABEL) {
            let debouncer = app.state::<WorkspaceSaveDebouncer>();
            debouncer.schedule(ws);
        }
    }
    // ... snap solver ...
}
tauri::WindowEvent::Resized(_) => {
    // 新增分支（pet/pomodoro 不 resize 所以此分支当前可能不存在）
    if label == WORKSPACE_WINDOW_LABEL {
        if let Some(ws) = app.get_webview_window(WORKSPACE_WINDOW_LABEL) {
            let debouncer = app.state::<WorkspaceSaveDebouncer>();
            debouncer.schedule(ws);
        }
    }
}
```

**关键设计**：同一个 `WorkspaceSaveDebouncer` 同时被 Moved 和 Resized 触发 → 每次 reset 计时器 → 拖动 + resize 期间只在停手后 200ms 落盘一次，避免 N 次 IPC + DB 写入。

### 7.2 持久化函数

```rust
pub async fn save_workspace_rect<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), WindowStateError> {
    let monitor = window.current_monitor()?.ok_or(WindowStateError::WindowMissing)?;
    let scale = monitor.scale_factor();
    let pos_physical = window.outer_position()?;
    let size_physical = window.outer_size()?;
    let pos_logical = LogicalPosition::<f64>::from_physical(pos_physical, scale);
    let size_logical = LogicalSize::<f64>::from_physical(size_physical, scale);

    let rect = LastRect {
        monitor_id: monitor_id(&monitor),
        logical_x: pos_logical.x,
        logical_y: pos_logical.y,
        logical_w: size_logical.width,
        logical_h: size_logical.height,
    };
    let json = serde_json::to_string(&rect)?;
    config::set(window.app_handle(), CONFIG_KEY_WORKSPACE_RECT, &json).await?;
    Ok(())
}
```

### 7.3 Debouncer

复用 SaveDebouncer pattern（参考 `window_state.rs` 现有 `SaveDebouncer` / `PomodoroSaveDebouncer`）：

```rust
pub struct WorkspaceSaveDebouncer(Mutex<Option<JoinHandle<()>>>);

impl WorkspaceSaveDebouncer {
    pub fn new() -> Self { Self(Mutex::new(None)) }

    pub fn schedule<R: Runtime>(&self, window: WebviewWindow<R>) {
        let mut guard = self.0.lock().unwrap();
        if let Some(handle) = guard.take() { handle.abort(); }
        *guard = Some(tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if let Err(e) = save_workspace_rect(&window).await {
                eprintln!("[workspace] save_rect failed: {e}");
            }
        }));
    }
}
```

`lib.rs` setup 阶段 `app.manage(WorkspaceSaveDebouncer::new())`。

## 8. 读路径

### 8.1 启动期 apply

`lib.rs` setup 阶段（同 `apply_initial_position` 时机）调：

```rust
if let Err(e) = window_state::apply_initial_workspace_rect(app.handle()) {
    eprintln!("[workspace] apply_initial_rect failed: {e}");
}
```

### 8.2 函数实现

```rust
pub fn apply_initial_workspace_rect<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = app.get_webview_window(WORKSPACE_WINDOW_LABEL)
        .ok_or_else(|| format!("workspace window '{WORKSPACE_WINDOW_LABEL}' not found"))?;

    let raw = tauri::async_runtime::block_on(config::get(app, CONFIG_KEY_WORKSPACE_RECT))
        .map_err(|e| format!("load workspace rect: {e}"))?;

    let last: Option<LastRect> = raw.and_then(|s| serde_json::from_str(&s).ok());

    let monitors = window.available_monitors().map_err(|e| format!("monitors: {e}"))?;

    let (logical_x, logical_y, w, h) = match last {
        Some(r) => match monitors.iter().find(|m| monitor_id(m) == r.monitor_id) {
            Some(monitor) => {
                let w = r.logical_w.max(WORKSPACE_MIN_W);
                let h = r.logical_h.max(WORKSPACE_MIN_H);
                let (x, y) = clamp_into_monitor(monitor, w, h, r.logical_x, r.logical_y);
                (x, y, w, h)
            }
            None => fallback_workspace_default(app)?,
        },
        None => fallback_workspace_default(app)?,
    };

    window.set_size(LogicalSize::new(w, h)).map_err(|e| format!("set_size: {e}"))?;
    window.set_position(LogicalPosition::new(logical_x, logical_y))
        .map_err(|e| format!("set_position: {e}"))?;
    Ok(())
}

fn fallback_workspace_default<R: Runtime>(app: &AppHandle<R>) -> Result<(f64, f64, f64, f64), String> {
    // 主屏 center + 默认 1100×720（与 tauri.conf.json 一致）
    let primary = app.primary_monitor().map_err(|e| format!("primary: {e}"))?
        .ok_or("no primary monitor")?;
    let scale = primary.scale_factor();
    let phys_size = primary.size();
    let logical_size_w = phys_size.width as f64 / scale;
    let logical_size_h = phys_size.height as f64 / scale;
    let w = WORKSPACE_DEFAULT_W;
    let h = WORKSPACE_DEFAULT_H;
    let x = (logical_size_w - w) / 2.0;
    let y = (logical_size_h - h) / 2.0;
    Ok((x, y, w, h))
}

const WORKSPACE_MIN_W: f64 = 800.0;
const WORKSPACE_MIN_H: f64 = 520.0;
const WORKSPACE_DEFAULT_W: f64 = 1100.0;
const WORKSPACE_DEFAULT_H: f64 = 720.0;
```

### 8.3 调用顺序

`lib.rs` setup 阶段先 apply rect 再 show（show 由用户 IPC 触发，apply 走 setup 同步路径）：

```rust
// setup 阶段（已存在的 pet/pomodoro 初始化代码之后）
let _ = window_state::apply_initial_view_preset(app.handle());
let _ = window_state::apply_initial_position(app.handle());
let _ = window_state::apply_initial_workspace_rect(app.handle());  // 新增
```

workspace `visible: false`，apply 期 set_size + set_position 不抖动。

## 9. Edge case 处理

| 场景 | 行为 |
|---|---|
| 首次启动（KV 空）| fallback → 主屏 center 1100×720 |
| KV 损坏 / 解析失败 | fallback → 主屏 center 1100×720（serde_json 错误吞掉 + warn）|
| monitor_id 不在场（拔屏）| fallback → 主屏 center 1100×720 |
| 保存的 size 比 min 还小（用户手贱 / 旧数据迁移）| size clamp 到 min 800×520 |
| 保存的 size 比 monitor 大（max 不锁）| OS / Tauri 自处理（一般 OS 会 clamp 到屏幕）|
| 跨屏拖动 + resize 后关闭 | Moved + Resized 各自 schedule debouncer，最后一次 schedule 落盘合并状态 |
| workspace 还 hidden 时被 resize | tauri.conf.json `visible: false` + show 由 IPC 触发；hidden 期间无 Resized 事件，OK |
| 启动期 monitor enumeration 失败 | `apply_initial_workspace_rect` 返回 Err，lib.rs setup 吞掉 + warn，窗口走 tauri.conf.json 默认（1100×720 center）|

## 10. 不动

- 前端代码（Vue 组件 / Pinia store / TS 服务）零改动
- pet (`apply_initial_position`) / pomodoro 行为零改动
- `LastPosition` struct 不动（新增 `LastRect` 与之并存）
- tauri.conf.json 不改（已有 minWidth/minHeight/default 都对）
- workspace 其他窗属性（透明 / decorations / always-on-top / skipTaskbar）

## 11. 测试

### 11.1 单测

`src-tauri/src/services/window_state.rs::tests` 加 2 例（参考现有 `clamp_into_monitor` 等测试 pattern）：

```rust
#[test]
fn last_rect_roundtrip_serde() {
    let r = LastRect {
        monitor_id: "monitor-1920x1080-0-0".into(),
        logical_x: 100.0, logical_y: 80.0,
        logical_w: 1200.0, logical_h: 760.0,
    };
    let json = serde_json::to_string(&r).unwrap();
    let parsed: LastRect = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.logical_w, 1200.0);
    assert_eq!(parsed.logical_h, 760.0);
}

#[test]
fn last_rect_parse_legacy_position_json_fails_gracefully() {
    // 老 KV blob 是 LastPosition（无 w/h）→ 用 LastRect 解必失败
    let legacy = r#"{"monitor_id":"x","logical_x":0.0,"logical_y":0.0}"#;
    let parsed: Result<LastRect, _> = serde_json::from_str(legacy);
    assert!(parsed.is_err());  // workspace 不会读 pet/pomodoro KV，但守正确性
}
```

### 11.2 手动 e2e（5 例）

| 例 | 操作 | 期望 |
|---|---|---|
| 1 拖位置持久 | 拖 workspace 到屏幕左上角 → 关 → 重启 | 回到左上角 |
| 2 改尺寸持久 | resize workspace 到 1400×900 → 关 → 重启 | 回到 1400×900 |
| 3 拖 + resize 合并 | 同时改位置 + 大小 → 关 → 重启 | 两者都恢复 |
| 4 跨屏 + 拔副屏 | 拖到副屏 → 关 → 拔副屏 → 启动 | 不消失，回主屏 center |
| 5 size 越下限自愈 | 手动改 KV 把 w=100 → 重启 | clamp 到 800×520 |

### 11.3 不做

- 自动化 e2e（截图 diff / Playwright）— 单人项目 YAGNI
- vitest 不改（纯 Rust 路径）

## 12. 工时与文档同步

| 任务 | 时长 |
|---|---|
| window_state.rs 新增 LastRect + save + apply + Debouncer | 20 min |
| lib.rs 钩 setup + Moved + Resized + manage debouncer | 10 min |
| cargo test 单测 + lib-only build 验证（lesson §4）| 10 min |
| 手动 e2e 5 例 | 30 min |
| commit + STATUS + ADR-024 段（不新增 ADR）+ issue close | 20 min |

合计 ~1.5h。

文档同步：

- `docs/STATUS.md`：M2 W3 完成行追加 #34（10/10 ✅）+ current session / 下一步 字段同步
- `docs/decisions.md`：**不新增 ADR**（属 ADR-021 P3 收尾，无新设计决策），可选在 ADR-021 末追一段说明
- 不更新 lessons.md（无新坑）

## 13. 风险

- **风险 1**：`window.outer_size()` 在 Resized 事件期间读到的值可能与最终值不同（连续 Resized 事件 + 200ms debounce → 最后一次读取的应该是稳定值，pet/pomodoro 同 pattern 验证过）
- **风险 2**：跨屏拖动后 monitor_id 变化 → save 时记新 monitor_id，重启时只要新 monitor 还在就还原。无需特殊处理
- **风险 3**：lesson §10「Tauri 2 `#[tauri::command] async` 内部链路不能 block_on tokio future」— 本设计 `apply_initial_workspace_rect` 是 sync fn 在 `lib.rs::setup`（同步上下文）调，符合 lesson §10「只在 setup / 其他确认是同步上下文的入口用 block_on」例外条款。OK
- **风险 4**：`save_workspace_rect` 是 async fn，被 sync Debouncer.schedule spawn → 内部 await 路径全 async，无 nested block_on。OK
- **风险 5**：lib.rs `WindowEvent::Resized(_)` 分支若不存在需要新增；现有代码仅有 `Moved` 分支（已在 §7.1 注释中标明）

## 14. 关联

- 父 issue [#34](https://github.com/tl0502/APET/issues/34)（已原地覆写）
- 同 pattern：[#10](https://github.com/tl0502/APET/issues/10) pet last_position + [#28 follow-up](https://github.com/tl0502/APET/issues/31) pomodoro last_position
- ADR：[decisions.md ADR-021](../../../decisions.md)（workspace 多 panel 壳；本 spec 不修订 ADR）
- 既有相关代码：[window_state.rs](../../../../src-tauri/src/services/window_state.rs)（`SaveDebouncer` / `LastPosition` / `clamp_into_monitor` / `fallback_default` 等可复用）
