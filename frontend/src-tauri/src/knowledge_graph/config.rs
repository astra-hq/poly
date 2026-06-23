use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Top-level knowledge graph settings container.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnowledgeGraphSettings {
    pub profiles: Vec<KnowledgeGraphProfile>,
    #[serde(default)]
    pub active_profile: KnowledgeGraphSelection,
}

/// A named knowledge graph profile (server + embedding configuration).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnowledgeGraphProfile {
    pub name: String,
    #[serde(default)]
    pub embedding: EmbeddingConfig,
    pub lightrag_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Embedding provider and model configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingConfig {
    pub provider: String,
    pub model: String,
    pub dimensions: usize,
}

/// Which profile is currently selected.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeGraphSelection {
    None,
    Profile(String),
}

impl Default for KnowledgeGraphSelection {
    fn default() -> Self {
        KnowledgeGraphSelection::None
    }
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        EmbeddingConfig {
            provider: "mlx".to_string(),
            model: "BAAI/bge-m3".to_string(),
            dimensions: 1024,
        }
    }
}

impl Default for KnowledgeGraphProfile {
    fn default() -> Self {
        KnowledgeGraphProfile {
            name: "default".to_string(),
            embedding: EmbeddingConfig::default(),
            lightrag_url: "http://localhost:9621".to_string(),
            api_key: None,
        }
    }
}

impl Default for KnowledgeGraphSettings {
    fn default() -> Self {
        KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile::default()],
            active_profile: KnowledgeGraphSelection::default(),
        }
    }
}

/// Parse `KnowledgeGraphSettings` from a JSON string.
pub fn load(json: &str) -> Result<KnowledgeGraphSettings> {
    let settings: KnowledgeGraphSettings =
        serde_json::from_str(json).map_err(|e| anyhow!("Failed to parse KG settings: {}", e))?;
    Ok(settings)
}

/// Resolve the currently active profile, if any.
pub fn resolve(settings: &KnowledgeGraphSettings) -> Option<&KnowledgeGraphProfile> {
    match &settings.active_profile {
        KnowledgeGraphSelection::None => None,
        KnowledgeGraphSelection::Profile(name) => {
            settings.profiles.iter().find(|p| p.name == *name)
        }
    }
}

/// Validate a `KnowledgeGraphSettings` value.
///
/// Checks:
/// - at least one profile exists
/// - no empty profile names
/// - no empty LightRAG URLs
/// - embedding config is well-formed
/// - the active profile (if set) exists in the profile list
pub fn validate(settings: &KnowledgeGraphSettings) -> Result<()> {
    if settings.profiles.is_empty() {
        return Err(anyhow!("At least one profile is required"));
    }

    for profile in &settings.profiles {
        if profile.name.trim().is_empty() {
            return Err(anyhow!("Profile name cannot be empty"));
        }
        if profile.lightrag_url.trim().is_empty() {
            return Err(anyhow!(
                "LightRAG URL cannot be empty for profile '{}'",
                profile.name
            ));
        }
        if profile.embedding.provider.trim().is_empty() {
            return Err(anyhow!(
                "Embedding provider cannot be empty for profile '{}'",
                profile.name
            ));
        }
        if profile.embedding.model.trim().is_empty() {
            return Err(anyhow!(
                "Embedding model cannot be empty for profile '{}'",
                profile.name
            ));
        }
        if profile.embedding.dimensions == 0 {
            return Err(anyhow!(
                "Embedding dimensions must be greater than 0 for profile '{}'",
                profile.name
            ));
        }
    }

    match &settings.active_profile {
        KnowledgeGraphSelection::Profile(name) => {
            if !settings.profiles.iter().any(|p| p.name == *name) {
                return Err(anyhow!(
                    "Active profile '{}' not found in profiles list",
                    name
                ));
            }
        }
        KnowledgeGraphSelection::None => {}
    }

    Ok(())
}

