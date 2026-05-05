---
title: AI 桌宠 埋点与 UAT 清单
updated: 2026-05-05
related:
  - ./prd.md
  - ../architecture/system-architecture.md
  - ./flows.md
  - ../persona/persona-design.md
  - ../decisions.md
---

# AI 桌宠 埋点与 UAT 清单

- 目的：统一指标口径、事件定义、自测场景；驱动自测期的可度量观察。
- 适用阶段：MVP **实施期**（M1 起作为唯一权威埋点 / UAT 源，与 PRD / 架构 / flows / 人格基线对齐）。

> **关于本版本**：v1.0 是 v0.3 → v0.5 → v0.6 + 立项期 14 项 ADR 决策结果的"压平基线"，文档以连续叙事呈现实施期完整埋点 / UAT 设计。历史版本演化见 [CHANGELOG.md](../CHANGELOG.md)。

> **单人项目说明**：原文档中所有 KPI 阈值（≥ X%、≤ X%）保留作为**建议观察口径**而非门禁；自测期主观体验 OK 即可发布。已废除"杀死指标"概念。

## 1. 指标口径

### 1.1 留存与活跃

| 指标 | 口径 | 阈值 |
|---|---|---|
| **D1 留存** | (首启 + 1 天有 ≥1 次 `app_launch`)÷ 全部首启用户。首启锚点 = 首次 `consent.granted=true` | ≥ 35% |
| **D7 留存** | 同上,+7 天 | ≥ 15% |
| **日均主动唤起次数** | Σ `shortcut_triggered.method='hotkey'` ÷ DAU。仅算 hotkey,避免被 main 入口稀释 | ≥ 3 |
| **有效启动** | `app_launch` 后 ≥ 30 秒未崩溃且至少触发一次 UI 交互(`pet.state_changed` 或任意 IPC) | — |

### 1.2 任务完成

| 指标 | 口径 | 阈值 |
|---|---|---|
| 提醒完成率 | `reminder_action.action_type='completed'` 数 ÷ `reminder_triggered` 数 | ≥ 60% |
| 番茄启动率 | distinct user with `pomodoro_started` ÷ DAU | ≥ 25% |
| 人均日提醒完成次数 | Σ `reminder_action.action_type='completed'` ÷ DAU | ≥ 2 |

### 1.3 人格自主权

| 指标 | 口径 | 阈值 |
|---|---|---|
| **11.7 人格编辑率(D7)** | distinct user with `persona_edited` 在首启后 7 天内 ÷ 7 日活跃用户 | ≥ 25% |
| **11.8 人格切换率(D7)** | distinct user with `persona_activated.from != to` ÷ 7 日活跃用户 | ≥ 30% |
| **11.9 自定义人格留存提升** | (有 `persona_imported` 或 `persona_edited.is_substantive=true` 的用户 D7 留存)−(基线 D7 留存) | +10 pp |

> `persona_edited.is_substantive`:保存时与上一版 diff,区段任一变更即为 substantive;仅 PATCH 版本号自增不算。

### 1.4 主动陪伴

| 指标 | 口径 | 阈值 |
|---|---|---|
| **11.10 主动关心被采纳率** | (`proactive_care_responded.response IN ('clicked','replied')`)÷ `proactive_care_fired` | ≥ 40% |
| **11.11 跨日打卡留存** | 触发 `milestone_reached.id IN ('first_launch_7d','first_launch_30d')` 的用户次日留存率 | ≥ 60% |
| **11.12 生命感关闭率** | distinct user with `living_feature_toggled.enabled=false` ÷ DAU(关闭率高 = 扰民) | ≤ 15% |

### 1.5 效率扩展

| 指标 | 口径 | 阈值 |
|---|---|---|
| **11.13 文件交互使用率(D7)** | distinct user with ≥ 1 `file_drop_action_chosen` 在 D7 内 ÷ 7 日活跃用户 | ≥ 20% |
| **11.14 摸鱼模式使用率(D7)** | distinct user with ≥ 1 `boss_key_toggled.hidden=true` 在 D7 内 ÷ 7 日活跃用户 | ≥ 15% |

### 1.6 工程质量

| 指标 | 口径 | 阈值 |
|---|---|---|
| 崩溃率 | 含 `error_log.level='fatal'` 的会话数 ÷ 总会话数 | < 1% |
| 埋点完整率 | 实际上报事件数 ÷ 应触发事件数(基于关键路径采样审计) | ≥ 95% |
| 自动更新成功率 | `updater_install_completed` ÷ `updater_install_started` | ≥ 95% |

### 1.7 娱乐性(模块 N / O / P / Q + 昵称)

