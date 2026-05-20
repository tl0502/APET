// snap.rs — Rust 端磁吸 solver（#30 follow-up I：链式抖动修复）。
//
// 背景：前端 group-drag 路径在 60Hz onMoved 时，每帧 N 次 setPosition IPC + N 次
// REGISTRY_BROADCAST emit 形成 IPC 排队。Windows webview2 上 setPosition IPC roundtrip
// ≥5ms，N=2 链就跌到 33Hz，N=3 跌到 22Hz，叠加 startDragging 的 OS-level move 抢锁
// → 严重视觉抖动。
//
// 解法（业界标准）：Rust 端直接订阅 WindowEvent::Moved，本地维护 constraint forest，
// 批量 set_position 所有 dep。同进程 Win32 SetWindowPos 是 μs 级，60fps 完全顶得住。
//
// 同步策略：前端是 constraint 的权威源（drag commit / detach / persistence load 都在前端
// 进行），constraint 变化时通过 snap_sync_constraints IPC 把全量推到 Rust 端。Rust 端
// 只读不写，避免双向写入冲突。
//
// 防死循环：Rust 端 set_position 会触发 dep 自己的 WindowEvent::Moved → 又被本服务接住
// → 死循环。用 internal_until guard（按 label 分桶 + 100ms TTL）跳过自己刚移动的窗，
// 与前端 internalMove.ts 同款思路。
//
// visualInset：前端有 visualInset 模型（chat 12px padding 不计入贴边几何），Rust 端
// 同步时一并接收每窗 inset，compute_final_rect 与前端 applyConstraint 行为对齐。

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, LogicalPosition, Manager, Runtime, WebviewWindow};

/// 防死循环 guard TTL。覆盖 Windows webview2 set_position → OS SetWindowPos → Moved 事件
/// 回灌的 IPC roundtrip（典型 20-50ms，留 100ms 余量）。
const INTERNAL_GUARD_TTL: Duration = Duration::from_millis(100);

/// primary 窗 label 集合（与前端 src/lib/snap/roles.ts PRIMARY_LABELS 对齐）。
/// Rust 端硬编码 'pet'：M3 多 primary 配置 UI 后改为通过 snap_sync_primaries IPC 同步即可。
///
/// 为何需要：Rust on_window_moved 只看 has_dependents(label) 会让 secondary 也能触发 BFS。
/// 例如 chat→pomodoro 已存在时拖 pomodoro，Rust 仍会把 chat 拖过来 — 违反角色模型
/// （secondary 拖动应立即脱钩，不是 group-drag）。加 primary 守卫后 secondary 拖动 Rust
/// 完全跳过，把整族平移行为限定到 primary 拖动场景。
const PRIMARY_LABELS: &[&str] = &["pet"];

fn is_primary(label: &str) -> bool {
    PRIMARY_LABELS.contains(&label)
}

/// 与前端 src/lib/snap/types.ts SnapConstraint 字段镜像（camelCase 化）。
/// 仅磁吸所需子集（不含 enabled / createdAt — Rust solver 不用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapConstraint {
    pub source_id: String,
    pub target_id: String,
    pub source_edge: Edge,
    pub target_edge: Edge,
    pub offset: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// 与前端 WindowRegistration.visualInset 镜像。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualInset {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// Logical-pixel 矩形（与前端 Rect 一致）。
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn apply_inset(&self, inset: &VisualInset) -> Rect {
        Rect {
            x: self.x + inset.left,
            y: self.y + inset.top,
            w: (self.w - inset.left - inset.right).max(0.0),
            h: (self.h - inset.top - inset.bottom).max(0.0),
        }
    }
    fn reverse_inset(&self, inset: &VisualInset) -> Rect {
        Rect {
            x: self.x - inset.left,
            y: self.y - inset.top,
            w: self.w + inset.left + inset.right,
            h: self.h + inset.top + inset.bottom,
        }
    }
}

/// SnapState：Rust 端 constraint forest + visualInset 表 + internal-move guard。
/// 单一 Mutex（写少读多，竞争极低），简单可靠。
pub struct SnapState {
    inner: Mutex<SnapStateInner>,
}

struct SnapStateInner {
    /// source_id → constraint（与前端 ConstraintStore I1 同结构：每 source 单出向）
    by_source: HashMap<String, SnapConstraint>,
    /// target_id → set of source_ids（反向索引，O(1) dependents 查询）
    by_target: HashMap<String, HashSet<String>>,
    /// label → visualInset；缺省走 VisualInset::default()
    insets: HashMap<String, VisualInset>,
    /// 防死循环：label → 解锁时间戳。set_position 前 mark，move handler 检查 skip
    internal_until: HashMap<String, Instant>,
}

