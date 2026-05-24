-- Phase A0: Safety & Secrets migration
-- - messages: 加 token_count + safety_scan_status (7-state enum)
-- - 新增 secrets 表 (Task 5 CryptoService 用)
-- - 新增 context_access_log 表 (Task 3 PermissionService 用)
-- - 新增 error_logs 表 (kernel 失败降级用)

-- 1. messages 表加字段
ALTER TABLE messages ADD COLUMN token_count INTEGER DEFAULT NULL;
ALTER TABLE messages ADD COLUMN safety_scan_status TEXT NOT NULL DEFAULT 'pending';

-- 2. secrets 表 (Task 5 DPAPI 加密 KV)
CREATE TABLE IF NOT EXISTS secrets (
    key TEXT PRIMARY KEY,
    ciphertext BLOB NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- 3. context_access_log 表 (Task 3 PermissionService 写入)
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

-- 4. error_logs 表 (kernel 失败降级写入)
CREATE TABLE IF NOT EXISTS error_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    level TEXT NOT NULL,
    source TEXT NOT NULL,
    message TEXT NOT NULL,
    details TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_error_logs_source_time
    ON error_logs(source, created_at DESC);
