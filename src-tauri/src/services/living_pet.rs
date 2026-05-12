// LivingPetService（#21 M1 收尾，模块 I.1）— 自由活动初版（flows §10）。
//
// 设计原则（M1 极简版，与用户拍板）：
// - 状态机骨架先到位，主状态 enum 预留 M2-M3 variant（Focus/Remind/InGame/BossKey）
//   但 M1 实际只填 Idle；IDLE 子状态只填 Still / Wandering，DailyAction 留给 M3 R.2
// - 调度器 5-15min 随机抖动 → 25% 概率 wander，75% 原地不动（still 在 M1 = 跳过）
// - wander：当前位置 ± 屏宽 5%（≤96px @ 1920w），cubic easeInOut tween 5-15s
//   30fps（33ms / step）在后端直接 set_position，不走前端 IPC（每帧 IPC 太重，
//   且 M1 前端无消费方）
// - 不做 look_around（需 VRM 美术 clip / M2）、不做"归位"（与 #10 持久化合流：
//   wander 终点即下次起点）、不做前端 IPC（M1 无消费方）
// - 拒绝引 rand crate：xorshift64 + chrono 纳秒种子完全够 5-15min 触发使用
//
// 与窗口位置持久化（#10）的协作：
// - wander tween 每帧 set_position → 触发 WindowEvent::Moved → SaveDebouncer 200ms
//   防抖落 DB。即 wander 完成后约 200ms 写一次最终位置；中间过程不写
// - 用户在 wander 期间手动拖动会触发 startDragging（系统级），与 tween set_position
//   争夺窗口位置；M1 不处理这个 race（实测如果体验差再加 cancel flag）
//
// dev 期实测：默认 5-15min 太久，设环境变量 `LIVING_PET_DEV_INTERVAL=5` 可缩到 5s

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{AppHandle, LogicalPosition, Manager, Runtime};

use crate::services::consent_gate::ConsentGate;
use crate::services::window_actions::PET_WINDOW_LABEL;

/// 主状态机；M1 实际只有 Idle 路径有代码消费，其余 variant 给 M2-M3 接入用
/// （FocusService 开启专注、GameService 进入 IN_GAME、TaskService REMIND、模块 K 摸鱼）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // M2-M3 variant 当前未消费
pub enum MainState {
    Idle,
    Focus,
    Remind,
    InGame,
    BossKeyHidden,
}

/// IDLE 子状态；M1 实际只切 Still ↔ Wandering，DailyAction 给 M3 R.2 时段表用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum IdleSubState {
    Still,
    Wandering,
    DailyAction,
}

#[derive(Debug, Clone, Copy)]
pub struct PetStateSnapshot {
    pub main: MainState,
    pub idle_sub: IdleSubState,
}

/// 全局状态容器（app.manage）— 当前内部 mutable 走 std::sync::Mutex。
/// 选 std Mutex 不选 parking_lot：M1 无热路径锁竞争（每 5-15min + tween 每 33ms
/// 各 ~1 次 lock），std 性能足够；与 ChatService.active_streams 那种"被 cancel 路径
/// 高并发"场景不同，不强求 parking_lot。
#[derive(Default)]
pub struct LivingPet {
    state: Mutex<PetStateInternal>,
}

#[derive(Debug, Clone, Copy)]
struct PetStateInternal {
    main: MainState,
    idle_sub: IdleSubState,
}

impl Default for PetStateInternal {
    fn default() -> Self {
        Self {
            main: MainState::Idle,
            idle_sub: IdleSubState::Still,
        }
    }
}

impl LivingPet {
    /// 当前状态快照（不持有锁返回，调用方可放心打印 / 比较）。
    #[allow(dead_code)] // M2 接入前无消费方；保留以便调度器与未来 IPC 使用
    pub fn snapshot(&self) -> PetStateSnapshot {
        let g = self.state.lock().unwrap_or_else(|e| e.into_inner());
        PetStateSnapshot {
            main: g.main,
            idle_sub: g.idle_sub,
        }
    }

    fn set_idle_sub(&self, sub: IdleSubState) {
        let mut g = self.state.lock().unwrap_or_else(|e| e.into_inner());
        g.idle_sub = sub;
    }
}

// ───── xorshift64 伪随机 ─────
// 用 static AtomicU64 持种子；首次 load=0 时用 chrono 纳秒补初值（避免 panic on unwrap）。
// 比 SystemTime 路径更可控（chrono 已是项目依赖）；比 thread_rng 省一个 crate。

static RNG_STATE: AtomicU64 = AtomicU64::new(0);

