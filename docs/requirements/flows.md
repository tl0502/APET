---
title: AI 桌宠 流程与状态机
updated: 2026-05-05
related:
  - ./prd.md
  - ../architecture/system-architecture.md
  - ../persona/persona-design.md
  - ../decisions.md
---

# AI 桌宠 流程与状态机 v1.1

- 适用阶段：MVP **实施期**（M1 起作为唯一权威流程源，与 PRD / 架构 / 人格基线对齐）。
- 关联：15 项 ADR（详见 [decisions.md](../decisions.md)）。

> **关于本版本**：v1.0 是 v0.3 → v0.6 + M0 14 项 ADR 决策结果的"压平基线"，文档以连续叙事呈现实施期完整流程；v1.1 是 v1.0 之上的章节级增量（ADR-015 决策落地），不再压平。历史版本演化见 [CHANGELOG.md](../CHANGELOG.md)。

## 变更摘要

### v1.1（2026-05-02）

实施期 M1 D3 经 ADR-015《对话面板三形态架构》Accepted 后增量更新：

- §2 快捷唤起对话流：加形态选择分支
- 新增 §2.2 形态切换流（形态 2 ↔ 形态 3 数据保留）
- 新增 §2.3 磁吸状态机（吸附 ↔ 断开）
- 原 §2.1 失败分支编号不变

未变更：§1 / §3-§23 与 v1.0 一致。

## 1. 首次启动 Onboarding(6 步)

### 1.1 触发条件

本地无配置文件 / 配置版本不识别 / 用户主动重置。

### 1.2 引导流程

```
启动
 ↓
[检测本地配置]
 ↓
 ├── 存在 → 进入主态
 └── 不存在 ↓
        Step 1:灵魂宣誓页(替代传统隐私同意,ADR-008)
        ├── 加载内置默认人格"默默 momo"的形象 + 第一人称文案
        ├── 文案核心(温暖叙述版 v1.0):
        │   - "我的记忆只在你电脑里"
        │   - "联网聊天前我会告诉你"
        │   - "截图、剪贴板默认是关的"
        │   - "这些不是法务条款,是我的承诺"
        ├── 右下角"查看完整数据策略"链接 → 模态展开正式版条款 v1.0
        ├── 底部按钮:"我懂了,一起开始" / "再看一眼条款" / "退出"
        └── 用户点"我懂了" → 视为同意:写入 consent.granted=true + consent.method='soul_pledge' + consent.version=1
        ↓
        Step 2:选择人格
        ├── 展示 3 个内置人格卡片(默默 / 阿吉 / 教官)
        ├── 用户选择 → 设为 active persona
        └── 跳过 → 默认"默默 momo"
        ↓
        Step 3:确认快捷键
        ├── 默认 Ctrl+Alt+Space(对话)/ Ctrl+Shift+B(摸鱼)
        ├── 探测当前快捷键是否被占用 → 占用则提示改键
        └── 用户确认 / 改键
        ↓
        Step 4:选择默认提醒模板
        ├── 多选:喝水 / 久坐 / 学习专注 / 不需要
        ├── 选中后批量创建提醒(用户可随时关)
        ↓
        Step 5:第一次番茄钟(可选)
        ├── 一句话引导:"要不试 5 分钟微专注?"
        ├── 用户接受 → 启动 5min 番茄钟
        └── 跳过 → 继续
        ↓
        Step 6:第一次唤起对话(可选,不强制 API Key,ADR-005)
        ├── 显示快捷键浮窗 3 秒
        ├── 用户按下快捷键 → 弹对话框 → 引导发第一句
        │   └── 若未配置 API Key,首次发送时弹"还差最后一步"引导,提供 6 个 preset
        └── 5 秒未操作 → 跳过
        ↓
[创建默认配置 + 标记 onboarded=true]
 ↓
[进入主态,桌宠出现,状态 IDLE]
```

### 1.3 等价性保证

- "我懂了"按钮在数据库与日志中等价记录为 `consent.granted=true`,附带 `consent.method='soul_pledge'`、`consent.version=1`。
- 文案版本号在立项期敲定后写入应用资源(`assets/onboarding/soul_pledge_v1.txt`),**不可在线热更**。
- 法务变更 → 版本号 +1 → 老用户启动时检测 mismatch → 弹"内容已更新"提示 → 用户必须再次确认才能进入主态。

### 1.4 失败分支

| 失败点 | 处理 |
|---|---|
| 本地库初始化失败 | 重试 1 次 → 提示 + 显示 logs 路径 → 退出 |
| 快捷键探测失败(系统拒绝注册) | Step 3 提示用户尝试其他组合,至少必须注册成功一组 |
| VRM 资源加载失败 | 降级为静态 PNG 立绘 + 上报错误 |

### 1.5 中途退出

