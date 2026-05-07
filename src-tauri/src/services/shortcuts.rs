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

pub const CONFIG_KEY_SHORTCUT_CHAT: &str = "shortcut:chat";
pub const DEFAULT_SHORTCUT_CHAT: &str = "Ctrl+Alt+Space";
pub const SHORTCUT_CHAT_EVENT: &str = "shortcut:chat";
pub const SHORTCUT_REGISTER_FAILED_EVENT: &str = "shortcut:register-failed";

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
#[derive(Default)]
pub struct ShortcutRegistry {
    pub current_chat: Mutex<Option<String>>,
}

fn parse_shortcut(s: &str) -> Result<Shortcut, String> {
    s.parse::<Shortcut>()
        .map_err(|e| format!("解析快捷键失败 '{s}': {e}"))
}

/// 启动期入口：从 config 读 shortcut:chat（无则用默认）→ register。
/// 失败 emit `shortcut:register-failed` 不阻断启动（PRD §7.2.2 fallback 文字 chat）。
pub fn register_chat_on_startup<R: Runtime>(app: &AppHandle<R>) {
    let stored = block_on(config::get(app, CONFIG_KEY_SHORTCUT_CHAT)).unwrap_or(None);
    let shortcut_str = stored.unwrap_or_else(|| DEFAULT_SHORTCUT_CHAT.to_string());

    match register_internal(app, &shortcut_str) {
        Ok(()) => {
            eprintln!("[shortcut] registered chat shortcut: {shortcut_str}");
        }
        Err(e) => {
            eprintln!("[shortcut] register failed for '{shortcut_str}': {e}");
            let _ = app.emit(
                SHORTCUT_REGISTER_FAILED_EVENT,
                ShortcutRegisterFailedPayload {
                    shortcut: shortcut_str,
                    error: e,
                },
            );
        }
    }
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

/// 改 chat 快捷键：unregister 旧 + register 新 + 落 config。
/// 失败时不影响旧快捷键状态（已先成功 unregister 再 register；如果 register 新的失败，
/// 调用方会失去旧的，可由前端在 set 失败后手动 register 旧值兜底；M1 不做这层 transaction）。
pub fn set_chat_shortcut<R: Runtime>(app: &AppHandle<R>, new_shortcut: &str) -> Result<(), String> {
    let _new = parse_shortcut(new_shortcut)?;
    unregister_current(app)?;
    register_internal(app, new_shortcut)?;
    block_on(config::set(app, CONFIG_KEY_SHORTCUT_CHAT, new_shortcut))
        .map_err(|e| e.to_string())?;
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
        let payload = ShortcutChatPayload {
            source: "global_shortcut",
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        };
        if let Err(e) = app.emit(SHORTCUT_CHAT_EVENT, &payload) {
            eprintln!("[shortcut] emit chat failed: {e}");
        }
    }
}
