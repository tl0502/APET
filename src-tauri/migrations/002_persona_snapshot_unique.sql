-- Migration v2: enforce unique persona snapshot version
--
-- 背景:
-- - v1 未对 persona_snapshots(persona_id, version) 建唯一约束
-- - 并发启动或重复 seed 时可能出现重复快照
--
-- 处理:
-- 1) 先按 (persona_id, version) 去重，保留最小 rowid
-- 2) 再建立唯一索引，保证后续幂等

DELETE FROM persona_snapshots
WHERE rowid NOT IN (
  SELECT MIN(rowid)
  FROM persona_snapshots
  GROUP BY persona_id, version
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_persona_snapshots_unique_persona_version
  ON persona_snapshots(persona_id, version);

