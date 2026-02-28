use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    pub identity_path: PathBuf,
    #[serde(default = "default_session_dir")]
    pub session_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct LlmConfig {
    pub base_url: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_model() -> String { "qwen2.5".into() }
fn default_max_tokens() -> u32 { 4096 }
fn default_temperature() -> f32 { 0.7 }
fn default_session_dir() -> PathBuf { "sessions".into() }

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn load_identity(&self, base_dir: &Path) -> anyhow::Result<String> {
        let path = base_dir.join(&self.identity_path);
        Ok(std::fs::read_to_string(path)?)
    }
}
