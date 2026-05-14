// LivingPetService（#21 M1 收尾，模块 I.1）— 自由活动初版（flows §10）。
//
// 设计原则（M1 极简版，与用户拍板）：
// - 状态机骨架先到位，主状态 enum 预留 M2-M3 variant（Focus/Remind/InGame/BossKey）
//   但 M1 实际只填 Idle；IDLE 子状态只填 Still / Wandering，DailyAction 留给 M3 R.2
// - 调度器 5-15min 随机抖动 → 25% 概率 wander，75% 原地不动（still 在 M1 = 跳过）
// - wander：**多段折线**（参考 Reynolds steering wander 的离散版）—— 一次 wander 拆 3-5 段，
//   每段方向 = 上段方向 ± 30° 小幅扰动，让运动看起来"有意图"而非"被拖去某个点"
// - **Home 回归**（参考 Stardew Valley NPC home location）：偏离启动位置超 10% 屏宽时
//   方向加权偏向 home，防止桌宠 N 次 wander 漂到屏幕角落卡住
// - **打断**（L1）：CancellationToken 监听用户拖动 / 唤起 chat 等事件 → tween 立即停在
//   当前位置（capture current state，不 snap 到段终点），符合 Unity Animator 风格的
//   "current state interrupt"。前端 PetCanvas pointerdown 主键判定后 invoke 取消。
// - 不做 look_around（需 VRM 美术 clip / M2）、不做前端状态 IPC（M1 无消费方）
// - 拒绝引 rand crate：xorshift64 + chrono 纳秒种子完全够 5-15min 触发使用
//
// 与窗口位置持久化（#10）的协作：
// - wander tween 每帧 set_position → 触发 WindowEvent::Moved → SaveDebouncer 200ms
//   防抖落 DB。即 wander 完成后约 200ms 写一次最终位置；中间过程不写
// - 用户拖动时前端 invoke cancel → tween 退出 → SaveDebouncer 200ms 后写"拖动结束的位置"
//
// dev 期实测：默认 5-15min 太久，设环境变量 `LIVING_PET_DEV_INTERVAL=5` 可缩到 5s

use std::f64::consts::PI;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{AppHandle, LogicalPosition, Manager, Runtime};
use tokio_util::sync::CancellationToken;

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
/// 各 ~1 次 lock），std 性能足够。
///
/// `wander_cancel`（L1）：当前 wander tween 的取消句柄；None = 当前没在 wander。
/// 用户拖动 / chat 唤起等输入路径调 cancel_wander() 让 tween 立即退出。
/// `home`（L3）：桌宠"老巢"逻辑坐标；启动期 lazy init 为窗口位置；wander 不更新（避免
/// 漂移累积），用户主动拖动 / 改设置才会变（M2+ 决定要不要支持改 home）。
#[derive(Default)]
pub struct LivingPet {
    state: Mutex<PetStateInternal>,
    wander_cancel: Mutex<Option<CancellationToken>>,
    home: Mutex<Option<(f64, f64)>>,
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

    /// 用户输入路径（拖动 / chat 唤起 / 托盘点击等）调此方法立即取消 wander tween。
    /// 当前没在 wander → no-op；当前在 wander → 触发 token cancel，tween 退出循环。
    /// 不阻塞，不返错（用户输入路径要快）。
    pub fn cancel_wander(&self) {
        let slot = self
            .wander_cancel
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(token) = slot.as_ref() {
            token.cancel();
        }
    }

    /// 写入 wander 取消句柄；run_wander 进入前调用。run_wander 退出（正常或 cancel）
    /// 时调 take 清空，避免下一次调度被旧句柄误中。
    fn arm_cancel_token(&self) -> CancellationToken {
        let token = CancellationToken::new();
        let mut slot = self
            .wander_cancel
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *slot = Some(token.clone());
        token
    }

    fn disarm_cancel_token(&self) {
        let mut slot = self
            .wander_cancel
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *slot = None;
    }

