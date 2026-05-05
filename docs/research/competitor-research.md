---
title: AI 桌宠项目竞品调研记录
updated: 2026-05-05
related:
  - ../requirements/prd.md
---

# AI 桌宠项目竞品调研记录

> 本文档为产品立项基线，无版本号。后续如需迭代，按 [WORKFLOW.md](../WORKFLOW.md) 升版本号。

- 记录日期：2026-04-30，最近更新：2026-05-05
- 项目：AI 桌宠（个人开发）
- 目的：对"桌宠 + AI 陪伴 + AI 效率助手"赛道做竞品调查，形成可执行的定位与 MVP 输入。
- 范围说明：本项目为小范围使用，不进入公开分发，故不在本文档讨论合规、未成年人保护、内容边界等限制；仅在涉及竞品差异化时作为外部信号引用。

## 1. 调研范围与方法

### 1.1 范围
- 桌面宠物 / 桌面角色（Desktop Pet / Desktop Mascot）
- AI 陪伴（Companion）
- 桌面 AI 助手（效率与系统层入口）

### 1.2 方法
- 一手信息源：官网、应用商店、Steam、GitHub、官方公告、主流科技媒体
- 优先采纳可验证项：平台、功能、上线状态、定价、更新节奏
- 时间口径：截至 2026-05-05（美国东部时间）

## 2. 一页结论（Summary）

1. 桌宠形态已被 Steam 生态显著验证有付费空间（Desktop Mate 系列 IP 联营路线奏效）。
2. AI 桌宠真正进入加速期：**OpenClaw 框架** 60 天内拿到 250k+ GitHub Star，成为 Clawster / PetClaw / Molty 共同底座。
3. 大厂首次直接下场：**OpenAI Codex Pets** 把 Tamagotchi 风格桌宠作为后台任务回执通道，与本项目核心定位高度重合。
4. 桌面 AI 助手（**Copilot Vision** 转正、**Amazon Quick** macOS/Win Preview）快速吃下"看屏问答"价值，但 Copilot 主动放弃"代用户操作"，留下明显空缺。
5. 纯聊天陪伴（Replika / Character.AI / Nomi）规模与心智已成型，不适合正面竞争。
6. **AI 桌宠的机会**：以"宠物在场感"做拉新，以"代你点 + 任务闭环"做留存——这是 Copilot Vision 让出的位、Amazon Quick 偏 to-B 让出的 to-C 位。

## 3. 竞品分层地图

### 3.1 A 层：直接竞品（AI 桌宠 / 桌面伴生角色）

| 产品 | 状态 (2026-05) | 平台 | 关键能力 | 差异点 / 风险 |
|---|---|---|---|---|
| **OpenClaw**（框架） | v2026.5.1-beta / 5.2 / 5.3 活跃迭代 | 跨平台 | 绑 ChatGPT Plus 直接用 GPT-5.4（无 API 费）/ Grok 4.3 默认 / 扩 Discord / Teams / WhatsApp / LINE / Twitch 集成 | 生态护城河；下游桌宠都在其上 → 必须先回答"是否合作 / 是否绕开" |
| **Clawster** | 活跃，依赖 OpenClaw | macOS 15+ | `Cmd+Shift+Space` 唤起 / `Cmd+Shift+/` 截屏问答 / 12 状态 / 完全本地网关；当前免费 | 形态最接近本项目；体验高度依赖 OpenClaw；单平台 |
| **PetClaw AI** | 2026-03-20 SF 发布，公测中 | macOS / Windows | 一键安装、语音、跨 App 任务自动化（Gmail / WhatsApp / Discord / GitHub）、Skill Store、Telegram 远控、长期记忆 | 已具备"陪伴 + 执行"双轮；Skill Store 是最大威胁；定价未公开（Beta 内可申请额度） |
| **OpenAI Codex Pets** | 已上线 | macOS（Codex 内） | Tamagotchi 风格 + Dynamic Island 风格的后台任务可视化回执 | **大厂入场，分发优势压倒性；与"陪你完成事情"主张正面冲突** |
| **Molty** | OpenClaw 原生使用方 | macOS | 太空龙虾 IP，AI 助手 | OpenClaw 起家产品，IP 化路线参照 |
| Your Friendly AI | 早期 | Windows | Ollama / OpenAI / Claude 多后端可切换 | 多模型可插拔，但产品节奏慢 |
| YCamie / Shimeji AI | 持续运营 | 桌面 | AI 生成桌宠角色 | 偏内容生成、非助手形态 |
| PopClaw | Kickstarter 预告 | 独立硬件 | 离线 / 开源 | 硬件路线，时间线长 |
| Mac Pet 系列 | 2026 测评热度高 | macOS | 内容化角色合集 | 弱 AI、强 IP，验证"皮肤即留存"心智 |

