// 全局快捷键服务（#11 PRD §7.2.2）
//
// 范围：
// - 启动期注册 chat 快捷键（默认 Ctrl+Alt+Space，可被 config 覆盖）
// - 触发时 emit `shortcut:chat`（payload: { source, timestamp_ms }）
// - 注册失败 emit `shortcut:register-failed`（不阻断启动；M1 不弹用户提示）
// - probe(shortcut)：尝试 register→unregister 验可用性（给 #17 Onboarding Step 3 用）
// - set_shortcut_chat(new)：unregister 旧 + register 新 + 写 config 持久化
//
// 存储：sqlite `config` 表，key=`CONFIG_KEY_SHORTCUT_CHAT`，value=快捷键字符串。
//   注：issue #11 字面是 `settings` 表 `shortcut_chat` 字段，但 schema 没保留 settings 表
//   （27 表零迁移原则）。改用 config 表的 KV 存储（语义零损失，与 #10 同款偏离）。

use std::sync::Mutex;

use serde::Serialize;
use tauri::async_runtime::block_on;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState as GsState};

use crate::services::config;
use crate::services::consent_gate::ConsentGate;

pub const CONFIG_KEY_SHORTCUT_CHAT: &str = "shortcut:chat";
pub const DEFAULT_SHORTCUT_CHAT: &str = "Ctrl+Alt+Space";
pub const SHORTCUT_CHAT_EVENT: &str = "shortcut:chat";
pub const SHORTCUT_REGISTER_FAILED_EVENT: &str = "shortcut:register-failed";

/// #35 ADR-021 P1 workspace 主窗快捷键（Phase E）。
/// MVP 阶段只暴露启动期注册 + 失败留痕查询；用户改键 UI 留 M2 follow-up（plan 风险 5 决策）。
pub const CONFIG_KEY_SHORTCUT_WORKSPACE: &str = "shortcut:workspace";
pub const DEFAULT_SHORTCUT_WORKSPACE: &str = "Ctrl+Alt+W";
pub const SHORTCUT_WORKSPACE_EVENT: &str = "shortcut:workspace";

#[derive(Serialize, Clone)]
pub struct ShortcutChatPayload {
    pub source: &'static str,
    pub timestamp_ms: i64,
}

#[derive(Serialize, Clone)]
pub struct ShortcutRegisterFailedPayload {
    pub shortcut: String,
    pub error: String,
}

#[derive(Serialize)]
pub struct ProbeResult {
    pub available: bool,
    pub error: Option<String>,
}

/// 当前已注册的 chat 快捷键。set_shortcut_chat 用它 unregister 旧值。
/// Mutex<Option<String>>：存原始字符串（解析后的 Shortcut 不实现 Clone in 2.x，重新 parse 即可）
///
/// `last_chat_error`：启动期 register 失败的留痕，给前端 App.vue mount 时查询用（#21
/// 收尾 #2）。解决 setup 内 emit 早于 webview 完成 JS 初始化 / listener 挂载的 race：
/// emit `shortcut:register-failed` 单走会丢，listener 挂之后查 registry 兜底拿到状态。
/// set_chat_shortcut 成功路径中 clear；后续动态 register 失败时（M2 摸鱼快捷键场景）也走这里。
#[derive(Default)]
pub struct ShortcutRegistry {
    pub current_chat: Mutex<Option<String>>,
    pub last_chat_error: Mutex<Option<ShortcutRegisterFailedPayload>>,
    /// #35 Phase E：workspace 启动期注册留痕（与 chat 平行；不复用因事件名 / 默认键不同）。
    pub current_workspace: Mutex<Option<String>>,
    pub last_workspace_error: Mutex<Option<ShortcutRegisterFailedPayload>>,
}

fn parse_shortcut(s: &str) -> Result<Shortcut, String> {
    s.parse::<Shortcut>()
        .map_err(|e| format!("解析快捷键失败 '{s}': {e}"))
}

