// #16 Consent IPC commands（M.1 灵魂宣誓后端管道）
//
// 4 个 commands：
// - consent_get(): 读单行 consent 当前状态（dev 验证 + #17 启动期路由用）
// - consent_grant(method, version): 写 granted=1 + method + version=CURRENT + now
//     IPC 层硬卡 method == 'soul_pledge' && version == CURRENT_CONSENT_VERSION：
//     - method 防 dev console 调 invoke('consent_grant', {method:'classic'}) 绕过 ADR-008
//       唯一灵魂宣誓路径
//     - version 防前端硬编码 1 在 v2 上线时 stale 写入历史污染 DB
//     校验通过后调 service 层 grant_consent(&method)，version 由 service 用常量写入
// - consent_check_version(): 比对 DB 版本与 CURRENT_CONSENT_VERSION，返
//     Match | NeedReconsent | NotGranted；#17 状态机据此路由
// - consent_get_current_version(): 返 CURRENT_CONSENT_VERSION 常量给前端做"双方一致校验"
//
// 前端 src/services/consent.ts 提供 binding；前端视图 SoulPledgeView 留 #16b。
//
// dev console 验证（任一 DevTools）：
//   await window.__TAURI__.core.invoke('consent_get')
//   await window.__TAURI__.core.invoke('consent_check_version')
//   const v = await window.__TAURI__.core.invoke('consent_get_current_version')
//   await window.__TAURI__.core.invoke('consent_grant', { method: 'soul_pledge', version: v })

use crate::services::consent::{
    self, ConsentRecord, ConsentStatus, CURRENT_CONSENT_VERSION, METHOD_SOUL_PLEDGE,
};
use tauri::AppHandle;

#[tauri::command]
pub async fn consent_get(app: AppHandle) -> Result<ConsentRecord, String> {
    consent::get_consent(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn consent_grant(app: AppHandle, method: String, version: i64) -> Result<(), String> {
    if method != METHOD_SOUL_PLEDGE {
        return Err(format!(
            "invalid method: '{method}'（ADR-008 唯一同意路径是 soul_pledge；'classic' 仅用于 schema seed 不接受 grant）"
        ));
    }
    if version != CURRENT_CONSENT_VERSION {
        return Err(format!(
            "version mismatch: 前端传入 {version}，后端当前版本 {CURRENT_CONSENT_VERSION}（请前端用 consent_get_current_version 校验后再 grant）"
        ));
    }
    consent::grant_consent(&app, &method)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn consent_check_version(app: AppHandle) -> Result<ConsentStatus, String> {
    consent::check_version(&app)
        .await
        .map_err(|e| e.to_string())
}

/// 前端在 grant 之前先调，确保 version 与后端常量同步（M3 v2 上线时前端无需改字面量）。
#[tauri::command]
pub fn consent_get_current_version() -> i64 {
    CURRENT_CONSENT_VERSION
}
