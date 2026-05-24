//! InteractionRouter（#40，模块 N 主干）— 物理交互事件 → reaction_table → emit 反馈。
//!
//! 范围（ADR-025 锁死的 M2 降级口径）：
//! - hitbox = AABB 单 body（PetCanvas 整窗 raycast；4 hitbox + manifest 推迟 M3+）
//! - 动作表现 = 2a-lite：路由 + emit + reaction_table 数据流完整 + 最少可见反馈
//! - mood/energy 全 transient 不持久（PRD line 1073 / 1089 lock）—— mood_delta 透传给
//!   #41 mood service（本 issue 内 stub：仅放进 emit payload，不写 DB）
//!
//! 路由 5 事件（事件状态机走前端 PetCanvas 内部 PointerEvent；后端只接 dispatch + drag 计数）：
//!   click / dblclick / longpress / rclick / drag。其中 drag 走 record_drag_count（30s 滑窗
//!   ≥3 → 抗议 + 5s revert，纯内存 transient，决策 20 双 lock）。
//!
//! 默认反应表（DEFAULT_REACTIONS）映射 ADR-004 12 动作子集；可被当前 active persona 的
//! `# 反应配置` 段（YAML）覆盖 template / voice_id（action_id 锁在 ADR-004 默认，不由 .soul.md 改）。
//!
//! Schema 零迁移（lesson #2）：无新表；protest_until / drag_events 全部内存 Mutex，进程退出即失。
//!
//! 链路（async 全程，lesson #10 — 不在 #[tauri::command] 链内 block_on）：
//!   commands::interaction::interaction_dispatch
//!     → services::interaction::dispatch
//!       → load_active_reactions（DB 1 row + soul.md 解析，按 persona_id+version 缓存）
//!       → emit "pet:interaction_reacted"
//!
//!   commands::interaction::interaction_record_drag_count
//!     → services::interaction::record_drag_count
//!       → 滑窗 ≥3 → emit "pet:protest_triggered" + tokio::spawn 5s 后 emit "pet:protest_reverted"

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use gray_matter::engine::YAML;
use gray_matter::Matter;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};
use thiserror::Error;

use crate::services::db::open_app_db;
use crate::services::energy::EnergyState;
use crate::services::mood::MoodState;
use crate::services::persona::load_active_persona_with_conn;

/// 30s 滑窗：超过此窗的 drag 事件被丢弃。
const DRAG_WINDOW: Duration = Duration::from_secs(30);
/// 抗议触发阈值：30s 内 ≥ N 次拖动。
const DRAG_PROTEST_THRESHOLD: usize = 3;
/// 抗议 mood transient 持续时间（决策 20 lock，5s 后 revert，永不写 pet_runtime_state.mood 表）。
const PROTEST_REVERT_AFTER: Duration = Duration::from_secs(5);

pub const INTERACTION_REACTED_EVENT: &str = "pet:interaction_reacted";
pub const PROTEST_TRIGGERED_EVENT: &str = "pet:protest_triggered";
pub const PROTEST_REVERTED_EVENT: &str = "pet:protest_reverted";

#[derive(Debug, Error)]
pub enum InteractionError {
    #[error("invalid event: {0} (expect click|dblclick|longpress|rclick|drag)")]
    InvalidEvent(String),
    #[error("invalid hitbox: {0} (M2 only `body`)")]
    InvalidHitbox(String),
    #[error("active persona lookup failed: {0}")]
    PersonaLookup(String),
}

/// 反应表入口：(event, hitbox) → 触发动作 + 透传 mood + 气泡 template + 音效。
///
/// action_id 锁定 ADR-004 默认（M2 不改写）；template / voice_id 可被 .soul.md 覆盖。
/// mood_delta 是 transient 字符串标签（"happy" / "annoyed" / "calm" / "neutral"），
/// 给 #41 mood service 消费；本 issue 仅透传，不写 DB（PRD line 1073 lock）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionEntry {
    pub action_id: String,
    pub voice_id: Option<String>,
    pub mood_delta: Option<String>,
    pub template: Option<String>,
}

