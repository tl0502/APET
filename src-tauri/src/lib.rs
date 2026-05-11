// M1 D1 脚手架 → M1 D4 IPC 框架（#4）→ M1 D5 数据层（#5）→ 2026-05-06 code-review 重构
// → #6 系统托盘 + 关闭语义（M1 W2 主态可达交付物）。
// 后续按 milestone 节奏接入：shortcuts / cursor_tracker / window:Moved emit 等。

mod commands;
mod services;

use services::shortcuts::ShortcutRegistry;
use services::window_actions::{CHAT_WINDOW_LABEL, PET_WINDOW_LABEL, SETTINGS_WINDOW_LABEL};
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
        description:
            "init schema v1 per architecture v1.1 §4 (ADR-015 三形态共享 ConversationStore)",
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
            // 用户增补 LLM Providers：测试连通用的活跃 CancellationToken 槽（llm_test_provider
            // ↔ 同时仅 1 个；新调用抢占旧的）。原 #12 单 namespace setup 已退役。
            crate::commands::llm_providers::setup(app.handle());
            // #13 ChatService 业务编排（chat_send / chat_cancel / chat_history）
            app.manage(crate::services::chat::service::ChatService::new());
            // #6 启动期 GC：清理上次进程退出时 detached spawn 没收尾留下的孤儿 assistant
            // placeholder（content='' 在 chat_history 视图里渲染为空气泡）。cutoff = 启动
            // 时间快照；新进程启动后 prepare 写的 placeholder created_at >= cutoff 不会被
            // 误删。失败仅 eprintln 不阻断启动（GC 失败用户最多看到一个空气泡，不致命）。
            {
                let app_handle = app.handle().clone();
                let cutoff = chrono::Utc::now().to_rfc3339();
                tauri::async_runtime::block_on(async move {
                    match crate::services::memory::cleanup_orphan_assistant_placeholders(
                        &app_handle,
                        &cutoff,
                    )
                    .await
                    {
                        Ok(0) => {}
                        Ok(n) => eprintln!("[setup] cleaned {n} orphan assistant placeholder(s)"),
                        Err(e) => eprintln!("[setup] cleanup orphan placeholders failed: {e}"),
                    }
                });
            }
            // 用户增补：多 provider migration（旧 #12 单 namespace 三键 → 多 provider 默认条目 + active）
            // 失败仅 eprintln 不阻断启动；用户可在设置面板手动添加 provider。
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::block_on(async move {
                    match crate::services::llm_providers::migrate_legacy_if_needed(&app_handle)
                        .await
                    {
                        Ok(true) => {
                            eprintln!("[llm_providers] migrated legacy llm:openai:* into default provider");
                        }
                        Ok(false) => {}
                        Err(e) => {
                            eprintln!("[llm_providers] migrate_legacy_if_needed failed: {e}");
                        }
                    }
                });
            }
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
                    if label == PET_WINDOW_LABEL
                        || label == SETTINGS_WINDOW_LABEL
                        || label == CHAT_WINDOW_LABEL
                    {
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
            commands::persona::persona_get_active,
            // #5 nickname（2026-05-09：删 pet 系列，加 announce 开关）
            commands::nickname::nickname_get_user,
            commands::nickname::nickname_set_user,
            commands::nickname::nickname_get_announce_user_change,
            commands::nickname::nickname_set_announce_user_change,
            // #5 KV 偏好（services::preferences；IPC 名沿用 memory_*，与 schema 列名 `memory` 一致）
            commands::memory::memory_get,
            commands::memory::memory_set,
            commands::memory::memory_list,
            commands::memory::memory_delete,
            // #9 window 控制（settings show/hide）+ #10 pet 位置 get/save + #14 chat show/hide/toggle
            commands::window::settings_show,
            commands::window::settings_hide,
            commands::window::get_pet_position,
            commands::window::save_pet_position,
            commands::window::chat_show,
            commands::window::chat_hide,
            commands::window::chat_toggle,
            // #11 shortcuts（probe + set chat）
            commands::shortcuts::probe_global_shortcut,
            commands::shortcuts::set_shortcut_chat,
            // 用户增补 LLM Providers（多 provider 实例 CRUD + activate + test，参考 cc-switch UI）
            commands::llm_providers::llm_list_providers,
            commands::llm_providers::llm_get_provider,
            commands::llm_providers::llm_add_provider,
            commands::llm_providers::llm_update_provider,
            commands::llm_providers::llm_delete_provider,
            commands::llm_providers::llm_activate_provider,
            commands::llm_providers::llm_test_provider,
            commands::llm_providers::llm_probe_models,
            // #13 ChatService 业务编排（流式对话 + 取消 + 历史）+ 多会话切换
            commands::chat::chat_send,
            commands::chat::chat_cancel,
            commands::chat::chat_history,
            commands::chat::chat_list_conversations,
            commands::chat::chat_create_conversation,
            commands::chat::chat_set_active_conversation,
            commands::chat::chat_rename_conversation,
            commands::chat::chat_archive_conversation,
            commands::chat::chat_delete_conversation,
            // V3 多对话并发草稿持久化（config 表 KV chat:draft:<convId>）
            commands::chat::chat_get_draft,
            commands::chat::chat_set_draft,
            commands::chat::chat_delete_draft,
            // #16 Consent（灵魂宣誓 IPC 管道；前端视图 #16b）
            commands::consent::consent_get,
            commands::consent::consent_grant,
            commands::consent::consent_check_version,
            commands::consent::consent_get_current_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
