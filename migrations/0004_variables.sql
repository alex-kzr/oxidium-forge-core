CREATE TABLE IF NOT EXISTS variables (
    key          INTEGER PRIMARY KEY AUTOINCREMENT,
    instance_key INTEGER NOT NULL,
    scope_key    INTEGER NOT NULL,
    name         TEXT NOT NULL,
    value        TEXT NOT NULL,
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (instance_key, scope_key, name)
);

CREATE INDEX IF NOT EXISTS idx_variables_instance_scope
    ON variables (instance_key, scope_key);

CREATE TABLE IF NOT EXISTS variables_history (
    key          INTEGER PRIMARY KEY,
    instance_key INTEGER NOT NULL,
    scope_key    INTEGER NOT NULL,
    name         TEXT NOT NULL,
    value        TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    archived_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
