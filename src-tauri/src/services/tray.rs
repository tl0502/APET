// 系统托盘服务（#6 + #9 设置入口 + #35 ADR-021 P1 workspace 入口）
//
// 行为：
// - 启动时注册到 Windows 托盘（任务栏 ^ 展开后可见，用户可拖出常驻）
// - 左键单击 → 无操作；左键双击 → toggle workspace（#35 Phase E，与 VSCode-style "双击图标主面板"惯例对齐）
// - 右键 → 弹菜单：显示/隐藏 / 工作台 / 番茄 / 任务 / 设置 / AOT / 退出
// - 主窗 + settings/chat/tasks/pomodoro/workspace CloseRequested 拦截在 lib.rs，改 hide
// - 唯一退出路径 = 托盘"退出"
//
// 设计要点：
// - icon 复用 app.default_window_icon()（tauri.conf.json 已引用作 default window icon，0 新增资源）
// - 「显示/隐藏」1 项动态文案：MenuItem clone 多份传 closure，在 menu 点击 / tray hover 时 set_text
// - tray hover (Enter) 时刷新 show/hide 文案：兜底外部路径（Alt+F4 hide）造成的状态错位；
// - AOT label 走事件驱动（R3 修复 2026-05-19）：apply 时 emit window:always-on-top:changed，
//   tray setup 时 listen 该事件主动 set_text，避免 hover 时 block_on KV 读（频繁 hover 时
//   阻塞 tray 事件线程；潜在死锁风险）。

use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Listener, Manager, Wry};

use crate::services::window_actions::{self, PET_WINDOW_LABEL};
use crate::services::window_state;

const MENU_ID_SHOW_HIDE: &str = "tray:show-hide";
const MENU_ID_SETTINGS: &str = "tray:settings";
const MENU_ID_TASKS: &str = "tray:tasks";
const MENU_ID_POMODORO: &str = "tray:pomodoro";
/// #35 ADR-021 P1 workspace 主窗入口（与 chat 同款"开关式"）。
const MENU_ID_WORKSPACE: &str = "tray:workspace";
/// #31 follow-up：alwaysOnTop 全局开关菜单项
const MENU_ID_AOT: &str = "tray:always-on-top";
const MENU_ID_QUIT: &str = "tray:quit";

const TOOLTIP: &str = "AI 桌宠";
const LABEL_SHOW: &str = "显示桌宠";
const LABEL_HIDE: &str = "隐藏桌宠";
/// AOT label：用 set_text 模式带勾标（同 show_hide 同款；避免 Tauri 2 CheckMenuItem v2 实战示例少的风险）
const LABEL_AOT_ON: &str = "✓ 置于顶层";
const LABEL_AOT_OFF: &str = "  置于顶层";

/// AOT 切换跨窗口广播事件名（与 services/window_state.rs ALWAYS_ON_TOP_CHANGED_EVENT 同源）。
/// R3 修复：tray 监听此事件被动刷新 menu 文案，不在 hover 时 block_on KV。
const ALWAYS_ON_TOP_CHANGED_EVENT: &str = "window:always-on-top:changed";

fn current_label(app: &AppHandle) -> &'static str {
    let visible = app
        .get_webview_window(PET_WINDOW_LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if visible {
        LABEL_HIDE
    } else {
        LABEL_SHOW
    }
}

fn aot_label_for(on: bool) -> &'static str {
    if on {
        LABEL_AOT_ON
    } else {
        LABEL_AOT_OFF
    }
}

