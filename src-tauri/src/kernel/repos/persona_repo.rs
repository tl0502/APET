// PersonaRepo — owner of `personas` + `persona_snapshots` + `persona_snapshot_profiles` tables.
// Phase A0: 极简 stub, Phase A1 SoulCompiler 落地时扩接口。

use super::RepoError;
use sqlx::SqliteConnection;

pub struct PersonaRepo {}

impl PersonaRepo {
    pub fn new() -> Self {
        Self {}
    }

    /// Phase A1 才扩: insert_snapshot / get_latest_snapshot / get_by_id
    /// Phase A0 占位, 让 StateStore 可以 wire
    pub async fn _placeholder(&self, _conn: &mut SqliteConnection) -> Result<(), RepoError> {
        Ok(())
    }
}

impl Default for PersonaRepo {
    fn default() -> Self {
        Self::new()
    }
}