### 3.2 B 层：强替代（AI 陪伴）

| 产品 | 平台 | 优势 | 对 AI 桌宠的威胁 |
|---|---|---|---|
| Replika | iOS / Android / Web / VR | 陪伴关系与长期记忆产品化成熟 | 抢占情感陪伴心智与付费预算 |
| Character.AI | Web / iOS / Android | 角色生态、用户规模、UGC 角色 | 抢占泛人设聊天与角色时长 |
| Nomi | iOS / Android / Web | 高沉浸关系陪伴、口碑较好 | 抢占高粘性陪伴需求 |

> 行业信号（仅观察）：Character.AI 已对未成年关闭开放式聊天并设立独立 AI Safety Lab，FTC 同步向 7 家厂商发函。说明纯聊天赛道的品牌成本越来越高，间接印证"陪伴 + 执行"路线更稳健。

### 3.3 C 层：生态挤压者（桌面 AI 助手）

| 产品 | 最新动态 (2026-05) | 对 AI 桌宠的边际影响 |
|---|---|---|
| **Microsoft Copilot Vision** | Preview 转正；任务栏 "Share with Copilot" 默认开启（KB5072033 / Build 26200.7462+）；非 Copilot+ PC 也可用；`Win+Esc` 一键终止；**明确"看屏 + 回答，不代用户操作"** | 系统级看屏问答价值被吃掉；但 Copilot 主动放弃 taking actions → **这是本项目可独占的差异点** |
| **Amazon Quick (Desktop)** | 2026-04-28 macOS / Win Preview；本地文件直读、OS 级主动通知、原生桌面控制、内置 MCP（可挂 Kiro CLI / Claude Code）、个人知识图谱、后台监听邮件/日历/Salesforce；客户：3M、GoDaddy、AstraZeneca、NY Life、Mondelēz；当前仅 US East | 把"全工作流主动助理"卡位提前；偏企业起家，**to-C + 情感陪伴侧仍空白** |
| Microsoft 入口收缩 | 2026-03-20 从 Notepad / Snipping Tool 撤掉 Copilot 品牌 | 印证"少而精"是当前桌面 AI 主流叙事，本项目应避免功能堆叠 |

## 4. 关键观察

### 4.1 用户不只要"可爱"，更要"有用"
- 桌宠外形是点击与下载驱动力，但留存依赖任务完成率和打扰控制。
- 仅聊天或仅动画的产品会快速同质化。

### 4.2 本地优先是工程基本盘
- Clawster / PetClaw / OpenClaw 全员主打 local-first。
- 即便是个人项目，默认本地存储 + 记忆可视化 + 可清除 + 可导出仍是最低工程要求——未来想换设备 / 换模型 / 换桌宠形象时一定会用上。

### 4.3 大厂下场让"分发"不再可能赢——只能用"形态 + 关系"赢
- OpenAI Codex Pets 把桌宠作为 Codex 任务的可视化通道；Amazon Quick 把"主动通知"做成卖点。
- 本项目唯一能拉开差距的是：**长期记忆的人格化** + **代你点的执行闭环** + **本地优先的私密性**。

### 4.4 IP / 皮肤路线被验证
- Desktop Mate 在 2026 H1 密集上架 Kagamine Rin（01-27）、Sanrio 系列（03-04）、Kotonoha 姐妹（04-24）、SNOW MIKU 2026 Ver.，并预告"年内 10+ IP 入驻"。
- 即便自用，建议 MVP 即预留角色 / 皮肤接口，避免后期重构。

### 4.5 "主动通知"是新战场
- Amazon Quick、Codex Pets 都押注"主动告诉你后台进展"。
- 桌宠形态天然适配（在场感 + 可表演动作），但失败的打扰策略会让自用者两周内卸载——MVP 必须包含可调节的打扰策略。

