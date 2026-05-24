// kernel/repos — 每个 owner table 一个 Repository (Constitution #2)。
// raw sqlx::Pool 仅 kernel/db module (Phase A0 临时复用 services::db) 可见;
// subsystem 拿到的是 Arc<{Owner}Repo>, 只能调有限强类型方法。

pub mod conversation_repo;
pub mod memory_repo;
pub mod persona_repo;

pub use conversation_repo::ConversationRepo;
pub use memory_repo::MemoryRepo;
pub use persona_repo::PersonaRepo;
