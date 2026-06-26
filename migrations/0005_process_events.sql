CREATE TABLE IF NOT EXISTS process_events (
    key            INTEGER PRIMARY KEY AUTOINCREMENT,
    instance_key   INTEGER,
    definition_key INTEGER,
    element_id     TEXT,
    event_type     TEXT NOT NULL,
    payload        TEXT NOT NULL DEFAULT '{}',
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_process_events_instance
    ON process_events (instance_key, created_at);
