use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{fs, io};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_interface")]
    pub interface: String,
    #[serde(default = "default_port")]
    pub port: String,
}

fn default_interface() -> String {
    "0.0.0.0".into()
}

fn default_port() -> String {
    "8888".into()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            interface: default_interface(),
            port: default_port(),
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
