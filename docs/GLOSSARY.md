# 术语表（Glossary）

项目高频术语集中定义。文档内首次出现术语应链接到此处。

---

## 项目代号与阶段

- **AIPET** — 项目代号（AI Desktop Pet）。
- **W0 立项准备** — MVP 启动前的决策与脚手架准备阶段，不算独立 milestone。
- **M1 / M2 / M3 / M4 / M5** — MVP 实施期里程碑：
  - **M1**：壳层 + 对话（W1-W2）。
  - **M2**：任务三件套 + 物理交互（W3-W4）。
  - **M3**：记忆 + 主动陪伴（W5-W6）。
  - **M4**：装扮 + 声音 + 纪念日（W7-W8）。
  - **M5**：小游戏 + 自测（W9-W10）。
- **P1-R1 / P1-R2 / P1-R3** — 上线后扩展轮次（M6-M7 / M8-M9 / M10+）。

## 三引擎差异化

- **用户自主人格** — `.soul.md` 文件完全归属用户，可读、可编辑、可导出。
- **主动陪伴** — 桌宠基于本地空闲信号（`GetLastInputInfo` + RAWINPUT）触发主动关心，不读屏幕内容。
- **共同活动** — 物理交互、装扮、声音表情、本地 + LLM 小游戏。

## 核心数据对象

- **`.soul.md`** — 人格定义文件（Markdown 格式），含人物设定、tone_profile、离线模板、调侃池、反应配置等区段。详见 [persona/persona-design.md](persona/persona-design.md)。
- **`tone_profile`** — 人格语气画像（如温暖度、严厉度、活泼度等）。
- **`reaction_table`** — 物理交互响应表，把 hitbox 映射到 12 个核心动作 ID 之一；可被 `.soul.md` 覆盖。
- **`pet_runtime_state`** — 桌宠运行时状态（心情 mood / 精力 energy / 当前动作）。
- **`UnlockSpec`** — 装扮解锁条件 schema，支持 6 种 kind：`always` / `milestone` / `date_range` / `purchase` / `gift` / `user_upload`。

## 安全与合规

- **安全前缀**（safety prefix）— 系统级 system prompt 前缀，含通用核心（全球 5 条）+ 地区补充（`zh-CN` / `international`）；不可被人格、不可被游戏指令覆盖。详见 ADR-006。
- **灵魂宣誓**（soul pledge）— Onboarding Step 1，由"默默 momo"以第一人称叙述的隐私同意页；信息完整性等价于传统隐私页。详见 ADR-008。
- **杀死指标**（kill metric）— 历史叙事，单人项目已废除（详见 [CHANGELOG.md](CHANGELOG.md) 2026-05-05）。

## 模块编号（PRD §7）

- **A 桌宠壳层** / **B 对话** / **C 提醒** / **D 番茄钟** / **E 待办** / **F 记忆**
- **G 设置** / **H 人格系统** / **I 生命感** / **J 情境关心** / **K 摸鱼模式** / **L 文件拖入**
- **M 灵魂宣誓** / **N 物理交互** / **O 装扮** / **P 声音表情** / **Q 小游戏**
- **U.1 / U.2 昵称**（并入 F）/ **R.3 桌宠日常时段**（并入 I）/ **S.4 用户纪念日**（并入 MilestoneService）

## 三形态对话面板（v1.1，依 ADR-015）

- **形态 1 hub 总面板** — 独立 Tauri 窗口（1024×680，4 tab：对话 / 工坊 / 设置 / 游戏 launcher），M4 上线。
- **形态 2 磁吸浮窗** — 独立 Tauri 窗口（默认 380×480），可吸附桌宠或自由摆放，M1 极简版（B.3.a） → M2 完整版（B.3.c）。
- **形态 3 漫画气泡** — 角色窗内子组件，M5 上线（B.3.f）。
- **ConversationStore** — view-agnostic 数据层，三形态共享同一 conversation 数据。

## 关键 KPI 编号（埋点 §11）

- **11.1-11.6** 留存与活跃 + 任务完成
- **11.7-11.9** 人格自主权
- **11.10-11.12** 主动陪伴 + 跨日打卡 + 生命感关闭率
- **11.13-11.14** 文件交互 + 摸鱼模式
- **11.15-11.20** 装扮 + 声音 + 游戏 + 纪念日 + 上线总指标

## 性能预算关键词

- **常驻内存** ≤ 250MB
- **安装包** ≤ 80MB
- **冷启动** ≤ 5s
- **对话首 token** p50 ≤ 1.5s
- **物理交互响应** < 100ms
- **装扮切换** < 500ms
- **声音播放延迟** < 50ms
- **本地游戏每轮** < 50ms
- **摸鱼切换** < 200ms

## 技术栈

- **Tauri 2.x** — 桌面框架（Rust 主进程 + WebView2 前端）。
- **VRM** — `@pixiv/three-vrm` 渲染 3D 桌宠模型；通过 humanoid bone 挂载配饰。
- **DPAPI** — Windows Data Protection API，加密 secrets，绑定用户账户。
- **WAL** — SQLite Write-Ahead Logging 模式。
- **GameRoom** — 独立 Tauri 窗口（480×600），承载所有 5 个游戏（本地 3 + LLM 2）。
- **ChatService / PersonaService / MemoryService / NicknameService** — 主进程核心服务，详见 [architecture/system-architecture.md](architecture/system-architecture.md) §3。

## 决策记录术语

- **ADR-NNN** — 决策编号，全部记录在 [decisions.md](decisions.md)。
- **Supersedes** — 标记一个决策被后续决策覆盖；旧决策不删除，原条目末尾追加引用。

## 文档分级

- **Baseline**（基线）— 实施期权威源，PRD / 架构 / flows / UAT / 人格 / 路线图。
- **Decisions**（决策记录）— 单文件 `decisions.md`，记录关键技术与产品决策。
- **Research**（研究）— 立项基线，无版本号。
- **Archive**（归档）— v0.x 历史版本，仅供回溯。
