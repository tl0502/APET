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

const MENU_ID_SHOW_HIDE: &str = "tray:show-hide";
const MENU_ID_SETTINGS: &str = "tray:settings";
const MENU_ID_QUIT: &str = "tray:quit";

const TOOLTIP: &str = "AI 桌宠";
const LABEL_SHOW: &str = "显示桌宠";
const LABEL_HIDE: &str = "隐藏桌宠";

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
    let quit_item = MenuItemBuilder::with_id(MENU_ID_QUIT, "退出").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_hide_item)
        .separator()
        .item(&settings_item)
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
            MENU_ID_SETTINGS => window_actions::show_settings(app),
            MENU_ID_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(move |tray, event| {
            // hover 进图标时刷新文案，兜底外部路径（Alt+F4 hide）造成的状态错位
            if let TrayIconEvent::Enter { .. } = event {
                refresh_label(tray.app_handle(), &show_hide_for_tray);
            }
        })
        .build(app)?;

    Ok(())
}