    /// 读 home 坐标（lazy init：首次 wander 调度时如果 None → run_wander 用当前窗口
    /// 位置填）。
    fn home_position(&self) -> Option<(f64, f64)> {
        let slot = self.home.lock().unwrap_or_else(|e| e.into_inner());
        *slot
    }

    fn set_home(&self, x: f64, y: f64) {
        let mut slot = self.home.lock().unwrap_or_else(|e| e.into_inner());
        *slot = Some((x, y));
    }
}

// ───── xorshift64 伪随机 ─────
// 用 static AtomicU64 持种子；首次 load=0 时用 chrono 纳秒补初值（避免 panic on unwrap）。
// 比 SystemTime 路径更可控（chrono 已是项目依赖）；比 thread_rng 省一个 crate。
//
// 使用 fetch_update（CAS RMW）保证多线程并发调用时不丢/不重复:即使将来 M2 引入其他
// rand_u64 消费者（mood/energy 抖动等）也安全。当前实际只有 start_scheduler 单 task 调用,
// fetch_update 在单消费者下基本一次成功 = 零开销。

static RNG_STATE: AtomicU64 = AtomicU64::new(0);

fn rand_u64() -> u64 {
    // fetch_update 返回 update 前的值,我们要 update 后的新值 → 闭包外 capture。
    // 闭包永不返 None,所以 fetch_update 必返 Ok（但仍 unwrap_or 兜底以防类型签名变化）。
    let mut new_state: u64 = 0;
    let _ = RNG_STATE.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |s| {
        let mut s = if s == 0 {
            // 纳秒做种;timestamp_nanos_opt 在 2262 年后才会返 None（远超本应用生命周期）。
            // | 1 保证非 0,xorshift64 一旦初始化非 0 后续状态恒非 0（period = 2^64 - 1）。
            let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(1) as u64;
            nanos | 1
        } else {
            s
        };
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        new_state = s;
        Some(s)
    });
    new_state
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
/// IDLE.Still 时被调度时切到 wander 的概率；75% 留在原地避免桌宠太活跃打扰用户。
const WANDER_PROB: f64 = 0.25;

/// L2：单次 wander 折线段数；3-5 段对应 5-15s 总时长 ≈ 1.5-3s/段（视觉感"小步小步"
/// 而非"一直线冲过去"）。
const WANDER_SEGMENTS_MIN: u64 = 3;
const WANDER_SEGMENTS_MAX: u64 = 5;
/// L2：每段方向相对上段的扰动上限（弧度，±30°）。
/// 取自 Reynolds wander circle 的"长程方向连续性"原则：小幅扰动 → 转向不 jittery。
const WANDER_TURN_PERTURB: f64 = PI / 6.0;
/// L2：单段位移距离上限 = 屏宽 1.5%（@1920w ≈ 29px）。3-5 段累计最远 ≈ 7.5% 屏宽，
/// 与原 5% 旧策略相近，但路径不是直线 → 视觉远距离更"丰富"。
const WANDER_SEGMENT_DIST_RATIO: f64 = 0.015;
/// L3：偏离 home 超此阈值（屏宽比例）时 wander 方向加权朝 home。
/// 10% @ 1920w ≈ 192px，约半个桌宠的 1.5 倍，刚好够"开始觉得跑远了"。
const HOME_DRIFT_THRESHOLD_RATIO: f64 = 0.10;
/// L3：方向偏向 home 的最大权重（0=完全随机，1=完全朝 home）。
/// 0.7 = 偏离阈值的 2 倍时几乎"直奔 home"，避免桌宠在远端拐弯磨蹭。
const HOME_PULL_WEIGHT_MAX: f64 = 0.7;

/// 桌宠窗口当前逻辑尺寸：每次 wander 调度时从 OS 读取（#24 视角档位动态化后，
/// 静态 const 不再可用；OS 窗口本身是单一真相源，setSize 落地后 outer_size 即新值）。
fn pet_logical_size<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    scale: f64,
) -> Result<(f64, f64), String> {
    let physical = window
        .outer_size()
        .map_err(|e| format!("outer_size: {e}"))?;
    let logical = physical.to_logical::<f64>(scale);
    Ok((logical.width, logical.height))
}
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
            }
            // 无论 Ok / Err / cancel：所有路径都要把状态切回 Still + 清掉 cancel token，
            // 否则下次调度永远被门禁挡住、或者新 wander 误中旧 token。
            living.set_idle_sub(IdleSubState::Still);
            living.disarm_cancel_token();
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

