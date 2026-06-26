-- System layer: migration tracking
-- This table is created by the runner itself before applying migrations,
-- but we include it here so schema_migrations tracks itself from v1.
CREATE TABLE IF NOT EXISTS schema_migrations (
    version    INTEGER PRIMARY KEY,
    name       TEXT    NOT NULL,
    applied_at TEXT    NOT NULL,
    checksum   TEXT    NOT NULL
);