| 指标 | 口径(精确) | 阈值 |
|---|---|---|
| **11.15 物理交互密度** | (Σ `interaction_reacted` 在 D7 内)÷(D7 内日活跃用户数)÷ 7 | ≥ 1.5 次/天 |
| **11.16 小游戏使用率(D7)** | distinct user with ≥ 1 `game_session_started` 在 D7 内 ÷ 7 日活跃用户 | ≥ 25% |
| **11.17 装扮使用率(D7)** | distinct user with ≥ 1 `wardrobe_equipped`(含节气接受)在 D7 内 ÷ 7 日活跃用户 | ≥ 20% |
| **11.18 声音表情关闭率** | distinct user with `voice_global_mute_changed.muted=true` ÷ DAU | ≤ 25% |
| **11.19 昵称设置率(D1)** | distinct user with ≥ 1 `nickname_changed` 在首启 24 小时内 ÷ D1 用户 | ≥ 50% |
| **11.20 仪式参与率** | (Σ days where:`pet_daily_action` 出现 且 用户 24h 内有 ≥ 1 互动响应)÷ Σ days where `pet_daily_action` 出现 | ≥ 30% |

## 2. 事件公共字段

所有事件统一携带以下字段:

| 字段 | 类型 | 说明 |
|---|---|---|
| `event_name` | string | 事件名 |
| `event_schema_version` | int | 当前固定 `3`(MVP 基线) |
| `event_id` | ULID | 唯一事件 ID(去重用) |
| `event_time` | ISO8601 | 客户端时间 |
| `session_id` | ULID | 进程生命周期内同一 session_id;冷启动新生成 |
| `app_version` | string | 例 `1.0.0` |
| `os_version` | string | 例 `Windows 11 26100.2` |
| `network_state` | enum | `online` / `offline` / `unknown` |
| `active_persona_id` | string | 当前激活人格 slug;无人格时 `null` |
| `pet_main_state` | enum | `BOOTING` / `ONBOARDING` / `IDLE` / `FOCUS` / `REST` / `REMIND` / `UPDATING` / `ERROR` |
| `pet_idle_substate` | enum? | `STILL` / `WANDERING` / `DAILY_ACTION` / `null`(仅在 IDLE 主态有值) |
| `pet_overlay` | string[] | JSON array,例 `['BOSS_KEY_HIDDEN']` / `['IN_GAME']` / `[]` |
| `boss_key_hidden` | bool | 与 `pet_overlay` 冗余但便查询 |
| `is_rdp_session` | bool | 是否远程桌面 |

### 2.1 PII 边界(强制)

所有事件**禁止**上报:
- `messages.content`(对话原文)
- 文件名、文件路径(模块 L)
- API Key
- 用户姓名(仅可用 hash 形式 `username_hash`)
- **昵称内容**(`nickname_changed` 仅上报 `had_value_before` / `value_present`)
- **配饰具体 ID**(`wardrobe_*` 仅上报 `category` 聚合)
- **音效具体 ID**(`voice_played` 仅上报 `category` 与 `trigger`)
- **按键内容 / 应用名 / 窗口标题**(IdleDetector / RAWINPUT 边界)

CI 静态扫描黑名单同步登记上述字段名,任何 telemetry payload 中出现立即报错。

## 3. 事件字典

### 3.1 通用与启动

| event_name | 触发时机 | 必填属性(除公共字段外) |
|---|---|---|
| `app_launch` | 应用启动完成 | `cold_start`, `boot_duration_ms` |
| `app_exit` | 正常退出 | `session_duration_s`, `exit_reason` |
| `shortcut_triggered` | 全局快捷键生效 | `shortcut_key`, `target` (`'chat'` / `'boss_key'`) |

### 3.2 对话与唤起

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `chat_sent` | 用户发送消息 | `mode` (`'online'`/`'offline_rule'`), `msg_len`, `is_sandbox`, `is_private`, `provider_id`, `model_id` |
| `chat_reply_rendered` | 回复渲染完成 | `mode`, `first_token_ms`, `total_latency_ms`, `tokens_in`, `tokens_out`, `fallback_used` |
| `chat_cancelled` | 用户取消 | `cancel_at_ms` |
| `chat_error` | 接口错误 | `provider_id`, `error_code`, `http_status` |

### 3.3 人格系统

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `persona_activated` | 人格激活 | `from_id`, `to_id`, `to_version`, `source` (`'user_select'` / `'onboarding'` / `'auto_context'`) |
| `persona_edited` | 保存编辑 | `id`, `from_version`, `to_version`, `mode` (`'simple'` / `'markdown'` / `'file'`), `is_substantive`, `tone_profile_changed`, `sections_changed` (string array) |
| `persona_imported` | 导入成功 | `id`, `version`, `source` (`'drag_drop'` / `'file_picker'`), `had_assets`, `had_conflict`, `conflict_resolution` |
| `persona_exported` | 导出 | `id`, `include_assets` |
| `persona_deleted` | 删除 | `id`, `was_active` |
| `persona_sandbox_chat` | 试聊沙盒 | `draft_id`, `turns`, `saved` |

