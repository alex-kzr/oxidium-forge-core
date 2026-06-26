-- Runtime layer: process definitions
CREATE TABLE IF NOT EXISTS process_definitions (
    key            INTEGER PRIMARY KEY AUTOINCREMENT,
    bpmn_process_id TEXT NOT NULL,
    version        INTEGER NOT NULL,
    resource_name  TEXT NOT NULL,
    bpmn_xml       TEXT NOT NULL,
    runtime_graph  TEXT NOT NULL,  -- JSON
    deployed_at    TEXT NOT NULL DEFAULT (datetime('now')),
    is_active      INTEGER NOT NULL DEFAULT 0,
    checksum       TEXT NOT NULL,
    UNIQUE (bpmn_process_id, version)
);

CREATE INDEX IF NOT EXISTS idx_process_definitions_active
    ON process_definitions (bpmn_process_id, is_active);

-- History table mirrors runtime
CREATE TABLE IF NOT EXISTS process_definitions_history (
    key            INTEGER PRIMARY KEY,
    bpmn_process_id TEXT NOT NULL,
    version        INTEGER NOT NULL,
    resource_name  TEXT NOT NULL,
    bpmn_xml       TEXT NOT NULL,
    runtime_graph  TEXT NOT NULL,
    deployed_at    TEXT NOT NULL,
    archived_at    TEXT NOT NULL DEFAULT (datetime('now')),
    checksum       TEXT NOT NULL
);
