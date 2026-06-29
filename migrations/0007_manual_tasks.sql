-- Phase 5: Manual Tasks (MT-01)

CREATE TABLE IF NOT EXISTS manual_tasks (
    key                  INTEGER PRIMARY KEY AUTOINCREMENT,
    instance_key         INTEGER NOT NULL REFERENCES process_instances(key),
    element_instance_key INTEGER NOT NULL,
    element_id           TEXT    NOT NULL,
    state                TEXT    NOT NULL DEFAULT 'open'
                                 CHECK(state IN ('open','completed','cancelled')),
    variables            TEXT    NOT NULL DEFAULT '{}',
    created_at           TEXT    NOT NULL DEFAULT (datetime('now')),
    completed_at         TEXT
);

CREATE INDEX IF NOT EXISTS idx_manual_tasks_state    ON manual_tasks(state);
CREATE INDEX IF NOT EXISTS idx_manual_tasks_instance ON manual_tasks(instance_key);

CREATE TABLE IF NOT EXISTS manual_tasks_history (
    key                  INTEGER PRIMARY KEY,
    instance_key         INTEGER NOT NULL,
    element_instance_key INTEGER NOT NULL,
    element_id           TEXT    NOT NULL,
    state                TEXT    NOT NULL,
    variables            TEXT    NOT NULL DEFAULT '{}',
    created_at           TEXT    NOT NULL,
    completed_at         TEXT,
    archived_at          TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_manual_tasks_history_instance ON manual_tasks_history(instance_key);
