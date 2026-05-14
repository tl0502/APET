// 头像（#25 用户上传 + #26 VRM 导出）公共服务。
//
// 设计要点：
// - 落盘目录：<app_config_dir>/avatars/，与 aipet.db 同级；assetProtocol scope
//   `$APPCONFIG/avatars/**` 让 webview 能直接 `<img src>` 加载。
// - 文件名约定：user.<ext>（用户头像，扩展名跟源文件） + persona-<id>.png（VRM 导出，
//   一律 PNG）。同 persona 多次导出覆盖；切到不同 persona 不影响旧 persona 的头像。
// - 校验：magic byte 识别 PNG/JPG（不信扩展名）；max 5MB；扩展名只允许 png/jpg/jpeg。
// - 不写 DB —— 路径由前端通过 memory_set 写到 KV（`user:avatar_path` /
//   `persona:<id>:avatar_path`），保持与现有 KV 偏好层的统一存储。
//
// 错误：thin AvatarError 枚举；commands 层 .to_string() 翻成 IpcError 给前端。

use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use tauri::{AppHandle, Manager, Runtime};
use thiserror::Error;
/// 头像文件大小上限 5MB —— 同步用 chat 窗 28px 圆头像 + 设置预览的最大用例，
/// 远超即"用户拿了张未压缩 RAW 转 PNG"的边界。
pub const MAX_FILE_SIZE: u64 = 5 * 1024 * 1024;

/// 允许的图片扩展名（白名单）。
pub const ALLOWED_EXTS: &[&str] = &["png", "jpg", "jpeg"];

#[derive(Debug, Error)]
pub enum AvatarError {
    #[error("config dir resolution failed: {0}")]
    AppConfigDir(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("invalid file: {0}")]
    Invalid(String),
    #[error("base64 decode failed: {0}")]
    Base64(String),
}

impl From<std::io::Error> for AvatarError {
    fn from(e: std::io::Error) -> Self {
        AvatarError::Io(e.to_string())
    }
}

/// 解析 <app_config>/avatars/ 目录路径并懒创建。任何后续 fs 操作都过此函数。
pub fn ensure_avatars_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, AvatarError> {
    let app_config = app
        .path()
        .app_config_dir()
        .map_err(|e| AvatarError::AppConfigDir(e.to_string()))?;
    let dir = app_config.join("avatars");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 取扩展名（小写）；无扩展名或不可解析返 None。
fn lower_ext(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
}

/// PNG / JPG magic byte 检查。比扩展名可靠 —— 拦"改扩展名伪装"的边角输入。
///
/// PNG: 89 50 4E 47 0D 0A 1A 0A（"\x89PNG\r\n\x1A\n"）
/// JPEG: FF D8 FF（JPEG SOI 必须以此 3 字节开头；后续 E0/E1/DB 不同子格式都行）
fn detect_image_kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 8 && bytes[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return Some("png");
    }
    if bytes.len() >= 3 && bytes[..3] == [0xFF, 0xD8, 0xFF] {
        return Some("jpg");
    }
    None
}

