// M1 D1 脚手架 → M1 D4 IPC 框架（#4）→ M1 D5 数据层（#5）→ 2026-05-06 code-review 重构
// → #6 系统托盘 + 关闭语义（M1 W2 主态可达交付物）。
// 后续按 milestone 节奏接入：shortcuts / cursor_tracker / window:Moved emit 等。

mod commands;
mod services;

use services::shortcuts::ShortcutRegistry;
use services::window_actions::{
    CHAT_WINDOW_LABEL, ONBOARDING_WINDOW_LABEL, PET_WINDOW_LABEL, SETTINGS_WINDOW_LABEL,
};
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
        // #25 文件选择对话框（用户头像上传：前端 open() 拿本地 PNG/JPG 路径）。
        .plugin(tauri_plugin_dialog::init())
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
            // #16 启动期 consent 路由：seed_builtin 后 DB 已可用 → 查 consent_check_version。
            // - Match + KV `onboarding:current_step` 不存在 → 不动（pet visible, onboarding hidden）
            // - Match + KV 存在 → onboarding 续接（ADR-019）：用户走完 Step 1 grant 后中途关窗,
            //   下次启动 consent=Match 但 onboarding 未完成 → 仍开 onboarding 让用户继续/重来/退出。
            //   修一个 latent bug：原逻辑 Match → 直接进 pet 主态，用户永远走不完 Step 2-6。
            // - Match + KV 读失败 → 保守路径同"KV 存在":hide pet + show onboarding。
            //   db 暂时故障下,误进 pet 主态使用未完成状态比"短暂多走一次 onboarding"风险更大。
            // - NotGranted / NeedReconsent → 隐藏 pet 窗 + 显示 onboarding 窗
            //   **先 clear 续接 KV**:NeedReconsent 必须重新同意 v2,不允许用旧 KV 跳过 Step 1
            //   绕过新版本条款（合规);NotGranted 下 KV 通常不存在,clear 是 no-op。
            // - consent check Err 分支：原逻辑只 eprintln 后任由 pet 默认 visible —— 实测下
            //   所谓的"再次被引导"逻辑根本不存在，所有主体功能开放给未同意用户。
            //   #21 锁死边界：改为按"最保守"等同 NotGranted —— hide pet + show onboarding，
            //   并初始化 ConsentGate=false。让用户走一次 onboarding，比"放任 pet 默认可见"安全。
            //
            // #21 锁死边界 ConsentGate：block_on 返回 gate_open，紧接着同步 manage(ConsentGate)。
            // 初值与分支判定结果一致（Match+KV None → true；其余 → false）。后续所有
            // `show_*` / `toggle_*` / 全局快捷键 handler / LivingPet scheduler 都过此门。
            let app_handle = app.handle().clone();
            let gate_open = tauri::async_runtime::block_on(async move {
                match crate::services::consent::check_version(&app_handle).await {
                    Ok(crate::services::consent::ConsentStatus::Match) => {
                        // ADR-019 续接判定
                        match crate::services::onboarding::load_current_step(&app_handle).await {
                            Ok(Some(_)) => {
                                if let Some(pet) = app_handle.get_webview_window(PET_WINDOW_LABEL)
                                {
                                    let _ = pet.hide();
                                }
                                crate::services::window_actions::show_onboarding(&app_handle);
                                false
                            }
                            Ok(None) => true, // 已完成 → gate=open
                            Err(e) => {
                                // 保守:db 故障下视同"KV 存在",show onboarding
                                eprintln!(
                                    "[setup] load_current_step failed, conservative show onboarding: {e}"
                                );
                                if let Some(pet) = app_handle.get_webview_window(PET_WINDOW_LABEL)
                                {
                                    let _ = pet.hide();
                                }
                                crate::services::window_actions::show_onboarding(&app_handle);
                                false
                            }
                        }
                    }
                    Ok(_) => {
                        // NotGranted | NeedReconsent：先 clear KV 防"绕过重新同意"。
                        // NeedReconsent 场景下若旧 KV 残留,前端 onMounted 会弹续接模态,
                        // 用户点"继续"就跳过 SoulPledgeView,导致用 v2 client 但 consent 仍是 v1。
                        if let Err(e) =
                            crate::services::onboarding::clear_current_step(&app_handle).await
                        {
                            eprintln!(
                                "[setup] clear_current_step on reconsent/not-granted failed (non-fatal): {e}"
                            );
                        }
                        if let Some(pet) = app_handle.get_webview_window(PET_WINDOW_LABEL) {
                            let _ = pet.hide();
                        }
                        crate::services::window_actions::show_onboarding(&app_handle);
                        false
                    }
                    Err(e) => {
                        // #21 锁死边界：原逻辑只 eprintln 后让 pet 默认 visible —— 等同绕过
                        // consent 把所有主体功能开放给未同意用户。改为同 NotGranted 路径。
                        eprintln!(
                            "[setup] consent check_version failed, conservative show onboarding: {e}"
                        );
                        if let Some(pet) = app_handle.get_webview_window(PET_WINDOW_LABEL) {
                            let _ = pet.hide();
                        }
                        crate::services::window_actions::show_onboarding(&app_handle);
                        false
                    }
                }
            });
            // 必须在 register_chat_on_startup / LivingPet scheduler / 任何 window_actions
            // 调用之前 manage —— 这些路径会 try_state::<ConsentGate>()，state 未 manage 时
            // 保守返 false（行为正确但不必要地走"未完成"分支）。
            app.manage(crate::services::consent_gate::ConsentGate::new(gate_open));
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
            // #21 M1 收尾：LivingPet 调度器（5-15min 抖动 → 25% wander）。
            // 状态机容器先 manage 让 scheduler task 能 app.state::<LivingPet>()。
            // dev 期实测设 env LIVING_PET_DEV_INTERVAL=5（秒）可强制 5s 间隔。
            app.manage(crate::services::living_pet::LivingPet::default());
            crate::services::living_pet::start_scheduler(app.handle().clone());
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
                    if label == ONBOARDING_WINDOW_LABEL {
                        // #16 onboarding 关闭语义：Alt+F4 / 系统关闭 / 前端"退出"按钮 都等同
                        // app.exit(0)（不写 consent；下次启动重弹）。与 pet/settings/chat 的
                        // "关 = hide" 反向：onboarding 是 ADR-008 强制路径，用户没同意完就关 →
                        // 应用应整体退出，不应留在后台"半同意"状态。
                        // prevent_close 先拦住默认走 hide 的路径，再统一 exit。
                        api.prevent_close();
                        window.app_handle().exit(0);
                    } else if label == PET_WINDOW_LABEL
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
            commands::persona::persona_list,
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
            // #16 灵魂宣誓"我懂了"切窗 IPC（hide onboarding + show pet + emit step-done）
            commands::window::onboarding_complete,
            // #21 ADR-019 Onboarding 进度持久化（current_step KV + 续接 / 退出）。
            // 「重来」改写 KV='soul-pledge' 而非 clear（见 commands/onboarding.rs 注释），
            // 故无需 onboarding_reset IPC。
            commands::onboarding::onboarding_save_step,
            commands::onboarding::onboarding_load_step,
            // #11 shortcuts（probe + set chat + get chat + 启动期失败留痕查询）
            commands::shortcuts::probe_global_shortcut,
            commands::shortcuts::set_shortcut_chat,
            commands::shortcuts::get_chat_shortcut,
            commands::shortcuts::get_chat_register_status,
            // #21 收尾 #2 L1：用户拖动 / 唤起 chat 时取消进行中的 wander tween
            commands::living_pet::living_pet_cancel_wander,
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
            // #25/#26 头像（user 上传 + persona VRM 导出）—— 落盘 <app_config>/avatars/
            commands::avatars::user_avatar_set,
            commands::avatars::user_avatar_clear,
            commands::avatars::avatar_read_to_data_url,
            commands::avatars::user_avatar_save_data_url,
            commands::avatars::persona_avatar_save,
            commands::avatars::persona_avatar_clear,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
