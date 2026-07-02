// Types for Local AI (Summary Models) integration
export interface LocalModelInfo {
  name: string;
  display_name: string;
  status: LocalModelStatus;
  path: string;
  size_mb: number;
  context_size: number;
  description: string;
  gguf_file: string;
  model_type: 'summary' | 'embedding';
}

export type LocalModelStatus =
  | { type: 'not_downloaded' }
  | { type: 'downloading', progress: number }
  | { type: 'available' }
  | { type: 'corrupted', file_size: number, expected_min_size: number }
  | { type: 'error', Error: string };

// Helper functions for status handling
export function isModelAvailable(status: LocalModelStatus): boolean {
  return status.type === 'available';
}

export function isModelDownloading(status: LocalModelStatus): boolean {
  return status.type === 'downloading';
}

export function isModelNotDownloaded(status: LocalModelStatus): boolean {
  return status.type === 'not_downloaded';
}

export function isModelCorrupted(status: LocalModelStatus): boolean {
  return status.type === 'corrupted';
}

export function isModelError(status: LocalModelStatus): boolean {
  return status.type === 'error';
}

export function getStatusColor(status: LocalModelStatus): string {
  switch (status.type) {
    case 'available': return 'green';
    case 'downloading': return 'blue';
    case 'not_downloaded': return 'gray';
    case 'corrupted': return 'red';
    case 'error': return 'red';
    default: return 'gray';
  }
}

export function getStatusLabel(status: LocalModelStatus): string {
  switch (status.type) {
    case 'available': return 'Available';
    case 'downloading': return `Downloading ${status.progress}%`;
    case 'not_downloaded': return 'Not Downloaded';
    case 'corrupted': return 'Corrupted';
    case 'error': return 'Error';
    default: return 'Unknown';
  }
}

export interface GgufCandidate {
  filename: string;
  download_url: string;
  size_bytes: number;
}

export interface HfRepoVerification {
  repo_id: string;
  gguf_files: GgufCandidate[];
  default_selection?: GgufCandidate;
}

// Tauri command wrappers for Local AI backend
import { invoke } from '@tauri-apps/api/core';

export class LocalAIAPI {
  static async listModels(): Promise<LocalModelInfo[]> {
    return await invoke('local_ai_list_models');
  }

  static async getModelInfo(modelName: string): Promise<LocalModelInfo | null> {
    return await invoke('local_ai_get_model_info', { modelName });
  }

  static async isModelReady(modelName: string, refresh: boolean = false): Promise<boolean> {
    return await invoke('local_ai_is_model_ready', { modelName, refresh });
  }

  static async getAvailableModel(): Promise<string | null> {
    return await invoke('local_ai_get_available_summary_model');
  }

  static async getAvailableEmbeddingModel(): Promise<string | null> {
    const models = await this.listModels();
    const available = models.find(
      (m) => m.model_type === 'embedding' && m.status.type === 'available'
    );
    return available?.name ?? null;
  }

  static async isAnyEmbeddingModelReady(): Promise<boolean> {
    const available = await this.getAvailableEmbeddingModel();
    return available !== null;
  }

  static async downloadModel(modelName: string): Promise<void> {
    await invoke('local_ai_download_model', { modelName });
  }

  static async cancelDownload(modelName: string): Promise<void> {
    await invoke('local_ai_cancel_download', { modelName });
  }

  static async deleteModel(modelName: string): Promise<void> {
    await invoke('local_ai_delete_model', { modelName });
  }

  static async getModelsDirectory(): Promise<string> {
    return await invoke('local_ai_get_models_directory');
  }

  static async verifyHfRepo(repo_id: string): Promise<HfRepoVerification> {
    return await invoke('verify_hf_repo', { repoId: repo_id });
  }

  static async addCustomModel(
    repo_id: string,
    filename: string,
    template: string,
    context_size: number,
    size_bytes: number
  ): Promise<void> {
    await invoke('add_custom_model', {
      repoId: repo_id,
      filename,
      template,
      contextSize: context_size,
      sizeBytes: size_bytes,
    });
  }
}
