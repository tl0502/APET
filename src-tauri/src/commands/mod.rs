// IPC commands 总入口。按 milestone 节奏在此挂载子模块：
// - #4 system（健康检查）
// - #5 persona / memory / nickname（M1 W1 数据层）
// - #9 window（settings 窗口 show/hide）+ #10 pet 位置 get/save
// - #11 shortcuts（全局快捷键 probe / set）
// - #12 llm（OpenAIProvider 测试 IPC，dev console 验证用）
// 后续：chat / interaction / wardrobe / 等。
pub mod system;
pub mod persona;
pub mod memory;
pub mod nickname;
pub mod shortcuts;
pub mod window;
pub mod llm;
