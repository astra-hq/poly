/**
 * Configuration Service
 *
 * Handles all configuration-related Tauri backend calls.
 * Pure 1-to-1 wrapper - no error handling changes, exact same behavior as direct invoke calls.
 */

import { invoke } from '@tauri-apps/api/core';
import { TranscriptModelProps } from '@/components/TranscriptSettings';

import type { ProviderConfig, ProviderModel } from '@/types/providers';

export interface ModelConfig {
  provider: string;
  model: string;
  /**
   * @deprecated Use providerApiKeys from ConfigContext instead.
   * This field may contain stale data when provider changes without saving.
   */
  apiKey?: string | null;
  ollamaEndpoint?: string | null;
}

export interface RecordingPreferences {
  preferred_mic_device: string | null;
  preferred_system_device: string | null;
}

export type CalendarProvider = 'apple';

export interface CalendarConfig {
  readonly metadata_pull_enabled: boolean;
  readonly auto_record_enabled: boolean;
  readonly provider: CalendarProvider;
  readonly lookahead_window_minutes: number;
  readonly start_grace_window_minutes: number;
  readonly end_grace_window_minutes: number;
  readonly selected_apple_calendar_identifiers: readonly string[];
  readonly show_calendar_status: boolean;
  readonly show_next_meeting_banner: boolean;
}

export type CalendarPermissionStatus =
  | 'not_determined'
  | 'restricted'
  | 'denied'
  | 'authorized'
  | 'full_access'
  | 'write_only'
  | 'unsupported_platform'
  | 'unknown';

export interface CalendarProviderHealth {
  readonly provider: string;
  readonly platform_supported: boolean;
  readonly permission_granted: boolean;
  readonly permission_status: CalendarPermissionStatus;
  readonly event_count: number | null;
  readonly error: string | null;
}

export interface CalendarCandidate {
  readonly id: string;
  readonly occurrence_key: string;
  readonly title: string;
  readonly start: string;
  readonly end: string;
  readonly calendar_id: string;
  readonly meeting_link: string | null;
  readonly is_cancelled: boolean;
  readonly category: string;
  readonly response_status: string;
  readonly eligible: boolean;
  readonly ineligibility_reason: string | null;
}

export interface CalendarCandidatesResponse {
  readonly candidates: CalendarCandidate[];
}

/**
 * Configuration Service
 * Singleton service for managing app configuration
 */
export class ConfigService {
  /**
   * Get saved transcript model configuration
   * @returns Promise with { provider, model, apiKey }
   */
  async getTranscriptConfig(): Promise<TranscriptModelProps> {
    return invoke<TranscriptModelProps>('api_get_transcript_config');
  }

  /**
   * Get saved summary model configuration
   * @returns Promise with { provider, model }
   */
  async getModelConfig(): Promise<ModelConfig> {
    return invoke<ModelConfig>('api_get_model_config');
  }

  /**
   * Get saved audio device preferences
   * @returns Promise with { preferred_mic_device, preferred_system_device }
   */
  async getRecordingPreferences(): Promise<RecordingPreferences> {
    return invoke<RecordingPreferences>('get_recording_preferences');
  }

  async getCalendarSettings(): Promise<CalendarConfig> {
    return invoke<CalendarConfig>('get_calendar_settings');
  }

  async saveCalendarSettings(settings: CalendarConfig): Promise<CalendarConfig> {
    return invoke<CalendarConfig>('save_calendar_settings', { settings });
  }

  async getCalendarPermissionStatus(): Promise<CalendarPermissionStatus> {
    return invoke<CalendarPermissionStatus>('get_calendar_permission_status');
  }

  async requestCalendarPermission(): Promise<CalendarPermissionStatus> {
    return invoke<CalendarPermissionStatus>('request_calendar_permission');
  }

  async getCalendarProviderHealth(): Promise<CalendarProviderHealth> {
    return invoke<CalendarProviderHealth>('get_calendar_provider_health');
  }

  async getUpcomingCalendarCandidates(): Promise<CalendarCandidatesResponse> {
    return invoke<CalendarCandidatesResponse>('get_upcoming_calendar_candidates');
  }

  async getSelectedCalendars(): Promise<string[]> {
    return invoke<string[]>('get_selected_calendars');
  }

  async getAppleCalendars(): Promise<{ id: string; title: string }[]> {
    return invoke<{ id: string; title: string }[]>('get_apple_calendars');
  }

  async skipCalendarOccurrence(eventId: string, occurrenceStart: string): Promise<void> {
    await invoke('skip_calendar_occurrence', { eventId, occurrenceStart });
  }

  /**
   * Get all configured providers from poly.yml.
   */
  async getProviders(): Promise<ProviderConfig[]> {
    return invoke<ProviderConfig[]>('api_get_providers');
  }

  /**
   * Fetch available models for a provider.
   */
  async getProviderModels(providerId: string): Promise<ProviderModel[]> {
    return invoke<ProviderModel[]>('api_get_provider_models', {
      providerId,
    });
  }

  /**
   * Create or update a provider in poly.yml.
   */
  async saveProvider(provider: ProviderConfig, apiKey?: string): Promise<void> {
    await invoke('api_save_provider', { provider, apiKey: apiKey ?? null });
  }

  /**
   * Delete a provider from poly.yml by id.
   */
  async deleteProvider(providerId: string): Promise<void> {
    await invoke('api_delete_provider', { providerId });
  }
}

// Export singleton instance
export const configService = new ConfigService();
