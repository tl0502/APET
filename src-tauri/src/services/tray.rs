// 系统托盘服务（#6 + #9 设置入口）
//
// 行为：
// - 启动时注册到 Windows 托盘（任务栏 ^ 展开后可见，用户可拖出常驻）
// - 左键单击/双击托盘图标 → 无操作（用户决策：仅菜单可操作，避免误触）
// - 右键 → 弹菜单：显示/隐藏（动态文案）/ 设置（#9 激活，唤起 settings 窗口）/ 退出
// - 主窗 + settings 窗 CloseRequested 拦截在 lib.rs，改 hide；唯一退出路径 = 托盘"退出"
//
// 设计要点：
// - icon 复用 app.default_window_icon()（tauri.conf.json 已引用作 default window icon，0 新增资源）
// - 「显示/隐藏」1 项动态文案：MenuItem clone 多份传 closure，在 menu 点击 / tray hover 时 set_text
// - tray hover (Enter) 时刷新文案：兜底外部路径（Alt+F4 hide）造成的状态错位；
//   不在 on_window_event 里刷新（避免跨函数传 menu item handle）

use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::services::window_actions::{self, PET_WINDOW_LABEL};
use crate::services::window_state;

const MENU_ID_SHOW_HIDE: &str = "tray:show-hide";
const MENU_ID_SETTINGS: &str = "tray:settings";
const MENU_ID_TASKS: &str = "tray:tasks";
const MENU_ID_POMODORO: &str = "tray:pomodoro";
/// #31 follow-up：alwaysOnTop 全局开关菜单项
const MENU_ID_AOT: &str = "tray:always-on-top";
const MENU_ID_QUIT: &str = "tray:quit";

const TOOLTIP: &str = "AI 桌宠";
const LABEL_SHOW: &str = "显示桌宠";
const LABEL_HIDE: &str = "隐藏桌宠";
/// AOT label：用 set_text 模式带勾标（同 show_hide 同款；避免 Tauri 2 CheckMenuItem v2 实战示例少的风险）
const LABEL_AOT_ON: &str = "✓ 顶层显示";
const LABEL_AOT_OFF: &str = "  顶层显示";

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

fn current_aot_label(app: &AppHandle) -> &'static str {
    let on = tauri::async_runtime::block_on(window_state::load_always_on_top(app))
        .unwrap_or(true);
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

fn refresh_aot_label(app: &AppHandle, item: &MenuItem<Wry>) {
    if let Err(e) = item.set_text(current_aot_label(app)) {
        eprintln!("[tray] failed to refresh always-on-top label: {e}");
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
    // #31 follow-up：alwaysOnTop 全局开关，label 带"✓"前缀指示当前状态。
    let aot_item = MenuItemBuilder::with_id(MENU_ID_AOT, current_aot_label(app)).build(app)?;
    let quit_item = MenuItemBuilder::with_id(MENU_ID_QUIT, "退出").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_hide_item)
        .separator()
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

    // clone 给两个 closure：on_menu_event（菜单点击后刷新）+ on_tray_icon_event（hover 后刷新）
    let show_hide_for_menu = show_hide_item.clone();
    let show_hide_for_tray = show_hide_item.clone();
    // AOT 同款双 clone：点击后刷自己 + tray Enter 时刷（兜底外部路径修改 KV）
    let aot_for_menu = aot_item.clone();
    let aot_for_tray = aot_item.clone();

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
            MENU_ID_SETTINGS => window_actions::show_settings(app),
            MENU_ID_AOT => {
                // 同步 block_on：托盘点击在 main thread，KV 读写 + set_always_on_top 都很快
                let app_for_async = app.clone();
                match tauri::async_runtime::block_on(window_state::toggle_always_on_top(
                    &app_for_async,
                )) {
                    Ok(_) => refresh_aot_label(app, &aot_for_menu),
                    Err(e) => eprintln!("[tray] toggle_always_on_top failed: {e}"),
                }
            }
            MENU_ID_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(move |tray, event| {
            // hover 进图标时刷新两个动态文案，兜底外部路径（Alt+F4 hide / 直改 KV）造成的状态错位
            if let TrayIconEvent::Enter { .. } = event {
                let app = tray.app_handle();
                refresh_label(app, &show_hide_for_tray);
                refresh_aot_label(app, &aot_for_tray);
            }
        })
        .build(app)?;

    Ok(())
}
