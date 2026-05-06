// 系统级 commands：健康检查、版本、能力查询等。
// ping/pong 用于 IPC 通道连通性验证（前端 src/services/ipc.ts）。

#[tauri::command]
pub async fn ping() -> String {
    "pong".to_string()
}
