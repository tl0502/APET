-- Migration v1: Initial schema (架构 v1.1 §4 完整版)
-- 实施: M1 D5 I.1 MigrationService
-- 设计参考:
--   - docs/AIPET-obsidian/架构设计/2026-05-01-system-architecture-v1.0.md §4
--   - ADR-015 三形态架构 → conversations 表加 title / archived 字段
--
-- 一次性创建所有表(空表 CREATE 极快,避免后续 M2-M5 频繁加 migration);
-- 后续模块若需加字段或新表,加 002_xxx.sql / 003_xxx.sql 等。

-- ==== 元信息(架构 §4 line 286) ====
CREATE TABLE schema_version (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);

-- ==== 配置(kv,active_conversation_id 等运行时配置走此表) ====
CREATE TABLE config (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- ==== 加密敏感数据(DPAPI 输出存 ciphertext;I.2 CryptoService 写入) ====
CREATE TABLE secrets (
  key TEXT PRIMARY KEY,           -- e.g. 'openai.api_key'
  ciphertext BLOB NOT NULL,       -- DPAPI 输出
  updated_at TEXT NOT NULL
);

-- ==== 同意记录(灵魂宣誓 / classic Onboarding 通过后写入) ====
CREATE TABLE consent (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  granted INTEGER NOT NULL DEFAULT 0,
  method TEXT NOT NULL,            -- 'soul_pledge' | 'classic'
  version INTEGER NOT NULL,        -- safety prefix / data policy 版本号
  accepted_at TEXT NOT NULL
);

-- ==== 人格 ====
CREATE TABLE personas (
  id TEXT PRIMARY KEY,            -- slug
  name TEXT NOT NULL,
  version TEXT NOT NULL,
  source TEXT NOT NULL,           -- 'builtin' | 'user' | 'imported'
  file_path TEXT NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE persona_snapshots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  persona_id TEXT NOT NULL,
  version TEXT NOT NULL,
  content TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (persona_id) REFERENCES personas(id) ON DELETE CASCADE
);

-- ==== 记忆 + 昵称 ====
CREATE TABLE memory (
  key TEXT PRIMARY KEY,           -- e.g. 'username', 'wake_time'
  value TEXT NOT NULL,
  source TEXT NOT NULL,           -- 'user_set' | 'inferred'
  updated_at TEXT NOT NULL
);

CREATE TABLE nicknames (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  pet_nickname TEXT,              -- nullable,null 表示用 .soul.md 默认
  pet_nickname_previous TEXT,     -- 上次自定义值,用于"恢复"按钮
  user_nickname TEXT,
  updated_at TEXT NOT NULL
);

-- ==== 对话(ADR-015 三形态共享 ConversationStore) ====
CREATE TABLE conversations (
  id TEXT PRIMARY KEY,            -- ULID
  persona_id TEXT NOT NULL,
  title TEXT,                     -- 用户自定义会话名(M3 B.3.d UI 使用;NULL 时显示"未命名 + 时间")
  archived INTEGER NOT NULL DEFAULT 0,  -- 归档标记
  started_at TEXT NOT NULL,
  last_activity_at TEXT NOT NULL,
  is_sandbox INTEGER NOT NULL DEFAULT 0  -- 试聊沙盒标记
);
CREATE INDEX idx_conversations_active ON conversations(archived, last_activity_at DESC);

CREATE TABLE messages (
  id TEXT PRIMARY KEY,            -- ULID
  conversation_id TEXT NOT NULL,
  role TEXT NOT NULL,             -- 'user' | 'assistant' | 'system'
  content TEXT NOT NULL,
  mode TEXT NOT NULL,             -- 'online' | 'offline_rule'
  created_at TEXT NOT NULL,
  FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);
CREATE INDEX idx_messages_conv ON messages(conversation_id, created_at);

-- ==== 任务三件套(M2 TaskService 使用) ====
CREATE TABLE reminders (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  trigger_type TEXT NOT NULL,     -- 'once' | 'daily' | 'weekly' | 'cron'
  trigger_spec TEXT NOT NULL,     -- ISO8601 / cron 表达式
  priority TEXT NOT NULL DEFAULT 'soft',  -- 'soft' | 'hard'
  enabled INTEGER NOT NULL DEFAULT 1,
  snooze_count INTEGER NOT NULL DEFAULT 0,
  next_fire_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE reminder_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  reminder_id TEXT NOT NULL,
  fired_at TEXT NOT NULL,
  action TEXT NOT NULL,           -- 'completed' | 'snoozed' | 'ignored' | 'overdue'
  snooze_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_reminder_history_rid ON reminder_history(reminder_id);

-- todos: #29 落地（原 M2 placeholder schema 含 source/parent_id/done_at 未被任何代码消费，
-- 2026-05-23 整体替换。lesson §2 零迁移：001 未对外发布前直接改）
CREATE TABLE todos (
  id           TEXT PRIMARY KEY NOT NULL,         -- ULID
  title        TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'open',      -- 'open' | 'done' | 'cancelled'
  due_at       TEXT,                              -- RFC3339 UTC; NULL = 无截止
  reminder_id  TEXT,                              -- 软引用 reminders.id; NULL = 无关联
  order_index  REAL NOT NULL DEFAULT 0,           -- 分数排序（拖排序）
  priority     TEXT NOT NULL DEFAULT 'normal',    -- 'low' | 'normal' | 'high'
  created_at   TEXT NOT NULL,                     -- RFC3339 UTC
  updated_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_todos_status_order ON todos(status, order_index);

CREATE TABLE pomodoro_sessions (
  id TEXT PRIMARY KEY,
  focus_min INTEGER NOT NULL,
  rest_min INTEGER NOT NULL,
  status TEXT NOT NULL,           -- 'running' | 'paused' | 'completed' | 'cancelled'
  started_at TEXT NOT NULL,
  ended_at TEXT
);

-- ==== 生命感(LivingPetService) ====
CREATE TABLE pet_runtime_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),  -- 单行表
  energy INTEGER NOT NULL DEFAULT 60,
  mood TEXT NOT NULL DEFAULT 'neutral',   -- 不存 transient mood
  last_interaction_at TEXT NOT NULL,
  disabled_features TEXT NOT NULL DEFAULT '[]',  -- JSON array
  updated_at TEXT NOT NULL
);

-- ==== 主动关心日志(M3 J 模块,频率控制 + KPI) ====
-- 7 天保留:每次启动清理 fired_at < now - 7d 的记录
CREATE TABLE proactive_care_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  fired_at TEXT NOT NULL,
  trigger TEXT NOT NULL,          -- 'idle' | 'late_night' | 'no_pomodoro_long' | 'milestone' | 'wardrobe_suggest'
  category TEXT NOT NULL,         -- 'empathy' | 'greeting' | 'gentle_remind' | 'celebration' | 'wardrobe_suggest'
  persona_id TEXT NOT NULL,
  template_idx INTEGER NOT NULL,  -- 命中模板的池内索引
  user_response TEXT,             -- 'clicked' | 'replied' | 'dismissed' | null
  responded_at TEXT
);
CREATE INDEX idx_pcl_fired ON proactive_care_log(fired_at);