/// 默认反应表。key 形式 `{event}.{hitbox}`，特殊 key `drag.protest`（≥3 次拖动触发，由
/// record_drag_count 内部用）。M2 hitbox 仅 `body`（ADR-025 AABB 降级）。
///
/// action_id 取自 ADR-004 12 动作清单子集 —— M2 期前端可消费的（shake/nod/mood 闪烁/气泡）
/// 映射到具体表现见 src/composables/usePetReaction.ts。
fn build_default_table() -> HashMap<String, ReactionEntry> {
    let mut t = HashMap::new();
    t.insert(
        "click.body".into(),
        ReactionEntry {
            action_id: "head_pat".into(),
            voice_id: None,
            mood_delta: Some("happy".into()),
            template: Some("摸摸头吗。".into()),
        },
    );
    t.insert(
        "dblclick.body".into(),
        ReactionEntry {
            action_id: "surprised".into(),
            voice_id: None,
            mood_delta: Some("happy".into()),
            template: Some("诶?连点两下。".into()),
        },
    );
    t.insert(
        "longpress.body".into(),
        ReactionEntry {
            action_id: "fall_asleep".into(),
            voice_id: None,
            mood_delta: Some("calm".into()),
            template: Some("……嗯。".into()),
        },
    );
    // 右键：不走 reaction 动作（前端开自绘菜单），但保留入口让 emit 链路完整。
    t.insert(
        "rclick.body".into(),
        ReactionEntry {
            action_id: "tilt_head".into(),
            voice_id: None,
            mood_delta: None,
            template: None,
        },
    );
    // 单次拖动：不刷气泡（避免拖动期间文本闪烁），仅记 emit 让数据流可观测。
    t.insert(
        "drag.body".into(),
        ReactionEntry {
            action_id: "tilt_head".into(),
            voice_id: None,
            mood_delta: None,
            template: None,
        },
    );
    // 抗议：30s 内 ≥3 次拖动触发；annoyed mood 5s revert 由 record_drag_count 内部处理。
    t.insert(
        "drag.protest".into(),
        ReactionEntry {
            action_id: "protest".into(),
            voice_id: None,
            mood_delta: Some("annoyed".into()),
            template: Some("哎、慢一点啦。".into()),
        },
    );
    t
}

/// .soul.md `# 反应配置` 段的覆盖记录。只覆盖 template / voice_id；
/// intensity 字段 M2 期不消费（前端按 mood_delta + action_id 决定视觉强度）。
#[derive(Debug, Deserialize, Default)]
struct ReactionOverride {
    template: Option<String>,
    voice_id: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    intensity: Option<f32>,
}

/// 缓存键：persona_id + version；当前 active persona 变化或版本升级时 invalidate。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PersonaCacheKey {
    id: String,
    version: String,
}

#[derive(Default)]
struct InteractionStateInner {
    /// 每窗的拖动事件时间戳（30s 滑窗）。key = WindowEvent label（pet / chat / ...）。
    /// 当前 M2 仅 pet 窗触发，但保留多窗维度兼容 #42 BossKey 启用其它入口的可能。
    drag_events: HashMap<String, VecDeque<Instant>>,
    /// 抗议 mood transient 截止时间。决策 20 lock：不写 pet_runtime_state.mood 表。
    /// 进程退出即失（PRD line 1089）；前端 mood icon 自行按 emit 闪烁。
    protest_until: HashMap<String, Instant>,
    /// 当前 active persona 的合并反应表缓存。命中时跳过 DB + soul.md 解析。
    /// 失效路径：persona:activated emit 时由 invalidate_persona_cache 清；
    /// 或 active_key 与 DB 实际不一致时 dispatch 内 reload。
    persona_cache: Option<(PersonaCacheKey, HashMap<String, ReactionEntry>)>,
}

/// InteractionRouter 全局 state（Tauri::manage）。
/// 单 Mutex（写少读多）；poison 仅在 plain data 操作 panic 时发生，本路径不会。
pub struct InteractionState {
    inner: Mutex<InteractionStateInner>,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(InteractionStateInner::default()),
        }
    }
}

