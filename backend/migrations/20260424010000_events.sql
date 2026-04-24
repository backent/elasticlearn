CREATE TABLE IF NOT EXISTS events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    ts         DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    sid        TEXT,
    kind       TEXT NOT NULL,
    status     TEXT NOT NULL,
    meta       TEXT
);

CREATE INDEX IF NOT EXISTS idx_events_ts   ON events (ts);
CREATE INDEX IF NOT EXISTS idx_events_kind ON events (kind);