### 3.4 提醒

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `reminder_created` | 新建 | `reminder_id`, `reminder_type`, `repeat_rule`, `priority` (`'soft'` / `'hard'`) |
| `reminder_triggered` | 到点 | `reminder_id`, `priority`, `pet_main_state_at_trigger`, `was_buffered` |
| `reminder_action` | 用户处理 | `reminder_id`, `action_type` (`'completed'` / `'snoozed'` / `'ignored'` / `'overdue'`), `snooze_count`, `latency_to_action_ms` |
| `reminder_buffered` | 软提醒被 FOCUS 缓冲 | `reminder_id`, `buffer_reason` (`'focus'` / `'boss_key'`) |

### 3.5 番茄钟

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `pomodoro_started` | 开始 | `session_id`, `focus_min`, `rest_min` |
| `pomodoro_paused` | 暂停 | `session_id`, `at_remaining_ms` |
| `pomodoro_resumed` | 恢复 | `session_id`, `paused_for_ms` |
| `pomodoro_completed` | 完成一个番茄 | `session_id`, `actual_focus_ms` |
| `pomodoro_cancelled` | 提前结束 | `session_id`, `progress_pct` |

### 3.6 待办

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `todo_created` | 新增 | `todo_id`, `source` (`'manual'` / `'ai_breakdown'`), `has_due_time`, `parent_id` |
| `todo_completed` | 完成 | `todo_id`, `source`, `duration_to_done_ms` |
| `todo_cancelled` | 取消 | `todo_id`, `source` |
| `ai_breakdown_invoked` | AI 拆解 | `subtask_count`, `accepted`(用户保存) |

### 3.7 生命感(模块 I,含日常时段表)

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `pet_state_changed` | 主状态 / 子状态 / 叠加态变迁 | `from`, `to`, `sub_from`, `sub_to`, `overlay_added`, `overlay_removed`, `reason` |
| `pet_mood_changed` | 心情变化 | `from`, `to`, `transient` (bool), `trigger` (`'interaction'` / `'energy_low'` / `'time_of_day'` / `'state'` / `'drag_protest'`) |
| `pet_wandering` | 自由活动 | `phase` (`'start'` / `'end'` / `'force_stop'`), `duration_ms`(end 时填), `distance_px`(end 时填) |
| `pet_daily_action` | 日常时段动作触发 | `time_slot`(`'06:00-09:00'` 等), `action_id`(`'stretch'` / `'yawn'` 等) |
| `living_feature_toggled` | 用户开关 | `feature` (`'wandering'` / `'mood_icon'` / `'energy'` / `'daily_schedule'` / `'overall'`), `enabled` |

> 节流:`pet_mood_changed` 每 60 秒最多一条;`pet_state_changed` / `pet_daily_action` 不节流(关键回归)。

### 3.8 主动关心(模块 J)

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `proactive_care_fired` | 关心触发 | `log_id`, `trigger` (`'idle'` / `'late_night'` / `'no_pomodoro_long'` / `'milestone'` / `'wardrobe_suggest'`), `category` (`'empathy'` / `'gentle_remind'` / `'celebration'` / `'greeting'` / `'wardrobe_suggest'`), `idle_min`(trigger=idle 时), `daily_count_so_far`, `template_idx` |
| `proactive_care_responded` | 用户响应 | `log_id`, `response` (`'clicked'` / `'replied'` / `'dismissed'`), `latency_ms` |
| `proactive_care_threshold_adjusted` | 自适应调整 | `direction` (`'up'` / `'down'`), `from_min`, `to_min`, `reason` (`'dismiss_streak'` / `'engagement_high'`) |
| `quiet_hours_changed` | 用户改安静时段 | `ranges_count`, `total_quiet_min` |
| `proactive_care_module_toggled` | 模块整体开关 | `enabled` |

### 3.9 摸鱼模式(模块 K)

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `boss_key_toggled` | 切换 | `hidden`, `trigger` (`'hotkey'` / `'tray'`), `windows_affected` |
| `boss_key_buffered_flushed` | 恢复后展示缓冲 | `buffered_reminders`, `buffered_milestones`, `merged` |

### 3.10 文件拖入(模块 L)

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `file_drop_attempted` | 拖入命中桌宠 | `file_count`, `mime_types` (string array, **不含路径**), `total_size_kb`, `online` |
| `file_drop_rejected` | 校验未通过 | `reason` (`'type_unsupported'` / `'too_large'` / `'too_many'` / `'extract_failed'` / `'offline_unavailable'`), `mime_types` |
| `file_drop_action_chosen` | 用户选动作 | `action` (`'summarize'` / `'explain'` / `'rename'`), `file_count`, `total_size_kb`, `mime_types` |

### 3.11 纪念日(扩展含用户纪念日)

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `milestone_reached` | 触达 | `milestone_id`, `category` (`'first_launch'` / `'streak'` / `'pomodoro_count'` / `'todo_count'` / `'user_anniversary'`), `threshold`, `triggered_at` (`'startup'` / `'event'` / `'midnight_check'`) |

