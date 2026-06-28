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
  provider: 'ollama' | 'groq' | 'claude' | 'openrouter' | 'openai' | 'builtin-ai';
  model: string;
  whisperModel: string;
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
   * @returns Promise with { provider, model, whisperModel }
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
