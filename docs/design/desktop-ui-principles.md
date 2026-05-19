---
title: 桌面应用 UI 范式（非 Web）
updated: 2026-05-19
related:
  - ../architecture/system-architecture.md
  - ../decisions.md
---

# 桌面应用 UI 范式（非 Web）

> 本项目是 Tauri 桌面应用，**不是网页**。所有 UI/UX 决策按桌面软件思路走（Discord / Telegram / Notion / VSCode / Linear），不按响应式网页或移动端思路走。

- 适用范围：所有窗口（pet / chat / settings / tasks / onboarding / pomodoro）和未来新增窗口
- 关联：[tauri.conf.json](../../src-tauri/tauri.conf.json) 已定义 7 个窗口，分两类（透明无装饰 vs 标准带装饰）

---

## 1. 多窗 ≠ 单页路由

桌宠是**多窗口程序**，不是 SPA。

- 新增功能时优先问"是不是一个新窗口"，而不是"塞到哪个 tab 里"
- 已确立的两类窗口风格：
  - **悬浮型**（pet / pomodoro / chat）：`decorations: false` + 自绘 chrome，承担长期可见 / 可贴边 / 可拖动的桌面伙伴角色
  - **工具型**（settings / tasks / onboarding）：`decorations: true` + 系统标题栏，承担一次性配置 / 列表浏览
- 跨窗口共享状态走 SQLite + IPC 事件，不走 URL query string

## 2. 表面分层（surface hierarchy）

每个窗口至少要有可辨识的**三层**：背景 / 内容 / 浮层。

- 背景层：窗口本体（透明窗时是 rounded container）
- 内容层：列表 / 消息流 / 配置项 —— **不要直接贴在背景上**，要有自己的容器
- 浮层：composer / toast / 右键菜单 / 弹窗 —— 用 shadow + 更大圆角 + 独立 z-index 抬起
- 判定标准：截图变灰度后还能看出层次 → 合格；一片平 → 不合格

## 3. 不依赖系统圆角

透明窗（pet / chat / pomodoro）的圆角由**前端自绘容器**实现，不靠 OS。

- 窗口设 `transparent: true` + `decorations: false` + `shadow: false`
- 内层根容器自己画 `border-radius` + `box-shadow`
- 不允许出现矩形 WebView 边缘漏出 / 背景从角落渗出

## 4. 桌面级交互

优先级：拖拽区 > 快捷键 > 右键菜单 > 按钮。

- 自绘 titlebar 必须有显著拖动区（`data-tauri-drag-region`），不要让用户只能从一个 8px 像素条拖
- 关键操作配快捷键（发送 Enter / 取消 Esc / 切换对话 Ctrl+数字）
- 列表项右键 = 上下文菜单，不是悬浮按钮组
- 调整大小：透明窗自己处理 hit zone；标准窗交给 OS
- **不要把响应式断点搬过来**（无 `sm:` / `md:` / `lg:`），桌面窗口是用户主动拖宽的，按可用宽度做布局而不是按预设尺寸切换

## 5. Composer / 输入区

输入框不是普通 textarea，是**独立浮层**。

- 用 elevation（背景色差 + shadow）和上方消息流明确分开
- 自己的圆角比内容容器更大
- 多行自适应，但有上限（避免吞掉整个窗口）
- 当前 [src/components/chat/ChatInput.vue](../../src/components/chat/ChatInput.vue) 已是这个形态，新窗口的输入区参照它

## 6. 动效

桌面动效原则：**低频、短促、物理可信**。

- 避免移动端 spring 弹跳堆叠
- 避免网页常见的长 transition（>300ms）
- 拖拽 / 贴边 / 窗口出现等位移动效允许稍长（已有的 snap 系统是范例）
- 列表项 hover、按钮按下：≤150ms，ease-out

## 7. 反例自检清单

写完一屏 UI，对照看是否踩到任意一条：

- [ ] 整个页面一片白 / 一片纯色，没有分层
- [ ] titlebar 像浏览器导航条（细横条 + logo + 一排链接）
- [ ] sidebar 是纯白列表，没有底色区分
- [ ] 输入框直接贴在消息流末尾，没有抬起感
- [ ] 出现了 `@media (max-width:...)` 或 Tailwind 响应式断点
- [ ] 关键操作只有点击按钮一种触发方式
- [ ] 用了 `position: sticky` 模拟桌面浮层（应该用真正的浮层容器）
- [ ] 圆角靠 OS 提供 / 角落能看见矩形漏底

任意一条 → 停下来重新设计该区域。

---

## 99. 附录

### 99.1 参考产品

- Discord（多服务器侧栏 + 频道 + 内容三栏，浮层 composer）
- Telegram Desktop（透明 + 自绘 chrome 的范本）
- Linear（桌面级密度 + 键盘优先）
- VSCode（活动栏 + 侧栏 + 编辑区 + 状态栏，多浮层）

### 99.2 待办

- [ ] M2 后回看本文档：现有 6 窗是否都符合 §7 自检
- [ ] 抽取通用 `<DesktopSurface>` 组件封装 §2 / §3 规范