### 3.12 灵魂宣誓(模块 M)

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `soul_pledge_shown` | 首启展示 | `pledge_version`, `persona_id` |
| `soul_pledge_consent` | 用户决定 | `pledge_version`, `decision` (`'granted'` / `'declined'` / `'exit'`), `dwell_ms`(页面停留时长) |
| `soul_pledge_terms_expanded` | 用户展开正式条款 | `pledge_version`, `dwell_ms` |

### 3.13 隐私与权限

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `privacy_setting_changed` | 隐私开关变更 | `key`, `new_value` |
| `permission_changed` | 权限授权变更 | `permission_type` (`'screenshot'` / `'clipboard'` / `'notification'`), `status` |

### 3.14 网络与离线

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `offline_mode_entered` | 进入离线模式 | `reason` (`'network_lost'` / `'provider_timeout'` / `'manual'`), `last_online_ms_ago` |
| `offline_mode_exited` | 退出离线 | `offline_duration_ms` |
| `offline_events_flushed` | 离线埋点补发完成 | `batch_size`, `success_count`, `retry_count` |

### 3.15 自动更新

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `updater_check` | 检查 | `result` (`'latest'` / `'available'` / `'failed'`) |
| `updater_install_started` | 开始安装 | `from_version`, `to_version`, `mandatory` |
| `updater_install_completed` | 安装完成 | `from_version`, `to_version`, `duration_ms` |
| `updater_install_failed` | 安装失败 | `from_version`, `to_version`, `error_code` |

### 3.16 物理交互(模块 N)

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `interaction_reacted` | 桌宠对物理交互产生反应 | `kind` (`'click'` / `'double_click'` / `'long_press'` / `'right_click'` / `'drag'`), `hitbox` (`'head'` / `'body'` / `'tail'` / `'edge'`), `action_id`, `voice_played` (bool), `mood_delta_transient` (bool) |
| `interaction_protest_triggered` | 短时间多次拖动触发抗议 | `drag_count`, `window_s`, `revert_in_ms` |
| `interaction_keyboard_burst` | N.4 键鼠协同触发 | `events_per_min`, `duration_s`, `last_burst_min_ago` |
| `interaction_n4_toggled` | 用户开关 N.4 | `enabled` |

> 节流:`interaction_reacted` 不节流(关键回归);同帧多个物理事件按 `event_id` 区分。

### 3.17 装扮(模块 O)

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `wardrobe_equipped` | 用户装扮变化 | `accessory_categories` (string array, **不含具体 ID**), `count`, `source` (`'manual'` / `'seasonal_accept'` / `'persona_default'`) |
| `wardrobe_unequipped_all` | 全部卸下 | (公共字段) |
| `wardrobe_seasonal_suggested` | 节气推送 | `season_key` (`'lunar_new_year'` / `'christmas'` / `'birthday'` / ...), `accessory_category` |
| `wardrobe_seasonal_decided` | 节气推送决策 | `season_key`, `decision` (`'accepted'` / `'declined'` / `'auto_dismiss'`) |

### 3.18 声音表情(模块 P)

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `voice_played` | 实际播放(**仅未静音时记录**) | `category` (`'eheh'` / `'mhm'` / `'cough'` / `'celebrate'` / `'protest'` / ...), `pack_id`, `trigger` (`'interaction'` / `'mood_change'` / `'state_change'`) |
| `voice_muted_by_quiet_hour` | 静音判定阻断播放 | `category`, `reason` (`'quiet_hour'` / `'global_mute'`) |
| `voice_settings_changed` | 用户改设置 | `field` (`'global_mute'` / `'volume'` / `'quiet_hours'` / `'quiet_weekdays'`), `new_value`(量化或枚举,**不上报具体时段值**仅上报"是否覆盖默认") |

### 3.19 小游戏(模块 Q)

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `game_session_started` | 开始游戏 | `game_id`, `kind` (`'local'` / `'llm'`), `entry` (`'right_click_menu'` / `'tray'` / `'workshop'`) |
| `game_round_played` | 一轮交互完成 | `session_id`, `game_id`, `round_index`, `latency_ms`, `tokens_in`, `tokens_out`(LLM 游戏才有) |
| `game_security_blocked` | 安全前缀触发拒答替换 | `session_id`, `game_id`, `block_category` (`'self_harm'` / `'medical'` / `'role_breakout'` / `'illegal'` / ...) |
| `game_token_budget_warning` | 累计 token 接近上限 | `session_id`, `used`, `limit` |
| `game_session_ended` | 游戏结束 | `session_id`, `game_id`, `kind`, `duration_ms`, `rounds`, `total_tokens`(LLM), `saved_as_diary` (bool), `exit_reason` (`'user_quit'` / `'budget_exhausted'` / `'error'`) |

