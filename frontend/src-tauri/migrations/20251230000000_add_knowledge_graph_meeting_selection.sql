-- Per-meeting knowledge-graph profile selection intent.
-- Persisted after the meeting ID exists (post-save/recovery), not when recording starts.
-- A NULL profile_id means explicit "none" selection.
CREATE TABLE IF NOT EXISTS knowledge_graph_meeting_selection (
    meeting_id TEXT NOT NULL PRIMARY KEY,
    profile_id TEXT,
    meeting_type TEXT,
    routing_reason TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);