/// 用户头像入口（#25）：复制源文件到 `<app_config>/avatars/user.<ext>`。
///
/// 路径模式 —— 没经过裁剪直接落盘的简单路径。裁剪流走 `save_user_avatar_from_data_url`。
///
/// 安全：
/// - 扩展名 + magic byte 双校验
/// - 文件大小 ≤ MAX_FILE_SIZE
/// - 覆盖前先把所有旧 `user.*` 清掉（避免新 png 上来旧 jpg 还在盘里造成路径不一致）
///
/// 返回最终落盘的绝对路径（前端写 KV 用）。
pub fn copy_user_avatar<R: Runtime>(
    app: &AppHandle<R>,
    src_path: &str,
) -> Result<PathBuf, AvatarError> {
    let src = Path::new(src_path);
    if !src.exists() {
        return Err(AvatarError::Invalid(format!("source not found: {src_path}")));
    }
    if !src.is_file() {
        return Err(AvatarError::Invalid("source is not a regular file".into()));
    }

    let ext = lower_ext(src)
        .ok_or_else(|| AvatarError::Invalid("missing file extension".into()))?;
    if !ALLOWED_EXTS.contains(&ext.as_str()) {
        return Err(AvatarError::Invalid(format!(
            "extension '{ext}' not allowed; want one of {ALLOWED_EXTS:?}"
        )));
    }

    let metadata = fs::metadata(src)?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(AvatarError::Invalid(format!(
            "file too large: {} bytes (max {MAX_FILE_SIZE})",
            metadata.len()
        )));
    }

    // 读全文做 magic byte 检查 + 后续 fs::write（一次性，<5MB 内存可接受）
    let bytes = fs::read(src)?;
    let detected = detect_image_kind(&bytes)
        .ok_or_else(|| AvatarError::Invalid("not a valid PNG/JPG (magic byte mismatch)".into()))?;
    // 扩展名 jpg/jpeg 都映射到 magic "jpg"
    let ext_normalized = if ext == "jpeg" { "jpg" } else { ext.as_str() };
    if detected != ext_normalized {
        return Err(AvatarError::Invalid(format!(
            "extension '{ext}' does not match content '{detected}'"
        )));
    }

    let dir = ensure_avatars_dir(app)?;
    // 清掉所有旧 user.* —— 防止从 png 切到 jpg 时盘里残留两份
    cleanup_user_avatars(&dir)?;

    let dest_ext = if ext_normalized == "jpg" { "jpg" } else { "png" };
    let dest = dir.join(format!("user.{dest_ext}"));
    fs::write(&dest, &bytes)?;
    Ok(dest)
}

/// 删除所有 `user.*` 头像（#25 clear）。返"被删了多少个"。
pub fn clear_user_avatar<R: Runtime>(app: &AppHandle<R>) -> Result<u32, AvatarError> {
    let dir = ensure_avatars_dir(app)?;
    cleanup_user_avatars(&dir)
}

/// 读源文件并以 `data:image/<mime>;base64,...` 形式返回，给前端 cropper 喂图（#25 裁剪流）。
///
/// 为什么不走 convertFileSrc？assetProtocol scope 限定在 `$APPCONFIG/avatars/**`，
/// 用户随便选的 D:/Pictures/x.jpg 不在 scope 内，加宽 scope 不安全。
/// 此 IPC 单次同步读小于 5MB 的图，返 base64 dataURL，cropper 直接 `<img src>` 使用。
///
/// 校验：扩展名 + magic byte + size。失败前端 toast，不会发出大文件污染 IPC channel。
pub fn read_image_to_data_url<R: Runtime>(
    app: &AppHandle<R>,
    src_path: &str,
) -> Result<String, AvatarError> {
    let _ = app; // 暂未使用 app，但保留以备未来路径权限校验
    let src = Path::new(src_path);
    if !src.exists() {
        return Err(AvatarError::Invalid(format!("source not found: {src_path}")));
    }
    if !src.is_file() {
        return Err(AvatarError::Invalid("source is not a regular file".into()));
    }
    let ext = lower_ext(src)
        .ok_or_else(|| AvatarError::Invalid("missing file extension".into()))?;
    if !ALLOWED_EXTS.contains(&ext.as_str()) {
        return Err(AvatarError::Invalid(format!(
            "extension '{ext}' not allowed; want one of {ALLOWED_EXTS:?}"
        )));
    }
    let metadata = fs::metadata(src)?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(AvatarError::Invalid(format!(
            "file too large: {} bytes (max {MAX_FILE_SIZE})",
            metadata.len()
        )));
    }
    let bytes = fs::read(src)?;
    let detected = detect_image_kind(&bytes)
        .ok_or_else(|| AvatarError::Invalid("not a valid PNG/JPG (magic byte mismatch)".into()))?;
    let mime = if detected == "png" { "image/png" } else { "image/jpeg" };
    let b64 = BASE64_STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

