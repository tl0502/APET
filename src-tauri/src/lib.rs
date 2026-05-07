// M1 D1 脚手架 → M1 D4 IPC 框架（#4）→ M1 D5 数据层（#5）→ 2026-05-06 code-review 重构
// → #6 系统托盘 + 关闭语义（M1 W2 主态可达交付物）。
// 后续按 milestone 节奏接入：shortcuts / cursor_tracker / window:Moved emit 等。

mod commands;
mod services;

use services::window_actions::{PET_WINDOW_LABEL, SETTINGS_WINDOW_LABEL};
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
        .setup(|app| {
            eprintln!("[setup] reached");
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
            Ok(())
        })
        // #6 关闭语义：Alt+F4 / 系统命令关闭主窗口时不退出进程，改 hide。
        // 唯一退出路径 = 托盘"退出"菜单。理由：桌宠是常驻应用，误触关闭就杀进程会损害预期。
        // tauri.conf.json 已 decorations:false，故无标题栏 X 按钮；只走 Alt+F4 / 系统命令路径。
        //
        // #9 settings 窗口同款语义：关闭仅 hide，保留 webview 与 tab 状态供下次唤起；
        // 重新打开走托盘菜单 → settings_show 走 show + set_focus（window_actions::show_settings）。
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let label = window.label();
                if label == PET_WINDOW_LABEL || label == SETTINGS_WINDOW_LABEL {
                    api.prevent_close();
                    let _ = window.hide();
                }
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
            // #9 window 控制（settings show/hide）
            commands::window::settings_show,
            commands::window::settings_hide,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
