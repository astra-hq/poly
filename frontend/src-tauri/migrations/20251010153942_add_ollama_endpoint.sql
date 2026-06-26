CREATE TABLE IF NOT EXISTS settings (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    whisperModel TEXT NOT NULL DEFAULT '',
    groqApiKey TEXT,
    openaiApiKey TEXT,
    anthropicApiKey TEXT,
    ollamaApiKey TEXT,
    openRouterApiKey TEXT,
    knowledge_graph_settings TEXT
);
ALTER TABLE settings ADD COLUMN ollamaEndpoint TEXT;
