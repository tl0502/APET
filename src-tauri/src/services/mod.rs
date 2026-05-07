// 业务服务层 — #5 PersonaService MVP + Memory/Nickname 骨架（M1 W1 数据层）。
// 后续按 milestone 接入：crypto / secrets / chat / llm / interaction / wardrobe / 等。

pub mod db;

// memory.rs 整文件复用旧项目（含 messages 表 CRUD 超集），#5 范围内仅 nickname/persona 在用；
// 其 messages 函数将由 M1 W2 ChatService MVP（B.3.a）接入，届时去掉本 attr。
#[allow(dead_code)]
pub mod memory;

pub mod config;
// #12 LLMProvider trait + OpenAIProvider（M1 W2 ADR-018 Layer 1）。
// trait + 类型 typed only：M1 W2 LLMProvider 还没被消费方（#13 ChatService 才调），
// commands::llm IPC 仅做 dev console 验证；多模态 / 工具调用相关 variant 也是 M3+ 才用。
// 整模块 #[allow(dead_code)]，#13 上线后真消费 trait + 类型时去掉本 attr。
#[allow(dead_code)]
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
#[allow(dead_code)]
pub mod window_actions;
pub mod tray;

// 仅测试期编译：DB 集成测试共享 fixture（详 旧项目 progress/test-coverage-2026-05-04.md）
#[cfg(test)]
pub mod test_db;
