-- Knowledge graph ingestion ledger: tracks each chunk submitted per meeting/profile
CREATE TABLE IF NOT EXISTS knowledge_graph_ingestion_chunks (
    meeting_id TEXT NOT NULL,
    profile_id TEXT NOT NULL,
    chunk_sequence INTEGER NOT NULL,
    chunk_fingerprint TEXT NOT NULL,
    file_source TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    track_id TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(meeting_id, profile_id, chunk_sequence, chunk_fingerprint)
);
