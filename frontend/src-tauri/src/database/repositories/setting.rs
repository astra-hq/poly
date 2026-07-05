use crate::secrets::store::SecretStore;

pub struct SettingsRepository;

// Transcript providers: localWhisper, deepgram, elevenLabs, groq, openai
// Summary providers: openai, claude, ollama, groq, added openrouter
// NOTE: Handle data exclusion in the higher layer as this is database abstraction layer(using SELECT *)

impl SettingsRepository {
    pub async fn save_api_key(
        store: &(dyn SecretStore + Sync),
        provider: &str,
        api_key: &str,
    ) -> std::result::Result<(), String> {
        if provider == "local" {
            return Ok(()); // No API key needed
        }

        let secret_ref = if provider == "custom-openai" {
            crate::secrets::refs::custom_openai_key()
        } else {
            crate::secrets::refs::summary_provider_key(provider)
        };

        store
            .set(&secret_ref, api_key)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get_api_key(
        store: &(dyn SecretStore + Sync),
        provider: &str,
    ) -> std::result::Result<Option<String>, String> {
        if provider == "local" {
            return Ok(None); // No API key needed
        }

        let secret_ref = if provider == "custom-openai" {
            crate::secrets::refs::custom_openai_key()
        } else {
            crate::secrets::refs::summary_provider_key(provider)
        };

        store.get(&secret_ref).await.map_err(|e| e.to_string())
    }

    pub async fn save_transcript_api_key(
        store: &(dyn SecretStore + Sync),
        provider: &str,
        api_key: &str,
    ) -> std::result::Result<(), String> {
        if provider == "parakeet" || provider == "local" {
            return Ok(()); // Local transcription doesn't need an API key
        }

        let secret_ref = crate::secrets::refs::transcript_provider_key(provider);
        store
            .set(&secret_ref, api_key)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get_transcript_api_key(
        store: &(dyn SecretStore + Sync),
        provider: &str,
    ) -> std::result::Result<Option<String>, String> {
        if provider == "parakeet" || provider == "local" {
            return Ok(None); // Local transcription doesn't need an API key
        }

        let secret_ref = crate::secrets::refs::transcript_provider_key(provider);
        store.get(&secret_ref).await.map_err(|e| e.to_string())
    }

    pub async fn delete_api_key(
        store: &(dyn SecretStore + Sync),
        provider: &str,
    ) -> std::result::Result<(), String> {
        if provider == "local" {
            return Ok(()); // No API key needed
        }

        let secret_ref = if provider == "custom-openai" {
            crate::secrets::refs::custom_openai_key()
        } else {
            crate::secrets::refs::summary_provider_key(provider)
        };

        store.delete(&secret_ref).await.map_err(|e| e.to_string())
    }

    /// Delete the transcript API key for the given provider from the SecretStore.
    pub async fn delete_transcript_api_key(
        store: &(dyn SecretStore + Sync),
        provider: &str,
    ) -> std::result::Result<(), String> {
        if provider == "parakeet" || provider == "local" {
            return Ok(()); // Local transcription doesn't need an API key
        }

        let secret_ref = crate::secrets::refs::transcript_provider_key(provider);
        store.delete(&secret_ref).await.map_err(|e| e.to_string())
    }
}