fn rand_u64() -> u64 {
    let mut s = RNG_STATE.load(Ordering::Relaxed);
    if s == 0 {
        // 纳秒做种；timestamp_nanos_opt 在 2262 年后才会返 None（远超本应用生命周期）
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(1) as u64;
        s = nanos | 1; // 保证非 0
    }
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    RNG_STATE.store(s, Ordering::Relaxed);
    s
}

fn rand_range_u64(min: u64, max_inclusive: u64) -> u64 {
    debug_assert!(max_inclusive >= min);
    min + rand_u64() % (max_inclusive - min + 1)
}

fn rand_range_f64(min: f64, max: f64) -> f64 {
    debug_assert!(max >= min);
    min + (rand_u64() as f64 / u64::MAX as f64) * (max - min)
}

// ───── 配置常量 ─────

/// 调度间隔（flows §10.1：每 5-15 分钟随机抖动）。
const INTERVAL_MIN_SEC: u64 = 5 * 60;
const INTERVAL_MAX_SEC: u64 = 15 * 60;
/// 单次 wander 总时长（flows §10.1：移动 5-15 秒）。
const WANDER_DURATION_MIN_S: f64 = 5.0;
const WANDER_DURATION_MAX_S: f64 = 15.0;
/// tween 步长（33ms ≈ 30fps）；后端直接 set_position 不走 IPC，OS SetWindowPos
/// 调用是百微秒级，30fps 完全顶得住，看起来比 60fps 还稳（避免 webview 同步重绘竞争）。
const WANDER_TICK_MS: u64 = 33;
/// wander 目标点偏移范围 = 屏宽 5%（flows §10.1）；@1920w 等于 96px。
const WANDER_RANGE_RATIO: f64 = 0.05;
/// IDLE.Still 时被调度时切到 wander 的概率；75% 留在原地避免桌宠太活跃打扰用户。
const WANDER_PROB: f64 = 0.25;

/// 桌宠窗口逻辑尺寸（与 window_state.rs 一致；提到 const 避免 magic number）。
const PET_W: f64 = 320.0;
const PET_H: f64 = 320.0;
/// 边界裁剪安全边距，与 #10 apply_initial_position 一致。
const SAFE_MARGIN: f64 = 16.0;

// ───── 调度器 ─────

/// 启动期 spawn 调度 task。lib.rs::setup 调用一次即可，task 与进程同生命周期。
///
/// dev 期实测：默认 5-15min 等不起 → 设 env var `LIVING_PET_DEV_INTERVAL=5`（秒）
/// 即可强制固定 5s 间隔，看 wander 效果。release build 用户机器不会动 env，自然按
/// 5-15min 正常分布。
pub fn start_scheduler<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        loop {
            let sleep_sec = next_interval_sec();
            tokio::time::sleep(Duration::from_secs(sleep_sec)).await;

            // #21 锁死边界：onboarding 未完成时跳过 wander。pet 窗 hidden 时 set_position
            // 仍会改实际位置，dev `LIVING_PET_DEV_INTERVAL=5` 下 5s 就触发，完成 onboarding
            // 后用户会看到 pet 在被偷偷移动过的奇怪位置。状态机不进 Wandering，下次调度仍可正常工作。
            let gate_open = app
                .try_state::<ConsentGate>()
                .map(|g| g.is_open())
                .unwrap_or(false);
            if !gate_open {
                continue;
            }

            // 前置检查：pet 窗存在且可见。覆盖"用户托盘隐藏 / 主态 hide"路径——ConsentGate
            // 仅 gate onboarding 边界，gate.open() 之后用户仍可主动隐藏 pet 窗；hidden 时
            // wander 会偷偷移动 + 触发 SaveDebouncer 持久化，下次显示位置漂移。
            match app.get_webview_window(PET_WINDOW_LABEL) {
                Some(w) => match w.is_visible() {
                    Ok(true) => {}
                    Ok(false) => continue,
                    Err(e) => {
                        eprintln!("[living_pet] is_visible failed, skip this tick: {e}");
                        continue;
                    }
                },
                None => continue,
            }

            let living = app.state::<LivingPet>();
            let s = living.snapshot();
            // 前置检查：主状态必须 Idle + 子状态必须 Still。
            // M1 主状态恒 Idle，子状态在 wander 结束前会留 Wandering（防止上一次 wander
            // 未结束时新一次 wander 重入），都能正确门禁。
            if s.main != MainState::Idle || s.idle_sub != IdleSubState::Still {
                continue;
            }

            // dev 实测模式（LIVING_PET_DEV_INTERVAL=N 已生效）强制 wander 概率 100%——
            // env 设了 5s 间隔但 25% 概率，用户可能等 1 分钟还没命中 wander 误判为 bug。
            // release build 用户机器不会动这个 env，自然按 25% 正常分布。
            if !is_dev_mode() && rand_range_f64(0.0, 1.0) >= WANDER_PROB {
                continue;
            }

            if let Err(e) = run_wander(&app).await {
                eprintln!("[living_pet] wander failed: {e}");
                // 兜底：异常路径下确保状态回 Still，否则下次调度永远被门禁挡住
                living.set_idle_sub(IdleSubState::Still);
            }
        }
    });
}

