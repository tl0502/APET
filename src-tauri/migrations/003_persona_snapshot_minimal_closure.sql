-- Phase A1: Persona Snapshot minimal closure.
-- Adds stable snapshot binding for runtime conversations and compiled runtime profiles.

ALTER TABLE personas ADD COLUMN active_snapshot_id INTEGER;

CREATE TABLE persona_snapshot_profiles (
  snapshot_id INTEGER PRIMARY KEY,
  persona_id TEXT NOT NULL,
  runtime_profile_json TEXT NOT NULL,
  source_hash TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (snapshot_id) REFERENCES persona_snapshots(id) ON DELETE CASCADE,
  FOREIGN KEY (persona_id) REFERENCES personas(id) ON DELETE CASCADE
);

CREATE INDEX idx_persona_snapshot_profiles_persona
  ON persona_snapshot_profiles(persona_id, created_at DESC);

ALTER TABLE conversations ADD COLUMN persona_snapshot_id INTEGER;

CREATE INDEX idx_conversations_persona_snapshot
  ON conversations(persona_snapshot_id);

-- Backfill personas to their current-version snapshot when possible.
UPDATE personas
SET active_snapshot_id = (
  SELECT s.id
  FROM persona_snapshots s
  WHERE s.persona_id = personas.id
    AND s.version = personas.version
  ORDER BY s.id DESC
  LIMIT 1
)
WHERE active_snapshot_id IS NULL;

-- Fallback for rows whose persona.version no longer has a matching snapshot.
UPDATE personas
SET active_snapshot_id = (
  SELECT s.id
  FROM persona_snapshots s
  WHERE s.persona_id = personas.id
  ORDER BY s.id DESC
  LIMIT 1
)
WHERE active_snapshot_id IS NULL;

-- Existing conversations keep a stable snapshot binding based on their persona history.
UPDATE conversations
SET persona_snapshot_id = (
  SELECT s.id
  FROM persona_snapshots s
  WHERE s.persona_id = conversations.persona_id
  ORDER BY s.id DESC
  LIMIT 1
)
WHERE persona_snapshot_id IS NULL;