/// 启动期入口：从 config 读 shortcut:chat（无则用默认）→ register。
/// 失败 emit `shortcut:register-failed` + 写 last_chat_error 不阻断启动（PRD §7.2.2
/// fallback 文字 chat）。emit + 留痕双路径解决 setup 内 emit 早于前端 listener 挂载
/// 的 race（详 ShortcutRegistry.last_chat_error 注释）。
pub fn register_chat_on_startup<R: Runtime>(app: &AppHandle<R>) {
    let stored = block_on(config::get(app, CONFIG_KEY_SHORTCUT_CHAT)).unwrap_or(None);
    let shortcut_str = stored.unwrap_or_else(|| DEFAULT_SHORTCUT_CHAT.to_string());

    match register_internal(app, &shortcut_str) {
        Ok(()) => {
            eprintln!("[shortcut] registered chat shortcut: {shortcut_str}");
            clear_last_chat_error(app);
        }
        Err(e) => {
            eprintln!("[shortcut] register failed for '{shortcut_str}': {e}");
            let payload = ShortcutRegisterFailedPayload {
                shortcut: shortcut_str,
                error: e,
            };
            set_last_chat_error(app, payload.clone());
            let _ = app.emit(SHORTCUT_REGISTER_FAILED_EVENT, payload);
        }
    }
}

fn set_last_chat_error<R: Runtime>(app: &AppHandle<R>, payload: ShortcutRegisterFailedPayload) {
    if let Some(registry) = app.try_state::<ShortcutRegistry>() {
        if let Ok(mut slot) = registry.last_chat_error.lock() {
            *slot = Some(payload);
        }
    }
}

fn clear_last_chat_error<R: Runtime>(app: &AppHandle<R>) {
    if let Some(registry) = app.try_state::<ShortcutRegistry>() {
        if let Ok(mut slot) = registry.last_chat_error.lock() {
            *slot = None;
        }
    }
}

/// 读取启动期 register 留痕。前端 App.vue mount 时查询，None = 当前无错误（启动期 OK 或
/// 用户已通过 set_chat_shortcut 改键成功）；Some(p) = 启动期未恢复的失败，需提示用户。
pub fn last_chat_register_error<R: Runtime>(
    app: &AppHandle<R>,
) -> Option<ShortcutRegisterFailedPayload> {
    let registry = app.try_state::<ShortcutRegistry>()?;
    let slot = registry.last_chat_error.lock().ok()?;
    slot.clone()
}

fn register_internal<R: Runtime>(app: &AppHandle<R>, shortcut_str: &str) -> Result<(), String> {
    let shortcut = parse_shortcut(shortcut_str)?;
    // 用 on_shortcut（不是 register）— per-shortcut handler 更可靠：
    // plugin 全局 handler（with_handler）在某些 Windows 环境下不触发（global_hotkey
    // set_event_handler callback 与 plugin handler 分发链路有 bug 报告）；on_shortcut
    // 把 handler 直接绑到该 shortcut 的 RegisteredShortcut.handler 字段，plugin closure
    // 第一段 `if let Some(handler) = &shortcut.handler` 就会命中。
    app.global_shortcut()
        .on_shortcut(shortcut, |app, shortcut, event| {
            handle_shortcut_pressed(app, shortcut, event);
        })
        .map_err(|e| e.to_string())?;
    let registry = app.state::<ShortcutRegistry>();
    let mut slot = registry
        .current_chat
        .lock()
        .map_err(|e| format!("registry lock poisoned: {e}"))?;
    *slot = Some(shortcut_str.to_string());
    Ok(())
}

fn unregister_current<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let registry = app.state::<ShortcutRegistry>();
    let mut slot = registry
        .current_chat
        .lock()
        .map_err(|e| format!("registry lock poisoned: {e}"))?;
    if let Some(prev) = slot.take() {
        let s = parse_shortcut(&prev)?;
        app.global_shortcut()
            .unregister(s)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 测试快捷键是否可用（被其他 app 占用 → false）。试 register → 立刻 unregister。
/// 给 #17 Onboarding Step 3 冲突探测用。
///
/// **fast-path**：如果探测目标 == 本应用当前已注册的快捷键，直接返 available=true。
/// 没有这条路径时，"探测默认值"会假报占用 —— register_chat_on_startup 已注册默认键,
/// 再 register 同 key 会被 plugin 拒绝（HotKey already registered）。fast-path 让
/// onboarding Step 3 显示"默认可用"成立。
pub fn probe<R: Runtime>(app: &AppHandle<R>, shortcut_str: &str) -> ProbeResult {
    let s = match parse_shortcut(shortcut_str) {
        Ok(s) => s,
        Err(e) => {
            return ProbeResult {
                available: false,
                error: Some(e),
            }
        }
    };
    // fast-path：自己已注册的不算占用
    if let Some(registry) = app.try_state::<ShortcutRegistry>() {
        if let Ok(slot) = registry.current_chat.lock() {
            if slot.as_deref() == Some(shortcut_str) {
                return ProbeResult {
                    available: true,
                    error: None,
                };
            }
        }
    }
    match app.global_shortcut().register(s) {
        Ok(()) => {
            // 立刻清理（probe 不应残留注册）
            let _ = app.global_shortcut().unregister(s);
            ProbeResult {
                available: true,
                error: None,
            }
        }
        Err(e) => ProbeResult {
            available: false,
            error: Some(e.to_string()),
        },
    }
}

/// 读取当前已注册的 chat 快捷键。
///
/// Some(s) = 启动期 register 成功，s 是当前生效值；
/// None = 启动期 register 失败（系统占用 / 解析失败 / 平台异常），当前没快捷键能用。
///
/// 给 Onboarding Step 3 用：前端拿到准确状态后渲染（None → 显示 DEFAULT + 真实 probe；
/// Some → 显示 s + fast-path probe），避免前后端 default 字面不一致带来的漂移。
pub fn current_chat_shortcut<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    let registry = app.try_state::<ShortcutRegistry>()?;
    let slot = registry.current_chat.lock().ok()?;
    slot.clone()
}

