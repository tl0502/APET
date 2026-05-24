// StateStore — kernel-owned DB 抽象 (Spec §8.1).
// Phase A0: 暴露 Arc<Repo> 给 subsystem; raw Pool 在 services::db 已私有 (lib.rs 也不 export);
// Phase A1 拆出 kernel::db 完整收口 + 加 UoW。

use std::sync::Arc;

use crate::kernel::repos::{ConversationRepo, MemoryRepo, PersonaRepo};

pub struct StateStore {
    conversation: Arc<ConversationRepo>,
    persona: Arc<PersonaRepo>,
    memory: Arc<MemoryRepo>,
}

impl StateStore {
    pub fn new() -> Self {
        Self {
            conversation: Arc::new(ConversationRepo::new()),
            persona: Arc::new(PersonaRepo::new()),
            memory: Arc::new(MemoryRepo::new()),
        }
    }

    pub fn conversation_repo(&self) -> Arc<ConversationRepo> {
        Arc::clone(&self.conversation)
    }

    pub fn persona_repo(&self) -> Arc<PersonaRepo> {
        Arc::clone(&self.persona)
    }

    pub fn memory_repo(&self) -> Arc<MemoryRepo> {
        Arc::clone(&self.memory)
    }
}

impl Default for StateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_store_returns_arc_clone_each_call() {
        let store = StateStore::new();
        let r1 = store.conversation_repo();
        let _r2 = store.conversation_repo();
        // store 内部 1 + r1 + r2 = 3
        assert_eq!(Arc::strong_count(&r1), 3);
    }
}
