use anyhow::Result;
use webserver_rs::{Config, server};

fn main() -> Result<()> {
    let config = Config::from_file("conf.toml")?;
    server::run(&config)
}