/// 改 chat 快捷键：unregister 旧 + register 新 + 落 config。
///
/// **async** 因为内部 config::set 是 async；从 #[tauri::command] async fn 调入时，
/// 必须 await 而不是 block_on —— 后者在 tokio runtime 内嵌套会死锁（IPC 卡住,前端
/// 永远拿不到结果）。register_chat_on_startup 用 block_on 是因为它在 lib.rs setup
/// 同步上下文,与本 async fn 路径分离。
///
/// 失败时不影响旧快捷键状态（已先成功 unregister 再 register；如果 register 新的失败，
/// 调用方会失去旧的，可由前端在 set 失败后手动 register 旧值兜底；M1 不做这层 transaction）。
pub async fn set_chat_shortcut<R: Runtime>(
    app: &AppHandle<R>,
    new_shortcut: &str,
) -> Result<(), String> {
    let _new = parse_shortcut(new_shortcut)?;
    unregister_current(app)?;
    register_internal(app, new_shortcut)?;
    config::set(app, CONFIG_KEY_SHORTCUT_CHAT, new_shortcut)
        .await
        .map_err(|e| e.to_string())?;
    // 用户改键成功 → 清启动期遗留的失败留痕，避免后续 mount 还 toast
    clear_last_chat_error(app);
    Ok(())
}

/// per-shortcut handler（M1 只有 1 个：chat 唤起）。
/// 由 register_internal 通过 on_shortcut 绑定，每次按下都被 plugin closure 第一段触发。
pub fn handle_shortcut_pressed<R: Runtime>(
    app: &AppHandle<R>,
    shortcut: &Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    // dev 期诊断：handler 未触发 → 看不到此行；触发但 emit 失败 → 看到 fired 但前端无 toast
    eprintln!(
        "[shortcut] handler fired: shortcut={:?} state={:?}",
        shortcut,
        event.state()
    );
    if event.state() == GsState::Pressed {
        // #21 锁死边界：onboarding 未完成时静默不 emit。pet 窗虽 hidden 但其 webview
        // 仍 alive（hide 不卸载），App.vue 的 'shortcut:chat' listener 仍在跑——若放行
        // emit，会一路触发 chat_toggle → chat 窗冒出来，绕过宣誓页。
        // 不 unregister 快捷键、只在 handler 单点拦截：注册保持简单，行为可预测。
        let gate_open = app
            .try_state::<ConsentGate>()
            .map(|g| g.is_open())
            .unwrap_or(false);
        if !gate_open {
            eprintln!("[shortcut] suppressed (onboarding not complete)");
            return;
        }
        let payload = ShortcutChatPayload {
            source: "global_shortcut",
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        };
        if let Err(e) = app.emit(SHORTCUT_CHAT_EVENT, &payload) {
            eprintln!("[shortcut] emit chat failed: {e}");
        }
    }
}

// ============================================================
// #35 Phase E：workspace 快捷键（与 chat 平行；handler 单独 emit shortcut:workspace）
// ============================================================

