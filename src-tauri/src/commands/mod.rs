// IPC commands 总入口。按 milestone 节奏在此挂载子模块：
// - #4 system（健康检查）
// - #5 persona / memory / nickname（M1 W1 数据层）
// - #9 window（settings 窗口 show/hide）+ #10 pet 位置 get/save
// - #11 shortcuts（全局快捷键 probe / set）
// - #12 llm（OpenAIProvider 测试 IPC，dev console 验证用）
// - #13 chat（ChatService 业务编排：chat_send / chat_cancel / chat_history）
// 后续：interaction / wardrobe / 等。
pub mod chat;
pub mod llm;
pub mod memory;
pub mod nickname;
pub mod persona;
pub mod shortcuts;
pub mod system;
pub mod window;
