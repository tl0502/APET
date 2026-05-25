// SecretRepo — owner of `secrets` table (DPAPI 加密 KV)。
// Spec §15.4: API Key 明文 → DPAPI 是 P0 技术债, Phase A0 必修。
// 与 CryptoService 配合: 写入时加密 / 读取时解密, 明文从不入 DB。

use std::sync::Arc;

use chrono::Utc;
use sqlx::SqliteConnection;
use thiserror::Error;

use crate::kernel::crypto::{CryptoError, CryptoService, SecretValue};
use super::RepoError;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("repo: {0}")]
    Repo(#[from] RepoError),
    #[error("crypto: {0}")]
    Crypto(#[from] CryptoError),
    #[error("sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("not found: {0}")]
    NotFound(String),
}

pub struct SecretRepo {
    crypto: Arc<dyn CryptoService>,
}

impl SecretRepo {
    pub fn new(crypto: Arc<dyn CryptoService>) -> Self {
        Self { crypto }
    }

    pub async fn set(
        &self,
        conn: &mut SqliteConnection,
        key: &str,
        plaintext: &[u8],
    ) -> Result<(), SecretError> {
        let ciphertext = self.crypto.encrypt(plaintext)?;
        let now = Utc::now().to_rfc3339();
        // Prod schema = 001 (key/ciphertext/updated_at) + 002 ALTER ADD created_at。
        // 002 line 14 设计意图: INSERT 显式写真实时间, ON CONFLICT 不更新 created_at（审计列保留首次创建时间）。
        sqlx::query(
            "INSERT INTO secrets (key, ciphertext, created_at, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(key) DO UPDATE SET ciphertext = excluded.ciphertext, updated_at = excluded.updated_at"
        )
            .bind(key)
            .bind(&ciphertext)
            .bind(&now)
            .bind(&now)
            .execute(&mut *conn)
            .await?;
        Ok(())
    }

    pub async fn get(
        &self,
        conn: &mut SqliteConnection,
        key: &str,
    ) -> Result<SecretValue, SecretError> {
        let ciphertext: Vec<u8> = sqlx::query_scalar("SELECT ciphertext FROM secrets WHERE key = ?")
            .bind(key)
            .fetch_optional(&mut *conn)
            .await?
            .ok_or_else(|| SecretError::NotFound(key.to_string()))?;
        let plaintext = self.crypto.decrypt(&ciphertext)?;
        Ok(SecretValue(plaintext))
    }

    pub async fn delete(
        &self,
        conn: &mut SqliteConnection,
        key: &str,
    ) -> Result<(), SecretError> {
        let res = sqlx::query("DELETE FROM secrets WHERE key = ?")
            .bind(key)
            .execute(&mut *conn)
            .await?;
        if res.rows_affected() == 0 {
            return Err(SecretError::NotFound(key.to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;
    use crate::kernel::crypto::DpapiCryptoService;
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::ConnectOptions;

    async fn setup_test_db() -> SqliteConnection {
        let mut conn = SqliteConnectOptions::new().in_memory(true).connect().await.unwrap();
        sqlx::query(
            "CREATE TABLE secrets (
                key TEXT PRIMARY KEY,
                ciphertext BLOB NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )"
        ).execute(&mut conn).await.unwrap();
        conn
    }

    #[tokio::test]
    async fn set_get_roundtrip_via_dpapi() {
        let mut conn = setup_test_db().await;
        let repo = SecretRepo::new(Arc::new(DpapiCryptoService));
        repo.set(&mut conn, "openai_key", b"sk-test-12345").await.unwrap();
        let secret = repo.get(&mut conn, "openai_key").await.unwrap();
        assert_eq!(secret.0, b"sk-test-12345");
    }

    #[tokio::test]
    async fn db_stores_ciphertext_not_plaintext() {
        let mut conn = setup_test_db().await;
        let repo = SecretRepo::new(Arc::new(DpapiCryptoService));
        repo.set(&mut conn, "key1", b"plaintext-value").await.unwrap();
        let stored: Vec<u8> = sqlx::query_scalar("SELECT ciphertext FROM secrets WHERE key = 'key1'")
            .fetch_one(&mut conn).await.unwrap();
        let stored_str = String::from_utf8_lossy(&stored);
        assert!(!stored_str.contains("plaintext-value"));
    }

    #[tokio::test]
    async fn get_returns_not_found_for_missing_key() {
        let mut conn = setup_test_db().await;
        let repo = SecretRepo::new(Arc::new(DpapiCryptoService));
        let result = repo.get(&mut conn, "ghost").await;
        assert!(matches!(result, Err(SecretError::NotFound(_))));
    }

    #[tokio::test]
    async fn set_same_key_twice_overwrites() {
        let mut conn = setup_test_db().await;
        let repo = SecretRepo::new(Arc::new(DpapiCryptoService));
        repo.set(&mut conn, "k", b"v1").await.unwrap();
        repo.set(&mut conn, "k", b"v2").await.unwrap();
        let secret = repo.get(&mut conn, "k").await.unwrap();
        assert_eq!(secret.0, b"v2");
    }

    #[tokio::test]
    async fn delete_removes_key_and_subsequent_get_returns_not_found() {
        let mut conn = setup_test_db().await;
        let repo = SecretRepo::new(Arc::new(DpapiCryptoService));
        repo.set(&mut conn, "k", b"v").await.unwrap();
        repo.delete(&mut conn, "k").await.unwrap();
        assert!(matches!(repo.get(&mut conn, "k").await, Err(SecretError::NotFound(_))));
        // 二次 delete 也应返 NotFound
        assert!(matches!(repo.delete(&mut conn, "k").await, Err(SecretError::NotFound(_))));
    }
}