/// 启动期注册 workspace 快捷键。失败 emit + 留痕，不阻断启动（plan 风险 5：键冲突时
/// 仍可走托盘 / DoubleClick 三入口的另外两路）。
pub fn register_workspace_on_startup<R: Runtime>(app: &AppHandle<R>) {
    let stored = block_on(config::get(app, CONFIG_KEY_SHORTCUT_WORKSPACE)).unwrap_or(None);
    let shortcut_str = stored.unwrap_or_else(|| DEFAULT_SHORTCUT_WORKSPACE.to_string());

    match register_workspace_internal(app, &shortcut_str) {
        Ok(()) => {
            eprintln!("[shortcut] registered workspace shortcut: {shortcut_str}");
            clear_last_workspace_error(app);
        }
        Err(e) => {
            eprintln!("[shortcut] register workspace failed for '{shortcut_str}': {e}");
            let payload = ShortcutRegisterFailedPayload {
                shortcut: shortcut_str,
                error: e,
            };
            set_last_workspace_error(app, payload.clone());
            // 复用同一个失败事件名（前端按 payload.shortcut 判定是哪个快捷键）
            let _ = app.emit(SHORTCUT_REGISTER_FAILED_EVENT, payload);
        }
    }
}

fn register_workspace_internal<R: Runtime>(
    app: &AppHandle<R>,
    shortcut_str: &str,
) -> Result<(), String> {
    let shortcut = parse_shortcut(shortcut_str)?;
    app.global_shortcut()
        .on_shortcut(shortcut, |app, shortcut, event| {
            handle_workspace_shortcut_pressed(app, shortcut, event);
        })
        .map_err(|e| e.to_string())?;
    let registry = app.state::<ShortcutRegistry>();
    let mut slot = registry
        .current_workspace
        .lock()
        .map_err(|e| format!("registry lock poisoned: {e}"))?;
    *slot = Some(shortcut_str.to_string());
    Ok(())
}

fn set_last_workspace_error<R: Runtime>(
    app: &AppHandle<R>,
    payload: ShortcutRegisterFailedPayload,
) {
    if let Some(registry) = app.try_state::<ShortcutRegistry>() {
        if let Ok(mut slot) = registry.last_workspace_error.lock() {
            *slot = Some(payload);
        }
    }
}

fn clear_last_workspace_error<R: Runtime>(app: &AppHandle<R>) {
    if let Some(registry) = app.try_state::<ShortcutRegistry>() {
        if let Ok(mut slot) = registry.last_workspace_error.lock() {
            *slot = None;
        }
    }
}

/// 读启动期 register workspace 留痕（前端 WorkspaceApp mount 时查询）。
///
/// MVP 暂未消费（plan 风险 5 决策：M1 失败时仍可走托盘 / 双击两入口；M2 settings UI
/// 重设快捷键时才暴露给前端）。allow dead_code 标记预留位。
#[allow(dead_code)]
pub fn last_workspace_register_error<R: Runtime>(
    app: &AppHandle<R>,
) -> Option<ShortcutRegisterFailedPayload> {
    let registry = app.try_state::<ShortcutRegistry>()?;
    let slot = registry.last_workspace_error.lock().ok()?;
    slot.clone()
}

/// 读当前已注册的 workspace 快捷键（None = 启动期 register 失败）。
///
/// 同 last_workspace_register_error：M2 settings UI 消费方上线后启用。
#[allow(dead_code)]
pub fn current_workspace_shortcut<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    let registry = app.try_state::<ShortcutRegistry>()?;
    let slot = registry.current_workspace.lock().ok()?;
    slot.clone()
}

pub fn handle_workspace_shortcut_pressed<R: Runtime>(
    app: &AppHandle<R>,
    shortcut: &Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    eprintln!(
        "[shortcut] workspace handler fired: shortcut={:?} state={:?}",
        shortcut,
        event.state()
    );
    if event.state() == GsState::Pressed {
        // 同 chat：onboarding 未完成时静默不 emit
        let gate_open = app
            .try_state::<ConsentGate>()
            .map(|g| g.is_open())
            .unwrap_or(false);
        if !gate_open {
            eprintln!("[shortcut] workspace suppressed (onboarding not complete)");
            return;
        }
        // workspace 不需要 source/timestamp（toggle 是幂等动作），payload 为空对象
        if let Err(e) = app.emit(SHORTCUT_WORKSPACE_EVENT, serde_json::json!({})) {
            eprintln!("[shortcut] emit workspace failed: {e}");
        }
    }
}