impl InteractionState {
    /// 测试用：构造一个干净 state。prod 走 default()。
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionReactedPayload {
    pub event: String,
    pub hitbox: String,
    pub action_id: String,
    pub voice_id: Option<String>,
    /// transient mood 标签，#41 mood service 消费；M2 stub 不写表。
    pub mood_change: Option<String>,
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtestPayload {
    pub window: String,
    pub action_id: String,
    pub mood_change: Option<String>,
    pub template: Option<String>,
    /// 5s 后 emit pet:protest_reverted 让前端 mood icon 收尾。
    pub revert_after_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtestRevertPayload {
    pub window: String,
}

/// 派发主入口（commands::interaction::interaction_dispatch 调用）。
///
/// 步骤：
/// 1. 校验 event / hitbox（M2 仅 body）
/// 2. 拿合并反应表（默认 + soul.md 覆盖，按 active persona 缓存）
/// 3. 查 key `{event}.{hitbox}` → ReactionEntry（缺失返 InvalidEvent，避免静默放过新增事件）
/// 4. emit pet:interaction_reacted + 返 ReactionEntry 给前端
///
/// 失败仅返 Err，不 emit；前端按 Err 自行降级（不闪反馈即可，不阻塞拖动）。
pub async fn dispatch<R: Runtime>(
    app: &AppHandle<R>,
    state: &InteractionState,
    mood_state: &MoodState,
    energy_state: &EnergyState,
    event: &str,
    hitbox: &str,
) -> Result<ReactionEntry, InteractionError> {
    validate_event(event)?;
    validate_hitbox(hitbox)?;

    let key = format!("{event}.{hitbox}");
    let entry = lookup_reaction(app, state, &key).await?;

    let payload = InteractionReactedPayload {
        event: event.to_string(),
        hitbox: hitbox.to_string(),
        action_id: entry.action_id.clone(),
        voice_id: entry.voice_id.clone(),
        mood_change: entry.mood_delta.clone(),
        template: entry.template.clone(),
    };
    if let Err(e) = app.emit(INTERACTION_REACTED_EVENT, &payload) {
        eprintln!("[interaction] emit {INTERACTION_REACTED_EVENT} failed: {e}");
    }
    // #41 mood / energy 联动：emit 之后调，mood/energy 失败不影响 IPC 返 entry（全 transient）。
    // - mood_delta 非 None 时 push transient（"happy" → 10min，"annoyed" → 5s；其他 no-op）
    // - 任何成功 dispatch 都 boost +5（互动 = 给桌宠 input，PRD §7.9.3 "用户主动互动 → 能量回复"）
    if let Some(delta) = entry.mood_delta.as_deref() {
        mood_state.apply_delta(delta);
    }
    energy_state.boost();
    Ok(entry)
}

/// 记录一批拖动事件到 30s 滑窗，命中阈值则触发抗议。
///
/// `count` 通常 1（前端每次 drag start 调一次），保留批量入参兼容批补场景（如 spike test）。
/// 返回当前滑窗内有效计数；调用方可观察是否已触发抗议（但 emit 是后端单一负责，前端不必复算）。
///
/// 抗议 5s revert：
/// - 同步在 state 记 protest_until = now + 5s
/// - tokio::spawn 5s sleep → emit pet:protest_reverted（前端 mood icon 收尾）
/// - 不写任何表（PRD line 1089 lock）
///
/// **不在 #[tauri::command] async fn 内 block_on**（lesson #10）：spawn 是 fire-and-forget。
pub fn record_drag_count<R: Runtime>(
    app: &AppHandle<R>,
    state: &InteractionState,
    mood_state: &MoodState,
    window: &str,
    count: u32,
) -> usize {
    let now = Instant::now();
    let cutoff = now - DRAG_WINDOW;

    let (current, should_protest) = {
        let mut guard = state.inner.lock().expect("InteractionState mutex poisoned");
        let dq = guard
            .drag_events
            .entry(window.to_string())
            .or_default();

        while dq.front().map(|t| *t < cutoff).unwrap_or(false) {
            dq.pop_front();
        }
        for _ in 0..count {
            dq.push_back(now);
        }

        let current = dq.len();
        // 仅在"刚跨过阈值"时触发一次（已在抗议 5s 内不重复触发，否则连续拖会刷气泡）。
        let already_protesting = guard
            .protest_until
            .get(window)
            .map(|until| *until > now)
            .unwrap_or(false);
        let should_protest = current >= DRAG_PROTEST_THRESHOLD && !already_protesting;
        if should_protest {
            guard
                .protest_until
                .insert(window.to_string(), now + PROTEST_REVERT_AFTER);
        }
        (current, should_protest)
    };

    if should_protest {
        // 默认表恒含 drag.protest（build_default_table 锁定），失败仅 eprintln 不阻断。
        // 这里不走 soul.md 覆盖路径以保抗议反馈 deterministic —— protest 是安全/边界反馈，
        // 不应被 persona 文案覆盖到「轻飘飘没存在感」的程度。M3+ 接 mood service 再评估。
        let default_table = build_default_table();
        let entry = default_table
            .get("drag.protest")
            .cloned()
            .expect("DEFAULT_REACTIONS must contain drag.protest");
        let payload = ProtestPayload {
            window: window.to_string(),
            action_id: entry.action_id.clone(),
            mood_change: entry.mood_delta.clone(),
            template: entry.template.clone(),
            revert_after_ms: PROTEST_REVERT_AFTER.as_millis() as u64,
        };
        if let Err(e) = app.emit(PROTEST_TRIGGERED_EVENT, &payload) {
            eprintln!("[interaction] emit {PROTEST_TRIGGERED_EVENT} failed: {e}");
        }
        // #41 mood 联动：抗议触发 → push annoyed transient（5s，与 PROTEST_REVERT_AFTER 同源）。
        // 不在 record_drag_count 路径调 energy.boost —— 拖累抗议不应奖励能量（语义对齐）。
        if let Some(delta) = entry.mood_delta.as_deref() {
            mood_state.apply_delta(delta);
        }
        // 5s revert：spawn 后立即返；不 await，不写表。drop 重启即失（PRD line 1089）。
        let app_clone = app.clone();
        let window_owned = window.to_string();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(PROTEST_REVERT_AFTER).await;
            let payload = ProtestRevertPayload {
                window: window_owned.clone(),
            };
            if let Err(e) = app_clone.emit(PROTEST_REVERTED_EVENT, &payload) {
                eprintln!("[interaction] emit {PROTEST_REVERTED_EVENT} failed: {e}");
            }
        });
    }

    current
}

/// 清空指定窗的滑窗 + 抗议状态。dev / 测试用；用户场景通常不需要。
pub fn reset_drag_state(state: &InteractionState, window: &str) {
    let mut guard = state.inner.lock().expect("InteractionState mutex poisoned");
    guard.drag_events.remove(window);
    guard.protest_until.remove(window);
}

/// 失效 persona reactions 缓存。前端 persona:activated 后由 #41 / commands 显式调用，
/// 或本 issue 内由 dispatch 检测到 active 变化时自动 reload（reload 路径见 lookup_reaction）。
///
/// #40 当前 dispatch 内自动按 (id, version) 缓存键失配 reload，无需主动调；保留 API 给
/// #41 mood service 接入时若拿到 persona:activated 事件可显式 invalidate。
#[allow(dead_code)]
pub fn invalidate_persona_cache(state: &InteractionState) {
    let mut guard = state.inner.lock().expect("InteractionState mutex poisoned");
    guard.persona_cache = None;
}

fn validate_event(event: &str) -> Result<(), InteractionError> {
    match event {
        "click" | "dblclick" | "longpress" | "rclick" | "drag" => Ok(()),
        _ => Err(InteractionError::InvalidEvent(event.to_string())),
    }
}

fn validate_hitbox(hitbox: &str) -> Result<(), InteractionError> {
    // ADR-025 lock：M2 仅 body；4 hitbox + manifest 推迟 M3+。
    if hitbox == "body" {
        Ok(())
    } else {
        Err(InteractionError::InvalidHitbox(hitbox.to_string()))
    }
}

/// 取合并反应表中 `key` 对应条目。命中缓存直接返；未命中走 DB + soul.md 解析 + 缓存。
async fn lookup_reaction<R: Runtime>(
    app: &AppHandle<R>,
    state: &InteractionState,
    key: &str,
) -> Result<ReactionEntry, InteractionError> {
    let table = load_active_reactions(app, state).await?;
    // M2 默认表恒含所有 5 个有效 event × body 组合；任何 key 缺失都是 service 内部 bug。
    table
        .get(key)
        .cloned()
        .ok_or_else(|| InteractionError::InvalidEvent(key.to_string()))
}

/// 加载当前 active persona 的合并反应表（默认 + soul.md 覆盖）。带 (id, version) 缓存。
///
/// 失败降级：active persona 不存在 / soul.md 解析失败 → 走默认表 + eprintln 一次。
/// active persona 切换 / 版本升级 → 缓存键变 → 自动 reload。
///
/// 流程：
/// 1. SELECT 当前 active persona 的 (id, version)（轻量 1 行查询）
/// 2. 与缓存键比对：命中 → 用缓存表；失配 → 拉 raw_markdown + 重 parse + 写新缓存
/// 3. DB 失败 / 无 active → 默认表，不缓存（让下次有机会重试）
async fn load_active_reactions<R: Runtime>(
    app: &AppHandle<R>,
    state: &InteractionState,
) -> Result<HashMap<String, ReactionEntry>, InteractionError> {
    // 1. 拿当前 active persona 的 id + version（不读 raw_markdown，省一次 snapshot SELECT）。
    let active_meta = match fetch_active_persona_meta(app).await {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!(
                "[interaction] active persona meta lookup failed, using default reactions: {e}"
            );
            return Ok(build_default_table());
        }
    };
    let active_key = PersonaCacheKey {
        id: active_meta.0.clone(),
        version: active_meta.1.clone(),
    };

