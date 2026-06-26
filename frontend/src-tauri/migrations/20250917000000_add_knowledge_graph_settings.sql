-- On fresh databases the settings table may not exist yet (removed from
-- the initial schema for fresh installs).  Create it with the columns that
-- exist at this point, then add the new column.
CREATE TABLE IF NOT EXISTS settings (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    whisperModel TEXT NOT NULL DEFAULT '',
    groqApiKey TEXT,
    openaiApiKey TEXT,
    anthropicApiKey TEXT,
    ollamaApiKey TEXT
);
ALTER TABLE settings ADD COLUMN knowledge_graph_settings TEXT;
