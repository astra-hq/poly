/**
 * Glossary Service
 *
 * Thin 1-to-1 wrappers around Tauri commands for the glossary.
 * All validation and normalization is owned by the Rust commands —
 * this service is a pure transport layer.
 *
 * Pattern follows `knowledgeGraphService.ts`: singleton class,
 * no error handling changes, exact same behavior as direct invoke calls.
 *
 * Argument names use camelCase which Tauri automatically converts to
 * snake_case for the Rust command parameters.
 */

import { invoke } from '@tauri-apps/api/core';
import type { Glossary, GlossarySyncResult } from '@/types/glossary';

export class GlossaryService {
  /**
   * Retrieve the current glossary.
   * Backend: `api_get_glossary` (no args)
   * Returns an empty glossary when no file exists yet.
   */
  async getGlossary(): Promise<Glossary> {
    return invoke<Glossary>('api_get_glossary');
  }

  /**
   * Save (validate, normalize, and atomically persist) the glossary.
   * Backend: `api_save_glossary(glossary: Glossary)`
   * Returns the canonical stored copy after validation and normalization.
   */
  async saveGlossary(glossary: Glossary): Promise<Glossary> {
    return invoke<Glossary>('api_save_glossary', { glossary });
  }

  /**
   * Sync the glossary to the Knowledge Graph for the active profile.
   * Backend: `api_sync_glossary_to_knowledge_graph` (no args)
   * Uses the global active KG profile only — does not query per-meeting selection.
   * Returns a sync result with profile info, success/failure status, and skip reasons.
   */
  async syncGlossaryToKnowledgeGraph(): Promise<GlossarySyncResult> {
    return invoke<GlossarySyncResult>(
      'api_sync_glossary_to_knowledge_graph'
    );
  }

  /**
   * Delete the glossary document from the Knowledge Graph.
   * Backend: `api_delete_glossary_from_knowledge_graph` (no args)
   * Best-effort delete by file_source — non-fatal if the document doesn't exist.
   */
  async deleteGlossaryFromKnowledgeGraph(): Promise<GlossarySyncResult> {
    return invoke<GlossarySyncResult>(
      'api_delete_glossary_from_knowledge_graph'
    );
  }
}

// Export singleton instance
export const glossaryService = new GlossaryService();
