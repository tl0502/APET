// M1 D1 脚手架 → M1 D4 IPC 框架（#4）→ M1 D5 数据层（#5）→ 2026-05-06 code-review 重构。
// 后续按 milestone 节奏接入：tray / shortcuts / cursor_tracker / on_window_event 等。

mod commands;
mod services;

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
