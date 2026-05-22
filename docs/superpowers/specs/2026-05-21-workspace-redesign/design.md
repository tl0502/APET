---
title: Workspace 重设计（chrome L 型框 + 用户 Popup + 容器公约）
updated: 2026-05-21
related:
  - ../../decisions.md
  - ../../design/desktop-ui-principles.md
  - ../../roadmap/development-roadmap.md
  - ../../STATUS.md
---

# Workspace 重设计 spec

> 本 spec 是 brainstorm 产出物，作为 GitHub issue 落地的设计基线。Phase 1 / Phase 2 各开一个 issue 执行，详见 §7。
>
> 配套 mockup：[scratch/01-skeleton.html](scratch/01-skeleton.html) · [scratch/02-popup.html](scratch/02-popup.html) · [scratch/03-nav-spec.html](scratch/03-nav-spec.html)

---

## 1. 问题与目标

### 1.1 现状疼点

#36 chrome 适配落地后，[src/views/workspace/](../../../src/views/workspace/) 仍存在两类视觉问题：

- **chrome 没成型**：顶部是 32px 不可见 drag-bar + 飘三个按钮，整窗缺一个真正的"顶栏"作视觉骨架；avatar 在 BrandBar 列顶被 32px 让位区挤着
- **内部组件没适配窗口**：3 栏壳做完后，panel 内层组件没跟着调整容器约束。典型例子：[ChatThreadPane](../../../src/components/chat/ChatThreadPane.vue) 的 chat-messages 没限制容器最大宽度，宽窗下消息行长超 100 字伤阅读。其余 7 个 panel 都有类似问题

用户原话："现有界面错位，无法对齐"；"整个面板内的组件都没进行窗口适配"。

### 1.2 目标

- 把 chrome 做成实色 L 型框（topbar + sidebar + master 同色，detail 主舞台白色），让骨架可见
- 用户头像从 BrandBar 顶搬到 topbar 左端；sidebar 底部 help 按钮换成用户头像，点击呼出用户 popup
- 把现有"用户级"设置项（昵称 → Profile / 关于 → 关于）从 workspace 设置类搬进 popup；workspace 设置类只剩"应用级"项（外观 + LLM Provider）
- 立一套 panel 容器公约（`.panel--form` / `--list` / `--chat`，对应三种 max-width 策略），治根
- 立一套 sidebar nav 通用规范（扁平 + Accordion 两种结构 + Active/Normal/Disabled 三态）

### 1.3 非目标

- 不重做颜色系统（tokens.css Apple/Bear 中灰 + #8b7cff 紫已经成熟，本次只做应用映射）
- 不引入第三档色阶（除 surface-soft / bg 两档）
- 不做 ⌘K 命令面板实功能（仅留 topbar 中央占位）
- 不做账户系统（登录 / 密码 / 2FA / 设备等，全部 M3+ 占位 disabled）
- 不做通知 / 数据隐私 panel 的真实功能（M3+ 占位 disabled）

---

## 2. 核心决策（brainstorm 期定下来的 5 条）

| # | 决策 | 取值 |
|---|---|---|
| D1 | topbar 正中胶囊位最终承担什么 | **占位**，最后再定（Phase 1 纯视觉，无 click 无 tooltip） |
| D2 | panel 内部横向尺寸规则 | **按类型分**：form 720 居中 / list 全宽 / chat 880 居中 |
| D3 | topbar chrome 视觉处理 | **实色 L 型框**（topbar+sidebar+master 同 surface-soft，detail 白） |
| D4 | 重设计节奏 | **分两段**：Phase 1 chrome+popup+1 示范，Phase 2 其余 6 panel 套规范 |
| D5 | 用户 popup IA 站位 | **Restructure**：昵称→Profile / 关于→popup 关于；workspace 设置类只剩外观+LLM |
| D6 | Accordion 落地范围 | **只定规范 + Phase 1 用扁平**；accordion 实做留待账户系统起来 |

---

## 3. 整窗骨架

### 3.1 Layout

