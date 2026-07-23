/**
 * Glossary Types
 *
 * TypeScript mirrors of the Rust serde types in
 * `src-tauri/src/glossary/types.rs` and
 * `src-tauri/src/knowledge_graph/commands.rs` (GlossarySyncResult).
 *
 * Serialization conventions:
 * - Rust fields with `skip_serializing_if = "Option::is_none"` map to
 *   `T | undefined` to match JSON omission.
 * - Rust fields with `skip_serializing_if = "Vec::is_empty"` map to
 *   optional arrays (defaulting to []).
 * - The Rust `Glossary.version` defaults to 1 via `default_version()`.
 */

// ── Glossary Kind ────────────────────────────────────────────────────

/**
 * Valid entry kinds matching Rust `VALID_KINDS` in glossary/types.rs.
 */
export type GlossaryKind =
  | 'person'
  | 'team'
  | 'project'
  | 'code_name'
  | 'component'
  | 'acronym'
  | 'other';

export const GLOSSARY_KINDS: GlossaryKind[] = [
  'person',
  'team',
  'project',
  'code_name',
  'component',
  'acronym',
  'other',
];

// ── Glossary Entry ───────────────────────────────────────────────────

export interface GlossaryEntry {
  id: string;
  term: string;
  kind: GlossaryKind;
  pronunciation?: string;
  aliases: string[];
  definition?: string;
  notes?: string;
  references?: string[];
}

// ── Glossary ─────────────────────────────────────────────────────────

export interface Glossary {
  version: number;
  entries: GlossaryEntry[];
}

export const DEFAULT_GLOSSARY: Glossary = {
  version: 1,
  entries: [],
};

export function generateEntryId(): string {
  if (typeof crypto !== 'undefined' && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

// ── Glossary Sync Result (KG commands) ───────────────────────────────

/**
 * Returned by `api_sync_glossary_to_knowledge_graph` and
 * `api_delete_glossary_from_knowledge_graph`.
 * Mirrors Rust `GlossarySyncResult` in `knowledge_graph/commands.rs`.
 */
export interface GlossarySyncResult {
  profile_id: string | null;
  synced: boolean;
  track_id: string | null;
  document_id: string | null;
  error: string | null;
  skipped_reason: string | null;
}