fn refresh_label(app: &AppHandle, item: &MenuItem<Wry>) {
    if let Err(e) = item.set_text(current_label(app)) {
        eprintln!("[tray] failed to refresh show/hide label: {e}");
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let show_hide_item =
        MenuItemBuilder::with_id(MENU_ID_SHOW_HIDE, current_label(app)).build(app)?;
    // #9 设置面板上线 → 激活菜单项；点击走 window_actions::show_settings（show + set_focus）。
    let settings_item = MenuItemBuilder::with_id(MENU_ID_SETTINGS, "设置...").build(app)?;
    // #22 任务三件套：托盘"任务..."入口，点击唤起独立 tasks 窗。
    let tasks_item = MenuItemBuilder::with_id(MENU_ID_TASKS, "任务...").build(app)?;
    // #28 follow-up 番茄独立窗：托盘"番茄..."入口，与 tasks tab 按钮 / pomodoro_start 自动 show 三入口并列。
    let pomodoro_item = MenuItemBuilder::with_id(MENU_ID_POMODORO, "番茄...").build(app)?;
    // #35 ADR-021 P1 workspace 入口；放 pomodoro 之前（与 plan 一致：workspace > 番茄 > 任务 > 设置 重要性序）
    let workspace_item = MenuItemBuilder::with_id(MENU_ID_WORKSPACE, "工作台...").build(app)?;
    // #31 follow-up：alwaysOnTop 全局开关，label 带"✓"前缀指示当前状态。
    // R3 修复：setup 初值仍 block_on KV 一次（仅启动期单次，无热路径影响），之后由 listen 驱动。
    let initial_aot = tauri::async_runtime::block_on(window_state::load_always_on_top(app))
        .unwrap_or(true);
    let aot_item =
        MenuItemBuilder::with_id(MENU_ID_AOT, aot_label_for(initial_aot)).build(app)?;
    let quit_item = MenuItemBuilder::with_id(MENU_ID_QUIT, "退出").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_hide_item)
        .separator()
        .item(&workspace_item)
        .item(&pomodoro_item)
        .item(&tasks_item)
        .item(&settings_item)
        .separator()
        .item(&aot_item)
        .separator()
        .item(&quit_item)
        .build()?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;

    // clone 给 closure 用
    let show_hide_for_menu = show_hide_item.clone();
    let show_hide_for_tray = show_hide_item.clone();
    let aot_for_menu = aot_item.clone();
    let aot_for_listener = aot_item.clone();

    // R3 修复：listen AOT 切换事件被动刷新 menu 文案。
    // toggle_always_on_top（托盘点击 / 未来 settings UI 改 KV）都会 emit 此事件，
    // listener 收到后直接 set_text(label_for(payload))。hover 不再需要 block_on KV 读。
    app.listen(ALWAYS_ON_TOP_CHANGED_EVENT, move |event| {
        // payload 形如 "true" / "false"
        let on = event.payload() == "true";
        if let Err(e) = aot_for_listener.set_text(aot_label_for(on)) {
            eprintln!("[tray] AOT listener set_text failed: {e}");
        }
    });

    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip(TOOLTIP)
        .menu(&menu)
        .show_menu_on_left_click(false) // 左键无操作；菜单只走右键
        .on_menu_event(move |app, event| match event.id().as_ref() {
            MENU_ID_SHOW_HIDE => {
                window_actions::toggle_pet(app);
                refresh_label(app, &show_hide_for_menu);
            }
            MENU_ID_TASKS => window_actions::show_tasks(app),
            MENU_ID_POMODORO => window_actions::show_pomodoro(app),
            MENU_ID_WORKSPACE => window_actions::show_workspace(app),
            MENU_ID_SETTINGS => window_actions::show_settings(app),
            MENU_ID_AOT => {
                // 同步 block_on：托盘点击在 main thread，KV 读写 + set_always_on_top 都很快。
                // toggle 内部 emit ALWAYS_ON_TOP_CHANGED_EVENT，上面的 listener 自动刷 aot_for_menu。
                // 但 listener 是异步触达，立即 hover 看到旧文案，所以这里仍主动 set 一次。
                let app_for_async = app.clone();
                match tauri::async_runtime::block_on(window_state::toggle_always_on_top(
                    &app_for_async,
                )) {
                    Ok(next) => {
                        if let Err(e) = aot_for_menu.set_text(aot_label_for(next)) {
                            eprintln!("[tray] toggle aot set_text failed: {e}");
                        }
                    }
                    Err(e) => eprintln!("[tray] toggle_always_on_top failed: {e}"),
                }
            }
            MENU_ID_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(move |tray, event| {
            // hover 仅刷 show/hide（pet 可见状态 OS 即时查），AOT 文案由上面的 listener 维护，
            // 不在 hover 路径 block_on KV（避免频繁 hover 阻塞 tray 事件线程）。
            match event {
                TrayIconEvent::Enter { .. } => {
                    let app = tray.app_handle();
                    refresh_label(app, &show_hide_for_tray);
                }
                // review P1 修复（F-6.1）：左键双击 → **show**（与菜单"工作台..."一致），不 toggle。
                // 原设计是 toggle，但与菜单点击（show）语义不一致——用户用两路径会感知行为不同；
                // 且 toggle 会让"连击三次"第二次双击意外隐藏窗口。语义统一为"图标 = 打开 / 切到前台"，
                // 隐藏走 Ctrl+Alt+W 或 ✕ 关闭按钮。
                // 保持 show_menu_on_left_click(false)：单击仍无操作，仅双击触发，避免误触。
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => {
                    let app = tray.app_handle();
                    window_actions::show_workspace(app);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}