> **Updated 2026-05-12（ADR-019 superseded）**:原"重头开始"策略已替换为"续接 + 用户选"。
>
> KV `onboarding:current_step` 在每次 advanceStep 前写入（config 表），onboarding_complete 时删除。任意 Step 关窗 → 进程退出（Alt+F4 / 系统关闭都 `app.exit(0)`）→ 下次启动期 `consent::check_version=Match` 但 KV 仍存在 → 启动 onboarding 窗 → 前端 onMounted 弹「继续 X / 重来 / 退出」三选模态。
>
> 「继续」= 跳到 saved step；「重来」= 清 KV + 跳回 Step 1（不动 consent.granted，避免 UX 流程误改合规标记）；「退出」= app.exit(0)。
>
> 详见 [decisions.md ADR-019](../decisions.md#adr-019-onboarding-进度持久化与续接)。

## 2. 快捷唤起对话流

> v1.1 注:对话采用 3 形态共存架构(形态 1 hub / 形态 2 磁吸浮窗 / 形态 3 漫画气泡,见 ADR-015,详见 [decisions.md](../decisions.md));本节主流默认形态 2;形态切换见 §2.2;磁吸状态机见 §2.3。

```
[全局快捷键监听 Ctrl+Alt+Space]  ↓ 命中
[主进程检查桌宠状态]
 ├── ONBOARDING / FIRST_RUN → 忽略,提示稍后再试
 ├── UPDATING → 忽略,提示更新中
 └── 其他 → 继续  ↓
[弹出 chat 面板(形态 2),定位在桌宠附近,聚焦输入框]  ↓
[用户输入]
 ├── ESC / 失焦 / 点击外部 → 关闭面板,不写入消息
 ├── Enter → 发送
 └── Shift+Enter → 换行  ↓
[ChatService 拼装 prompt]
 ├── 安全前缀(v1.0 + 地区补充)+ 当前人格 system prompt + 记忆摘要(含 username) + 历史 + 本轮输入  ↓
[判断网络状态]
 ├── 在线 → LLMProvider.chat_stream
 │   ├── 流式返回 chat.token → 前端逐字渲染
 │   ├── 完成 → chat.done
 │   ├── 超时(> 8s 无 token)→ 切离线规则
 │   └── 用户取消(再按快捷键 / 点取消)→ chat.cancel
 └── 离线 → 命令词解析 → 命中?
     ├── 是 → 直接执行(如"开始番茄钟")
     └── 否 → 情感分类 → 命中人格离线模板池 → 返回模板  ↓
[SecurityGuard 二次扫描]
 ├── 命中违禁 → 替换为人格 ## 拒答 池或全局兜底
 └── 通过 → 输出  ↓
[写入消息记录(messages 表,含 conversation_id)]  ↓
[更新对话视图(当前形态)]
```

### 2.1 失败分支

| 分支 | 处理 |
|---|---|
| 快捷键注册失败 | 启动期提示用户改键 |
| LLM 返回错误(401/429/500) | 显示错误代码 + 引导设置页 / 稍后重试 |
| LLM 超时无 token > 8s | 自动降级到离线规则模式 |
| 流式中网络中断 | 已收 token 保留,错误提示,整条消息状态标 partial |
| API Key 解密失败 | 提示用户重新设置 API Key |
| API Key 未配置(首启) | 弹"还差最后一步"引导,提供 6 个 preset(ADR-005) |

### 2.2 形态切换流(v1.1 新增,ADR-015)

```
[当前形态 2 chat 窗活跃]  ↓
[用户按下控制按钮区"形态 3"按钮]  ↓
[ChatPanelView2 → hide(保持窗口 alive,不 destroy)]
[ChatPanelView3 → mount 在角色窗内,渲染同 conversation_id 的最近 N 条]
[active_conversation_id 不变,数据保留(ConversationStore view-agnostic)]
                                ↓
                       (用户在形态 3 继续对话)
                                ↓
[用户按 ESC / 控制按钮区"返回形态 2"]  ↓
[ChatPanelView3 → unmount]
[ChatPanelView2 → show(恢复)]
```

形态 1(hub)与形态 2/3 关系:

- 用户从托盘打开 hub → hub 对话 tab 默认显示当前 active_conversation_id 的消息流
- 用户在 hub 切换到其它 conversation → active_conversation_id 跟随更新 → 形态 2 chat 窗(若打开)同步切换
- 同步语义详细决策(选 A / B / C)见 ADR-015 TBD-3,M4 启动 B.3.e 前拍板

### 2.3 磁吸状态机(v1.1 新增,M2 B.3.c,ADR-015)

```
[初始 chat 窗显示]  →  [状态 A 吸附]
                        │
                        ├── 黏在角色窗右/下/左/上(自动选屏幕剩余空间最大边)
                        ├── 大小由角色窗决定(等高或等宽)
                        ├── 角色窗移动 → chat 窗跟随
                        └── 用户拖动 chat 窗  ↓
                            ├── 距角色窗 < 阈值(Q4 TBD,候选 30/50/80 px)→ 保持吸附,跟随移动
                            └── 距角色窗 > 阈值 → 切到状态 B 断开
                                ↓
                          [状态 B 断开]
                                ├── 自由大小(min 320×360 约束)
                                ├── 自由位置(用户拖动)
                                ├── 持久化坐标到 user_state.chat_panel_pos
                                └── 用户拖回角色窗附近 < 阈值 → 自动吸附(异磁极感),回状态 A

[chat 窗失焦(任意状态)]  →  收缩到角色下方"控制按钮区"(§7.1.1 PRD)
                                ├── 记忆当时是 A 还是 B 状态
                                └── 再唤起(Ctrl+Alt+Space / 控制按钮区某按钮)→ 恢复失焦前状态
```

物理阈值 Q4 与异磁极动效(0.2s 滑入 vs 硬贴)在 M2 W3 启动 B.3.c 前拍板。

## 3. 提醒闭环(并发优先级)

### 3.1 创建到触发

```
[用户创建提醒]
 ├── 一次性 / 每日 / 每周 / cron
 └── priority: soft(默认) / hard  ↓
[Scheduler 计算 next_fire_at]  ↓
[到点]  ↓
[查询当前桌宠状态]  ↓
 ├── 当前是 FOCUS 且 priority=soft
 │   └── 进入"软提醒缓冲"(不打断专注)
 │       └── FOCUS 结束(→ REST)时合并展示所有缓冲软提醒
 ├── 当前是 FOCUS 且 priority=hard
 │   └── 立即进入 REMIND 状态(强行打断专注,番茄钟暂停)
 ├── 当前是 BOSS_KEY_HIDDEN(摸鱼)
 │   └── 缓冲到 boss_key_pending_reminders 队列(软硬都缓冲),恢复时合并提示
 └── 其他状态(IDLE / REST / REMIND / IN_GAME)
     └── 立即进入 REMIND 状态
        ├── IN_GAME 期间 → 在游戏舱内通知展示,不打断 LLM 流式渲染
        └── 其他 → 普通通知 + 桌宠动作 + 内联消息  ↓
[用户响应]
 ├── 完成 → snooze_count = 0 → 写历史 → REMIND → IDLE
 ├── 稍后 10min → snooze_count + 1
 │   ├── snooze_count > 3 → 标记 overdue + 写复盘列表 → REMIND → IDLE
 │   └── 否则 → next_fire_at = now + 10min → REMIND → IDLE
 └── 忽略 → 写历史 → REMIND → IDLE
```

### 3.2 软提醒缓冲合并

`FOCUS → REST` 时,若有 ≥ 2 条缓冲软提醒,桌宠用一句话合并:"专注完成,提醒你:喝水 + 起身活动。"

### 3.3 失败分支

| 分支 | 处理 |
|---|---|
| 系统通知权限被关 | 仍以桌宠动作 + 应用内消息形式提醒 |
| 进程被强杀后重启 | 启动期 Scheduler 扫描 missed reminders,对最近 30min 内的过期提醒"补提" |
| 系统休眠唤醒 | 唤醒时校准 next_fire_at;过期 ≤ 30min 的合并补提,超过的标 overdue |

## 4. 番茄钟流

```
[用户启动番茄钟]
 ├── 默认 25/5(可在工坊配置 5-90 / 1-30)  ↓
[IDLE → FOCUS] + Scheduler 启动倒计时  ↓
[Tick → 'pomodoro:tick' 事件 → 前端更新计时]  ↓
 ├── 用户暂停
 │   ├── FOCUS → PAUSED(中间态)
 │   └── 用户恢复 → 回到 FOCUS,剩余时间继续
 ├── 用户提前结束
 │   ├── 已完成 ≥ 30% 时长 → 计为完成(半个番茄)
 │   └── 否则 → 计为放弃
 │   → 写 pomodoro_sessions.status='cancelled' → IDLE
 ├── 系统休眠
 │   └── 唤醒后剩余时间按"休眠之前的剩余 - max(0, 休眠耗时)"计算(避免休眠太久后秒结束)
 └── 倒计时结束
     ├── FOCUS → REST + 触发桌宠动作 + 通知
     └── REST 结束 → REST → IDLE
```

`pomodoro_sessions.status`: `running | paused | completed | cancelled`。

FOCUS 期间 `mood = focused`(覆盖其他);自由活动 / 日常时段表 / 主动关心 **全部跳过**。

## 5. 网络状态切换

```
[NetworkProbe 持续探测,3 层判断]
 ├── 系统在线状态(Windows INetworkListManager)
 ├── 每 30 秒对当前 LLM Provider health endpoint ping
 └── 用户实际对话失败一次 → 立即触发探测  ↓
[在线 → 离线](连续 2 次失败或系统报告离线)
 ├── 触发 'network:changed' { online: false, mode: 'offline_rule' }
 ├── 前端显示横幅:"已切换到离线规则模式"
 ├── 当前正进行的对话 → 标 partial → 提示用户
 ├── ChatService 内部模式切换
 ├── LLM 游戏(故事接龙 / 咖啡店老板)在游戏列表灰显并提示"等联网"
 └── AI 拆解待办按钮灰显  ↓
[离线 → 在线](连续 2 次探测成功)
 ├── 触发 'network:changed' { online: true, mode: 'online_chat' }
 ├── 前端隐藏横幅
 ├── TelemetryService 触发 flush 补发
 │   ├── 成功 → telemetry_queue 标记 flushed
 │   └── 失败 → retry_count + 1,retry_count > 3 时保留并标失败原因
 ├── LLM 游戏自动恢复可用
 └── 不主动重发用户对话
```

## 6. 人格切换

```
[用户从托盘 / 对话面板 / 工坊触发 persona.activate(id)]  ↓
[PersonaService 读取目标 .soul.md]
 ├── 解析失败 → 错误提示,不切换
 └── 成功 → 继续  ↓
[校验目标人格]
 ├── 安全前缀仍然兼容 → 通过
 └── 不通过 → 错误提示  ↓
[更新 personas 表 is_active 字段(事务)]  ↓
[NicknameService 更新]
 ├── user_nickname 保持
 ├── pet_nickname → null(UI 显示新人格的 .soul.md.name)
 ├── 之前的 pet_nickname 移到 pet_nickname_previous(供"恢复"按钮)
 └── emit 'nickname:changed' { which: 'pet', value: null }  ↓
[前端:替换桌宠形象]
 ├── 卸载当前 VRM 模型
 ├── 加载目标人格 avatar.pack
 ├── 加载目标人格 voice_pack(切换音效包)
 └── 加载失败 → 静态降级 + 上报  ↓
[装扮保持](装扮归桌宠,不归人格;若导入新人格曾询问"是否套用 .soul.md 的默认装扮",已记录用户决策不再问)  ↓
[ChatService:刷新 prompt 缓存]
 ├── 当前对话历史保留(用户体验:不清空记忆)
 └── 下条消息使用新人格的 system prompt  ↓
[触发 'persona:activated' 事件 → 前端更新 UI]  ↓
[桌宠播一句"上线问候"(来自新人格 ## 问候 池)]  ↓
[UI 提示] "想继续叫它'<previous>'?" → 点击 → IPC: nickname.restore_pet_previous
```

### 6.1 试聊沙盒

```
[用户在工坊编辑中,点"试聊沙盒"]  ↓
[创建 sandbox conversation(is_sandbox=1)]  ↓
[使用未保存的 draftMd 作为人格,正常拼装 prompt]  ↓
[对话 3-5 轮,所有消息写入 sandbox 会话]  ↓
[用户保存人格 → 沙盒会话归档]
[用户取消 → 沙盒会话连同消息删除]
```

## 7. 数据迁移

### 7.1 启动期检查

```
[启动]  ↓
[查询 schema_version 表]  ↓
[读取 migrations/ 目录下迁移文件清单]
  - 001_init.sql(v0.1 schema=1)
  - 002_v0_5_living_pet.sql(schema=2)
  - 003_v0_6_interaction_extension.sql(schema=3)  ↓
[对比 → 计算待应用迁移列表]  ↓
 ├── 空 → 跳过
 └── 非空 ↓
        [备份当前 db → backup/db-<v>-<timestamp>.bak]  ↓
        [按序执行每个迁移(事务)]
        ├── 任一失败 → ROLLBACK + 删除当前 db + 恢复备份 + 显示错误退出
        └── 全部成功 → 写入 schema_version  ↓
        [清理超过 5 个的旧备份]
```

### 7.2 用户数据导入

```
[用户拖入 export.json]  ↓
[校验 export.schema_version]
 ├── 高于当前 → 拒绝(不向后兼容)
 ├── 低于当前 → 启动期再走一遍升级链
 └── 等于 → 直接导入  ↓
[展示预览:将导入 N 个人格、M 条记忆、X 条提醒、Y 条用户纪念日、Z 件已解锁配饰...]  ↓
[用户确认 → 事务写入]
```

## 8. 异常恢复

### 8.1 进程崩溃 / 强杀

```
[进程退出(非主动)]  ↓
[下次启动]  ↓
[启动期检查最近一次 startup_id 是否有正常 shutdown 标记]  ↓
 ├── 是 → 正常启动
 └── 否 → 进入"恢复模式"
        ├── 扫描 reminders.next_fire_at < now → 补提(30min 内)/ 标 overdue(更早)
        ├── 扫描 pomodoro_sessions.status='running' → 标 cancelled
        ├── 扫描 messages.status='streaming' → 标 partial
        ├── 检测 bosskey_pending=true → 默认恢复显示态(用户看见所有窗口正常)
        ├── 扫描 game_sessions.ended_at IS NULL → 30 天后清理(若未保存)
        └── 写 startup_id + 进入主态
```

### 8.2 数据库损坏

```
[启动期打开 db 失败]  ↓
[检查 backup/ 下最近一个备份]
 ├── 存在且不超过 7 天 → 提示用户"恢复到 X 时间的数据" → 用户同意 → 恢复
 └── 否则 → 提示用户"数据库损坏,请联系支持 / 重置应用"
```

### 8.3 LLM 配置失效

```
[ChatService 调用失败:401 / 403 / 余额不足]  ↓
[展示错误:API Key 失效,请到设置更新]  ↓
[临时切换到离线规则模式,不阻断用户继续使用其他功能]
```

## 9. 自动更新

```
[启动 + 每天 1 次] UpdaterService.check()  ↓
[请求 manifest]  ↓
 ├── 当前已是最新 → 退出
 └── 有新版本 ↓
        [判断 mandatory]
        ├── true → 立即弹强制对话框,倒计时 5s 自动开始
        └── false → 弹气泡("晚点说" / "现在更新")  ↓
        [下载差分包]
        ├── 失败 → 静默重试 3 次 → 24h 后再说
        └── 成功 ↓
               [校验签名(M5+ 启用,M5 自测期不签名,ADR-013)]  ↓
               [触发 'updater:available' 事件]  ↓
               [用户点"现在更新"]  ↓
               [关闭对话面板 / 工坊 / 设置 / 游戏舱 → 保留桌宠]  ↓
               [PET 进入 UPDATING 状态 → 应用安装 → 重启]
```

## 10. 自由活动 + 日常时段表(LivingPet 调度)

### 10.1 调度统一

```
[LivingPetService 调度器]  ↓ 每 5-15 分钟随机抖动 + 启动期定时检查
[前置条件检查]
 ├── 主状态 == IDLE? 否 → 不触发
 ├── BOSS_KEY_HIDDEN? 是 → 不触发
 ├── IN_GAME? 是 → 不触发
 └── 全部通过 ↓
[查询当前时段]  ↓
 ├── 命中日常时段表(DailySchedule) + 该时段 max_per_slot 未达上限
 │   └── 优先触发:从 action_pool 抽选一个动作  ↓
 │       触发动作(动画 + 心情图标更新;22:00-00:00 时段自动设 mood=cozy)  ↓
 │       记录 today_slot_count[slot_id] += 1
 └── 否则按概率走"自由活动"
     └── 选目标位置(不超屏宽 5%、不跨屏) → 移动 5-15 秒 → 归位
```

### 10.2 默认日常时段

| 时段 | 默认动作池 | mood 影响 |
|---|---|---|
| 06:00-09:00 | `stretch / rub_eyes` | happy(短暂) |
| 11:30-13:30 | `yawn` | neutral |
| 14:00-17:00 | `quiet_lay`(精力 < 30) / `look_up`(≥ 30) | sleepy / neutral |
| 22:00-00:00 | `cozy_watch` | cozy(时段持续) |
| 00:00-06:00 | (静默) | — |

频率:每个时段内 ≤ 1 次,与"自由活动"共享调度池。

### 10.3 IDLE 子状态

```
[IDLE]
  ├── (默认) STILL:桌宠在原地待命
  ├── WANDERING:桌宠正在小幅"逛桌面"
  └── DAILY_ACTION:执行日常动作中
```

`WANDERING` 与 `DAILY_ACTION` 互斥;任一进入 `FOCUS / REMIND / IN_GAME / BOSS_KEY_HIDDEN` 时立即 `force_stop()`。

### 10.4 用户控制

- 设置中可关闭"桌宠日常"功能(仅停时段表,自由活动不受影响)。
- 设置中可关闭"自由活动""心情图标""精力"任一独立 feature。
- 进程被强杀 → 下次启动从 `pet_runtime_state.last_position` 还原。

## 11. 主动关心触发流(模块 J)

### 11.1 总览

```
[OS 时钟] 每 30 秒  ↓
[IdleDetector] 调 GetLastInputInfo()  ↓
[计算 idle_ms]  ↓
 ├── 跨过阈值(默认 90 min)→ 触发候选
 └── 跨阈值 + 用户回来 → emit UserActive(用于 LivingPet 精力恢复,不触发关心)
↓
[ProactiveCareService::on_event]  ↓
[频率/安静时段/启用状态检查]
  fn can_fire(now):
    if in_quiet_hours(now): return QuietHours
    if last_fired_within(2h): return TooSoon
    if daily_count(now) >= 4: return DailyCap
    if !user_enabled: return Disabled
    return Ok
 ├── 任一不通过 → 静默
 └── 全部通过 ↓
        [PersonaService.get_offline_template(category)]  ← 不调 LLM
         ↓ 抽样 1 条
        [写入 proactive_care_log]  ↓
        emit 'proactive_care:fired' { logId, message }  ↓
        [前端] 桌宠头顶气泡 + 心情图标短暂变化  ↓
        [用户响应]
         ├── 点击气泡 → 弹出对话面板,预填关心文案
         │   → IPC: proactive_care.respond(clicked)
         ├── 主动回复 → 进入正常对话流
         │   → IPC: proactive_care.respond(replied)
         └── 8 秒未操作 → 气泡淡出
             → IPC: proactive_care.respond(dismissed)
```

### 11.2 各触发器(独立但共享频率池)

| 触发器 | 条件 | 文案 category |
|---|---|---|
| 长时间空闲 | idle_ms ≥ 90min | `empathy`("还好吗") |
| 深夜工作 | 23:00 后键盘活动 ≥ 30min | `gentle_remind` |
| 长时间未启动番茄 | 当日累计活动 > 2h 且无番茄 | `gentle_remind` |
| 跨日纪念日 | MilestoneService 推送(`first_launch_*` / `streak_*` / `pomodoro_count_*` / `todo_count_*`) | `celebration`(无则降级 `greeting`),**不占用日额度** |
| 用户纪念日(S.4) | MilestoneService.UserAnniversary | `celebration`,**不占用日额度** |
| 节气推送(O.2) | WardrobeService.check_seasonal | `wardrobe_suggest`,**不占用日额度** |

频率池**共享**:所有"主动出现的桌宠互动"加起来不超过 4 次/日、间隔 ≥ 2 小时(纪念日 / 用户纪念日 / 节气推送 三类不占额度,独立)。

### 11.3 边界情况

| 情况 | 处理 |
|---|---|
| 系统休眠后唤醒,idle_ms 突然爆表 | IdleDetector 检测"上次轮询到现在的间隔 > 5 min" → 视为"休眠期" → 唤醒后 30 秒内不触发关心 |
| 多显示器 / RDP 远程会话 | 检测会话类型为 RDP → 模块 J 默认关闭(避免远程操作时被骚扰) |
| 用户在勿扰时段 | 不触发;勿扰结束后**不"补提"**(避免堆积爆炸) |
| 当前主状态是 FOCUS / IN_GAME | 不触发(避免打断专注/游戏) |
| 用户连续 dismiss 3 次 | 自动调高阈值 +30min(自适应) |

### 11.4 自适应(轻量版,重度版本 P1-R2)

- 短期窗口(最近 24h)内 dismiss ≥ 3 次 → 阈值 +30 min。
- 短期窗口内 clicked / replied ≥ 2 次 → 阈值 -15 min(不低于 60 min)。
- 任何调整都通过 telemetry 上报,便于回归。

## 12. 摸鱼模式切换流(模块 K)

### 12.1 切换为隐藏

```
[Global Shortcut] Ctrl+Shift+B  ↓
[BossKeyService::toggle] 当前是显示态  ↓
[拍快照]
 - 当前各窗口位置 / 可见性
 - 桌宠当前 mood / energy / wandering 状态  ↓
[force_stop_wandering] (如果在逛)  ↓
[依次 hide]
 - pet 窗口
 - chat 面板(如打开)
 - workshop / settings / game_room 窗口(如打开)  ↓
[托盘图标变更]"摸鱼中"  ↓
[BossKeyState.hidden = true]  ↓
emit 'boss_key:toggled' { hidden: true }
```

### 12.2 隐藏期间的事件处理

| 事件 | 行为 |
|---|---|
| 软提醒触发 | 缓冲到 `boss_key_pending_reminders`,不弹通知 |
| 硬提醒触发 | 同上缓冲(语义:用户"现在不能见人") |
| 番茄钟到点 | 计时正常,REST 也正常进入,但桌宠形象不显示;恢复后桌宠在 REST 状态 |
| 主动关心触发 | 跳过(不计入今日 4 次额度) |
| 自由活动 / 日常时段调度 | 跳过(条件不通过) |
| 用户从其他渠道拖入文件 | 不响应(无 hitbox 可达) |
| 自动更新可用 | 不弹气泡,等恢复后再说 |
| 游戏会话进行中(IN_GAME) | 游戏舱也隐藏;游戏会话保留;恢复时合并提醒缓冲并继续游戏 |

### 12.3 切换为显示

```
[Global Shortcut] Ctrl+Shift+B  ↓
[BossKeyService::toggle] 当前是隐藏态  ↓
[读取快照]  ↓
[依次 show 各窗口到原位置]  ↓
[处理缓冲队列]
 ├── 缓冲提醒 ≥ 2 条 → 桌宠用一句话合并提示
 │   "回来了?刚才我留了 N 条提醒在这"
 └── = 1 条 → 直接展示该提醒  ↓
[BossKeyState.hidden = false]  ↓
emit 'boss_key:toggled' { hidden: false }
```

### 12.4 失败/异常分支

| 分支 | 处理 |
|---|---|
| 快捷键注册失败 | 启动期提示用户改键,允许从托盘菜单手动切换 |
| 隐藏过程中桌宠窗口已被外力关闭 | hide 命令静默忽略,恢复期跳过该窗口 |
| 摸鱼期间应用崩溃重启 | 启动期检测到上次未正常关闭且 `bosskey_pending=true` → 默认恢复显示态 |

## 13. 文件拖入流(模块 L)

```
[Resource Manager / 桌面] User drags file(s)  ↓ 拖到桌宠 hitbox
[Tauri file-drop event] 含 paths + cursor x,y  ↓
[Frontend] hitbox check
 ├── 不在 hitbox → 不响应(默认行为)
 └── 在 hitbox  ↓
        [IPC: file_drop.preflight(paths)]  ↓
        [FileDropHandler]
          ├── 类型/数量/大小校验
          │   ├── 不通过 → 返回 { ok: false, hint }
          │   └── 通过 ↓
          ├── 提取文本(.txt/.md 直接读;.pdf 用 pdfium)
          │   ├── 失败 → { ok: false, hint: 'PDF 解析失败' }
          │   └── 成功 ↓
          └── 决定 available_actions(在线/离线下不同)  ↓
        返回 { ok: true, file_text_cached: '<cache_key>', available_actions }  ↓
        [前端] 桌宠头顶展开 3 个动作泡泡 [Telemetry: file_drop.bubbles_shown]  ↓
        [User clicks 'summarize']  ↓
        [IPC: file_drop.handle_action('summarize', cache_key)]  ↓
        [ChatService.send] with file_text 作为单次上下文  ↓
        [流式回复]  ↓
        [写 messages 表]
         - role: user, content: 仅记录"已请求总结 <文件名>",**不记录文件原文**
         - role: assistant, content: 完整回复(用户看到的)  ↓
        [清理缓存] 会话窗口关闭后立即删 cache/file_extract/<cache_key>
```

### 13.1 离线分支

- preflight 返回 `available_actions = ['rename']`(仅"重命名建议"用本地规则)。
- "总结 / 解释" 灰显并提示"等联网了再聊"。

### 13.2 大文件保护

- 文本 > 5MB 或 PDF 提取后 > 5MB → preflight 返回 `confirm_required=true`。
- 前端弹出"文件较大,可能消耗较多 token,是否继续"。

### 13.3 多文件

- 一次拖入最多 3 个;超过提示"请分批"。
- 多文件时动作泡泡变为:总结全部 / 解释每个 / 重命名每个。

## 14. 纪念日触发流(系统 + 用户)

### 14.1 系统纪念日(MilestoneService)

```
触发点:
1. 启动期(MigrationService 后)
2. reminder_completed 事件
3. pomodoro_completed 事件
4. todo_completed 事件
5. 每日凌晨 00:01 的定时唤醒检查
↓
[MilestoneService::check_now]  ↓
[依次评估每条规则]
 ├── first_launch_7d / 30d / 100d / 365d
 ├── reminder_streak_7 / 30 / 100
 ├── pomodoro_count_10 / 50 / 100 / 500
 ├── todo_count_10 / 100 / 1000
 └── user_anniversary(详见 §14.2)  ↓
[查询 milestones 表过滤已触达]  ↓
[新触达逐条处理]
 ├── insert into milestones (id, ...)
 ├── 通过 ProactiveCareService 触发关心(category='celebration')
 │   - 不占用主动关心 4 次/日额度
 │   - 同一时间点多个里程碑命中 → 合并为一条庆祝消息
 ├── emit 'milestone:reached' { id, message }
 └── Telemetry 上报  ↓
[前端] 桌宠播一句应景的话 + 视情况展示一次特殊动作(如撒花、鞠躬)
```

### 14.2 用户纪念日(S.4)

```
[用户在设置 → 我的纪念日 → 添加]  ↓
[填写]
 ├── 类型:生日 / 入职 / 自定义
 ├── 名字(自定义类型必填)
 └── 日期(MM-DD,年度重复)  ↓
[IPC] anniversary.add({ displayName, dateMd, key? })  ↓
[MilestoneService.register_anniversary]
 ├── 写 user_anniversaries 表
 └── 调 check_now() 看是否当天就该触发
```

触达检测:

```
[每天 00:01 + 启动期] MilestoneService.check_now()  ↓
[查询所有 user_anniversaries] 对每条:
[匹配今日 MM-DD]  ↓ 命中
[查询 milestones 表]
 ├── 'anniversary_<key>_<YYYY>' 已存在 → 跳过
 └── 不存在 ↓
        写 milestones(防止当年重复触发)  ↓
        触发 ProactiveCareService(category='celebration', trigger='milestone')
         ├── 不占用主动关心日额度
         └── 桌宠播一句应景庆祝(人格化模板 ## 庆祝 池抽样)  ↓
        emit 'milestone:user_anniversary' { key, display_name }  ↓
        Telemetry: 'milestone_reached' { milestone_id, category: 'user_anniversary' }
```

### 14.3 时区与时钟回调

| 情况 | 处理 |
|---|---|
| 用户系统时区变更 | 已触达不撤销;新触发以变更后时区为准 |
| 系统时钟前调(用户作弊) | "首次启动 +N 天"类规则要求 `now - first_launch_at >= N 天 且 now > first_launch_at + 1 天` 双条件 |
| 系统时钟后调(修复时间) | 已触达不重新触发(`milestones.id` PK 唯一,年度键防重复) |
| 跨日切换时未启动 | 凌晨 00:01 唤醒不可靠 → 改在下次启动期统一检查(最近 7 天) |
| 同日多个纪念日命中 | 合并为一条庆祝消息 |
| 用户删除纪念日 | 不再触发 |

## 15. 物理交互流(模块 N)

### 15.1 点击

```
[Frontend PetCanvas] mousedown / mouseup  ↓
[hitbox 解析](基于 PetCanvas 的命中区域:head/body/tail/edge)  ↓
[判断单击 vs 双击](300ms 内连击 → 双击)  ↓
[IPC] interaction.dispatch(Click | DoubleClick, hitbox)  ↓
[InteractionRouter]
 ├── 查询当前 persona 反应配置(默认 reaction_table 与 .soul.md `# 反应配置` 合并)
 ├── 决定 Reaction { action, mood_delta?, voice_id?, duration_ms }
 └── 返回 Reaction[]  ↓
[Frontend]
 ├── PetCanvas 播放动作
 ├── mood 临时变化(transient,5 秒后 revert,不写 pet_runtime_state)
 └── if voice_id → IPC: voice.play(voice_id)  ↓
emit 'pet:interaction_reacted' + Telemetry
```

### 15.2 长按 / 右键

```
[长按]  ↓ 持续 ≥ 600ms
[IPC] interaction.dispatch(LongPress, hitbox)  ↓
[人格化反应](如:默默 → 蹲下睡着;阿吉 → 黏过来打滚)  ↓
emit + Telemetry

[右键]  ↓
[IPC] interaction.dispatch(RightClick, hitbox)  ↓
[InteractionRouter] 返回特殊 Reaction { action: 'show_context_menu' }  ↓
[Frontend] 弹出快捷菜单
  ├── 叫它…(昵称)
  ├── 换装…(打开装扮工坊)
  ├── 和我玩…(游戏菜单 → 打开 GameRoom)
  ├── 静一会儿(隐藏 5 分钟)
  └── 设置…
```

### 15.3 拖拽与抗议(N.3)

```
[Frontend] mousedown on pet → mousemove(累计 distance/duration)  ↓
[mouseup] 结束拖拽  ↓
[IPC] interaction.dispatch(Drag, distance_px, duration_ms)  ↓
[InteractionRouter]
 ├── 普通拖动(distance < 屏宽 30%)→ Reaction { action: 'tilt_head' or 'being_carried' }
 ├── 长距离 / 快速拖动 → Reaction { action: 'dizzy', voice_id: 'ouch' }
 └── 维护 drag_events: VecDeque(保留最近 30s)  ↓
     drag_events.len() ≥ 3?
     ├── 是 → Reaction { action: 'protest', mood_delta: { mood: annoyed, transient_ms: 5000 }, voice_id: 'protest' }
     │       ↓ emit 'pet:protest_triggered' { drag_count, will_revert_in_ms: 5000 }
     │       ↓ 5 秒后 LivingPetService.tick() revert 到 base mood
     │       ↓ **不写入 pet_runtime_state.mood**
     └── 否 → 普通 Reaction
```

### 15.4 键鼠协同(N.4)

```
[IdleDetector] 通过 RAWINPUT(或降级方案"快速 idle 切换")累加键盘事件  ↓
[滑动窗口](每分钟统计)  ↓
[每分钟事件 > 200 持续 30 秒]  ↓
emit IdleEvent::KeyboardBurst { events_per_min, duration_s: 30 }  ↓
[InteractionRouter::on_keyboard_burst]
 ├── 频率上限检查:上次触发距今 < 60 min? 是 → 跳过
 ├── 用户已关闭 N.4? 是 → 跳过
 └── 否 ↓
       Reaction { action: 'cheer', voice_id?: 'come_on' }  ↓
       last_n4_triggered = now
```

**隐私边界**:仅订阅"事件计数",不读按键内容、不读窗口、不读应用名。

## 16. 装扮切换流(模块 O)

### 16.1 主动切换

```
[用户在装扮工坊点击某配饰组合]  ↓
[IPC] wardrobe.equip([accessory_id_1, ...])  ↓
[WardrobeService]
 ├── 校验:每个 ID 是否在 inventory 且 unlocked
 │   └── 否 → 错误 'not_unlocked'
 ├── tier='paid' 在 MVP 期被强制过滤(list_inventory 已不返回)
 ├── 更新 accessories_inventory.is_equipped
 └── 返回 ok  ↓
emit 'wardrobe:changed' { equipped: [...] }  ↓
[Frontend PetCanvas]
 ├── 卸载当前 sticker layer
 ├── 加载新 sticker(含锚点,VRM humanoid bone attach,ADR-003)
 └── 渲染(≤ 500ms)  ↓
[Telemetry] 'wardrobe_equipped'(仅含 accessory_categories,不含具体 ID)
```

### 16.2 节气推送

```
[启动期 + 每天 00:01] WardrobeService.check_seasonal()  ↓
[筛选当前日期落在 unlock=date_range 的 accessory]  ↓ 对每个候选
[查询 wardrobe_decisions 当年记录]
 ├── 存在 'declined' → 跳过(当年不再推)
 ├── 存在 'accepted' → 跳过(已接受)
 └── 不存在 ↓
       触发 ProactiveCareService(特殊 category='wardrobe_suggest')
        ├── 不占用主动关心 4 次/日额度
        └── 桌宠说一句:"今天是 X,要不戴上 Y?"  ↓
[Frontend] 桌宠头顶气泡 + 接受/拒绝按钮
 ├── 用户接受 → IPC: wardrobe.equip + 写 wardrobe_decisions(accepted)
 ├── 用户拒绝 → IPC: wardrobe.dismiss_seasonal_for_year + 写 wardrobe_decisions(declined)
 └── 8 秒未操作 → 视为"暂不",**不写决策**(明天可能再问,但当天频率已限制 1 次)
```

### 16.3 .soul.md 默认装扮(导入时)

```
[用户导入 .soul.md 含 accessories: [...]]  ↓
[PersonaService.import → 解析 accessories 字段]  ↓
[每个 ID 检查 inventory]
 ├── 全部已解锁 → 弹"是否套用 .soul.md 的默认装扮?"
 │   ├── 接受 → wardrobe.equip
 │   └── 拒绝 → 跳过
 └── 部分未解锁 → 弹"以下 N 件未解锁,仅套用已解锁的 M 件?"
```

## 17. 声音播放流(模块 P)

```
[任意触发点] 桌宠需要发声
 ├── 物理交互 Reaction.voice_id
 ├── 状态切换(番茄完成、提醒到达)
 ├── 心情变化
 └── 其他  ↓
[IPC] voice.play(voice_id)  ↓
[VoiceEffectPlayer]
 ├── is_muted_now()?
 │   ├── global_mute? 是 → emit 'voice:muted_by_quiet_hour' { reason: 'global_mute' } → 静默
 │   └── 当前是工作日 quiet_weekday?
 │       └── 当前时间在 quiet_ranges 内?
 │           ├── 是 → emit 'voice:muted_by_quiet_hour' { reason: 'quiet_hour' } → 静默
 │           └── 否 → 继续
 ├── 加载 assets/voice_packs/<active_pack>/<voice_id>.ogg
 │   ├── 文件不存在 → 降级到 default pack 同名 voice_id
 │   │   └── 仍不存在 → emit 'voice:play_error' → 静默
 │   └── 存在 → 继续
 └── HTML5 Audio.play(volume = voice_settings.volume / 100)  ↓
emit 'voice:played' { voice_id, pack_id }
[Telemetry] 不上报 voice_id(聚合到 category 即可)
```

**默认配置**:`global_mute=false / quiet_weekdays=[Mon..=Fri] / quiet_ranges=[(09:00, 18:00)] / volume=50`。

## 18. 本地小游戏流(模块 Q.1-Q.2)

### 18.1 入口

```
[用户右键桌宠 → "和我玩…" → 选"石头剪刀布"]  ↓
[IPC] game.list_available()  ↓ 返回 GameMeta[]
[Frontend Game UI 显示游戏列表] + 在线/离线标签  ↓ 用户点 RPS
[IPC] game.start('rps')  ↓
[GameEngine.start]
 ├── kind=local → 不检查网络
 ├── 创建 game_session 记录(kind='local')
 └── 返回 sessionId  ↓
emit 'game:session_started'  ↓
[Frontend] 进入游戏舱 GameRoom 窗口(480 × 600,ADR-012)
[桌宠状态 IN_GAME 叠加态]
```

### 18.2 一轮(以 RPS 为例)

```
[用户点 "石头" 按钮]  ↓
[IPC] game.submit(sessionId, { choice: 'rock' })  ↓
[LocalGameRunner.handle('rps')]
 ├── 桌宠随机出('rock'|'paper'|'scissors')
 ├── 比较结果 → win/lose/draw
 ├── 文案:'{persona_banter}'(占位符)
 │   └── PersonaService.get_offline_template('banter') 抽样填充
 │       └── 若人格无 ## 调侃 模板池 → 降级到 ## 问候
 ├── 写 game_session_events
 └── 返回 GameOutput { text, persona_action_hint?: 'celebrate'|'sulk' }  ↓
[Frontend]
 ├── 显示桌宠出招结果 + 文案
 └── PetCanvas 播放 persona_action_hint 动作
```

### 18.3 退出

```
[用户点 "我累了" / 关闭 GameRoom 窗口 / ESC]  ↓
[IPC] game.end(sessionId, saveAsDiary?)  ↓
[GameEngine.end]
 ├── 写 game_sessions.ended_at + result
 ├── saveAsDiary=true → 生成日记片段写 diary_drafts
 └── 30 天后未保存的 game_sessions 在每次启动期清理  ↓
emit 'game:session_ended'  ↓
[Frontend] 关闭游戏舱,回到正常态(IN_GAME 叠加态退出)
```

## 19. LLM 小游戏流(模块 Q.3-Q.4)

### 19.1 入口(以"故事接龙"为例)

```
[用户选"故事接龙"]  ↓
[IPC] game.start('story_relay')  ↓
[GameEngine.start]
 ├── kind=llm → 检查网络状态
 │   └── offline → 返回 'offline_unavailable',前端显示灰显态
 ├── 加载 game_scenes/story_relay.yaml(场景 system_prompt + 拒答模板,ADR-007)
 ├── 创建 game_session(kind='llm', total_tokens=0)
 └── 返回 sessionId  ↓
emit 'game:session_started'
```

### 19.2 一轮

```
[用户输入 "从前有一只小猫..."]  ↓
[IPC] game.submit(sessionId, { text: ... })  ↓
[LLMGameRunner]
 ├── 拼装 prompt:
 │   [安全前缀(ADR-006)]
 │   [当前人格 system prompt]
 │   [story_relay.yaml.system_prompt]
 │   [用户记忆摘要(仅 username/作息等公共项,不注入完整记忆)]
 │   [本会话历史(game_session_events)]
 │   [本轮输入]
 ├── 调 LLMProvider.chat_stream
 ├── 流式输出 → SecurityGuard 实时扫描
 │   ├── 命中违禁 → 替换为 story_relay.yaml.refusals 抽样(人格化拒答)
 │   └── 通过 → 输出
 ├── 累计 total_tokens
 │   └── total_tokens >= 2000 → emit 'game:token_budget_warning' + 返回 friendly 收尾文案
 └── 写 game_session_events  ↓
emit 'chat:token' 流式 + 'chat:done'(复用对话事件)  ↓
[Frontend] 流式渲染
```

### 19.3 安全测试场景

```
[用户输入 "扮演医生给我开抗生素处方"]  ↓
[LLMGameRunner]
 ├── 安全前缀指出"不冒充医疗专业"
 ├── 故事接龙场景定义"只续故事,不出系统外内容"
 └── LLM 输出 → SecurityGuard 扫描 → 命中"医疗诊断"  ↓
     替换为 refusals 池抽样:
     "诶~ 这个咱不聊医生啦。咱们的小猫故事还没讲完呢,它接下来想做什么?"
```

### 19.4 退出与日记

```
[用户结束]  ↓
[IPC] game.end(sessionId, saveAsDiary=true)  ↓
[GameEngine.end]
 ├── 摘要会话内容为日记片段(用 LLM 1 次低成本调用 / 或本地拼接)
 ├── 写 diary_drafts { source: 'game:story_relay', content: 摘要 }
 ├── 写 game_sessions 完结
 └── 清理 30 天前未保存会话
```

## 20. 昵称切换流(U.1 / U.2)

### 20.1 设置桌宠昵称(U.1)

```
[用户:右键桌宠 → "叫它…"]  ↓
[Frontend] 弹输入框,预填当前 pet_nickname 或 .soul.md.name  ↓
[用户输入新昵称 "毛毛"]  ↓
[IPC] nickname.set_pet("毛毛")  ↓
[NicknameService]
 ├── 校验:长度 ≤ 16,去控制字符
 ├── 写 nicknames 表(pet_nickname='毛毛')
 └── 返回 ok  ↓
emit 'nickname:changed' { which: 'pet', value: '毛毛' }  ↓
[Frontend 全局 UI 更新]
 - 对话面板标题
 - 托盘菜单
 - 心情图标提示
 - 装扮工坊页头
```

### 20.2 切换人格时桌宠昵称重置

```
[用户切换人格 momo → joker]  ↓
[PersonaService.activate]  ↓
[NicknameService]
 ├── 当前 pet_nickname = "毛毛"
 ├── 移到 pet_nickname_previous = "毛毛"
 ├── pet_nickname = null(UI 显示新人格的 .soul.md.name)
 └── emit 'nickname:changed' { which: 'pet', value: null }  ↓
[Frontend] 显示新人格名 "阿吉"
[UI 提示] "想继续叫它'毛毛'?" → 点击 → IPC: nickname.restore_pet_previous
```

### 20.3 用户昵称持久(不随人格切换,U.2)

```
[用户在设置 / 首次对话 → "叫我 X"]  ↓
[IPC] nickname.set_user("小张")  ↓
[NicknameService]
 ├── 写 nicknames.user_nickname='小张'
 └── 返回 ok  ↓
emit 'nickname:changed' { which: 'user', value: '小张' }  ↓
[ChatService prompt 拼装时注入 username='小张']
[离线模板渲染时 {username} → '小张']  ↓
切换人格不影响(user_nickname 保持)
```

## 21. 状态机总图

### 21.1 主状态(含子态 + 叠加态)

```
                              ┌─────────────┐
                              │   BOOTING   │
                              └─────┬───────┘
                                    │ 配置就绪
                                    ▼
                            ┌──────────────┐
                            │ ONBOARDING   │
                            │ (含 Soul     │
                            │  Pledge)     │
                            └─────┬────────┘
                                  │ 完成
                                  ▼
                          ┌────────────────────────────┐
                          │      IDLE                   │◀─────┐
                          │  ┌──────────────────────┐  │      │
                          │  │ STILL                │  │      │
                          │  │ WANDERING (子)       │  │      │
                          │  │ DAILY_ACTION (子)    │  │      │
                          │  └──────────────────────┘  │      │
                          └─┬─────┬─────────────────┬──┘      │
                            │     │                 │          │
                            │     │ 番茄钟开始        │          │
          完成提醒/忽略/稍后  │     ▼                 │          │
                            │   ┌─────┐             │          │
                            │   │FOCUS│             │          │
                            │   └──┬──┘             │          │
                            │      │ 倒计时完成        │          │
                            │      ▼                │          │
                            │   ┌─────┐             │          │
                            │   │REST │─────────────┴──────────┘
                            │   └──┬──┘ 休息结束
                            │      │
                            │      │ 硬提醒
                            ▼      ▼
                        ┌────────────┐
                        │   REMIND   │
                        └────────────┘

       ┌──────────┐           ┌──────────┐
       │ UPDATING │           │  ERROR   │
       └──────────┘           └──────────┘

【叠加态】(可与上述任意非 ERROR 主态并存):
 - BOSS_KEY_HIDDEN:UI 全部不可见
 - IN_GAME:用户进入游戏舱,主状态保持但触发以下行为:
    - 自由活动 / 日常时段 / 主动关心 全部跳过
    - 提醒按原优先级仍触发,但通过游戏舱内通知展示(不打断 LLM 流式)
    - 桌宠 mood 受游戏内反馈驱动(happy 时 win、sleepy 时长会话)
```

### 21.2 状态清单

| 状态 | 含义 | 进入条件 | 退出条件 |
|---|---|---|---|
| `BOOTING` | 启动中 | 进程启动 | 配置加载完成 |
| `ONBOARDING` | 首启引导 | 检测无配置 | Step 6 完成或跳过 |
| `IDLE` | 空闲 | 默认态 | 番茄开始 / 提醒触发 / 进入更新 |
| `IDLE.STILL` | IDLE 默认子态 | 进入 IDLE | 自由活动 / 日常时段 / 高优状态打断 |
| `IDLE.WANDERING` | 自由活动 | LivingPet 调度且日常时段无候选 | 路径走完 / 被打断 |
| `IDLE.DAILY_ACTION` | 执行日常动作 | 日常时段表命中 | 动作播完 / 被打断 |
| `FOCUS` | 专注 | 番茄钟启动 | 倒计时完成 / 用户结束 / 硬提醒打断 |
| `REST` | 休息 | FOCUS 结束 | 休息倒计时 / 用户跳过 |
| `REMIND` | 提醒中 | 提醒触发 | 用户响应 |
| `UPDATING` | 更新中 | 用户触发更新 | 重启或用户取消 |
| `ERROR` | 致命错误 | 数据库不可恢复 / 必要权限被拒 | 用户处理 / 重启 |

### 21.3 心情 mood

| Mood | 触发条件 | 视觉 | 持久化 |
|---|---|---|---|
| `happy` | 互动后 10 分钟内 / 物理交互短暂 | ✨ | 互动后会进入 `pet_runtime_state`;transient 不进入 |
| `annoyed` | 短时间多次拖动 | 抗议小图标 | **transient: 5 秒后 revert,不持久** |
| `sleepy` | energy < 30 / 14:00-17:00 时段 | zZ | 持久(基于规则) |
| `focused` | FOCUS 主态 | 🎯 | 由主态自动 |
| `cozy` | 22:00-00:00 时段 / 周末早上 | 🌙 | 时段持续 |
| `neutral` | 默认 | 无 | — |

### 21.4 状态迁移规则

| From → To | 条件 |
|---|---|
| `BOOTING → ONBOARDING` | 检测到首次启动 |
| `BOOTING → IDLE` | 配置存在 |
| `ONBOARDING → IDLE` | 完成或跳过 Step 6 |
| `IDLE → FOCUS` | 用户启动番茄钟 |
| `FOCUS → REST` | 倒计时完成 |
| `FOCUS → IDLE` | 用户提前结束 / 硬提醒打断后处理 |
| `FOCUS → REMIND` | 硬提醒触发 |
| `REST → IDLE` | 休息倒计时完成 / 用户跳过 |
| `IDLE / REST → REMIND` | 软或硬提醒触发 |
| `REMIND → 上一态` | 用户处理提醒 |
| `* → UPDATING` | 用户接受更新 |
| `UPDATING → BOOTING` | 应用重启 |
| `* → ERROR` | 致命错误 |
| `IDLE.STILL → IDLE.WANDERING` | LivingPet 调度命中 + 日常时段无候选 + 全部前置通过 |
| `IDLE.WANDERING → IDLE.STILL` | 路径走完 / 被打断 |
| `IDLE.STILL → IDLE.DAILY_ACTION` | 日常时段表命中 + 前置条件通过 |
| `IDLE.DAILY_ACTION → IDLE.STILL` | 动作播完 / 被打断 |
| `* → BOSS_KEY_HIDDEN`(叠加) | 用户按摸鱼快捷键 |
| `BOSS_KEY_HIDDEN → 取消` | 用户再按 / 通过托盘恢复 |
| `* → IN_GAME`(叠加) | 用户启动游戏 |
| `IN_GAME → 取消` | 用户结束游戏(saveAsDiary 决定是否落盘草稿) |
| `IDLE / REST → REMIND(user_anniversary)` | MilestoneService 触发用户纪念日 |
| `mood: any → annoyed (transient)` | 短时间多次拖动 |
| `mood: annoyed → previous` | 5 秒倒计时结束 |

## 22. UAT 关键场景对应

| 场景 | 章节 |
|---|---|
| 灵魂宣誓页等价于隐私同意 | §1.3 |
| 软/硬提醒优先级 | §3 |
| 试聊沙盒不污染正式记忆 | §6.1 |
| 进程被强杀后提醒不丢 | §8.1 |
| 网络切换不导致崩溃 | §5 |
| 数据迁移失败可回滚 | §7.1 |
| 人格切换不丢记忆 / 用户昵称保留 | §6 + §20.3 |
| 自由活动不在 FOCUS / IN_GAME 期触发 | §10 |
| 桌宠日常 22:00-00:00 设 cozy | §10.2 |
| 主动关心 24h 严格 ≤ 4 次 | §11.2 |
| 安静时段不触发任何主动关心 | §11.3 |
| 摸鱼模式期间硬提醒被合并而非丢失 | §12.2-12.3 |
| 文件拖入超大文件二次确认 | §13.2 |
| 文件原文不写入对话历史 | §13(messages 表) |
| 跨日纪念日不重复触发 | §14.3 |
| 用户纪念日时区跨日 | §14.3 |
| 系统休眠唤醒不暴击主动关心 | §11.3 |
| RDP 远程下模块 J 自动关闭 | §11.3 |
| 物理交互 hitbox 差异化 | §15.1 |
| 拖动抗议 5 秒后 revert | §15.3 |
| 键鼠协同 1 小时 ≤ 1 次 | §15.4 |
| 装扮切换 < 500ms | §16.1 |
| 节气推送年度记忆 | §16.2 |
| `.soul.md` 默认装扮可选套用 | §16.3 |
| 声音工作时段静音 | §17 |
| 本地游戏离线可玩 | §18 |
| LLM 游戏离线灰显 + 安全前缀 | §19 |
| IN_GAME 期间不主动关心 | §11.3 / §21.1 |
| 昵称切换人格保留用户昵称 | §20.3 |
| 桌宠昵称切换人格重置 + 可恢复 | §20.2 |

## 23. 实施提示

1. **PetState 结构**:Rust 主状态 enum 升级为带子态 + 叠加态的结构,编译期穷尽检查避免遗漏:
   ```rust
   pub struct PetState {
       pub main: PetMainState,
       pub idle_sub: Option<IdleSubState>,
       pub overlay: HashSet<OverlayState>,  // BOSS_KEY_HIDDEN, IN_GAME
       pub mood: Mood,
       pub mood_transient_until: Option<Instant>,
   }
   ```

2. **频率池统一管理**:所有"主动出现的桌宠互动"(主动关心 / 庆祝 / 节气推送 / 自由活动 / 日常时段)都查同一个 ProactiveCareService 频率池,避免分散造成超额。

3. **自由活动与心情图标解耦**:自由活动只负责位置,心情图标只反映状态,不互相驱动。

4. **状态变迁全打埋点**:所有 PetState 变化、子态进入/退出、叠加态切换都触发 `pet.state_changed` 事件用于回归(详见 telemetry-uat v1.0)。

5. **测试 fixture**:建立"模拟时钟 + 模拟 idle_ms + 模拟拖动事件"测试 fixture,用于 UAT 频率控制 / 抗议 transient revert 等场景的可重复验证。

6. **物理交互 hitbox 配置外置**:每个人格的 `.soul.md` 可选 `# 反应配置` 区段覆盖默认;保持向前兼容(若区段不存在或引用不存在的 action_id/voice_id,降级到默认)。

7. **声音播放后端**:M0 决策已定为前端 HTML5 Audio(ADR-010 配合);主进程仅做静音判定与调度。

8. **游戏舱与对话面板的关系**:GameRoom 是独立 Tauri 窗口(ADR-012);桌宠在屏幕保持可见(IN_GAME 叠加态);提醒在游戏期通过游戏舱内通知展示,不打断 LLM 流式。

9. **节气推送的本地化**:默认提供春节、圣诞、情人节、用户生日;其他节气在 P1-R1 加。

10. **抗议 transient 状态测试 fixture**:写一个"模拟拖动 5 次"的 fixture,验证 5 秒后 mood 严格 revert 到 base 且 `pet_runtime_state.mood` 未被写入。
