use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub api_id: i32,
    pub api_hash: String,
    pub session_path: String,

    #[serde(default = "default_prefix")]
    pub prefix: String,

    #[serde(default = "default_save_dir")]
    pub save_dir: String,

    #[serde(default = "default_afk_msg")]
    pub afk_msg: String,

    #[serde(default = "default_bash_limit")]
    pub bash_char_limit: usize,
}

fn default_prefix() -> String {
    ".".into()
}
fn default_save_dir() -> String {
    "saved".into()
}
fn default_afk_msg() -> String {
    "I'm AFK right now".into()
}
fn default_bash_limit() -> usize {
    4000
}

pub fn load() -> anyhow::Result<Config> {
    let raw = fs::read_to_string("config.toml")
        .map_err(|_| anyhow::anyhow!("config.toml not found: copy config.toml.example"))?;
    Ok(toml::from_str(&raw)?)
}
