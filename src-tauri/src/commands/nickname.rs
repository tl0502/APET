// NicknameService IPC commands（M1 W1-W2，2026-05-09 重构后）
//
// 4 个 commands：
// - nickname_get_user / nickname_set_user：用户昵称读写（set_user 触发 system 转场注入）
// - nickname_get_announce_user_change / nickname_set_announce_user_change：
//     转场注入开关（config 表 KV `nickname:user_change_announce`，默认 ON）
//
// 已删除（2026-05-09）：
// - nickname_get_pet / nickname_set_pet / nickname_restore_pet（pet_nickname 机制移除，
//   宠物名字源唯一化为 .soul.md persona.name；ChatService 拼 prompt 时直接走 PersonaSummary.name）

use crate::services::config;
use crate::services::nickname;
use crate::services::nickname_announcement::{
    read_announce_user_change, CONFIG_KEY_ANNOUNCE_USER_CHANGE,
};
use tauri::AppHandle;

/// PRD §7.6.4 第 4 条 + 前端 NicknameForm.vue NICKNAME_MAX 对齐。后端是 IPC 权威边界
/// （前端只是 UX 早提示）；任何绕过前端的请求 — dev tools 直调 invoke / 未来 CLI / 集成 —
/// 都必须被这里收住，不让 17+ 字符或含 \n / \r 的昵称落库后混进 system 注入与 system prompt。
const NICKNAME_MIN_CHARS: usize = 1;
const NICKNAME_MAX_CHARS: usize = 16;

fn validate_nickname(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("昵称不能为空或仅含空白".to_string());
    }
    // 控制字符（U+0000 到 U+001F + U+007F DEL）— 与前端 CONTROL_CHAR_RE 对齐。
    // 落库后会污染 system 转场消息、system prompt、UI 显示。
    if trimmed
        .chars()
        .any(|c| (c as u32) < 0x20 || (c as u32) == 0x7F)
    {
        return Err("昵称不能含控制字符".to_string());
    }
    let chars = trimmed.chars().count();
    if chars < NICKNAME_MIN_CHARS || chars > NICKNAME_MAX_CHARS {
        return Err(format!(
            "昵称长度需 {NICKNAME_MIN_CHARS}-{NICKNAME_MAX_CHARS} 字符"
        ));
    }
    Ok(trimmed.to_string())
}

#[tauri::command]
pub async fn nickname_get_user(app: AppHandle) -> Result<Option<String>, String> {
    nickname::get_user_nickname(&app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn nickname_set_user(app: AppHandle, name: String) -> Result<(), String> {
    let name = validate_nickname(&name)?;
    nickname::set_user_nickname(&app, name)
        .await
        .map_err(|e| e.to_string())
}

/// 读"昵称变更时通知 AI"开关；缺省视为 true（默认 ON）。
///
/// #12 修复：早先这里自己写 `match raw { Some("false") => false; _ => true }`，与
/// `nickname_announcement::read_announce_user_change_with_conn` 重复了"字符串 KV 当布尔"
/// 的转换规则。直接调用 service 层 helper，避免双源（将来加第三态 'ask' 只改一处）。
#[tauri::command]
pub async fn nickname_get_announce_user_change(app: AppHandle) -> Result<bool, String> {
    read_announce_user_change(&app).await.map_err(|e| e.to_string())
}

/// 写"昵称变更时通知 AI"开关。
#[tauri::command]
pub async fn nickname_set_announce_user_change(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    config::set(&app, CONFIG_KEY_ANNOUNCE_USER_CHANGE, value)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_trims_and_accepts_normal_name() {
        assert_eq!(validate_nickname("  Alice  ").unwrap(), "Alice");
        assert_eq!(validate_nickname("默默").unwrap(), "默默");
    }

    #[test]
    fn validate_rejects_empty_and_whitespace_only() {
        assert!(validate_nickname("").is_err());
        assert!(validate_nickname("   ").is_err());
        assert!(validate_nickname("\t\n").is_err());
    }

    #[test]
    fn validate_rejects_control_chars() {
        // 与前端 NicknameForm.vue CONTROL_CHAR_RE 对齐：[\x00-\x1F\x7F]
        assert!(validate_nickname("Ali\nce").is_err(), "LF should be rejected");
        assert!(validate_nickname("Ali\rce").is_err(), "CR should be rejected");
        assert!(validate_nickname("A\x00").is_err(), "NUL should be rejected");
        assert!(
            validate_nickname("A\x7Fb").is_err(),
            "DEL (0x7F) should be rejected"
        );
        assert!(
            validate_nickname("A\x1Fb").is_err(),
            "US (0x1F) should be rejected"
        );
    }

    #[test]
    fn validate_enforces_max_chars_count_not_bytes() {
        // 16 字符（中文也按字符数计）
        let sixteen_zh = "一二三四五六七八九十一二三四五六";
        assert_eq!(sixteen_zh.chars().count(), 16);
        assert!(validate_nickname(sixteen_zh).is_ok());

        let seventeen_zh = "一二三四五六七八九十一二三四五六七";
        assert_eq!(seventeen_zh.chars().count(), 17);
        assert!(validate_nickname(seventeen_zh).is_err());

        let seventeen_en = "abcdefghijklmnopq";
        assert_eq!(seventeen_en.chars().count(), 17);
        assert!(validate_nickname(seventeen_en).is_err());
    }
}

