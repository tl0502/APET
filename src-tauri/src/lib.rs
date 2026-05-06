// M1-D1 脚手架：仅最小骨架。
// 后续按 milestone 节奏接入：plugin_sql / cursor_tracker / tray / shortcuts / on_window_event。

mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("\n=== APP PANIC ===\n{info}\n=================");
    }));

    tauri::Builder::default()
        .setup(|_app| {
            eprintln!("[setup] reached");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::system::ping])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
