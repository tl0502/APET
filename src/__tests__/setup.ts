// Vitest 全局 setup（jsdom env）。当前为空骨架；后续视需要补：
// - element-plus 全局 install（如果某个组件单测要 mount 真组件）
// - Tauri @tauri-apps/api/window mock（避免 invoke 失败）
// - matchMedia mock（jsdom 默认未实现，element-plus 主题切换会用到）
//
// 现状：纯算法单测（snap solver / cycle / candidates 等）零依赖，setup.ts 不需要内容；
// 留文件为未来 S6+ composable 测试预埋。