CSS grid，固定栏 + 流式 detail：

```
grid-template-rows:    48px      1fr
grid-template-columns: 60px 240px 1fr
grid-template-areas:
  "topbar  topbar  topbar"
  "sidebar master  detail"
```

**关键水平基线**：topbar 48px = master header 48px，让"顶栏底线"与"master 内容起跑线"在 y 轴同一条线上。这是错位感的根源之一。

### 3.2 区域

| 区域 | 宽 | 高 | 背景 token | 内容 |
|---|---|---|---|---|
| topbar | full | 48px | `--aipet-color-surface-soft` | 桃宝 avatar(36) · capsule 占位 · chrome 三按钮 |
| sidebar | 60px | flex | `--aipet-color-surface-soft` | 4 类别 icon · spacer · 用户头像(32) |
| master | 240px (180–380 可拖) | flex | `--aipet-color-surface-soft` | sticky header(48) + 列表/对话 |
| detail | flex | flex | `--aipet-color-bg` | 8 个 panel v-show 永久 mount |

zone 之间 1px hairline = `--aipet-color-border-faint`。

### 3.3 Topbar 内容布局

```
[12px] [36×36 avatar pulse] [drag] [capsule 240-400 居中] [drag] [─][□][✕]
```

- **Avatar**：36×36 圆形，搬现有 [BrandBar.vue](../../../src/views/workspace/BrandBar.vue) 的 pulse 动画；点击 = `setCategoryAndItem('creation', 'SettingsPersona')`（保留现有逻辑）
- **Capsule**：240–400px 自适应，28px 高，`bg` 背景 + 1px `border`，rounded 16，居中。**Phase 1 留空**（不显示任何文字），无 click 无 tooltip，仅作视觉占位 + drag region 分隔
- **Chrome 三按钮**：46×48 一组（沿用 [buttons.css](../../../src/styles/buttons.css) `.aipet-chrome-btn`），但不再 `position: absolute`，作为 grid 末端 cell
- **Drag region**：avatar 与 capsule 之间、capsule 与按钮之间用空 div + `data-tauri-drag-region`，不覆盖元素

### 3.4 Sidebar 改造（删头像，加用户）

| 项 | 变化 |
|---|---|
| `brand-bar__top`（顶部 avatar 容器） | 删 |
| `brand-bar__divider` | 删 |
| 顶部 32px 让位 padding | 删（topbar 接管 drag） |
| 4 类别 icon 起始位置 | 直接顶 10px padding 开始 |
| `brand-bar__spacer` | 保留 |
| 底部 help 按钮 | 替换为用户头像 32×32 圆形 + 2px primary border |

类名沿用 `brand-bar`（语义保留），文件名可以保持 `BrandBar.vue` 或改为 `Sidebar.vue`（实现期决定，不影响 spec）。

---

## 4. 用户 Popup

### 4.1 Shell

| 属性 | 取值 |
|---|---|
| 模式 | in-workspace overlay（不是 Tauri 独立窗口） |
| 尺寸 | 880×580 居中 |
| 容器圆角 | 12px |
| 容器阴影 | `--aipet-shadow-float` |
| Backdrop | `--aipet-color-overlay`（40% 黑 / 暗色 60%） |
| 进入动效 | scale(0.96)→1 + opacity 0→1，220ms `--aipet-ease-emphasized` |
| 关闭触发 | ESC / 点 backdrop / 点 × 按钮 |
| Focus trap | 启用（首个可聚焦元素是用户区按钮） |
| 触发 | sidebar 底部用户头像点击 |

### 4.2 内部 layout

```
grid-template-columns: 240px 1fr
```

**左 240 sidebar**（三段）：
1. **User identity card**（固定）：头像 44 + 用户名 + "编辑资料"（hover 变 primary）。整块作 `<button>`，点击 = `setNav('profile')`
2. **搜索框**（固定）：rounded full + 左 search icon，placeholder "搜索设置..."。Phase 1 做客户端 nav 项过滤（5 项过滤本身不大有用，但保留视觉一致性）
3. **Nav 列表**（滚动）：3 分组（个人 / 应用 / 支持），见 §4.3

