-- Add openRouterApiKey column to settings table.
-- On fresh databases the settings table is created by the preceding
-- migration (add_knowledge_graph_settings); on legacy databases it
-- already exists from the original schema.
CREATE TABLE IF NOT EXISTS settings (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    whisperModel TEXT NOT NULL DEFAULT '',
    groqApiKey TEXT,
    openaiApiKey TEXT,
    anthropicApiKey TEXT,
    ollamaApiKey TEXT,
    knowledge_graph_settings TEXT
);
ALTER TABLE settings ADD COLUMN openRouterApiKey TEXT;
