//! Todo IPC commands（#29）— 6 命令。
//!
//! 命名遵循 Tauri 2.x runtime 规范：snake_case [a-zA-Z0-9_]（架构 §566 + 已有
//! reminder_create / chat_send 等同风）。架构 §604 文档逻辑写 `todo.create` 仅作分组语义，
//! 注册名是 `todo_create`。

use tauri::AppHandle;

use crate::services::todo::{self, CreateInput, Todo, UpdateInput};

#[tauri::command]
pub async fn todo_create(app: AppHandle, input: CreateInput) -> Result<Todo, String> {
    todo::create(&app, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn todo_list(app: AppHandle) -> Result<Vec<Todo>, String> {
    todo::list(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn todo_update(
    app: AppHandle,
    id: String,
    input: UpdateInput,
) -> Result<Todo, String> {
    todo::update(&app, id, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn todo_complete(app: AppHandle, id: String) -> Result<Todo, String> {
    todo::complete(&app, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn todo_breakdown(app: AppHandle, id: String) -> Result<Vec<String>, String> {
    todo::breakdown(&app, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn todo_reorder(
    app: AppHandle,
    id: String,
    after_id: Option<String>,
) -> Result<Todo, String> {
    todo::reorder(&app, id, after_id).await.map_err(|e| e.to_string())
}
