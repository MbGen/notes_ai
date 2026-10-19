use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub const SETTINGS_PATH: &str = "data/settings.json";

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub use_laya: bool,
    pub laya_fallback: bool,
    pub llm_model: String,
    pub embeddings_model: String,
    pub search_limit: u32,
    pub classification_web: bool,
    pub sync_enabled: bool,
    pub sync_port: u16,
    pub sync_token: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            use_laya: false,
            laya_fallback: true,
            llm_model: "qwen3:4b-thinking".into(),
            embeddings_model: "nomic-embed-text-v2-moe:latest".into(),
            search_limit: 10,
            classification_web: true,
            sync_enabled: false,
            sync_port: 8743,
            sync_token: uuid::Uuid::new_v4().to_string(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = Path::new(SETTINGS_PATH);
        if !path.exists() {
            return Self::default();
        }
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = Path::new(SETTINGS_PATH).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(SETTINGS_PATH, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_settings_keep_models_and_gain_sync_defaults() {
        let s: Settings = serde_json::from_str(
            r#"{"llm_model":"custom","embeddings_model":"embed","search_limit":7}"#,
        )
        .unwrap();
        assert_eq!(s.llm_model, "custom");
        assert_eq!(s.search_limit, 7);
        assert!(!s.use_laya);
        assert!(s.laya_fallback);
        assert!(!s.sync_enabled);
        assert!(uuid::Uuid::parse_str(&s.sync_token).is_ok());
        let loaded: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(loaded.sync_token, s.sync_token);
    }
}