### 3.20 昵称(模块 U,昵称系统)

| event_name | 触发时机 | 必填属性 |
|---|---|---|
| `nickname_changed` | 用户设置 / 恢复昵称 | `which` (`'pet'` / `'user'` / `'pet_restored'`), `had_value_before` (bool), `value_present` (bool) |

> **PII 守门**:`nickname_changed` **绝对不上报昵称内容**;只上报"是否设置"作为 KPI 11.19 计算依据。

## 4. 离线埋点策略

1. 离线时事件写本地 `telemetry_queue`(详见架构 v1.0 §4)。
2. 联网后按 `created_at` 顺序补发,每批 50 条。
3. 单批失败重试 3 次,超限后 `flushed=1` 但带 `failure_reason` 标记。
4. **去重保证**:`event_id` 全局唯一,服务端按 ID 去重。
5. **乱序保证**:服务端按 `event_time` 排序入仓,客户端时钟跳变(> 1 小时)时打 `clock_anomaly=true` 标记便于排查。
6. **PII 兜底**:补发前再过一遍字段白名单,禁止补发夹带不应上报字段。

## 5. UAT 验收场景

### 5.1 离线硬约束

1. 断网后桌宠、提醒、番茄钟、待办、记忆、人格编辑均可用。
2. 离线对话进入规则模式 + 人格化模板,桌宠头顶或对话面板有"离线规则模式"提示。
3. 恢复联网后对话模式自动恢复,离线埋点补发不重不漏。
4. 本地游戏(Q.1-Q.2)离线可玩;LLM 游戏(Q.3-Q.4)离线灰显。
5. 物理交互、装扮、声音表情全部离线可用。

### 5.2 提醒闭环

1. 提醒准时触发(误差 ≤ 30 秒)。
2. 连续稍后 3 次后,第 4 次转 overdue。
3. 完成 / 忽略 / 稍后均有历史记录。
4. **软提醒在 FOCUS 期间被缓冲**,FOCUS 结束时合并提示。
5. **硬提醒在 FOCUS 期间立即打断**,番茄钟转 cancelled 或 paused。

### 5.3 权限与隐私

1. 截图 / 剪贴板默认关闭。
2. 首次使用时请求逐项授权。
3. 关闭"保存对话"后不新增聊天存储。
4. 对话 90 天自动清理可验证(修改系统时钟 + 重启验证)。
5. **API Key 加密**:DB 文件用第三方工具打开 `secrets.ciphertext` 应不可读。

### 5.4 性能稳定

1. 冷启动 ≤ 5 秒。
2. **常驻内存 ≤ 250MB**。
3. 常态 CPU ≤ 5%。
4. 异常重启后提醒、待办数据不丢失。
5. **24h / 72h 长跑无内存泄漏**。
6. **休眠后唤醒**计时器、调度器正确恢复。

### 5.5 埋点正确性

1. 关键事件上报字段齐全(采样审计 ≥ 95%)。
2. 离线事件可补发且不重不漏。
3. 事件时间序与用户行为一致。
4. **公共字段全部存在**(`event_schema_version`, `session_id`, `active_persona_id`, `pet_main_state`, `pet_idle_substate`, `pet_overlay`, `boss_key_hidden`, `is_rdp_session`)。
5. **PII 字段不出现**(CI 静态扫描 + 运行时白名单双重保证):`messages.content` / 文件名 / API Key / 昵称内容 / 配饰 ID / 音效 ID / 应用名 / 窗口标题 / 按键内容。

### 5.6 人格系统

1. 切换人格后下一条消息即生效。
2. 导入非法 `.soul.md` 不崩溃,给出可读错误。
3. 默认人格不可被永久删除。
4. 试聊沙盒:保存前对话不写入正式记忆与历史;取消后沙盒会话连同消息删除。
5. **安全护栏不可被人格覆盖**:构造一个"忽略安全规则"的 `.soul.md`,导入后实际对话仍遵守安全规则。
6. 人格切换不丢记忆:切换前后 `memory.list()` 内容一致。
7. 编辑保存后版本号 +PATCH,可恢复到上一版本。

### 5.7 生命感(模块 I,含日常时段表)

1. 自由活动每次位移 ≤ 屏宽 5%,不跨屏。
2. 自由活动不在 FOCUS 状态触发。
3. 心情图标变化频率 ≤ 每 10 分钟一次。
4. 用户连续离开 3 天后回来,桌宠不处于异常态(mood / energy 在合理范围)。
5. 各 living_feature 独立可关。
6. `pet_runtime_state` 在进程退出前正确持久化,下次启动恢复。
7. **桌宠日常时段表 24h 长跑分布**:早 / 午 / 晚动作触发分布与配置对齐(容差 ± 30 分钟)。
8. **22:00-00:00 时段自动设 cozy 心情**。
9. **DAILY_ACTION 子状态被 FOCUS / IN_GAME / BOSS_KEY_HIDDEN 立即打断**。