**右主区**：
- 顶部 48px header：panel 标题 + × 关闭按钮
- 滚动 content：当前选中 nav 项对应 panel，padding 24/32

### 4.3 Nav 项映射

| 一级 | 分组 | 状态 | Phase 1 内容 |
|---|---|---|---|
| 个人资料 / Profile | 个人 | ✅ 实做 | 头像 cropper（复用 [#25](https://github.com/tl0502/APET/issues/25) 已有逻辑）+ 昵称编辑（复用 `useNicknameStore`）+ 个性资料 textarea（新字段） |
| 账户 / Account | 个人 | 🔒 Disabled · badge "登录后" | 不渲染 panel，nav 项灰显不响应 |
| 数据与隐私 | 应用 | 🔒 Disabled · badge "M3+" | 同上 |
| 通知 | 应用 | 🔒 Disabled · badge "M3+" | 同上 |
| 帮助 | 支持 | ✅ 实做 | 静态 panel：GitHub 链接 + 文档地址 + 快捷键速查表 |
| 关于 | 支持 | ✅ 实做 | 搬现 [SettingsAboutPanel.vue](../../../src/panels/settings/SettingsAboutPanel.vue) 内容 |

**默认选中**：每次打开 popup 都从 Profile 开始（不记忆，因为 popup 偶发，状态记忆没意义）。

### 4.4 Profile / Account 概念区分

| | Profile（个人资料） | Account（账户） |
|---|---|---|
| 范畴 | 对外展示的用户信息 | 身份认证与安全管理 |
| 字段 | 头像 / 昵称 / 个性资料 / 基础展示信息 | 账号信息 / 登录方式 / 密码 / 邮箱手机绑定 / 2FA / 设备登录 |
| 启用条件 | 一直可用 | 登录系统起来后才有 |
| Phase 1 | ✅ 实做 | 🔒 占位 |

**重要不混淆**：当前的 [SettingsNickname](../../../src/panels/settings/SettingsNicknamePanel.vue) 内容（头像 + 昵称）属于 Profile，**不是** Account。

---

## 5. Sidebar Nav 通用规范

> 跨 workspace sidebar / popup sidebar / 未来其他 nav 共用。

### 5.1 两种结构

**类型 A · 扁平**（一级 = 一页）

```
[图标] 首页
[图标] 消息       ← active
[图标] 下载
[图标] 隐私 [M3+] ← disabled
```

- 一级 hover → 5% 黑底 + 文字升 text-1
- 一级 active → 浅 primary 底（12%） + 左 2px primary 竖条 + 文字 primary 色 + 字重 500
- 点击直接切页，无展开/收起逻辑

**类型 B · Accordion**（一级 trigger / 二级 module group）

```
▼ 账户
   ├─ 账号信息
   ├─ 密码和安全中心  ← active（整条层级线同步变 primary）
   ├─ 账号信誉
   └─ 家庭中心        ← disabled
▶ 通知
▼ 数据
   ├─ 备份与恢复
   └─ 导出
```

- 一级 = accordion trigger，图标 + 文字 + ▶ 箭头
- 点击一级 = 展开/收起，▶ 旋转 90° (180ms ease)
- 二级用纵向 1px 层级线连接（`--aipet-color-border`）
- **二级 active 时层级线整体变 primary**（重点：层级线与 active 项同步高亮，不只是 active 项本身亮）

### 5.2 三态

| 状态 | 视觉 | 交互 |
|---|---|---|
| Normal | text-2 | hover 可触发 |
| Hover | text-1 + 5% 黑底 | — |
| Active | text-1 → primary + 浅 primary 底 12% + 左 2px 竖条 + 字重 500 | — |
| Disabled | text-3 + opacity 0.5 + cursor not-allowed | 无 hover，可选 badge "M3+" / "登录后" |

### 5.3 Phase 1 落地

- 类型 A 实做（popup nav 6 项扁平）
- 类型 B **只写规范进 design doc**；不写 `.nav__item--expanded` / `.nav__children` 等 CSS class，不写 accordion Vue 组件。等账户系统起来或其他场景实际用到时一并实做（届时回到 §5.1 / §5.2 取规范）

---

## 6. 容器公约 + 色区映射

### 6.1 色区 token 绑定

| 区域 | Token | 实色（亮/暗） |
|---|---|---|
| workspace L 型框（topbar + sidebar + master） | `--aipet-color-surface-soft` | `#f5f5f5` / `#1c1c1c` |
| workspace detail 主舞台 | `--aipet-color-bg` | `#ffffff` / `#171717` |
| zone 1px 分隔 | `--aipet-color-border-faint` | `rgba(0,0,0,0.06)` / `rgba(255,255,255,0.06)` |
| `panel__title` sticky 浮玻璃 | `--aipet-color-surface-blur` + blur(12) | — |
| nav active 浅底 | `color-mix(primary, transparent, 88%)` | 浅紫 12% |
| popup backdrop | `--aipet-color-overlay` | 40% / 60% |
| popup 容器 / 主区 | `--aipet-color-bg` | 同 detail |
| popup sidebar 240 | `--aipet-color-surface-soft` | 同 workspace 框 |
| popup user identity card | `--aipet-color-bg` 浮起 + `border-faint` + 12px 圆 | — |

**两档色阶不变量**：detail / popup 主区 = bg；chrome 框 / popup sidebar = surface-soft。不引入第三档。

### 6.2 容器公约（panel.css 追加）

```css
.panel__content {
  flex: 1 1 auto;
  /* 默认 list：全宽 */
}

.panel--form .panel__content {
  max-width: 720px;
  margin: 0 auto;
  width: 100%;
}

.panel--chat .panel__content {
  max-width: 880px;
  margin: 0 auto;
  width: 100%;
}
```

**关键 ux 规范**：`panel__title` **保留 sticky 全宽 breakout**（不被 max-width 约束），让标题条像 app titlebar 一样占满列宽；只有正文 `.panel__content` 走 max-width 居中。Linear / Notion / Vercel 都是这做法。

### 6.3 Panel SFC 模板

```vue
<template>
  <section class="panel panel--form">
    <h2 class="panel__title">外观</h2>
    <div class="panel__content">
      <p class="panel__hint">...</p>
      <div class="panel__section">...</div>
    </div>
  </section>
</template>
```

### 6.4 8 panel 类型分配

| Panel | 类型 | 理由 |
|---|---|---|
| `SettingsTheme` | `--form` | 单列 radio 表单 |
| `SettingsProvider` | `--form` | 配置项表单 |
| `SettingsPersona` | `--form` | 头像编辑器 + 预设按钮（VRM 预览自管 fit） |
| `TasksReminder` | `--list` | 长列表 + 时间字段 |
| `TasksPomodoro` | `--list` | 计时器 + 历史列表 |
| `TasksTodo` | `--list` | 待办列表 |
| `ChatThreadPane` | `--chat` | 长文本对话，max-width 880 防行长过长 |
| popup `UserProfile` / `UserAbout` / `UserHelp` | `--form` | 表单/静态文档 |

### 6.5 间距 ladder

| 层 | Token | 用法 |
|---|---|---|
| `detail-col__panel` padding | `space-5 space-6` = 20/24 | 所有 detail panel 同基线（保留现状） |
| `.panel` gap | `space-4` = 16 | section 之间 |
| `.panel__section` gap | `space-3` = 12 | section 内 field/control 之间 |
| field label-input gap | 6px | label 与 input 紧贴 |
| `.panel__title` padding | `space-3 space-6` = 12/24 | sticky title |
| popup 内 padding | 24/32 | 比 detail 略宽（modal 视觉气场） |

---

## 7. Phase 计划

### Phase 1：chrome + popup + 1 示范 panel

**Issue 名**：`workspace chrome L 型框重做 + Profile popup（ADR-021 P3）`

**改动清单**：

1. **Chrome 重做** — `WorkspaceApp.vue` + `BrandBar.vue`
   - Grid layout: `48px 1fr` rows / `60px 240px 1fr` cols
   - Topbar 新增：avatar(36) + capsule + chrome 三按钮（grid cell 化，不再 absolute）
   - BrandBar 去头像 + 顶部 32px 让位 padding 删除；底部 help 替换为用户头像
   - Drag region 用空 div + `data-tauri-drag-region` 填位
2. **Tokens 公约写进 [panel.css](../../../src/styles/panel.css)**
   - 追加 `.panel__content` / `.panel--form` / `.panel--chat`
   - 文件头注释加色区映射 + 间距 ladder 速查
3. **Popup 实做** — 新增 `src/components/popup/UserPopup.vue` + 新 panel
   - `UserPopup.vue`：overlay shell + ESC/backdrop/× 关闭 + focus trap
   - `src/panels/user/UserProfilePanel.vue`：头像 cropper + 昵称 + 个性资料 textarea
   - `src/panels/user/UserHelpPanel.vue`：GitHub + 文档 + 快捷键速查
   - `src/panels/user/UserAboutPanel.vue`：搬 `SettingsAboutPanel` 内容
   - `src/panels/user/UserAccountPanel.vue` / `UserPrivacyPanel.vue` / `UserNotificationsPanel.vue`：3 个空 panel + "M3+/登录后" hint
   - 新建 `src/stores/userPopup.ts`：`isOpen` / `activeNav` / `open()` / `close()` / `setNav()`
   - Trigger：sidebar 底部 avatar 点击 → `userPopup.open()`
4. **workspace IA 简化** — [workspaceLayout.ts](../../../src/stores/workspaceLayout.ts)
   - `BRAND_BAR_ITEMS.config.masterItems` 删 `SettingsNickname` + `SettingsAbout`
   - 删除 `SettingsNicknamePanel.vue` + `SettingsAboutPanel.vue`
   - `DetailColumn.vue` 删这两 panel 的 v-show 块
5. **示范 panel** — `SettingsThemePanel.vue` 加 `panel--form` + 包 `.panel__content`

**验证**：
- `pnpm typecheck && pnpm build && cargo check` 三绿
- 手动：切类别 / 拖 sash / 打开 popup / ESC 关 / backdrop 关 / disabled 项不响应 / Profile 改昵称保存 / 暗色切换三处都对（topbar / sidebar / popup）
- 视觉对照 [scratch/01-skeleton.html](scratch/01-skeleton.html) + [scratch/02-popup.html](scratch/02-popup.html)

**估时**：1-2 个 session（类似 #33 phase B-redo 体量）。

### Phase 2：6 panel 套规范

**Issue 名**：`workspace panel 容器公约批量套用（6 panel）`

| Panel | 改动 | Subagent |
|---|---|---|
| `SettingsProvider` | + `panel--form` + 包 | ✅ |
| `SettingsPersona` | + `panel--form` + 包；复核 VRM 容器是否依赖父宽 | ⚠️ 主线手动 |
| `TasksReminder` | + `panel--list` + 包 | ✅ |
| `TasksPomodoro` | + `panel--list` + 包 | ✅ |
| `TasksTodo` | + `panel--list` + 包 | ✅ |
| `ChatThreadPane` | + `panel--chat` + message 容器 `max-width: 880px; margin: 0 auto` | ⚠️ 主线手动 |

Subagent 并行：4 个简单 panel 一次发 4 个独立 subagent（独立文件无冲突）；Persona + Chat 自己做。

**估时**：~半个 session。

### Issue / 文档协同

| 动作 | 内容 |
|---|---|
| 关 [#36](https://github.com/tl0502/APET/issues/36) | follow-up 收口：chrome 三按钮已落地，但顶栏整体改为 L 型框（本次 Phase 1 覆盖） |
| 新建 issue A | Phase 1 任务 |
| 新建 issue B | Phase 2 任务 |
| [decisions.md](../../decisions.md) | 新增 ADR：sidebar nav 通用规范（扁平 + Accordion 三态） |
| [desktop-ui-principles.md](../../design/desktop-ui-principles.md) | Updated 段：补"两档色阶 + 三类容器" |
| ADR-021 | Updated 段：P3 引入 topbar 实色 L 型框 + popup 用户层 |
| [STATUS.md](../../STATUS.md) | #29 / #21 / #23 排队挪到 Phase 1 之后 |

---

## 8. 风险与已知坑

| 风险 | 说明 | 对策 |
|---|---|---|
| VRM 预览容器 max-width 后塌成方块 | `SettingsPersona` 的 VRM canvas 可能依赖父宽自适应 | Phase 2 先单独测，必要时 `panel__content` 内嵌固宽 VRM 容器 |
| `ChatThreadPane` sticky composer 在 `panel__content` max-width 后定位基准变化 | composer 是 floating 浮卡，依赖 `.detail-col__chat-pane` 宽度 | Phase 2 手动验证，必要时 composer 容器走全宽 + 内部 max-width 880 |
| 老用户 KV 残留旧 panel id | `workspace:item_per_category` 可能存有 `SettingsNickname` / `SettingsAbout` | `workspaceLayout.loadFromKv()` 已有 `knownItemIds` 过滤逻辑，会自动回 default fallback，无需新代码 |
| topbar `data-tauri-drag-region` 与 avatar/capsule 点击冲突 | drag region 覆盖按钮时按钮点不动 | 用空 div 填 drag 槽，元素本身不设 `data-tauri-drag-region`（与现 BrandBar `__btn z-index: 6 > drag-bar 5` 同思路） |
| `UserProfilePanel` 整合现有 `useAvatarsStore` + `useNicknameStore` 可能有 store 状态污染 | popup 关闭再开时，本地编辑未保存的字段会丢 | Phase 1 验证：编辑中关 popup → 重开 → 字段是否回到 store 值；如需草稿态走 popup-local state |

---

## 9. 附录

### 9.1 设计原则呼应

本次重设计严格对齐 [desktop-ui-principles.md](../../design/desktop-ui-principles.md)：

- §1 多窗 ≠ 单页路由：popup 是 in-workspace overlay 而非新 Tauri 窗（用户级偶发功能，不参与磁吸 / 不需 AOT / 不长期可见，符合"判定标准"）
- §2 表面分层：L 型框 + 主舞台 + popup 浮层，三层清晰
- §3 不依赖系统圆角：popup 容器自绘 12px 圆角
- §4 桌面级交互：ESC 关 popup / 拖 sash / 全局快捷键留给 Phase 1 之后
- §6 动效：popup 进入 220ms scale + opacity，nav 切换 120ms，accordion 展开 180ms

### 9.2 Mockup 文件

- [scratch/01-skeleton.html](scratch/01-skeleton.html) — 整窗骨架（L 型框 + 三列 + 48px topbar）
- [scratch/02-popup.html](scratch/02-popup.html) — 用户 popup 打开状态（Profile 视图）
- [scratch/03-nav-spec.html](scratch/03-nav-spec.html) — sidebar nav 规范（扁平 vs Accordion + 三态）

Mockup 都支持亮/暗切换。Phase 1 实做时以 mockup 为视觉对照。

### 9.3 Brainstorm 决策痕迹

| 时间 | 决策点 | 结果 |
|---|---|---|
| 2026-05-21 | D1: capsule 用途 | 占位，最后再定 |
| 2026-05-21 | D2: 容器规则 | 按类型分（form/list/chat） |
| 2026-05-21 | D3: chrome 视觉 | 实色 L 型框 |
| 2026-05-21 | D4: 节奏 | 分两段 |
| 2026-05-21 | D5: popup IA | Restructure |
| 2026-05-21 | D6: accordion 范围 | 仅规范 + Phase 1 扁平 |