/// 一次 wander：多段折线 + 方向连续性扰动 + home 回归倾向 + cancel 监听。
/// cancel 时立即返回（保留当前 tween 位置，不 snap 到段终点）；调用方负责把
/// idle_sub 切回 Still。
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
    // #24：pet 窗逻辑尺寸跟 view_preset 走，每次 wander 从 OS 读当前值（不再用 const）。
    let (pet_w, pet_h) = pet_logical_size(&window, scale)?;

    // 边界 clamp 参数（每段都用得上，提前算好）
    let min_x = mon_origin.x + SAFE_MARGIN;
    let min_y = mon_origin.y + SAFE_MARGIN;
    let max_x = mon_origin.x + mon_size.width - pet_w - SAFE_MARGIN;
    let max_y = mon_origin.y + mon_size.height - pet_h - SAFE_MARGIN;
    // monitor 太小（小于桌宠 + safe margin × 2）时 max < min，用 min.min/max 兜底防 NaN
    let bounds_x = (min_x.min(max_x), min_x.max(max_x));
    let bounds_y = (min_y.min(max_y), min_y.max(max_y));

    // L3 home lazy init：第一次 wander 时把当前位置记为 home。
    // 不在 setup 期记的原因：apply_initial_position 跑在 manage(LivingPet) 之前，
    // 此处 lazy init 顺带省一个外部 hook。home 一旦设定就不动（除非未来 M2+ 引入
    // "用户拖动后重置 home"路径）。
    let living = app.state::<LivingPet>();
    let home = living.home_position().unwrap_or_else(|| {
        living.set_home(start.x, start.y);
        (start.x, start.y)
    });

    // 段数 + 总时长 + 每段时长
    let segments = rand_range_u64(WANDER_SEGMENTS_MIN, WANDER_SEGMENTS_MAX);
    let total_duration_s = rand_range_f64(WANDER_DURATION_MIN_S, WANDER_DURATION_MAX_S);
    let segment_duration_ms = ((total_duration_s * 1000.0) / segments as f64) as u64;

    // 初始方向：完全随机 [0, 2π)
    let mut current_angle = rand_range_f64(0.0, 2.0 * PI);
    let mut current_pos = (start.x, start.y);

    living.set_idle_sub(IdleSubState::Wandering);
    let cancel_token = living.arm_cancel_token();

    let max_segment_dist = mon_size.width * WANDER_SEGMENT_DIST_RATIO;
    let home_drift_threshold = mon_size.width * HOME_DRIFT_THRESHOLD_RATIO;

    for _seg in 0..segments {
        if cancel_token.is_cancelled() {
            return Ok(()); // 段间被 cancel，保留 current_pos 不再移动
        }

        // 本段方向：上段方向 + 小幅扰动（±30°）
        let perturb = rand_range_f64(-WANDER_TURN_PERTURB, WANDER_TURN_PERTURB);
        let mut new_angle = current_angle + perturb;

        // L3 home 回归：偏离 home 超阈值 → 朝 home 方向加权
        let dx_home = home.0 - current_pos.0;
        let dy_home = home.1 - current_pos.1;
        let dist_home = (dx_home * dx_home + dy_home * dy_home).sqrt();
        if dist_home > home_drift_threshold {
            let home_angle = dy_home.atan2(dx_home);
            // 权重随偏离程度线性增长，封顶 HOME_PULL_WEIGHT_MAX
            let excess = (dist_home - home_drift_threshold) / home_drift_threshold;
            let weight = (excess * HOME_PULL_WEIGHT_MAX).clamp(0.0, HOME_PULL_WEIGHT_MAX);
            new_angle = lerp_angle(new_angle, home_angle, weight);
        }

        // 本段目标点
        let raw_target_x = current_pos.0 + max_segment_dist * new_angle.cos();
        let raw_target_y = current_pos.1 + max_segment_dist * new_angle.sin();
        let target_x = raw_target_x.clamp(bounds_x.0, bounds_x.1);
        let target_y = raw_target_y.clamp(bounds_y.0, bounds_y.1);

        // 本段 tween（cubic easeInOut）；select! 监听 cancel
        let seg_start = current_pos;
        let mut elapsed_ms: u64 = 0;
        while elapsed_ms < segment_duration_ms {
            tokio::select! {
                _ = cancel_token.cancelled() => return Ok(()),
                _ = tokio::time::sleep(Duration::from_millis(WANDER_TICK_MS)) => {}
            }
            elapsed_ms += WANDER_TICK_MS;
            let t = (elapsed_ms as f64 / segment_duration_ms as f64).min(1.0);
            let e = ease_in_out_cubic(t);
            let cx = seg_start.0 + (target_x - seg_start.0) * e;
            let cy = seg_start.1 + (target_y - seg_start.1) * e;
            window
                .set_position(LogicalPosition::new(cx, cy))
                .map_err(|e| format!("set_position mid-tween: {e}"))?;
            // 注：tween 内不更新 current_pos —— cancel 路径直接 return（外层不再读），
            // 不 cancel 路径段结束时下方 snap 行会用 (target_x, target_y) 重置。
        }
        // 段终点 snap（不 cancel 路径下，确保 current_pos 精确到段终点供下段起算）
        window
            .set_position(LogicalPosition::new(target_x, target_y))
            .map_err(|e| format!("set_position seg-end: {e}"))?;
        current_pos = (target_x, target_y);
        current_angle = new_angle;
    }

    Ok(())
}

