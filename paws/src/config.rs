use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{fs, io};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_interface")]
    pub interface: String,
    #[serde(default = "default_port")]
    pub port: String,
    #[serde(default = "default_php")]
    pub php: bool,
    #[serde(default = "default_root")]
    pub document_root: String,
}

fn default_interface() -> String {
    "0.0.0.0".into()
}

fn default_port() -> String {
    "80".into()
}

fn default_php() -> bool {
    true
}

fn default_root() -> String {
    let documents =
        dirs::document_dir().or_else(|| dirs::home_dir().map(|home| home.join("Documents")));
    match documents {
        Some(dir) => dir.join("htdocs").to_string_lossy().into_owned(),
        None => "htdocs".into(),
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            interface: default_interface(),
            port: default_port(),
            php: default_php(),
            document_root: default_root(),
        }
    }
}

impl Config {
    pub fn config_path() -> std::path::PathBuf {
        let mut candidates: Vec<std::path::PathBuf> = Vec::new();
        if let Some(config_dir) = dirs::config_dir() {
            let app_dir = config_dir.join("paws");
            let _ = std::fs::create_dir_all(&app_dir);
            candidates.push(app_dir.join("conf.toml"));
        }
        candidates.push("conf.toml".into());
        if let Some(existing) = candidates.iter().find(|p| p.exists()) {
            return existing.clone();
        }
        if let Some(config_dir) = dirs::config_dir() {
            let app_dir = config_dir.join("paws");
            let _ = std::fs::create_dir_all(&app_dir);
            return app_dir.join("conf.toml");
        }
        "conf.toml".into()
    }

    pub fn load() -> Result<Self> {
        Self::from_file(Self::config_path())
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        let contents = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                eprintln!("WARN: {} not found, using default options", path.display());
                return Ok(Self::default());
            }
            Err(e) => {
                return Err(e).with_context(|| format!("failed to read {}", path.display()));
            }
        };

        toml::from_str(&contents).with_context(|| format!("invalid config in {}", path.display()))
    }
}
