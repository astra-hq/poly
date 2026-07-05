/**
 * Provider Types
 *
 * TypeScript mirrors of the Rust serde types in
 * `src-tauri/src/providers/mod.rs`.
 */

/** Matches Rust `ProviderType` enum with `#[serde(rename_all = "snake_case")]`. */
export type ProviderType =
  | 'open_a_i'
  | 'anthropic'
  | 'groq'
  | 'ollama'
  | 'open_router'
  | 'custom'
  | 'local';

/** Display label for each provider type — matches Rust `ProviderType::label()`. */
export const PROVIDER_TYPE_LABELS: Record<ProviderType, string> = {
  open_a_i: 'OpenAI',
  anthropic: 'Anthropic',
  groq: 'Groq',
  ollama: 'Ollama',
  open_router: 'OpenRouter',
  custom: 'Custom',
  local: 'Local',
};

/** Default base URL for each provider type. */
export const PROVIDER_DEFAULT_URLS: Record<ProviderType, string> = {
  open_a_i: 'https://api.openai.com/v1',
  anthropic: 'https://api.anthropic.com',
  groq: 'https://api.groq.com/openai/v1',
  ollama: 'http://localhost:11434',
  open_router: 'https://openrouter.ai/api/v1',
  custom: '',
  local: '',
};

/** Default model for each provider type. */
export const PROVIDER_DEFAULT_MODELS: Partial<Record<ProviderType, string>> = {
  open_a_i: 'gpt-4o',
  ollama: 'llama3.1:8b',
};

/** Matches Rust `ProviderConfig` struct. */
export interface ProviderConfig {
  id: string;
  name: string;
  type: ProviderType;
  base_url: string;
  default_model: string;
}

/** Model returned by `api_get_provider_models`. */
export interface ProviderModel {
  id: string;
  name: string;
}
