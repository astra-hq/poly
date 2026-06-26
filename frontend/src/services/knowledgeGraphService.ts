/**
 * Knowledge Graph Service
 *
 * Thin 1-to-1 wrappers around Tauri commands for knowledge graph
 * settings, profile health-checks, meeting ingestion, and querying.
 *
 * Pattern follows `storageService.ts`: singleton class, no error
 * handling changes, exact same behavior as direct invoke calls.
 *
 * Argument names use camelCase which Tauri automatically converts to
 * snake_case for the Rust command parameters.
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  IngestionSummary,
  KnowledgeGraphHealth,
  KnowledgeGraphPipelineStatus,
  KnowledgeGraphQueryResponse,
  KnowledgeGraphSelection,
  KnowledgeGraphSettings,
  KnowledgeGraphTrackStatus,
  MeetingKnowledgeGraphSelection,
  MeetingKnowledgeGraphStatus,
  QueryMode,
  SummaryIngestResult,
} from '@/types/knowledgeGraph';
import { DEFAULT_QUERY_MODE, DEFAULT_TOP_K } from '@/types/knowledgeGraph';

export class KnowledgeGraphService {
  /**
   * Retrieve the current knowledge graph settings.
   * Backend: `api_get_knowledge_graph_settings` (no args)
   * Returns defaults when nothing has been persisted yet.
   */
  async getSettings(): Promise<KnowledgeGraphSettings> {
    return invoke<KnowledgeGraphSettings>(
      'api_get_knowledge_graph_settings'
    );
  }

  /**
   * Save (validate and persist) knowledge graph settings.
   * Backend: `api_save_knowledge_graph_settings(settings: KnowledgeGraphSettings)`
   * Returns the canonical stored copy after validation.
   */
  async saveSettings(
    settings: KnowledgeGraphSettings
  ): Promise<KnowledgeGraphSettings> {
    return invoke<KnowledgeGraphSettings>(
      'api_save_knowledge_graph_settings',
      { settings }
    );
  }

  /**
   * Health-check a specific profile by calling its LightRAG endpoint.
   * Backend: `api_test_knowledge_graph_profile(profile_id: String)`
   * Returns `{ healthy: bool, version?: string }`.
   */
  async testProfile(profileId: string): Promise<KnowledgeGraphHealth> {
    return invoke<KnowledgeGraphHealth>(
      'api_test_knowledge_graph_profile',
      { profileId }
    );
  }

  /**
   * Ingest all transcript chunks for a meeting into the knowledge graph.
   * Backend: `api_ingest_meeting_to_knowledge_graph(meeting_id: String, profile_id: String)`
   * Returns a summary of submitted/already-submitted/failed chunks.
   */
  async ingestMeeting(
    meetingId: string,
    profileId: string
  ): Promise<IngestionSummary> {
    return invoke<IngestionSummary>(
      'api_ingest_meeting_to_knowledge_graph',
      { meetingId, profileId }
    );
  }

  /**
   * Query the knowledge graph for a given profile.
   * Backend: `api_query_knowledge_graph(profile_id: String, query: String, mode: Option<QueryMode>, top_k: Option<usize>)`
   * Defaults: mode=hybrid, top_k=5 (matching Rust defaults).
   */
  async queryKnowledgeGraph(
    profileId: string,
    query: string,
    mode?: QueryMode,
    topK?: number
  ): Promise<KnowledgeGraphQueryResponse> {
    return invoke<KnowledgeGraphQueryResponse>(
      'api_query_knowledge_graph',
      {
        profileId,
        query,
        mode: mode ?? DEFAULT_QUERY_MODE,
        topK: topK ?? DEFAULT_TOP_K,
      }
    );
  }

  /**
   * Retrieve the persisted KG profile selection for a meeting.
   * Backend: `api_get_meeting_knowledge_graph_selection(meeting_id: String)`
   * Returns `null` when no selection has been persisted for the meeting.
   */
  async getMeetingSelection(
    meetingId: string
  ): Promise<MeetingKnowledgeGraphSelection | null> {
    return invoke<MeetingKnowledgeGraphSelection | null>(
      'api_get_meeting_knowledge_graph_selection',
      { meetingId }
    );
  }

  /**
   * Persist the KG profile selection intent for a meeting.
   * Backend: `api_set_meeting_knowledge_graph_selection(meeting_id, profile_id, meeting_type, routing_reason)`
   * `profile_id` must reference an existing profile, or be `null` for "none".
   * Does NOT trigger provider calls or auto-indexing.
   */
  async setMeetingSelection(
    meetingId: string,
    profileId: string | null,
    meetingType?: string | null,
    routingReason?: string | null
  ): Promise<MeetingKnowledgeGraphSelection> {
    return invoke<MeetingKnowledgeGraphSelection>(
      'api_set_meeting_knowledge_graph_selection',
      {
        meetingId,
        profileId,
        meetingType: meetingType ?? null,
        routingReason: routingReason ?? null,
      }
    );
  }

  /**
   * Retrieve the KG profile selection for a meeting as a `KnowledgeGraphSelection`.
   * Backend: `api_get_meeting_knowledge_graph_selection(meeting_id: String)`
   * Returns `null` when no selection has been persisted for the meeting.
   * Maps the stored `profile_id` to the `KnowledgeGraphSelection` enum:
   * `null` profile_id → `'none'`, non-null → `{ profile: id }`.
   */
  async getMeetingKnowledgeGraphSelection(
    meetingId: string
  ): Promise<KnowledgeGraphSelection | null> {
    const result = await invoke<MeetingKnowledgeGraphSelection | null>(
      'api_get_meeting_knowledge_graph_selection',
      { meetingId }
    );
    if (result === null) return null;
    if (result.profile_id === null) return 'none';
    return { profile: result.profile_id };
  }

  /**
   * Set the KG profile selection for a meeting.
   * Backend: `api_set_meeting_knowledge_graph_selection(meeting_id, profile_id, meeting_type, routing_reason)`
   * `profileId` must reference an existing profile, or be `null` for "none".
   */
  async setMeetingKnowledgeGraphSelection(
    meetingId: string,
    profileId: string | null
  ): Promise<void> {
    await invoke('api_set_meeting_knowledge_graph_selection', {
      meetingId,
      profileId,
      meetingType: null,
      routingReason: null,
    });
  }

  /**
   * Query the ingestion ledger for a meeting's knowledge-graph status.
   * Backend: `api_get_meeting_knowledge_graph_status(meeting_id: String)`
   * Returns chunk counts by status, last errors, and indexing-available flag.
   * Always succeeds — returns zeroes when no profile is selected.
   */
  async getMeetingStatus(
    meetingId: string
  ): Promise<MeetingKnowledgeGraphStatus> {
    return invoke<MeetingKnowledgeGraphStatus>(
      'api_get_meeting_knowledge_graph_status',
      { meetingId }
    );
  }

  async getPipelineStatus(
    profileId: string
  ): Promise<KnowledgeGraphPipelineStatus> {
    return invoke<KnowledgeGraphPipelineStatus>(
      'api_get_knowledge_graph_pipeline_status',
      { profileId }
    );
  }

  async getSummaryTrackStatus(
    meetingId: string
  ): Promise<KnowledgeGraphTrackStatus> {
    return invoke<KnowledgeGraphTrackStatus>(
      'api_get_summary_track_status',
      { meetingId }
    );
  }

  /**
   * Auto-ingest the meeting summary into the configured Knowledge Graph.
   * Backend: `api_ingest_summary_to_knowledge_graph(meeting_id)`
   * Uses the meeting's KG profile selection (or the global active profile).
   * Returns ingest result with profile info and success/failure status.
   */
  async ingestSummaryToKnowledgeGraph(
    meetingId: string
  ): Promise<SummaryIngestResult> {
    return invoke<SummaryIngestResult>(
      'api_ingest_summary_to_knowledge_graph',
      { meetingId }
    );
  }

  /**
   * Delete the summary document from the Knowledge Graph (for regeneration).
   * Backend: `api_delete_summary_from_knowledge_graph(meeting_id)`
   * Non-fatal — deletion may fail if the document doesn't exist yet.
   */
  async deleteSummaryFromKnowledgeGraph(
    meetingId: string
  ): Promise<SummaryIngestResult> {
    return invoke<SummaryIngestResult>(
      'api_delete_summary_from_knowledge_graph',
      { meetingId }
    );
  }

  /**
   * Auto-provision a local knowledge graph stack via docker compose.
   * Backend: `api_setup_local_knowledge_graph()`
   * Returns a status message on success.
   */
  async setupLocalKnowledgeGraph(): Promise<string> {
    return invoke<string>('api_setup_local_knowledge_graph');
  }

  /**
   * Phase 1: Check system dependencies for local knowledge graph setup.
   * Backend: `api_setup_kg_check_deps()`
   * Returns dependency info for Docker and Ollama.
   */
  async checkDeps(): Promise<{
    docker: { installed: boolean; version: string | null };
    ollama: { installed: boolean; version: string | null };
  }> {
    return invoke('api_setup_kg_check_deps');
  }

  /**
   * Phase 2: Pull an Ollama model for local knowledge graph use.
   * Backend: `api_setup_kg_pull_model(model: String)`
   * Progress is streamed via `setup-progress` events (stage: pull-model).
   */
  async pullModel(model: string): Promise<void> {
    return invoke('api_setup_kg_pull_model', { model });
  }

  /**
   * Phase 3: Start the knowledge graph docker compose stack.
   * Backend: `api_setup_local_knowledge_graph()`
   * Progress is streamed via `setup-progress` events.
   * Returns a status message on success.
   */
  async startStack(): Promise<string> {
    return invoke<string>('api_setup_local_knowledge_graph');
  }
}

// Export singleton instance
export const knowledgeGraphService = new KnowledgeGraphService();