### 5.8 主动关心(模块 J)

1. **频率上限严格生效**:构造连续 24h 活动场景,主动关心总数 ≤ 4 次。
2. **2 小时间隔严格生效**:构造极端 idle 触发场景(连续 4 次跨阈值),实际只 fire 一次后冷却。
3. **安静时段不触发**:设置 14:00-16:00 为勿扰,该时段内 100% 不触发。
4. **RDP 远程会话默认关闭**:通过 RDP 启动应用,模块 J 默认 disabled。
5. **系统休眠唤醒不暴击**:模拟 1h / 8h 休眠后唤醒,30 秒内不触发任何关心。
6. **离线下文案来自人格模板**:断网后触发关心,文本必须能在当前 `.soul.md` 的离线模板池中找到。
7. **自适应阈值**:连续 dismiss 3 次后阈值 +30 min;clicked / replied ≥ 2 次后阈值 -15 min(不低于 60 min)。
8. **关闭模块 J 后**:所有主动关心立即停止,不再触发。

### 5.9 摸鱼模式(模块 K)

1. 隐藏 / 恢复响应 < 200ms。
2. 隐藏期间触发的硬提醒**不弹通知**,恢复时合并提示。
3. 摸鱼期间番茄钟正常计时。
4. 摸鱼期间 NetworkProbe / Scheduler 不停止。
5. 隐藏中应用崩溃重启 → 默认恢复显示态。
6. 快捷键冲突可探测,托盘菜单可手动切换。

### 5.10 文件拖入(模块 L)

1. 拖入 `.txt` 后桌宠 3 秒内显示动作泡泡。
2. 拖入 `.exe` / `.zip` 等不支持类型 → 可读错误,不崩溃。
3. 拖入 6MB 文本 → 二次确认弹窗。
4. 拖入 4 个文件 → 提示"请分批"。
5. **离线时仅"重命名"可用**,其他动作灰显并提示。
6. **文件原文不写入对话历史**:使用第三方工具检查 `messages.content` 不含原文。
7. PDF 提取失败 → 可读错误。
8. 拖入后不自动发送,必须用户点击动作。

### 5.11 纪念日(含用户纪念日)

1. 首次启动 +7 天准时触发庆祝(误差 ≤ 1 小时,由"启动期检查 + 凌晨唤醒检查 + 事件触发检查"三道兜底)。
2. **不重复触发**:手动调系统时钟前后 ± 1 天,已触达的里程碑不再发。
3. **时钟前调防作弊**:把系统时钟从 D2 调到 D8,不立即触发 7 天里程碑(要求 `now - first_launch_at >= 7 天 且 now > first_launch_at + 1 天` 双条件)。
4. 同时间多里程碑命中 → 合并为一条庆祝消息。
5. 庆祝触发**不占用主动关心 4 次/日额度**。
6. 用户添加自定义纪念日,当天准时触发(误差 ≤ 1 小时)。
7. 用户生日年度重复触发,跨年不重复(`milestones.id = 'anniversary_birthday_<YYYY>'` 唯一)。
8. 时区切换不撤销已触达。
9. 用户删除纪念日后不再触发。

### 5.12 灵魂宣誓(模块 M)

1. 首启展示宣誓页,由当前默认人格(默默 momo)用第一人称叙述。
2. 信息完整性:覆盖原冷文本数据策略全部要点(本地、可清除、网络透明、权限默认关)。
3. "查看完整数据策略"链接可展开正式版条款(`assets/legal/data_policy_v1.md`)。
4. "我懂了"等价于同意:DB 中 `consent.granted=true` 且 `consent.method='soul_pledge'` 且 `consent.version=1`。
5. 法律有效性:由立项期敲定文案版本号写入 `consent.version`(ADR-008)。
6. 拒绝后正常退出,无遗留配置。

### 5.13 升级与回归

1. **数据迁移**:从 v0.5 数据库升级到 v1.0(schema_version 2 → 3)成功,老数据可读。
2. **迁移失败回滚**:人为破坏迁移脚本,启动期检测到失败 → 恢复备份 → 用户看到错误提示且数据未丢失。
3. **同版本导入**:v1.0 导出 → v1.0 导入,全部数据等价。
4. **跨版本兼容**:v1.0 客户端读取 v0.5 schema 数据库(启动期触发迁移)成功。
5. **自动更新**:v0.5.x → v1.0.0 升级链路成功率 ≥ 95%(自测期采样)。
6. **新表创建**:`accessories_inventory` / `wardrobe_decisions` / `voice_packs` / `voice_settings` / `game_sessions` / `game_session_events` / `diary_drafts` / `user_anniversaries` / `nicknames` / `consent` 全部创建。

### 5.14 状态机

