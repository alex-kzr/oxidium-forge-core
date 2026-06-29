-- Phase 4: Service Tasks & Jobs  (SJ-01)
-- jobs, jobs_history, incidents, incidents_history

CREATE TABLE IF NOT EXISTS jobs (
    key                  INTEGER PRIMARY KEY AUTOINCREMENT,
    instance_key         INTEGER NOT NULL REFERENCES process_instances(key),
    element_instance_key INTEGER NOT NULL,
    element_id           TEXT    NOT NULL,
    task_type            TEXT    NOT NULL,
    state                TEXT    NOT NULL DEFAULT 'activatable'
                                 CHECK(state IN ('activatable','activated','completed','failed','dead')),
    retries              INTEGER NOT NULL DEFAULT 3,
    worker               TEXT,
    locked_until         TEXT,
    retry_at             TEXT,
    variables            TEXT    NOT NULL DEFAULT '{}',
    error_message        TEXT,
    created_at           TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at           TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_jobs_type_state     ON jobs(task_type, state);
CREATE INDEX IF NOT EXISTS idx_jobs_locked_until   ON jobs(locked_until);
CREATE INDEX IF NOT EXISTS idx_jobs_instance       ON jobs(instance_key);

CREATE TABLE IF NOT EXISTS jobs_history (
    key                  INTEGER PRIMARY KEY,
    instance_key         INTEGER NOT NULL,
    element_instance_key INTEGER NOT NULL,
    element_id           TEXT    NOT NULL,
    task_type            TEXT    NOT NULL,
    state                TEXT    NOT NULL,
    retries              INTEGER NOT NULL,
    worker               TEXT,
    locked_until         TEXT,
    retry_at             TEXT,
    variables            TEXT    NOT NULL DEFAULT '{}',
    error_message        TEXT,
    created_at           TEXT    NOT NULL,
    updated_at           TEXT    NOT NULL,
    archived_at          TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_jobs_history_instance ON jobs_history(instance_key);

CREATE TABLE IF NOT EXISTS incidents (
    key                  INTEGER PRIMARY KEY AUTOINCREMENT,
    instance_key         INTEGER NOT NULL,
    element_instance_key INTEGER,
    job_key              INTEGER,
    incident_type        TEXT    NOT NULL,
    message              TEXT    NOT NULL,
    state                TEXT    NOT NULL DEFAULT 'active'
                                 CHECK(state IN ('active','resolved')),
    created_at           TEXT    NOT NULL DEFAULT (datetime('now')),
    resolved_at          TEXT
);

CREATE INDEX IF NOT EXISTS idx_incidents_state    ON incidents(state);
CREATE INDEX IF NOT EXISTS idx_incidents_instance ON incidents(instance_key);

CREATE TABLE IF NOT EXISTS incidents_history (
    key                  INTEGER PRIMARY KEY,
    instance_key         INTEGER NOT NULL,
    element_instance_key INTEGER,
    job_key              INTEGER,
    incident_type        TEXT    NOT NULL,
    message              TEXT    NOT NULL,
    state                TEXT    NOT NULL,
    created_at           TEXT    NOT NULL,
    resolved_at          TEXT,
    archived_at          TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_incidents_history_instance ON incidents_history(instance_key);
