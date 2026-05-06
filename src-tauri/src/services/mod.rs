// 业务服务层 — #5 PersonaService MVP + Memory/Nickname 骨架（M1 W1 数据层）。
// 后续按 milestone 接入：crypto / secrets / chat / llm / interaction / wardrobe / 等。

pub mod db;

// memory.rs 整文件复用旧项目（含 messages 表 CRUD 超集），#5 范围内仅 nickname/persona 在用；
// 其 messages 函数将由 M1 W2 ChatService MVP（B.3.a）接入，届时去掉本 attr。
#[allow(dead_code)]
pub mod memory;

pub mod nickname;
pub mod persona;
pub mod preferences;

// 仅测试期编译：DB 集成测试共享 fixture（详 旧项目 progress/test-coverage-2026-05-04.md）
#[cfg(test)]
pub mod test_db;
