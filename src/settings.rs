use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub const SETTINGS_PATH: &str = "data/settings.json";

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub llm_model: String,
    pub embeddings_model: String,
    pub search_limit: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            llm_model: "qwen3:4b-thinking".into(),
            embeddings_model: "nomic-embed-text-v2-moe:latest".into(),
            search_limit: 10,
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