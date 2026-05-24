// CryptoService — Windows DPAPI 加密 secrets (Spec §0.6 / §15.4 P0 技术债).
// API Key / OAuth token 等 secrets 不能再明文存 config 表; 必经此 service。
//
// Phase A0: Windows DPAPI per-user 加密 (CryptProtectData / CryptUnprotectData)。
// Linux/macOS fallback P1 评估 (libsecret / Keychain)。

use thiserror::Error;
use zeroize::Zeroize;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("DPAPI encrypt failed: {0}")]
    EncryptFailed(String),
    #[error("DPAPI decrypt failed: {0}")]
    DecryptFailed(String),
    #[error("invalid ciphertext")]
    InvalidCiphertext,
}

/// CryptoService trait — kernel-owned。
pub trait CryptoService: Send + Sync {
    /// 明文 → DPAPI ciphertext bytes。明文使用后立刻 zeroize。
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError>;

    /// DPAPI ciphertext → 明文 (调用方负责 zeroize)。
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError>;
}

/// Windows DPAPI 实施。
#[cfg(target_os = "windows")]
pub struct DpapiCryptoService;

#[cfg(target_os = "windows")]
impl CryptoService for DpapiCryptoService {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};

        let mut in_blob = CRYPT_INTEGER_BLOB {
            cbData: plaintext.len() as u32,
            pbData: plaintext.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };

        let ok = unsafe {
            CryptProtectData(
                &mut in_blob,
                std::ptr::null(),       // description
                std::ptr::null_mut(),   // entropy
                std::ptr::null_mut(),   // reserved
                std::ptr::null_mut(),   // prompt
                0,                      // flags
                &mut out_blob,
            )
        };

        if ok == 0 {
            let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            return Err(CryptoError::EncryptFailed(format!("WIN32 error {}", err)));
        }

        let result = unsafe {
            std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec()
        };
        unsafe { LocalFree(out_blob.pbData as _); }
        Ok(result)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

        let mut in_blob = CRYPT_INTEGER_BLOB {
            cbData: ciphertext.len() as u32,
            pbData: ciphertext.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };

        let ok = unsafe {
            CryptUnprotectData(
                &mut in_blob,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                &mut out_blob,
            )
        };

        if ok == 0 {
            let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            return Err(CryptoError::DecryptFailed(format!("WIN32 error {}", err)));
        }

        let result = unsafe {
            std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec()
        };
        unsafe { LocalFree(out_blob.pbData as _); }
        Ok(result)
    }
}

/// 非 Windows 平台 stub (Phase A0 仅 Windows 目标, Linux/macOS P1 评估)。
#[cfg(not(target_os = "windows"))]
pub struct DpapiCryptoService;

#[cfg(not(target_os = "windows"))]
impl CryptoService for DpapiCryptoService {
    fn encrypt(&self, _: &[u8]) -> Result<Vec<u8>, CryptoError> {
        Err(CryptoError::EncryptFailed("non-Windows: DPAPI 未实施 (Phase A0 仅 Windows)".into()))
    }
    fn decrypt(&self, _: &[u8]) -> Result<Vec<u8>, CryptoError> {
        Err(CryptoError::DecryptFailed("non-Windows: DPAPI 未实施 (Phase A0 仅 Windows)".into()))
    }
}

/// 持有 secret plaintext 的 wrapper, Drop 时 zeroize。
#[derive(Zeroize)]
#[zeroize(drop)]
pub struct SecretValue(pub Vec<u8>);

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretValue(<{} bytes redacted>)", self.0.len())
    }
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;

    #[test]
    fn dpapi_encrypt_decrypt_roundtrip() {
        let svc = DpapiCryptoService;
        let plaintext = b"sk-test-api-key-xxxxxx";
        let ciphertext = svc.encrypt(plaintext).unwrap();
        assert_ne!(&ciphertext[..], plaintext);
        assert!(ciphertext.len() > plaintext.len());
        let decrypted = svc.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn dpapi_decrypt_returns_err_on_invalid_ciphertext() {
        let svc = DpapiCryptoService;
        let result = svc.decrypt(b"not-a-valid-ciphertext-blob");
        assert!(matches!(result, Err(CryptoError::DecryptFailed(_))));
    }

    #[test]
    fn secret_value_debug_does_not_leak_plaintext() {
        let s = SecretValue(b"super-secret-key".to_vec());
        let debug_str = format!("{:?}", s);
        assert!(!debug_str.contains("super-secret"));
        assert!(debug_str.contains("redacted"));
    }
}