### 4.6 OpenClaw 是必须先回答的架构题
- **依赖 OpenClaw**：白嫖 GPT-5.4（绑 ChatGPT Plus 即用，无 API 费）、现成集成生态、社区贡献。代价是与 Clawster / PetClaw 同质化、跟随框架方向波动。
- **绕开自研**：拥有差异化护城河。代价是时间窗紧（PetClaw AI、OpenAI Codex Pets 都已在跑）、模型成本与集成都要自己扛。
- 该题需要在 M0 之前定。

## 5. 对 AI 桌宠项目的定位启发

### 5.1 一句话定位
> **"会代你点的桌面 AI 宠物"**

- 对标 Copilot Vision 主动让出的 taking actions 空缺。
- 对标 Amazon Quick to-B 偏向让出的 to-C + 情感陪伴空缺。
- 对标 PetClaw AI 已在跑的 Skill Store，但聚焦"代点击 + 在场感"而非"全工作流"。

### 5.2 核心主张
- `陪伴感` × `执行力`，而非纯聊天，也不复制全工作流助理。

### 5.3 MVP 优先场景（建议 3 个可验证任务）
1. **截屏 → 找到按钮 → 桌宠主动点击** —— 直接拉开与 Copilot Vision 的差距（Vision 看得懂，但不会动手）。
2. **任务到点 → 桌宠主动跳出处置（番茄钟 / 待办拆分 / 复盘）** —— "在场感 × 主动通知"的桌宠化表达。
3. **邮件 / 消息待回 → 桌宠主动起草并请求一次确认** —— 对标 Amazon Quick 的主动型工作流，但走"宠物代办"语气，而不是企业助理。

辅助底座：个性化记忆（称呼、偏好、固定作息、常用模板）作为 1+2+3 的共享上下文。

### 5.4 应避免的方向
1. 先做复杂 3D 大世界，弱化核心任务能力。
2. 只做聊天壳子，不做系统级快捷动作。
3. 记忆黑盒化，不提供可视化与可控删除。
4. 试图复制 Amazon Quick 的"全工作流"——对个人项目过重。

## 6. 风险清单

1. **OpenClaw 单点依赖风险**：若选择基于 OpenClaw，框架方向变更或商业化策略变动会直接波及本项目；与 Clawster / PetClaw 同质化。
2. **大厂下场风险（P0）**：OpenAI Codex Pets 是大厂首次切入"桌宠 + 任务可视化"赛道，必须以"local-first + 代你点 + 长期记忆人格化"做隔离。
3. **产品重心失衡风险**：陪伴与执行权重失衡，要么沦为聊天皮肤，要么沦为没有人格的 Quick 仿品。
4. **打扰策略风险**：主动通知是新战场（Amazon Quick / Codex Pets 都押注），桌宠形态天然适配，但失败的打扰策略会让用户两周内放弃。
5. **能力封顶风险**：本地优先 + 个人项目算力 → 高质量截图理解 / 实时屏幕感知模型选型需要在 M0 锁死。
6. **执行层权限风险**："代你点"涉及鼠标 / 键盘 / 系统 API 钩子，不同 OS 的可达性与稳定性差异大，跨平台代价需提前评估。

## 7. 下一步建议（用于 PRD 输入）

1. **OpenClaw 路线决议**：deadline ≤ 2026-05-15，给出"基于 OpenClaw"和"自研"两条路线的 MVP 时间表与成本对比，再决策。
2. **竞品矩阵 v1**：本表沉淀为可月度更新的追踪表，重点跟 PetClaw AI、OpenAI Codex Pets、Amazon Quick。
3. **定位声明 v1**：目标用户（自用 / 极客）、核心场景、成功指标。建议自用版以"周内主动调用次数 / 成功代点率 / 记忆命中率"为指标，而非传统留存。
4. **MVP 功能边界**：必须做（5.3 三个任务 + 记忆底座 + 打扰策略）；可延后（皮肤接口、IP 角色、跨平台）；明确不做（全工作流、纯聊天、3D 世界）。
5. **架构草案**：本地存储 + 模型可插拔（OpenAI / Claude / Ollama 三选一可配）+ 截图理解模型 + 一个能"代点击"的执行层（含一次性确认 UX 防误触）+ 可视化记忆面板。

