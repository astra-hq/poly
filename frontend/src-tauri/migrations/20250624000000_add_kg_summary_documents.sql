-- Track summary document ingestion status per meeting/profile
CREATE TABLE IF NOT EXISTS knowledge_graph_summary_documents (
    meeting_id TEXT NOT NULL,
    profile_id TEXT NOT NULL,
    file_source TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(meeting_id, profile_id)
);
