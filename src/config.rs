use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub email: String,
    pub password: String,
    pub download_dir: PathBuf,
    #[serde(default)]
    pub sync_enabled: bool,
}

impl Config {
    pub fn load(path: &Path) -> eyre::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(toml::from_str(&content)?)
        }
    }
}
