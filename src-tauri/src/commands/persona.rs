// #5 PersonaService IPC commands（H.1 持久化层入口）
//
// 2026-05-06 code-review #3+#5+#6 重写：
// - 业务逻辑下沉到 services/persona.rs（load_persona / activate_persona + with_conn 测试覆盖）
// - 此处只做 thin wrapper：输入校验 → 调 service → 错误转字符串 → emit event
// - 错误类型映射 thiserror enum → 前端拿到的 IpcError.message 含语义前缀

use crate::services::persona::{
    activate_persona, activate_snapshot, delete_persona, export_persona_snapshot_to_path,
    get_snapshot_profile, import_persona_from_path, list_personas, load_active_persona,
    load_persona, save_draft, validate_draft, PersonaDraftValidationResult, PersonaError,
    PersonaExportResult, PersonaImportResult, PersonaListItem, PersonaLookupError,
    PersonaSaveResult, PersonaSourceDraft, PersonaSummary, SoulRuntimeProfile,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

const PERSONA_ACTIVATED_EVENT: &str = "persona:activated";
const PERSONA_SNAPSHOT_ACTIVATED_EVENT: &str = "persona:snapshot-activated";
const PERSONA_ID_MAX_LEN: usize = 64;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersonaSnapshotActivatedPayload {
    persona_id: String,
    snapshot_id: String,
}

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

fn validate_file_path(path: &str) -> Result<&str, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    Ok(trimmed)
}

#[tauri::command]
pub async fn persona_load(app: AppHandle, id: String) -> Result<PersonaSummary, String> {
    let id = validate_persona_id(&id)?;
    load_persona(&app, id).await.map_err(lookup_err_to_string)
}

/// 列出所有人格 summary（不含 raw_markdown）。onboarding Step 2 / 设置面板列表用。
///
/// 排序由后端保证（active 优先 + id ASC）；前端直接渲染即可，不需要再 sort。
#[tauri::command]
pub async fn persona_list(app: AppHandle) -> Result<Vec<PersonaListItem>, String> {
    list_personas(&app)
        .await
        .map_err(|e: PersonaError| e.to_string())
}

/// 读当前激活人格 summary（含 raw_markdown）。
///
/// #14 ChatPanel 拿来显示 header 标题（active persona name），无需先知道 id。
/// 后端 `load_active_persona` helper 在 #13 已落地（services/persona.rs:186）。
#[tauri::command]
pub async fn persona_get_active(app: AppHandle) -> Result<PersonaSummary, String> {
    load_active_persona(&app)
        .await
        .map_err(lookup_err_to_string)
}

#[tauri::command]
pub async fn persona_activate(app: AppHandle, id: String) -> Result<(), String> {
    let id = validate_persona_id(&id)?.to_string();
    activate_persona(&app, &id)
        .await
        .map_err(lookup_err_to_string)?;
    // 与 nickname:changed 同款契约：跨窗口（M3 设置面板）切人格后角色窗能 listen 到刷新。
    // emit 失败仅 eprintln 不向上抛：DB 写入已成功（active 已切换），把 emit 故障暴露给前端
    // 会让用户误以为 activate 没生效并触发 retry；retry 在 DB 端是 idempotent，但 UI 上会闪
    // 误报 toast，体验更糟。emit 故障本身极罕见（webview event 通道挂掉），降级为日志即可。
    let active = load_active_persona(&app)
        .await
        .map_err(lookup_err_to_string)?;
    emit_persona_activation_events(&app, &id, &active.snapshot_id);
    Ok(())
}

#[tauri::command]
pub async fn persona_validate_draft(
    draft: PersonaSourceDraft,
) -> Result<PersonaDraftValidationResult, String> {
    Ok(validate_draft(&draft))
}

#[tauri::command]
pub async fn persona_save_draft(
    app: AppHandle,
    draft: PersonaSourceDraft,
) -> Result<PersonaSaveResult, String> {
    save_draft(&app, draft, false)
        .await
        .map_err(|e: PersonaError| e.to_string())
}

#[tauri::command]
pub async fn persona_save_and_activate_draft(
    app: AppHandle,
    draft: PersonaSourceDraft,
) -> Result<PersonaSaveResult, String> {
    let result = save_draft(&app, draft, true)
        .await
        .map_err(|e: PersonaError| e.to_string())?;
    emit_persona_activation_events(&app, &result.persona_id, &result.snapshot_id);
    Ok(result)
}

#[tauri::command]
pub async fn persona_import(
    app: AppHandle,
    path: String,
    activate: bool,
) -> Result<PersonaImportResult, String> {
    let path = validate_file_path(&path)?.to_string();
    let result = import_persona_from_path(&app, path, activate)
        .await
        .map_err(|e: PersonaError| e.to_string())?;
    if result.activated {
        emit_persona_activation_events(&app, &result.persona_id, &result.snapshot_id);
    }
    Ok(result)
}

#[tauri::command]
pub async fn persona_export_snapshot(
    app: AppHandle,
    #[allow(non_snake_case)] snapshotId: i64,
    path: String,
) -> Result<PersonaExportResult, String> {
    if snapshotId <= 0 {
        return Err("snapshot id 必须为正数".to_string());
    }
    let path = validate_file_path(&path)?.to_string();
    export_persona_snapshot_to_path(&app, snapshotId, path)
        .await
        .map_err(|e: PersonaError| e.to_string())
}

#[tauri::command]
pub async fn persona_delete(app: AppHandle, id: String) -> Result<(), String> {
    let id = validate_persona_id(&id)?.to_string();
    delete_persona(&app, &id)
        .await
        .map_err(|e: PersonaError| e.to_string())
}

#[tauri::command]
pub async fn persona_activate_snapshot(
    app: AppHandle,
    #[allow(non_snake_case)] snapshotId: i64,
) -> Result<(), String> {
    activate_snapshot(&app, snapshotId)
        .await
        .map_err(|e: PersonaError| e.to_string())?;
    let active = load_active_persona(&app)
        .await
        .map_err(lookup_err_to_string)?;
    emit_persona_activation_events(&app, &active.id, &active.snapshot_id);
    Ok(())
}

#[tauri::command]
pub async fn persona_get_snapshot_profile(
    app: AppHandle,
    #[allow(non_snake_case)] snapshotId: i64,
) -> Result<SoulRuntimeProfile, String> {
    get_snapshot_profile(&app, snapshotId)
        .await
        .map_err(|e: PersonaError| e.to_string())
}

fn emit_persona_activation_events(app: &AppHandle, persona_id: &str, snapshot_id: &str) {
    if let Err(e) = app.emit(PERSONA_ACTIVATED_EVENT, persona_id) {
        eprintln!("[persona] emit persona:activated failed: {e}");
    }
    let payload = PersonaSnapshotActivatedPayload {
        persona_id: persona_id.to_string(),
        snapshot_id: snapshot_id.to_string(),
    };
    if let Err(e) = app.emit(PERSONA_SNAPSHOT_ACTIVATED_EVENT, payload) {
        eprintln!("[persona] emit persona:snapshot-activated failed: {e}");
    }
}