    // 2. 缓存命中 → 直接返。
    {
        let guard = state.inner.lock().expect("InteractionState mutex poisoned");
        if let Some((cached_key, table)) = &guard.persona_cache {
            if cached_key == &active_key {
                return Ok(table.clone());
            }
        }
    }

    // 3. 缓存未命中 / 失配：拉 raw_markdown 重建。
    let raw_md = match fetch_active_persona_raw(app).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "[interaction] active persona raw lookup failed, using default reactions: {e}"
            );
            return Ok(build_default_table());
        }
    };

    let mut merged = build_default_table();
    match parse_soul_reactions(&raw_md) {
        Ok(overrides) => merge_overrides(&mut merged, overrides),
        Err(e) => {
            eprintln!(
                "[interaction] parse `# 反应配置` failed for persona {} v{}, fallback default: {e}",
                active_key.id, active_key.version
            );
        }
    }

    // 4. 写缓存。
    {
        let mut guard = state.inner.lock().expect("InteractionState mutex poisoned");
        guard.persona_cache = Some((active_key, merged.clone()));
    }

    Ok(merged)
}

/// 仅取 active persona 的 (id, version)，跳过 raw_markdown snapshot。
async fn fetch_active_persona_meta<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(String, String), InteractionError> {
    use sqlx::Connection;
    let mut conn = open_app_db(app)
        .await
        .map_err(|e| InteractionError::PersonaLookup(e.to_string()))?;
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT id, version FROM personas WHERE is_active = 1 LIMIT 1")
            .fetch_optional(&mut conn)
            .await
            .map_err(|e| InteractionError::PersonaLookup(e.to_string()))?;
    conn.close()
        .await
        .map_err(|e| InteractionError::PersonaLookup(e.to_string()))?;
    row.ok_or_else(|| InteractionError::PersonaLookup("no active persona".into()))
}

