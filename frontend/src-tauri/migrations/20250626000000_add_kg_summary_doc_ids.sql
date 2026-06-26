-- Add track_id and document_id columns to knowledge_graph_summary_documents
-- These store the LightRAG track_id and document_id returned after ingestion,
-- needed for correct deletion and status polling.
ALTER TABLE knowledge_graph_summary_documents
ADD COLUMN track_id TEXT;

ALTER TABLE knowledge_graph_summary_documents
ADD COLUMN document_id TEXT;
