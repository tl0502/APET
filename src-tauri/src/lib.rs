// M1 D1 脚手架 → M1 D4 IPC 框架（#4）→ M1 D5 数据层（#5）→ 2026-05-06 code-review 重构
// → #6 系统托盘 + 关闭语义（M1 W2 主态可达交付物）。
// 后续按 milestone 节奏接入：shortcuts / cursor_tracker / window:Moved emit 等。

mod commands;
mod services;

use services::shortcuts::ShortcutRegistry;
use services::window_actions::{PET_WINDOW_LABEL, SETTINGS_WINDOW_LABEL};
use services::window_state::SaveDebouncer;
use tauri::Manager;
use tauri_plugin_sql::{Migration, MigrationKind};

const DB_URL: &str = "sqlite:aipet.db";

/// SQLite migrations。
///
/// 每次新加 migration 必须：
/// - 用单调递增的 version
/// - **不修改**已发布的 migration（plugin 已记录 hash，改动会让用户启动失败）
/// - 新增字段用 ALTER TABLE 走新 migration（00X_xxx.sql）
///
/// 历史：002 (persona_snapshots unique idx) 在 2026-05-06 code-review #7
/// 合并回 001（同一 PR 引入，从未对外发布；保留两个 migration 是无意义历史负担）。
fn migrations() -> Vec<Migration> {
    vec![Migration {
        version: 1,
        description: "init schema v1 per architecture v1.1 §4 (ADR-015 三形态共享 ConversationStore)",
        sql: include_str!("../migrations/001_init.sql"),
        kind: MigrationKind::Up,
    }]
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("\n=== APP PANIC ===\n{info}\n=================");
    }));

    tauri::Builder::default()
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations(DB_URL, migrations())
                .build(),
        )
        // #11 全局快捷键 plugin：handler 由 register_internal 通过 on_shortcut 逐个绑定
        // （比 plugin 全局 with_handler 更可靠，详 services/shortcuts.rs::register_internal 注释）。
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            eprintln!("[setup] reached");
            // #11 ShortcutRegistry：先 manage 让 register_chat_on_startup 能拿到 state
            app.manage(ShortcutRegistry::default());
            // #12 LLM 测试 IPC 的活跃 CancellationToken 槽（chat_send_test ↔ cancel_test 共享）
            crate::commands::llm::setup(app.handle());
            // #6 系统托盘 + 菜单（显示/隐藏 / 设置占位 / 退出）
            crate::services::tray::setup(app.handle())?;
            // #5 H.1 内置人格 seed：plugin migrations 已建表，这里 UPSERT momo 行。
            //
            // 2026-05-06 code-review #4：从 spawn(fire-and-forget) 改 block_on 同步等。
            // 理由：fire-and-forget 下，前端在 spawn 完成前调 persona_load("momo")
            // 会拿到 not found，与 onboarding 首启 race。冷启 50-200ms 代价低于人感知阈值。
            let app_handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                if let Err(e) = crate::services::persona::seed_builtin(&app_handle).await {
                    eprintln!("[setup] seed_builtin failed: {e}");
                }
            });
            // #10 桌宠位置还原：读 last_position（无则 fallback 主屏右下角偏左 80px）
            // + 当前 monitor 边界裁剪（16px 安全边距）。同步 block_on 与 seed_builtin 同款理由。
            if let Err(e) = crate::services::window_state::apply_initial_position(app.handle()) {
                eprintln!("[setup] apply_initial_position failed: {e}");
            }
            // #10 Moved 防抖保存：高频 Moved 通过 200ms debounce 节流写 DB。
            app.manage(SaveDebouncer::default());
            // #11 启动期注册 chat 快捷键（DB 无记录 → 默认 Ctrl+Alt+Space）。
            // 失败仅 emit shortcut:register-failed 不阻断启动。
            crate::services::shortcuts::register_chat_on_startup(app.handle());
            Ok(())
        })
        // #6 关闭语义：Alt+F4 / 系统命令关闭主窗口时不退出进程，改 hide。
        // 唯一退出路径 = 托盘"退出"菜单。理由：桌宠是常驻应用，误触关闭就杀进程会损害预期。
        // tauri.conf.json 已 decorations:false，故无标题栏 X 按钮；只走 Alt+F4 / 系统命令路径。
        //
        // #9 settings 窗口同款语义：关闭仅 hide，保留 webview 与 tab 状态供下次唤起；
        // 重新打开走托盘菜单 → settings_show 走 show + set_focus（window_actions::show_settings）。
        //
        // #10 pet 窗口 Moved 事件：高频触发（每像素一次），通过 SaveDebouncer 200ms 节流
        // 防抖落 DB；不消费 pointerup（startDragging 系统级拖动 OS 接管 mouse，pointerup
        // 不冒泡到 webview，单 Moved + debounce 已足够可靠）。
        .on_window_event(|window, event| {
            let label = window.label();
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    if label == PET_WINDOW_LABEL || label == SETTINGS_WINDOW_LABEL {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
                tauri::WindowEvent::Moved(_) if label == PET_WINDOW_LABEL => {
                    let app = window.app_handle();
                    if let Some(pet) = app.get_webview_window(PET_WINDOW_LABEL) {
                        let debouncer = app.state::<SaveDebouncer>();
                        debouncer.schedule(pet);
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            // #4 system
            commands::system::ping,
            // #5 persona
            commands::persona::persona_load,
            commands::persona::persona_activate,
            // #5 nickname
            commands::nickname::nickname_get_pet,
            commands::nickname::nickname_get_user,
            commands::nickname::nickname_set_pet,
            commands::nickname::nickname_set_user,
            commands::nickname::nickname_restore_pet,
            // #5 KV 偏好（services::preferences；IPC 名沿用 memory_*，与 schema 列名 `memory` 一致）
            commands::memory::memory_get,
            commands::memory::memory_set,
            commands::memory::memory_list,
            commands::memory::memory_delete,
            // #9 window 控制（settings show/hide）+ #10 pet 位置 get/save
            commands::window::settings_show,
            commands::window::settings_hide,
            commands::window::get_pet_position,
            commands::window::save_pet_position,
            // #11 shortcuts（probe + set chat）
            commands::shortcuts::probe_global_shortcut,
            commands::shortcuts::set_shortcut_chat,
            // #12 LLM 测试 IPC（dev console 验证用；#13 ChatService MVP 上线后真消费 LLMProvider trait）
            commands::llm::set_openai_api_key,
            commands::llm::get_openai_api_key_set,
            commands::llm::chat_send_test,
            commands::llm::cancel_test,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