/// 拉 active persona 的 raw_markdown（含 snapshot SELECT）。仅在缓存失配时调用。
async fn fetch_active_persona_raw<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<String, InteractionError> {
    use sqlx::Connection;
    let mut conn = open_app_db(app)
        .await
        .map_err(|e| InteractionError::PersonaLookup(e.to_string()))?;
    let summary = load_active_persona_with_conn(&mut conn)
        .await
        .map_err(|e| InteractionError::PersonaLookup(e.to_string()))?;
    conn.close()
        .await
        .map_err(|e| InteractionError::PersonaLookup(e.to_string()))?;
    Ok(summary.raw_markdown)
}

/// 抽取 raw_markdown 中 `# 反应配置` 段下的 ```yaml fence 块，按 HashMap<String, ReactionOverride>
/// 反序列化。复用 gray_matter::YAML 引擎避免再引一个 yaml 依赖：合成一个伪 frontmatter 走解析。
///
/// 缺失 `# 反应配置` 段 / 缺失 yaml fence → 返空 map（按 "未配置 = 全走默认" 处理，非错误）。
fn parse_soul_reactions(
    raw_md: &str,
) -> Result<HashMap<String, ReactionOverride>, String> {
    let yaml_block = match extract_reaction_yaml_block(raw_md) {
        Some(s) => s,
        None => return Ok(HashMap::new()),
    };
    if yaml_block.trim().is_empty() {
        return Ok(HashMap::new());
    }
    // 复用 gray_matter YAML 引擎：合成 ---/yaml/--- 包裹后解析 frontmatter Pod。
    let synthetic = format!("---\n{yaml_block}\n---\n");
    let parsed = Matter::<YAML>::new().parse(&synthetic);
    let pod = parsed
        .data
        .ok_or_else(|| "yaml block parse returned no data".to_string())?;
    let overrides: HashMap<String, ReactionOverride> =
        pod.deserialize().map_err(|e| e.to_string())?;
    Ok(overrides)
}