/// 裁剪后保存用户头像（#25 裁剪流）：解码 PNG dataURL → 落盘 user.png。
///
/// 与 `copy_user_avatar` 的差异：source 是 base64 dataURL 而非文件路径，
/// 校验改成 strict PNG（cropperjs 输出固定 image/png）。
///
/// H5 修复 —— 原子写顺序：write tmp → rename → cleanup legacy ext。
/// - 写 tmp 失败：旧头像完全不动
/// - rename 失败：旧 user.png 不动（rename 是原子替换 OR 报错；NTFS 同目录原子）
/// - cleanup 失败：用户看到的仍是新 user.png，可能 user.jpg 孤儿（cosmetic，不影响显示）
/// 任一步骤失败下，"用户当前显示的头像"绝不会丢。
pub fn save_user_avatar_from_data_url<R: Runtime>(
    app: &AppHandle<R>,
    data_url: &str,
) -> Result<PathBuf, AvatarError> {
    let b64 = strip_png_data_url_prefix(data_url)?;
    let bytes = BASE64_STANDARD
        .decode(b64)
        .map_err(|e| AvatarError::Base64(e.to_string()))?;
    if bytes.len() as u64 > MAX_FILE_SIZE {
        return Err(AvatarError::Invalid(format!(
            "decoded image too large: {} bytes (max {MAX_FILE_SIZE})",
            bytes.len()
        )));
    }
    if detect_image_kind(&bytes) != Some("png") {
        return Err(AvatarError::Invalid("decoded bytes are not a valid PNG".into()));
    }
    let dir = ensure_avatars_dir(app)?;
    let tmp = dir.join("user.png.tmp");
    let dest = dir.join("user.png");

    // 1) 写 .tmp —— 失败时旧 user.* 完全不动
    fs::write(&tmp, &bytes)?;

    // 2) 原子 rename .tmp → user.png；目标存在则替换。同目录 rename 在 NTFS/ext4/APFS 都是原子的。
    if let Err(e) = fs::rename(&tmp, &dest) {
        // rename 失败：清掉 tmp 避免孤儿；旧 user.png 仍在
        let _ = fs::remove_file(&tmp);
        return Err(AvatarError::Io(format!(
            "rename {} -> {} failed: {}",
            tmp.display(),
            dest.display(),
            e
        )));
    }

    // 3) 现在 user.png 已是新的。清掉同目录下可能还在的旧扩展名（user.jpg / user.jpeg）。
    //    NOT 删 user.png —— 它是我们刚 rename 出来的目标。
    if let Err(e) = cleanup_legacy_user_extensions(&dir) {
        eprintln!("[avatars] cleanup legacy ext after save failed (non-fatal): {e}");
    }

    Ok(dest)
}

/// H5 配套：只删 user.jpg / user.jpeg，保留 user.png（save_user_avatar_from_data_url 用）。
fn cleanup_legacy_user_extensions(dir: &Path) -> Result<u32, AvatarError> {
    let mut removed = 0u32;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if name == "user.jpg" || name == "user.jpeg" {
            if let Err(e) = fs::remove_file(&path) {
                eprintln!("[avatars] remove {} failed: {}", path.display(), e);
                continue;
            }
            removed += 1;
        }
    }
    Ok(removed)
}

fn cleanup_user_avatars(dir: &Path) -> Result<u32, AvatarError> {
    let mut removed = 0u32;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        // 只删 user.png / user.jpg / user.jpeg —— persona-*.png 不动
        if name == "user.png" || name == "user.jpg" || name == "user.jpeg" {
            if let Err(e) = fs::remove_file(&path) {
                // 单个删除失败不阻断（可能 antivirus 锁文件等），log 即可
                eprintln!("[avatars] remove {} failed: {}", path.display(), e);
                continue;
            }
            removed += 1;
        }
    }
    Ok(removed)
}

/// 校验 persona id —— ASCII letters / digits / dash / underscore，1-64 字符。
/// 拦 `../` 等路径穿越与控制字符；与 .soul.md frontmatter id 字段语义保持一致。
fn validate_persona_id(id: &str) -> Result<(), AvatarError> {
    if id.is_empty() || id.len() > 64 {
        return Err(AvatarError::Invalid("persona id length out of range".into()));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AvatarError::Invalid(
            "persona id contains invalid characters (allowed: a-z A-Z 0-9 - _)".into(),
        ));
    }
    Ok(())
}

/// 解析 `data:image/png;base64,...` 形式的 data URL，返回纯 base64 部分。
fn strip_png_data_url_prefix(data_url: &str) -> Result<&str, AvatarError> {
    const PREFIX: &str = "data:image/png;base64,";
    data_url
        .strip_prefix(PREFIX)
        .ok_or_else(|| AvatarError::Invalid(format!("expected data URL starting with '{PREFIX}'")))
}

