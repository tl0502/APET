// 业务服务层 — #5 PersonaService MVP + Memory/Nickname 骨架（M1 W1 数据层）→
// #13 ChatService MVP（M1 W2，ADR-018 Layer 2）。
// 后续按 milestone 接入：crypto / secrets / interaction / wardrobe / 等。

pub mod db;

// memory.rs：messages 表 CRUD（#13 ChatService::send 真消费 insert_message_with_conn /
// list_messages_by_conversation；其余 delete_* / cleanup_* 函数留给 M3 设置面板"清空对话"按钮）。
pub mod memory;

pub mod config;
// #13 ChatService 业务编排层（M1 W2 ADR-018 Layer 2）— commands::chat 已真消费。
pub mod chat;
// #12 LLMProvider trait + OpenAIProvider（ADR-018 Layer 1）— #13 ChatService 真消费
// chat_stream / OpenAIProvider / ChatOptions / FinishReason / LLMError / StreamDelta。
// 多模态 ContentPart::ImageUrl 等 variant 仍 typed only，M3+ 接多模态时实现 impl 路径。
pub mod llm;
pub mod nickname;
pub mod persona;
pub mod preferences;
pub mod shortcuts;
pub mod window_state;

// #6 系统托盘 + 窗口动作 helper（M1 W2 主态可达交付物）。
// window_actions 的 show_pet / hide_pet 在 #6 范围内未消费，#7 shortcuts task
// 接入 Ctrl+Alt+Space 全局快捷键时启用；mod 级 #[allow(dead_code)] 屏蔽 dead_code warning，
// 届时去掉本 attr。
pub mod tray;
#[allow(dead_code)]
pub mod window_actions;

// 仅测试期编译：DB 集成测试共享 fixture（详 旧项目 progress/test-coverage-2026-05-04.md）
#[cfg(test)]
pub mod test_db;
