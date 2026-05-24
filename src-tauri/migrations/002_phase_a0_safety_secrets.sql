-- Phase A0: Safety & Secrets migration
-- - messages: 加 token_count + safety_scan_status (7-state enum)
-- - secrets: ALTER ADD created_at (001 已建 key/ciphertext/updated_at; Task 5 DPAPI 审计需 created_at)
-- - context_access_log: 新表 (Task 3 PermissionService 写入)
-- - error_logs: 001 已建 (level/module/message/context/created_at); Phase A0 不改 schema,
--   Task 6+ LifecycleManager 用 module/context 列, 不用 source/details

-- 1. messages 表加字段
ALTER TABLE messages ADD COLUMN token_count INTEGER DEFAULT NULL;
ALTER TABLE messages ADD COLUMN safety_scan_status TEXT NOT NULL DEFAULT 'pending';

-- 2. secrets 表加 created_at (001 已有 key/ciphertext/updated_at)
-- 注: SQLite ALTER ADD COLUMN NOT NULL 必须有常量 DEFAULT, 用空串占位;
-- Task 5 SecretRepo::set() INSERT 时显式写真实 RFC3339 时间, ON CONFLICT 不更新 created_at。
ALTER TABLE secrets ADD COLUMN created_at TEXT NOT NULL DEFAULT '';

-- 3. context_access_log 表 (Task 3 PermissionService 写入) — 新表
CREATE TABLE IF NOT EXISTS context_access_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL,
    granted INTEGER NOT NULL,
    actor TEXT NOT NULL,
    used_for TEXT NOT NULL,
    surface_id TEXT,
    retention_policy TEXT NOT NULL DEFAULT 'transient',
    created_at TEXT NOT NULL,
    permission_granted_at TEXT,
    context_captured_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_context_audit_scope
    ON context_access_log(scope, created_at DESC);

-- 4. error_logs: 001 已建 (id, level, module, message, context, created_at)
--    Phase A0 不在此 migration 改 schema 或加索引 (避免重名 / 旧 prod DB 兼容性问题);
--    Task 6+ 按 001 既有列名写入。
