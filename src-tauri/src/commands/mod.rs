// IPC commands 总入口。按 milestone 节奏在此挂载子模块：
// - #4 system（健康检查）
// - #5 persona / memory / nickname（M1 W1 数据层）
// - #9 window（settings 窗口 show/hide）+ #10 pet 位置 get/save + #14 chat show/hide/toggle
// - #11 shortcuts（全局快捷键 probe / set）
// - 用户增补 LLM Providers（多 provider 实例 CRUD + activate + test，参考 cc-switch UI；
//   #12 单 namespace IPC 已退役）
// - #13 chat（ChatService 业务编排：chat_send / chat_cancel / chat_history）
// - #16 consent（灵魂宣誓 grant / check_version；前端视图 #16b）
// 后续：interaction / wardrobe / 等。
pub mod chat;
pub mod consent;
pub mod llm_providers;
pub mod memory;
pub mod nickname;
pub mod persona;
pub mod shortcuts;
pub mod system;
pub mod window;