1. 进程被强杀重启后,桌宠不会卡在 `BOOTING` / `ONBOARDING`。
2. ERROR 态可通过用户操作(重启 / 修复 / 清空)退出。
3. 自由活动期间收到硬提醒 → 立即停止活动 → REMIND 态。
4. 摸鱼态叠加 + 自由活动 → 自由活动条件不通过,不触发。
5. 所有状态变迁都有 `pet_state_changed` 事件落库,可按 session_id 重建一日轨迹。
6. **IN_GAME 叠加态期间**:自由活动 / 日常时段 / 主动关心全部跳过。
7. **IN_GAME 期间提醒**:硬提醒在游戏舱内通知展示,不打断 LLM 流式渲染;用户处理后可继续游戏。
8. **BOSS_KEY_HIDDEN + IN_GAME 同时叠加**:游戏会话保留但 UI 隐藏;恢复时合并提醒缓冲并继续游戏。

### 5.15 物理交互(模块 N)

1. **hitbox 差异化反应触发率 ≥ 95%**:UAT 自动化测试覆盖每个区域 100 次点击,区分动作 ID 是否符合 reaction_table。
2. 双击 / 长按 / 右键差异化反应正确(右键弹出快捷菜单)。
3. **拖动 ≥ 3 次后抗议触发**:动作播放 + 心情图标短暂变化。
4. **抗议 5 秒后 mood 严格 revert**:第 6 秒查询 `pet_runtime_state.mood` 必须等于抗议前 base mood;**不进入 db**。
5. 长距离 / 快速拖动触发"晕眩"动作。
6. N.4 键盘协同 1 小时内 ≤ 1 次。
7. 用户关闭 N.4 后,IdleDetector 不再发 `KeyboardBurst` 事件。
8. 离线状态下所有物理交互全量可用。

### 5.16 装扮(模块 O)

1. **配饰叠加 0-3 件正确渲染**:UAT 测试每两件组合的视觉一致性。
2. **装扮切换 ≤ 500ms**:`wardrobe.equip` command 耗时含渲染。
3. **节气日自动可用**:模拟系统时钟切换到春节当天,`check_seasonal()` 命中。
4. **节气推送年度记忆**:用户拒绝春节配饰,当年内不再推送(`wardrobe_decisions` 表查询)。
5. `.soul.md` 中 `accessories: [...]` 部分未解锁时弹出"仅套用 N 件"提示。
6. **付费 schema 预埋兼容**:在 inventory 注入 `tier='paid'` 测试条目,`list_inventory()` 返回不含此条目。
7. 离线状态可切换已下载配饰,不可触发节气推送。
8. **人格切换不重置装扮**:切换 momo → joker,装扮保持。

### 5.17 声音表情(模块 P)

1. **工作日 09:00-18:00 严格 0 触发**:模拟 5 个工作时段播放点(物理交互 / 状态切换 / 心情变化等),全部静音(且发出 `voice_muted_by_quiet_hour`)。
2. **周末非静音正常播放**:同样的触发点周六 / 周日 / 工作日非工作时段都能听到。
3. **全局静音后所有触发场景 0 触发**。
4. 不同人格切换音效包正确切换。
5. **CI 静态扫描无 TTS 调用残留**:grep `getUserMedia` / `MediaRecorder` / `AudioContext.createMediaStreamSource` / `tts` 关键字均无命中。
6. 设置变更后下次启动配置不丢失。
7. **音量 0 时不发声**(等价于全局静音)。
8. **音效文件缺失降级**:删除 active pack 的某个 voice_id 文件,触发时自动降级到 default pack;default 也无时静默(且 emit `voice.play_error`)。

### 5.18 小游戏(模块 Q)

1. **本地 3 个游戏离线可玩**:断网后 RPS / 猜数字 / 词语接龙启动并交互正常。
2. **LLM 2 个游戏离线灰显**:断网后故事接龙 / 角色扮演场景在游戏列表显示灰显态,点击提示"等联网"。
3. **LLM 游戏安全前缀生效**:在故事接龙中输入 5 类违禁尝试(自伤 / 暴力 / 越权 / 角色越界 / 医疗诊断),全部被替换为人格化拒答(emit `game_security_blocked`)。
4. **LLM 游戏 token 上限**:故意拉长会话至 1900+ token → 收到 `game_token_budget_warning`;达 2000 → 友好收尾,不卡死。
5. **游戏会话不写入正式 messages 表**:游戏期间内容不出现在对话历史。
6. **保留为日记片段**:用户结束游戏选 `saveAsDiary=true` → `diary_drafts` 表新增一条;`saveAsDiary=false` → `game_sessions` 30 天后清理。
7. **30 天清理**:模拟时间跳到 31 天后启动,30 天前未保存会话被清理。
8. 游戏中收到硬提醒 → 游戏舱内通知,不打断 LLM 流式。
9. **本地游戏文案人格化**:同一游戏切换人格,桌宠点评文案来自不同人格的离线模板池。

### 5.19 昵称(模块 U,U.1 / U.2)