-- ==== 里程碑 + 用户纪念日 ====
CREATE TABLE milestones (
  id TEXT PRIMARY KEY,            -- e.g. 'first_launch_7d' / 'anniversary_birthday_2027'
  category TEXT NOT NULL,         -- 'first_launch'|'streak'|'pomodoro_count'|'todo_count'|'user_anniversary'
  threshold INTEGER NOT NULL,
  reached_at TEXT NOT NULL,
  context TEXT                    -- JSON: 触达时的辅助信息
);

CREATE TABLE user_anniversaries (
  key TEXT PRIMARY KEY,           -- 'birthday' | 'work_start' | 'custom_<ulid>'
  display_name TEXT NOT NULL,
  date_md TEXT NOT NULL,          -- 'MM-DD',年度重复
  created_at TEXT NOT NULL
);

-- ==== 装扮(M4 WardrobeService) ====
CREATE TABLE accessories_inventory (
  id TEXT PRIMARY KEY,            -- accessory_id(与资源文件对应)
  unlocked_at TEXT NOT NULL,
  unlock_reason TEXT NOT NULL,    -- 'always'|'seasonal'|'milestone:xxx'|'purchase:sku'|'gift'|'user_upload'
  is_equipped INTEGER NOT NULL DEFAULT 0,
  metadata TEXT                   -- JSON: tier / unlock spec / anchor / etc(ADR-011)
);

CREATE TABLE wardrobe_decisions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  accessory_id TEXT NOT NULL,
  year INTEGER NOT NULL,
  decision TEXT NOT NULL,         -- 'accepted' | 'declined'
  decided_at TEXT NOT NULL,
  UNIQUE(accessory_id, year)
);