/// 角度球面线性插值（取最短弧路径）；a/b ∈ [0, 2π)，weight ∈ [0,1]。
/// 直接 a + (b - a) * w 会在 a=350°, b=10° 时绕一大圈走错方向；这里规范化到 ±π 范围。
fn lerp_angle(from: f64, to: f64, weight: f64) -> f64 {
    let mut diff = to - from;
    while diff > PI {
        diff -= 2.0 * PI;
    }
    while diff < -PI {
        diff += 2.0 * PI;
    }
    from + diff * weight
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_angle_short_arc_across_2pi_boundary() {
        // 350° → 10° 应走 +20°（最短弧），而不是 -340°
        let from = 350.0_f64.to_radians();
        let to = 10.0_f64.to_radians();
        let result = lerp_angle(from, to, 0.5);
        // 中点应在 0° 附近（误差 ≤ 1°）
        let expected = 0.0_f64;
        let normalized = (result + 2.0 * PI) % (2.0 * PI);
        let dist = normalized
            .min((normalized - 2.0 * PI).abs())
            .min((expected - normalized).abs());
        assert!(
            dist < 0.02,
            "lerp_angle 短弧失败：from=350° to=10° w=0.5 应近 0°，实际 {}",
            normalized.to_degrees()
        );
    }

    #[test]
    fn lerp_angle_weight_zero_returns_from() {
        let from = 1.0;
        let result = lerp_angle(from, 2.5, 0.0);
        assert!((result - from).abs() < 1e-9);
    }

    #[test]
    fn lerp_angle_weight_one_returns_to_via_shortest() {
        let from = 0.1;
        let to = 0.9;
        let result = lerp_angle(from, to, 1.0);
        assert!((result - to).abs() < 1e-9);
    }

    #[test]
    fn ease_in_out_cubic_endpoints() {
        assert!((ease_in_out_cubic(0.0)).abs() < 1e-9);
        assert!((ease_in_out_cubic(1.0) - 1.0).abs() < 1e-9);
        // 中点应为 0.5（cubic ease 对称）
        assert!((ease_in_out_cubic(0.5) - 0.5).abs() < 1e-9);
    }
}