/// VRM 头像入口（#26）：解码前端 `toDataURL('image/png')` 写到
/// `<app_config>/avatars/persona-<id>.png`。
///
/// 同 persona 多次导出覆盖；切到不同 persona 不影响其它 persona 的头像。
pub fn save_persona_avatar<R: Runtime>(
    app: &AppHandle<R>,
    persona_id: &str,
    data_url: &str,
) -> Result<PathBuf, AvatarError> {
    validate_persona_id(persona_id)?;
    let b64 = strip_png_data_url_prefix(data_url)?;
    let bytes = BASE64_STANDARD
        .decode(b64)
        .map_err(|e| AvatarError::Base64(e.to_string()))?;
    if bytes.len() as u64 > MAX_FILE_SIZE {
        return Err(AvatarError::Invalid(format!(
            "decoded image too large: {} bytes (max {MAX_FILE_SIZE})",
            bytes.len()
        )));
    }
    // magic byte 兜底 —— toDataURL('image/png') 理论必出 PNG，但拦"前端被改 mime"边角
    if detect_image_kind(&bytes) != Some("png") {
        return Err(AvatarError::Invalid("decoded bytes are not a valid PNG".into()));
    }

    let dir = ensure_avatars_dir(app)?;
    let dest = dir.join(format!("persona-{persona_id}.png"));
    fs::write(&dest, &bytes)?;
    Ok(dest)
}

/// 删 `persona-<id>.png`。不存在视为 no-op，不报错。
pub fn clear_persona_avatar<R: Runtime>(
    app: &AppHandle<R>,
    persona_id: &str,
) -> Result<bool, AvatarError> {
    validate_persona_id(persona_id)?;
    let dir = ensure_avatars_dir(app)?;
    let path = dir.join(format!("persona-{persona_id}.png"));
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(&path)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_png_magic() {
        let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];
        assert_eq!(detect_image_kind(&png_header), Some("png"));
    }

    #[test]
    fn detect_jpg_magic() {
        let jpg_header = [0xFF, 0xD8, 0xFF, 0xE0, 0x00];
        assert_eq!(detect_image_kind(&jpg_header), Some("jpg"));
    }

    #[test]
    fn detect_unknown_returns_none() {
        let txt_header = b"Hello, world!";
        assert!(detect_image_kind(txt_header).is_none());
    }

    #[test]
    fn detect_too_short_returns_none() {
        assert!(detect_image_kind(&[0x89, 0x50]).is_none());
    }

    #[test]
    fn validate_persona_id_accepts_normal() {
        assert!(validate_persona_id("momo").is_ok());
        assert!(validate_persona_id("joker_v2").is_ok());
        assert!(validate_persona_id("user-coach-01").is_ok());
    }

    #[test]
    fn validate_persona_id_rejects_path_traversal() {
        assert!(validate_persona_id("../etc/passwd").is_err());
        assert!(validate_persona_id("a/b").is_err());
        assert!(validate_persona_id("a\\b").is_err());
    }

    #[test]
    fn validate_persona_id_rejects_empty_and_long() {
        assert!(validate_persona_id("").is_err());
        let long = "a".repeat(65);
        assert!(validate_persona_id(&long).is_err());
        let edge = "a".repeat(64);
        assert!(validate_persona_id(&edge).is_ok());
    }

    #[test]
    fn validate_persona_id_rejects_non_ascii() {
        assert!(validate_persona_id("默默").is_err());
    }

    #[test]
    fn strip_data_url_works() {
        assert_eq!(
            strip_png_data_url_prefix("data:image/png;base64,iVBORw0KGgo=").unwrap(),
            "iVBORw0KGgo="
        );
    }

    #[test]
    fn strip_data_url_rejects_jpeg() {
        assert!(strip_png_data_url_prefix("data:image/jpeg;base64,xxx").is_err());
    }

    #[test]
    fn strip_data_url_rejects_raw_base64() {
        assert!(strip_png_data_url_prefix("iVBORw0KGgo=").is_err());
    }

    #[test]
    fn lower_ext_normalizes_case() {
        assert_eq!(
            lower_ext(Path::new("/tmp/IMG.PNG")),
            Some("png".to_string())
        );
        assert_eq!(lower_ext(Path::new("no_ext")), None);
    }
}
