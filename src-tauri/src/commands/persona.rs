// #5 PersonaService IPC commands（H.1 持久化层入口）
//
// 2026-05-06 code-review #3+#5+#6 重写：
// - 业务逻辑下沉到 services/persona.rs（load_persona / activate_persona + with_conn 测试覆盖）
// - 此处只做 thin wrapper：输入校验 → 调 service → 错误转字符串 → emit event
// - 错误类型映射 thiserror enum → 前端拿到的 IpcError.message 含语义前缀

use crate::services::persona::{
    activate_persona, load_persona, PersonaLookupError, PersonaSummary,
};
use tauri::{AppHandle, Emitter};

const PERSONA_ACTIVATED_EVENT: &str = "persona:activated";
const PERSONA_ID_MAX_LEN: usize = 64;

fn validate_persona_id(id: &str) -> Result<&str, String> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err("persona id 不能为空".to_string());
    }
    if trimmed.len() > PERSONA_ID_MAX_LEN {
        return Err(format!(
            "persona id 长度超限（≤{} 字符）",
            PERSONA_ID_MAX_LEN
        ));
    }
    Ok(trimmed)
}

fn lookup_err_to_string(e: PersonaLookupError) -> String {
    match e {
        PersonaLookupError::NotFound(id) => format!("persona not found: {id}"),
        PersonaLookupError::Db(inner) => inner.to_string(),
    }
}

#[tauri::command]
pub async fn persona_load(app: AppHandle, id: String) -> Result<PersonaSummary, String> {
    let id = validate_persona_id(&id)?;
    load_persona(&app, id).await.map_err(lookup_err_to_string)
}

#[tauri::command]
pub async fn persona_activate(app: AppHandle, id: String) -> Result<(), String> {
    let id = validate_persona_id(&id)?.to_string();
    activate_persona(&app, &id).await.map_err(lookup_err_to_string)?;
    // 与 nickname:changed 同款契约：跨窗口（M3 设置面板）切人格后角色窗能 listen 到刷新
    app.emit(PERSONA_ACTIVATED_EVENT, &id)
        .map_err(|e| format!("emit persona:activated failed: {e}"))?;
    Ok(())
}
