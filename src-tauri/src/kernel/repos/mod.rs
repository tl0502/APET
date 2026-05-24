// kernel/repos — 每个 owner table 一个 Repository (Constitution #2)。
// raw sqlx::Pool 仅 kernel/db module (Phase A0 临时复用 services::db) 可见;
// subsystem 拿到的是 Arc<{Owner}Repo>, 只能调有限强类型方法。

use thiserror::Error;

pub mod conversation_repo;
pub mod memory_repo;
pub mod permission_repo;
pub mod persona_repo;
pub mod secret_repo;

pub use conversation_repo::ConversationRepo;
pub use memory_repo::MemoryRepo;
pub use permission_repo::PermissionRepo;
pub use persona_repo::PersonaRepo;
pub use secret_repo::SecretRepo;

/// 所有 kernel Repository 共享的错误类型。
/// Task 3 从 conversation_repo 提升至 repos/mod.rs (Task 2 code review 建议)。
#[derive(Debug, Error)]
pub enum RepoError {
    #[error("sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("not found: {0}")]
    NotFound(String),
}
