// M1 D1 脚手架 → M1 D4 IPC 框架（#4）→ M1 D5 数据层（#5）。
// 后续按 milestone 节奏接入：tray / shortcuts / cursor_tracker / on_window_event 等。

mod commands;
mod services;

use tauri_plugin_sql::{Migration, MigrationKind};

const DB_URL: &str = "sqlite:aipet.db";

/// SQLite migrations（#5 M1 W1 数据层入库 + 002 唯一索引补丁）。
///
/// 每次新加 migration 必须：
/// - 用单调递增的 version
/// - **不修改**已发布的 migration（plugin 已记录 hash，改动会让用户启动失败）
/// - 新增字段用 ALTER TABLE 走新 migration（00X_xxx.sql）
fn migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "init schema v1 per architecture v1.1 §4 (ADR-015 三形态共享 ConversationStore)",
            sql: include_str!("../migrations/001_init.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "enforce unique persona_snapshots(persona_id, version)",
            sql: include_str!("../migrations/002_persona_snapshot_unique.sql"),
            kind: MigrationKind::Up,
        },
    ]
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
            // #5 H.1 内置人格 seed：plugin migrations 已建表，这里 UPSERT momo 行
            // 异步跑 + 失败仅 eprintln，不阻塞启动也不弹错误 UI（MVP 期）
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::services::persona::seed_builtin(&app_handle).await {
                    eprintln!("[setup] seed_builtin failed: {e}");
                }
            });
            Ok(())
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
            // #5 memory（KV 偏好表，username/wake_time 等）
            commands::memory::memory_get,
            commands::memory::memory_set,
            commands::memory::memory_list,
            commands::memory::memory_delete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