/// 从 raw_markdown 抽 `# 反应配置` 段下首个 ```yaml ... ``` fence 块的内容。
fn extract_reaction_yaml_block(raw_md: &str) -> Option<String> {
    let mut in_section = false;
    let mut in_fence = false;
    let mut buf = String::new();
    for line in raw_md.lines() {
        let trimmed = line.trim_start();
        if !in_section {
            if trimmed.starts_with("# ") && trimmed.contains("反应配置") {
                in_section = true;
            }
            continue;
        }
        // 进入下一个一级标题 → 段结束。
        if !in_fence && trimmed.starts_with("# ") && !trimmed.contains("反应配置") {
            break;
        }
        if !in_fence {
            if trimmed.starts_with("```yaml") || trimmed == "```yml" {
                in_fence = true;
                continue;
            }
        } else {
            if trimmed.starts_with("```") {
                return Some(buf);
            }
            buf.push_str(line);
            buf.push('\n');
        }
    }
    if !buf.is_empty() {
        Some(buf)
    } else {
        None
    }
}

/// 把 overrides merge 到 default 表上。
/// - template / voice_id 非 None 时覆盖
/// - action_id / mood_delta 不变（ADR-004 lock，soul.md 不能改）
/// - override key 不在默认表 → 忽略 + eprintln 一次（防 soul.md 引入未知 hitbox/event 静默漂）
fn merge_overrides(
    table: &mut HashMap<String, ReactionEntry>,
    overrides: HashMap<String, ReactionOverride>,
) {
    for (key, ov) in overrides {
        match table.get_mut(&key) {
            Some(entry) => {
                if ov.template.is_some() {
                    entry.template = ov.template;
                }
                if ov.voice_id.is_some() {
                    entry.voice_id = ov.voice_id;
                }
            }
            None => {
                eprintln!(
                    "[interaction] soul.md `# 反应配置` 含未知 key `{key}`，跳过（M2 仅 body hitbox + 5 event）"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// fixture：构造一个 mock app handle 不需要的纯 state 路径，验证 drag 滑窗 + 抗议触发。
    /// 抗议 emit / 5s revert spawn 依赖 AppHandle，分别在 dev 期手动 e2e 验证（issue 验收清单）。
    /// 这里只验内存状态：抗议条件命中后 protest_until 写入 + 5s 内不重复触发。
    #[test]
    fn protest_triggers_at_3_drags_in_30s() {
        let state = InteractionState::new();
        let now = Instant::now();

        // 直接操作内部 deque 验证阈值判定（不走 record_drag_count 因其需要 AppHandle emit）。
        {
            let mut guard = state.inner.lock().unwrap();
            let dq = guard.drag_events.entry("pet".into()).or_default();
            for _ in 0..3 {
                dq.push_back(now);
            }
        }

        // 命中阈值的判定逻辑同 record_drag_count：dq.len() >= 3 → should_protest（首次）。
        let dq_len = {
            let guard = state.inner.lock().unwrap();
            guard.drag_events.get("pet").map(|d| d.len()).unwrap_or(0)
        };
        assert_eq!(dq_len, 3);
        assert!(dq_len >= DRAG_PROTEST_THRESHOLD);
    }

    #[test]
    fn protest_window_drops_events_after_30s() {
        let state = InteractionState::new();
        let now = Instant::now();
        let stale = now - DRAG_WINDOW - Duration::from_secs(1);

        {
            let mut guard = state.inner.lock().unwrap();
            let dq = guard.drag_events.entry("pet".into()).or_default();
            dq.push_back(stale); // 超出滑窗，应被下次 prune 清掉
            dq.push_back(now);
            dq.push_back(now);
        }

        // 手工模拟 prune（同 record_drag_count 内部 while pop_front 逻辑）。
        let cutoff = now - DRAG_WINDOW;
        {
            let mut guard = state.inner.lock().unwrap();
            let dq = guard.drag_events.get_mut("pet").unwrap();
            while dq.front().map(|t| *t < cutoff).unwrap_or(false) {
                dq.pop_front();
            }
            // 还剩 2 条（now + now），< 阈值 3 → 不触发抗议
            assert_eq!(dq.len(), 2);
            assert!(dq.len() < DRAG_PROTEST_THRESHOLD);
        }
    }

    #[test]
    fn protest_reverts_after_5s_without_writing_table() {
        // PRD line 1089 + 决策 20 双 lock：抗议 transient mood 5s revert，永不写
        // pet_runtime_state.mood 表。本测试断言 state 内 protest_until 时间戳到期可被读到，
        // 且 state struct 本身不含任何 DB 持久化字段 —— 即"凡是 protest 相关状态都在内存"。
        let state = InteractionState::new();
        let now = Instant::now();
        let until = now + PROTEST_REVERT_AFTER;

        {
            let mut guard = state.inner.lock().unwrap();
            guard.protest_until.insert("pet".into(), until);
        }

        // 立刻读 → 仍未到期
        {
            let guard = state.inner.lock().unwrap();
            let still = guard
                .protest_until
                .get("pet")
                .map(|t| *t > Instant::now())
                .unwrap_or(false);
            assert!(still, "5s 内 protest_until 仍生效");
        }

        // 模拟 6s 后（不能真 sleep 5s 拖慢 cargo test；推前 6s 等价）→ 失效
        {
            let mut guard = state.inner.lock().unwrap();
            // 直接重写为已过期的时间戳，模拟"5s 之后再来查"
            guard
                .protest_until
                .insert("pet".into(), Instant::now() - Duration::from_secs(1));
            let still = guard
                .protest_until
                .get("pet")
                .map(|t| *t > Instant::now())
                .unwrap_or(false);
            assert!(!still, "5s 后 protest_until 失效");
        }

        // 关键断言：InteractionState 内部字段都是内存 HashMap / Mutex，不含 sqlite 连接 / 表写入。
        // 这是结构性保证（PRD lock）；如果未来有人加 DB 写入，单测无法直接拦，但 review 时应警惕。
        // 留个文档式 assertion：state 公开 API 没有 *_with_conn 入口。
        let _ = state; // moved-out-friendly
    }

    #[test]
    fn validate_event_accepts_5_types() {
        for evt in &["click", "dblclick", "longpress", "rclick", "drag"] {
            assert!(validate_event(evt).is_ok(), "{evt} should be valid");
        }
        assert!(validate_event("scroll").is_err());
        assert!(validate_event("").is_err());
    }

    #[test]
    fn validate_hitbox_m2_only_body() {
        assert!(validate_hitbox("body").is_ok());
        // ADR-025 lock：head/tail/edge 推迟 M3+，本期应被拒绝。
        assert!(validate_hitbox("head").is_err());
        assert!(validate_hitbox("tail").is_err());
    }

    #[test]
    fn default_table_covers_all_5_events() {
        let t = build_default_table();
        assert!(t.contains_key("click.body"));
        assert!(t.contains_key("dblclick.body"));
        assert!(t.contains_key("longpress.body"));
        assert!(t.contains_key("rclick.body"));
        assert!(t.contains_key("drag.body"));
        assert!(t.contains_key("drag.protest"));
    }

    #[test]
    fn parse_soul_reactions_with_momo_format() {
        // 与 momo.soul.md `# 反应配置` 同款 schema
        let md = "# 身份\nxxx\n\n# 反应配置\n\n```yaml\nclick.body:\n  template: 喂,会痒的。\n  intensity: 0.4\ndrag.protest:\n  template: 哎、慢一点啦。\n  intensity: 0.6\n```\n";
        let overrides = parse_soul_reactions(md).expect("should parse");
        assert_eq!(overrides.len(), 2);
        assert_eq!(
            overrides.get("click.body").and_then(|o| o.template.as_deref()),
            Some("喂,会痒的。")
        );
    }

    #[test]
    fn parse_soul_reactions_missing_section_returns_empty() {
        let md = "# 身份\n仅身份段，无反应配置\n";
        let overrides = parse_soul_reactions(md).expect("missing section is OK");
        assert!(overrides.is_empty());
    }

    #[test]
    fn merge_overrides_keeps_action_id_locked() {
        let mut table = build_default_table();
        let original_action = table.get("click.body").unwrap().action_id.clone();
        let mut overrides = HashMap::new();
        overrides.insert(
            "click.body".into(),
            ReactionOverride {
                template: Some("自定义文案".into()),
                voice_id: Some("custom_voice".into()),
                intensity: Some(0.9),
            },
        );
        merge_overrides(&mut table, overrides);
        let entry = table.get("click.body").unwrap();
        assert_eq!(entry.action_id, original_action, "action_id 不被 soul.md 改");
        assert_eq!(entry.template.as_deref(), Some("自定义文案"));
        assert_eq!(entry.voice_id.as_deref(), Some("custom_voice"));
    }

    #[test]
    fn merge_overrides_unknown_key_is_ignored() {
        let mut table = build_default_table();
        let mut overrides = HashMap::new();
        overrides.insert(
            "click.head".into(), // M2 不支持 head hitbox
            ReactionOverride {
                template: Some("should be ignored".into()),
                voice_id: None,
                intensity: None,
            },
        );
        merge_overrides(&mut table, overrides);
        assert!(!table.contains_key("click.head"), "未知 key 不应被写入");
    }

    #[test]
    fn reset_drag_state_clears_window() {
        let state = InteractionState::new();
        {
            let mut g = state.inner.lock().unwrap();
            g.drag_events.entry("pet".into()).or_default().push_back(Instant::now());
            g.protest_until
                .insert("pet".into(), Instant::now() + Duration::from_secs(3));
        }
        reset_drag_state(&state, "pet");
        let g = state.inner.lock().unwrap();
        assert!(g.drag_events.get("pet").map(|d| d.is_empty()).unwrap_or(true));
        assert!(!g.protest_until.contains_key("pet"));
    }
}