impl Default for SnapState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(SnapStateInner {
                by_source: HashMap::new(),
                by_target: HashMap::new(),
                insets: HashMap::new(),
                internal_until: HashMap::new(),
            }),
        }
    }
}

impl SnapState {
    fn lock(&self) -> MutexGuard<'_, SnapStateInner> {
        // Mutex poison 只在 Rust panic 时发生；本 service 内的所有路径都是 plain data 操作，
        // 不在持锁期间调外部 fn，poison 不会发生。万一发生（panic during update）用 into_inner 兜底。
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 全量替换 constraints + insets（前端 snap_sync_constraints IPC 触发）。
    pub fn sync_constraints(
        &self,
        constraints: Vec<SnapConstraint>,
        insets: HashMap<String, VisualInset>,
    ) {
        let mut state = self.lock();
        state.by_source.clear();
        state.by_target.clear();
        for c in constraints {
            state
                .by_target
                .entry(c.target_id.clone())
                .or_default()
                .insert(c.source_id.clone());
            state.by_source.insert(c.source_id.clone(), c);
        }
        state.insets = insets;
    }

    /// 该 label 是否被任一 constraint 标为 target（即有 dependents）。
    /// 没 dependents → Moved 事件 fast-path 直接 return，零开销。
    pub fn has_dependents(&self, label: &str) -> bool {
        let state = self.lock();
        state.by_target.get(label).is_some_and(|s| !s.is_empty())
    }

    /// Moved 事件 fast-path：mark 自己为 internal 时返 true → 跳过 solver。
    /// 用于过滤"本 service 刚 set_position 触发的 Moved 回灌"。
    pub fn is_internal(&self, label: &str) -> bool {
        let mut state = self.lock();
        match state.internal_until.get(label) {
            Some(&until) => {
                if Instant::now() < until {
                    true
                } else {
                    state.internal_until.remove(label);
                    false
                }
            }
            None => false,
        }
    }

    fn mark_internal(&self, label: &str) {
        let mut state = self.lock();
        state
            .internal_until
            .insert(label.to_string(), Instant::now() + INTERNAL_GUARD_TTL);
    }
}

/// applyConstraint（与前端 geometry.ts applyConstraint 同语义）。
/// 输入 source/anchor 都已是 visual rect，输出 source 应到达的 visual rect。
fn compute_final_rect(source: Rect, anchor: Rect, c: &SnapConstraint) -> Rect {
    // sourceEdge 决定 final.x/y：
    // - left → final.x = anchor.x + anchor.w（贴 anchor 右边）
    // - right → final.x = anchor.x - source.w（贴 anchor 左边）
    // - top → final.y = anchor.y + anchor.h
    // - bottom → final.y = anchor.y - source.h
    match c.source_edge {
        Edge::Left => Rect {
            x: anchor.x + anchor.w,
            y: anchor.y + c.offset,
            w: source.w,
            h: source.h,
        },
        Edge::Right => Rect {
            x: anchor.x - source.w,
            y: anchor.y + c.offset,
            w: source.w,
            h: source.h,
        },
        Edge::Top => Rect {
            x: anchor.x + c.offset,
            y: anchor.y + anchor.h,
            w: source.w,
            h: source.h,
        },
        Edge::Bottom => Rect {
            x: anchor.x + c.offset,
            y: anchor.y - source.h,
            w: source.w,
            h: source.h,
        },
    }
}

/// 读 WebviewWindow logical rect。失败返 None（窗已销毁 / 异常）。
fn read_rect<R: Runtime>(window: &WebviewWindow<R>) -> Option<Rect> {
    let phys_pos = window.outer_position().ok()?;
    let phys_size = window.outer_size().ok()?;
    let sf = window.scale_factor().ok()?;
    let pos = phys_pos.to_logical::<f64>(sf);
    let size = phys_size.to_logical::<f64>(sf);
    Some(Rect {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    })
}