## 8. 调研来源（Links）

### 桌宠 / Steam 生态
- Desktop Mate (Steam)：https://store.steampowered.com/app/3301060/Desktop_Mate/
- Desktop Mate Snow Miku 2026 Ver.：https://store.steampowered.com/app/4018720
- Desktop Mate Kagamine Rin DLC：https://store.steampowered.com/app/4018690/
- Shijima / Desktop Mate site：https://getshijima.app/

### 直接竞品（A 层）
- Clawster：https://clawster.pet/
- OpenClaw 官方：https://clawd.bot/
- OpenClaw GitHub：https://github.com/openclaw/openclaw
- OpenClaw Releases（v2026.5.x）：https://github.com/openclaw/openclaw/releases
- PetClaw：https://petclaw.ai/
- PetClaw Pricing：https://petclaw.ai/pricing
- PetClaw AI 公测公告（Yahoo Finance, 2026-03）：https://finance.yahoo.com/sectors/technology/articles/petclaw-ai-launches-autonomous-desktop-130000356.html
- PetClaw AI 公测公告（Manila Times, 2026-03-20）：https://www.manilatimes.net/2026/03/20/tmt-newswire/globenewswire/petclaw-ai-launches-autonomous-desktop-ai-companion-for-247-productivity/2304467
- PetClaw AI Review（Medium, 2026-03）：https://medium.com/@eddyenos1/petclaw-ai-review-2026-the-best-desktop-ai-assistant-that-runs-locally-a6517ff15f66
- 9to5Mac（OpenAI Codex Pets, 2026-05-01）：https://9to5mac.com/2026/05/01/i-think-i-just-vibe-coded-lil-finder-guy-onto-my-mac/
- Your Friendly AI：https://www.yourfriendly.ai/
- YCamie / Shimeji AI：https://www.shimeji.ai/
- PopClaw：https://www.popclaw.ai/

### 陪伴产品（B 层，仅作信号引用）
- Replika：https://replika.ai/
- Character.AI：https://character.ai/
- Nomi：https://nomi.ai/

### 桌面 AI 助手（C 层）
- Microsoft Copilot Vision 支持文档：https://support.microsoft.com/en-us/topic/using-copilot-vision-with-microsoft-copilot-3c67686f-fa97-40f6-8a3e-0e45265d425f
- Tom's Guide：Copilot Vision 在 Windows 11 中的入口与开关（2026）：https://www.tomsguide.com/ai/microsoft-is-hiding-windows-11s-eyes-heres-how-to-find-copilot-vision-and-fully-delete-it
- Windows Latest：任务栏 Share with Copilot（2026-01）：https://www.windowslatest.com/2026/01/03/microsoft-wants-to-let-you-share-app-windows-with-copilot-right-from-the-windows-11-taskbar/
- Microsoft 365 Copilot 2026-04 What's New：https://techcommunity.microsoft.com/blog/microsoft365copilotblog/what%E2%80%99s-new-in-microsoft-365-copilot--april-2026/4510935
- Copilot Vision Desktop Share（2025-07-15）：https://blogs.windows.com/windows-insider/2025/07/15/copilot-on-windows-vision-desktop-share-begins-rolling-out-to-windows-insiders/
- Amazon Quick（macOS/Windows Preview, 2026-04-28）：https://aws.amazon.com/about-aws/whats-new/2026/04/amazon-quick-macos-windows-preview/
- Amazon Quick Desktop 下载页：https://aws.amazon.com/quick/desktop/
- About Amazon：Quick Desktop 介绍：https://www.aboutamazon.com/news/aws/amazon-quick-desktop-ai-assistant
- SiliconANGLE：Quick 转为主动型助理（2026-04-28）：https://siliconangle.com/2026/04/28/amazon-revamps-quick-proactive-desktop-app-gets-work-done/

## 9. 备注

- 本文档为个人项目立项调研，不进入对外分发，因此不包含未成年人保护、内容安全边界、监管合规等章节。
- 部分商店评分、评论数、DLC 数量为调研当日观察值，会随时间变化。
- A 层竞品（PetClaw AI、OpenAI Codex Pets、Amazon Quick）建议月度复查。
