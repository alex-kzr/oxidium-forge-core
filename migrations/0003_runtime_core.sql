-- Runtime layer: process instances, executions, element instances (+ history mirrors)

CREATE TABLE IF NOT EXISTS process_instances (
    key                 INTEGER PRIMARY KEY AUTOINCREMENT,
    definition_key      INTEGER NOT NULL REFERENCES process_definitions(key),
    bpmn_process_id     TEXT NOT NULL,
    version             INTEGER NOT NULL,
    status              TEXT NOT NULL DEFAULT 'active',
    started_at          TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at            TEXT,
    parent_instance_key INTEGER
);

CREATE INDEX IF NOT EXISTS idx_process_instances_status
    ON process_instances (status);
CREATE INDEX IF NOT EXISTS idx_process_instances_bpmn_id
    ON process_instances (bpmn_process_id);

CREATE TABLE IF NOT EXISTS executions (
    key                  INTEGER PRIMARY KEY AUTOINCREMENT,
    instance_key         INTEGER NOT NULL REFERENCES process_instances(key),
    parent_execution_key INTEGER,
    current_node_id      TEXT NOT NULL,
    state                TEXT NOT NULL DEFAULT 'active',
    created_at           TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_executions_instance
    ON executions (instance_key);

CREATE TABLE IF NOT EXISTS process_element_instances (
    key           INTEGER PRIMARY KEY AUTOINCREMENT,
    instance_key  INTEGER NOT NULL REFERENCES process_instances(key),
    execution_key INTEGER NOT NULL REFERENCES executions(key),
    element_id    TEXT NOT NULL,
    element_type  TEXT NOT NULL,
    state         TEXT NOT NULL DEFAULT 'active',
    entered_at    TEXT NOT NULL DEFAULT (datetime('now')),
    left_at       TEXT
);

CREATE INDEX IF NOT EXISTS idx_element_instances_instance
    ON process_element_instances (instance_key);
CREATE INDEX IF NOT EXISTS idx_element_instances_execution
    ON process_element_instances (execution_key);

-- History mirrors

CREATE TABLE IF NOT EXISTS process_instances_history (
    key                 INTEGER PRIMARY KEY,
    definition_key      INTEGER NOT NULL,
    bpmn_process_id     TEXT NOT NULL,
    version             INTEGER NOT NULL,
    status              TEXT NOT NULL,
    started_at          TEXT NOT NULL,
    ended_at            TEXT NOT NULL,
    parent_instance_key INTEGER
);

CREATE TABLE IF NOT EXISTS executions_history (
    key                  INTEGER PRIMARY KEY,
    instance_key         INTEGER NOT NULL,
    parent_execution_key INTEGER,
    current_node_id      TEXT NOT NULL,
    state                TEXT NOT NULL,
    created_at           TEXT NOT NULL,
    ended_at             TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS process_element_instances_history (
    key           INTEGER PRIMARY KEY,
    instance_key  INTEGER NOT NULL,
    execution_key INTEGER NOT NULL,
    element_id    TEXT NOT NULL,
    element_type  TEXT NOT NULL,
    state         TEXT NOT NULL,
    entered_at    TEXT NOT NULL,
    left_at       TEXT NOT NULL
);
