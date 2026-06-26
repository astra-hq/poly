-- Create meetings table
CREATE TABLE IF NOT EXISTS meetings (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Create transcripts table
CREATE TABLE IF NOT EXISTS transcripts (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    transcript TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    summary TEXT,
    action_items TEXT,
    key_points TEXT,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

-- Create summary_processes table
CREATE TABLE IF NOT EXISTS summary_processes (
    meeting_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    error TEXT,
    result TEXT,
    start_time TEXT,
    end_time TEXT,
    chunk_count INTEGER DEFAULT 0,
    processing_time REAL DEFAULT 0.0,
    metadata TEXT,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

-- Create transcript_chunks table
CREATE TABLE IF NOT EXISTS transcript_chunks (
    meeting_id TEXT PRIMARY KEY,
    meeting_name TEXT,
    transcript_text TEXT NOT NULL,
    model TEXT NOT NULL,
    model_name TEXT NOT NULL,
    chunk_size INTEGER,
    overlap INTEGER,
    created_at TEXT NOT NULL,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

-- Config tables (`settings`, `transcript_settings`) are no longer created
-- for fresh databases.  Legacy installations keep them in place so the
-- LegacyConfigExtractor (one-shot SQLite → YAML + SecretStore) can read
-- them.  Subsequent ALTER TABLE migrations that reference these tables
-- are guarded with CREATE TABLE IF NOT EXISTS so they succeed on both
-- fresh and legacy databases.
