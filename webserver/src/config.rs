use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{fs, io};

pub const CONFIG_PATH: &str = "conf.toml";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_interface")]
    pub interface: String,
    #[serde(default = "default_port")]
    pub port: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub php_fpm: String,
    #[serde(default = "default_root")]
    pub document_root: String,
}

fn default_interface() -> String {
    "0.0.0.0".into()
}

fn default_port() -> String {
    "8888".into()
}

fn default_root() -> String {
    "htdocs".into()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            interface: default_interface(),
            port: default_port(),
            php_fpm: String::new(),
            document_root: default_root(),
        }
    }
}

impl Config {
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