-- ==== 声音(M4 VoiceEffectPlayer) ====
CREATE TABLE voice_packs (
  id TEXT PRIMARY KEY,            -- 'default' | 'cute_v1' | 'user_imported_xxx'
  name TEXT NOT NULL,
  source TEXT NOT NULL,           -- 'builtin' | 'user_imported'
  manifest_path TEXT NOT NULL,
  installed_at TEXT NOT NULL
);

CREATE TABLE voice_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  global_mute INTEGER NOT NULL DEFAULT 0,
  volume INTEGER NOT NULL DEFAULT 50,
  quiet_weekdays TEXT NOT NULL DEFAULT '[1,2,3,4,5]',  -- JSON array
  quiet_ranges TEXT NOT NULL DEFAULT '[["09:00","18:00"]]', -- JSON array
  updated_at TEXT NOT NULL
);

-- ==== 游戏(M5 GameEngine) ====
CREATE TABLE game_sessions (
  id TEXT PRIMARY KEY,            -- ULID
  game_id TEXT NOT NULL,
  kind TEXT NOT NULL,             -- 'local' | 'llm'
  started_at TEXT NOT NULL,
  ended_at TEXT,
  result TEXT,                    -- JSON:游戏特定结果
  saved_as_diary INTEGER NOT NULL DEFAULT 0,
  total_tokens INTEGER NOT NULL DEFAULT 0  -- 仅 LLM 游戏
);

CREATE TABLE game_session_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  role TEXT NOT NULL,             -- 'user' | 'assistant' | 'system'
  content TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (session_id) REFERENCES game_sessions(id) ON DELETE CASCADE
);

-- 日记草稿(P1-R1 桌宠日记功能消费)
CREATE TABLE diary_drafts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source TEXT NOT NULL,           -- 'game:<game_id>' | 'manual' | ...
  content TEXT NOT NULL,
  created_at TEXT NOT NULL,
  consumed INTEGER NOT NULL DEFAULT 0
);

-- ==== 埋点 + 错误日志(TelemetryService) ====
CREATE TABLE telemetry_queue (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_name TEXT NOT NULL,
  schema_version INTEGER NOT NULL,
  payload TEXT NOT NULL,          -- JSON
  created_at TEXT NOT NULL,
  flushed INTEGER NOT NULL DEFAULT 0,
  retry_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_telemetry_unflushed ON telemetry_queue(flushed) WHERE flushed = 0;

CREATE TABLE error_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  level TEXT NOT NULL,            -- 'warn' | 'error' | 'fatal'
  module TEXT NOT NULL,
  message TEXT NOT NULL,
  context TEXT,                   -- JSON
  created_at TEXT NOT NULL
);

-- ==== 单行表 seed(确保 UPDATE 能命中) ====
-- 时间戳格式：strftime('%Y-%m-%dT%H:%M:%fZ','now') = RFC3339 UTC（"2026-05-10T14:25:30.123Z"），
-- 与 Rust 端 chrono::Utc::now().to_rfc3339() 同协议；避免 datetime('now') 的空格分隔格式
-- 与 service 层后续 UPDATE 写入混存导致字符串排序失序。
INSERT INTO schema_version (version, applied_at) VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ','now'));
INSERT INTO consent (id, granted, method, version, accepted_at)
  VALUES (1, 0, 'classic', 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'));
INSERT INTO nicknames (id, updated_at) VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ','now'));
INSERT INTO pet_runtime_state (id, last_interaction_at, updated_at)
  VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now'));
INSERT INTO voice_settings (id, updated_at) VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ','now'));

-- ==== Persona snapshot 唯一约束 ====
-- 同 (persona_id, version) 只能存在一条快照（idempotent seed + 防御直接 INSERT）。
-- 历史：原本走单独 002_persona_snapshot_unique.sql（含 DELETE 去重 + CREATE INDEX 两步），
-- 但 001+002 同一次 commit 入库，002 的 DELETE 在新装机上永远删 0 行；2026-05-06
-- code-review #7 合并回 001，仅保留 CREATE UNIQUE INDEX 一行（IF NOT EXISTS 兜底）。
CREATE UNIQUE INDEX IF NOT EXISTS idx_persona_snapshots_unique_persona_version
  ON persona_snapshots(persona_id, version);