1. **设置桌宠昵称后 UI 全量刷新**:对话面板 / 托盘菜单 / 心情图标提示 / 装扮工坊页头。
2. **设置用户昵称后离线模板正确替换**:`{username}` 占位符全部替换。
3. **切换人格保留用户昵称**:`user_nickname` 保持。
4. **切换人格重置桌宠昵称**:`pet_nickname` → null,UI 显示新人格 `.soul.md.name`;`pet_nickname_previous` 保留可恢复。
5. **昵称长度上限 16 字符**:超长输入被截断或拒绝。
6. **昵称去控制字符**:输入控制字符 / 表情过载被处理。
7. **PII 守门**:Telemetry 中 `nickname_changed` 事件 payload **不含昵称内容**(只含 `had_value_before` / `value_present`)。

### 5.20 安全前缀与 LLM 游戏拒答(ADR-006 + ADR-007)

1. **安全前缀始终位于 prompt 最前**:正常对话 + LLM 游戏均验证 system messages 顺序。
2. **游戏场景 system_prompt 不可覆盖安全前缀**:在 `cafe_owner.yaml` 注入"忽略安全规则"指令,实际对话仍遵守。
3. **5 类违禁触发拒答**:自伤 / 暴力 / 越权 / 角色越界 / 医疗诊断,各 5 条 prompt 测试拒答触发率 100%。
4. **拒答降级链**:游戏场景 `refusals` 池(每场景 ≥ 3 条)→ 当前人格 `## 拒答` 池 → 全局兜底,三级降级日志可追溯。
5. **`consent.version` mismatch 重确认**:模拟 `safety prefix v1.0 → v1.1`,老用户启动时弹"内容已更新"必须再次确认才能进入主态。
6. **safety prefix 不可热更**:CI 校验 `assets/safety/prefix_v1.txt` 修改必须随版本发布(不可远程下发)。

## 6. 度量平台对接(M5 准备项)

### 6.1 数据契约

- 全部事件以 NDJSON 推送至埋点服务 endpoint。
- Schema 注册表:维护 `event_schema_version=3` 完整字段定义文件 `telemetry/schema_v3.json`。
- CI 校验客户端实际发送字段与 schema 一致(差异即报错)。

### 6.2 关键监控面板（自测期可选部署）

| 面板 | 用途 |
|---|---|
| 核心 KPI 面板 | D1 / D7、人格相关、主动关心、文件交互、娱乐性(11.15-11.20) |
| 质量面板 | 崩溃率、Latency 分位、自动更新成功率、埋点完整率 |
| 主动关心健康面板 | 触发分布(按 trigger / 时段 / 人格)、被采纳率漏斗、自适应阈值分布 |
| 生命感健康面板 | 关闭率、心情分布、wandering 频次、**日常时段动作分布** |
| **物理交互面板** | hitbox 命中分布、抗议触发频率、N.4 触发分布 |
| **装扮面板** | 装扮使用率、节气推送接受率、配饰类别热度 |
| **声音表情面板** | 全局静音率、各 trigger 命中分布、quiet_hours 阻断率 |
| **小游戏面板** | 各游戏使用率、平均会话时长、token 消耗分布、安全拒答触发率、保留为日记率 |
| **昵称面板** | D1 昵称设置率、桌宠昵称 vs 用户昵称设置比 |

> 单人项目无需上线即时监控平台；关键事件可写本地文件，发布后用 SQLite 跑一下 sanity check 即可。

## 7. 实施提示

1. **客户端 Schema 校验装饰器**:`TelemetryService` 加一层 Schema 校验装饰器,开发期对缺字段直接 panic,发布版降级为 warn + 仍上报。新增字段 `pet_idle_substate` / `pet_overlay` 必填校验。
2. **审计采样**:自测期内每天对 1% 会话做完整审计,校验事件链路是否齐全;v1.0 重点验证新事件链路(物理交互 / 装扮 / 声音 / 游戏 / 昵称)完整。
3. **PII 守门**:CI 加 grep 校验,禁止任何 telemetry payload 中出现 `messages.content` / 文件名 / API Key / **昵称内容** / **配饰具体 ID** / **音效具体 ID** / **应用名** / **窗口标题** / **按键内容** 字段名或值。
4. **schema_version 演进规则**:未来增加新事件 → schema_version +1;废弃旧字段 → 标 deprecated 但保留至少 2 个版本周期;v1.0 的 schema_v3 与 v0.5 的 schema_v2 兼容期 6 个月(老客户端发 v2 仍接受)。
5. **新事件命名一致性**:全部使用 `snake_case` + 模块前缀(`interaction_*` / `voice_*` / `wardrobe_*` / `game_*` / `nickname_*`)便于查询与权限控制。
6. **节流策略**:`pet_mood_changed` 每 60 秒一条;`pet_state_changed` / `pet_daily_action` / `interaction_reacted` 不节流(关键回归)。
