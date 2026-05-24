// MemoryRepo — owner of `memory` table (KV). Phase A2 扩接口。

use super::RepoError;
use sqlx::SqliteConnection;

pub struct MemoryRepo {}

impl MemoryRepo {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn _placeholder(&self, _conn: &mut SqliteConnection) -> Result<(), RepoError> {
        Ok(())
    }
}

impl Default for MemoryRepo {
    fn default() -> Self {
        Self::new()
    }
}