fn next_interval_sec() -> u64 {
    if let Ok(raw) = std::env::var("LIVING_PET_DEV_INTERVAL") {
        if let Ok(n) = raw.parse::<u64>() {
            if n > 0 {
                return n;
            }
        }
    }
    rand_range_u64(INTERVAL_MIN_SEC, INTERVAL_MAX_SEC)
}

/// dev 实测模式判定（与 next_interval_sec 共享同一 env 解析规则）。
/// 设了有效值的 LIVING_PET_DEV_INTERVAL → true。release build 用户机器不会动此 env。
fn is_dev_mode() -> bool {
    std::env::var("LIVING_PET_DEV_INTERVAL")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .is_some_and(|n| n > 0)
}

/// 一次 wander：选目标 → cubic easeInOut tween → 状态回 Still。
async fn run_wander<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = app
        .get_webview_window(PET_WINDOW_LABEL)
        .ok_or_else(|| "pet window not found".to_string())?;
    let monitor = window
        .current_monitor()
        .map_err(|e| format!("current_monitor: {e}"))?
        .ok_or_else(|| "current_monitor None".to_string())?;
    let scale = monitor.scale_factor();
    let pos_physical = window
        .outer_position()
        .map_err(|e| format!("outer_position: {e}"))?;
    let start = LogicalPosition::<f64>::from_physical(pos_physical, scale);
    let mon_size = monitor.size().to_logical::<f64>(scale);
    let mon_origin = monitor.position().to_logical::<f64>(scale);

    // 随机目标（屏宽 5%）+ 随机时长（5-15s）
    let max_offset = mon_size.width * WANDER_RANGE_RATIO;
    let dx = rand_range_f64(-max_offset, max_offset);
    let dy = rand_range_f64(-max_offset, max_offset);
    let duration_s = rand_range_f64(WANDER_DURATION_MIN_S, WANDER_DURATION_MAX_S);

    let raw_tx = start.x + dx;
    let raw_ty = start.y + dy;

    // clamp 到 monitor 边界安全边距
    let min_x = mon_origin.x + SAFE_MARGIN;
    let min_y = mon_origin.y + SAFE_MARGIN;
    let max_x = mon_origin.x + mon_size.width - PET_W - SAFE_MARGIN;
    let max_y = mon_origin.y + mon_size.height - PET_H - SAFE_MARGIN;
    // monitor 太小（小于桌宠 + safe margin × 2）时 max < min，用 min.min/max 兜底防 NaN
    let tx = raw_tx.clamp(min_x.min(max_x), min_x.max(max_x));
    let ty = raw_ty.clamp(min_y.min(max_y), min_y.max(max_y));

    let living = app.state::<LivingPet>();
    living.set_idle_sub(IdleSubState::Wandering);

    let total_ms = (duration_s * 1000.0) as u64;
    let mut elapsed_ms: u64 = 0;
    while elapsed_ms < total_ms {
        let t = elapsed_ms as f64 / total_ms as f64;
        let e = ease_in_out_cubic(t);
        let cx = start.x + (tx - start.x) * e;
        let cy = start.y + (ty - start.y) * e;
        // set_position 失败时整次 wander 放弃；外层把状态切回 Still 兜底
        window
            .set_position(LogicalPosition::new(cx, cy))
            .map_err(|e| format!("set_position mid-tween: {e}"))?;
        tokio::time::sleep(Duration::from_millis(WANDER_TICK_MS)).await;
        elapsed_ms += WANDER_TICK_MS;
    }
    // 终点 snap（避免最后一步因 elapsed_ms 略小于 total_ms 而停在 99% 位置）
    window
        .set_position(LogicalPosition::new(tx, ty))
        .map_err(|e| format!("set_position final: {e}"))?;

    living.set_idle_sub(IdleSubState::Still);
    Ok(())
}

/// Cubic easeInOut（standard）：起步/收尾减速，中段加速；视觉比线性自然得多。
fn ease_in_out_cubic(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let f = 2.0 * t - 2.0;
        1.0 + f * f * f / 2.0
    }
}