/// Moved 事件入口：fast-path → BFS solve → 批量 set_position。
///
/// caller：lib.rs on_window_event WindowEvent::Moved 分支（与现有 SaveDebouncer 并列触发）。
/// 性能：has_dependents fast-path 让无 dep 的窗几乎 0 开销；有 dep 才进 solver。
///
/// 角色守卫：只有 primary 拖动才走 BFS solver。secondary 拖动（即使有 dependents）直接返。
/// 否则 chat→pomodoro 这种 secondary-secondary constraint 在 pomodoro 拖动时会让 Rust
/// 把 chat 拖过来 — 等于 pomodoro 获得了 primary 的整族拖动能力，违反 ADR-020 角色模型。
pub fn on_window_moved<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if !is_primary(label) {
        return; // secondary 拖动 — 不该触发 group-drag 整族跟随
    }
    let snap = app.state::<SnapState>();
    if snap.is_internal(label) {
        return; // 自己刚 set_position 触发的 Moved，跳过避免死循环
    }
    if !snap.has_dependents(label) {
        return; // 无 dep，无需 solver
    }
    let Some(anchor_win) = app.get_webview_window(label) else {
        return;
    };
    let Some(anchor_rect) = read_rect(&anchor_win) else {
        return;
    };
    apply_full_solve(app, label, anchor_rect);
}

/// 全量 BFS solve + apply：每个 dep 实地读 webview size + inset，算出 OS x/y，set_position。
///
/// 设计：
/// - 持锁阶段只算位置（plain data + 同进程 Win32 read，无 IPC）
/// - 收集结果后释放锁，再 mark_internal + set_position（同步调用，但 mark 与 set 之间需绑死避免 race）
/// - I2 forest 保证 BFS 无环 visited set 是保险
fn apply_full_solve<R: Runtime>(app: &AppHandle<R>, anchor_label: &str, anchor_new_rect: Rect) {
    let snap = app.state::<SnapState>();
    let state = snap.lock();
    let mut new_os_rects: HashMap<String, Rect> = HashMap::new();
    new_os_rects.insert(anchor_label.to_string(), anchor_new_rect);
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(anchor_label.to_string());

    // 收集 (label, new_x, new_y)，释放锁后再 set_position
    let mut to_apply: Vec<(String, f64, f64)> = Vec::new();

    while let Some(id) = queue.pop_front() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let Some(dep_ids) = state.by_target.get(&id).cloned() else {
            continue;
        };
        let Some(&anchor_os_rect) = new_os_rects.get(&id) else {
            continue;
        };
        let anchor_inset = state.insets.get(&id).cloned().unwrap_or_default();
        let anchor_visual = anchor_os_rect.apply_inset(&anchor_inset);

        for dep_id in dep_ids {
            let Some(c) = state.by_source.get(&dep_id) else {
                continue;
            };
            let dep_inset = state.insets.get(&dep_id).cloned().unwrap_or_default();
            // 持锁读 webview rect（同进程 Win32，μs 级，不会阻塞）
            let Some(dep_win) = app.get_webview_window(&dep_id) else {
                continue;
            };
            let Some(dep_rect) = read_rect(&dep_win) else {
                continue;
            };
            let dep_visual = dep_rect.apply_inset(&dep_inset);
            let dep_final_visual = compute_final_rect(dep_visual, anchor_visual, c);
            let dep_final_os = dep_final_visual.reverse_inset(&dep_inset);
            to_apply.push((dep_id.clone(), dep_final_os.x, dep_final_os.y));
            new_os_rects.insert(dep_id.clone(), dep_final_os);
            queue.push_back(dep_id);
        }
    }
    drop(state); // 释放锁

    // 逐个 set_position（mark_internal 在 set_position 前，防 Moved 回灌前 guard 未就位）
    for (dep_id, x, y) in to_apply {
        snap.mark_internal(&dep_id);
        if let Some(dep_win) = app.get_webview_window(&dep_id) {
            if let Err(e) = dep_win.set_position(LogicalPosition::new(x, y)) {
                eprintln!("[snap] set_position({dep_id}) failed: {e}");
            }
        }
    }
}

// ===== IPC commands =====

/// 前端 useSnapWindow 在以下场景调：
/// - constraint 变化（commit / detach / persistence load 后）
/// 全量替换 Rust 端 state，避免增量同步带来的状态分歧风险。
#[tauri::command]
pub fn snap_sync_constraints<R: Runtime>(
    app: AppHandle<R>,
    constraints: Vec<SnapConstraint>,
    insets: HashMap<String, VisualInset>,
) -> Result<(), String> {
    let snap = app.state::<SnapState>();
    snap.sync_constraints(constraints, insets);
    Ok(())
}
