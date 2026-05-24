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
// #30 磁吸：config 表 KV IPC（snap:constraints 持久化）
pub mod config;
pub mod consent;
pub mod living_pet;
pub mod llm_providers;
pub mod memory;
pub mod nickname;
pub mod onboarding;
pub mod persona;
// #22 reminder IPC（6 命令：create/list/update/delete/snooze/complete）。
pub mod reminder;
// #29 todo IPC（6 命令：create/list/update/complete/breakdown/reorder）。
pub mod todo;
// #28 pomodoro IPC（6 命令：start/pause/resume/stop/active/today_stats）。
pub mod pomodoro;
pub mod shortcuts;
pub mod system;
pub mod window;
// #25/#26 头像 IPC（user 上传 + persona VRM 导出）
pub mod avatars;
// #23-a IdleDetector IPC (#39) — idle_get_state 单命令返回 {idleMs, isIdle, recentlyWoke}
pub mod idle;