/// Compute a stable fingerprint for a profile so callers can detect
/// meaningful configuration changes.
pub fn fingerprint(profile: &KnowledgeGraphProfile) -> String {
    let mut hasher = DefaultHasher::new();
    profile.name.hash(&mut hasher);
    profile.embedding.provider.hash(&mut hasher);
    profile.embedding.model.hash(&mut hasher);
    profile.embedding.dimensions.hash(&mut hasher);
    profile.lightrag_url.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_embedding_config() {
        let ec = EmbeddingConfig::default();
        assert_eq!(ec.provider, "mlx");
        assert_eq!(ec.model, "BAAI/bge-m3");
        assert_eq!(ec.dimensions, 1024);
    }

    #[test]
    fn default_selection_is_none() {
        let sel = KnowledgeGraphSelection::default();
        assert_eq!(sel, KnowledgeGraphSelection::None);
    }

    #[test]
    fn default_settings_has_one_profile() {
        let s = KnowledgeGraphSettings::default();
        assert_eq!(s.profiles.len(), 1);
        assert_eq!(s.profiles[0].name, "default");
        assert_eq!(s.active_profile, KnowledgeGraphSelection::None);
    }

    #[test]
    fn load_valid_json() {
        let json = r#"{
            "profiles": [
                {
                    "name": "local",
                    "embedding": {
                        "provider": "mlx",
                        "model": "BAAI/bge-m3",
                        "dimensions": 1024
                    },
                    "lightrag_url": "http://localhost:9621"
                }
            ],
            "active_profile": {"profile": "local"}
        }"#;
        let s = load(json).unwrap();
        assert_eq!(s.profiles.len(), 1);
        assert_eq!(s.profiles[0].name, "local");
        assert_eq!(
            s.active_profile,
            KnowledgeGraphSelection::Profile("local".into())
        );
    }

    #[test]
    fn load_json_with_defaults() {
        let json = r#"{"profiles": [{"name": "x", "lightrag_url": "http://localhost:9621"}]}"#;
        let s = load(json).unwrap();
        assert_eq!(s.profiles[0].embedding.provider, "mlx");
        assert_eq!(s.profiles[0].embedding.model, "BAAI/bge-m3");
        assert_eq!(s.active_profile, KnowledgeGraphSelection::None);
    }

    #[test]
    fn load_invalid_json_fails() {
        let err = load("not json").unwrap_err();
        assert!(err.to_string().contains("Failed to parse KG settings"));
    }

    #[test]
    fn resolve_active_profile() {
        let s = KnowledgeGraphSettings {
            profiles: vec![
                KnowledgeGraphProfile {
                    name: "a".into(),
                    ..Default::default()
                },
                KnowledgeGraphProfile {
                    name: "b".into(),
                    ..Default::default()
                },
            ],
            active_profile: KnowledgeGraphSelection::Profile("b".into()),
        };
        let resolved = resolve(&s).unwrap();
        assert_eq!(resolved.name, "b");
    }

    #[test]
    fn resolve_none_returns_none() {
        let s = KnowledgeGraphSettings::default();
        assert!(resolve(&s).is_none());
    }

    #[test]
    fn validate_empty_profiles_fails() {
        let s = KnowledgeGraphSettings {
            profiles: vec![],
            active_profile: KnowledgeGraphSelection::None,
        };
        let err = validate(&s).unwrap_err();
        assert!(err.to_string().contains("At least one profile is required"));
    }

    #[test]
    fn validate_empty_profile_name_fails() {
        let s = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                name: "".into(),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        let err = validate(&s).unwrap_err();
        assert!(err.to_string().contains("Profile name cannot be empty"));
    }

    #[test]
    fn validate_empty_url_fails() {
        let s = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                name: "x".into(),
                lightrag_url: "".into(),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        let err = validate(&s).unwrap_err();
        assert!(err.to_string().contains("LightRAG URL cannot be empty"));
    }

    #[test]
    fn validate_zero_dimensions_fails() {
        let s = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                name: "x".into(),
                embedding: EmbeddingConfig {
                    provider: "mlx".into(),
                    model: "BAAI/bge-m3".into(),
                    dimensions: 0,
                },
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        let err = validate(&s).unwrap_err();
        assert!(err
            .to_string()
            .contains("dimensions must be greater than 0"));
    }

    #[test]
    fn validate_missing_active_profile_fails() {
        let s = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                name: "a".into(),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::Profile("missing".into()),
        };
        let err = validate(&s).unwrap_err();
        assert!(err
            .to_string()
            .contains("Active profile 'missing' not found"));
    }

    #[test]
    fn validate_valid_settings_succeeds() {
        let s = KnowledgeGraphSettings::default();
        assert!(validate(&s).is_ok());
    }

    #[test]
    fn fingerprint_is_stable() {
        let p = KnowledgeGraphProfile::default();
        let fp1 = fingerprint(&p);
        let fp2 = fingerprint(&p);
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 16);
    }

    #[test]
    fn fingerprint_changes_with_content() {
        let p1 = KnowledgeGraphProfile::default();
        let mut p2 = p1.clone();
        p2.name = "other".into();
        assert_ne!(fingerprint(&p1), fingerprint(&p2));
    }

    #[test]
    fn roundtrip_serialization() {
        let original = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                name: "prod".into(),
                embedding: EmbeddingConfig {
                    provider: "mlx".into(),
                    model: "BAAI/bge-m3".into(),
                    dimensions: 1024,
                },
                lightrag_url: "http://localhost:9621".into(),
                api_key: Some("secret".into()),
            }],
            active_profile: KnowledgeGraphSelection::Profile("prod".into()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: KnowledgeGraphSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }
}
