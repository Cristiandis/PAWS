use anyhow::Result;
use clap::Parser;
use webserver::{Config, server};

#[derive(Parser, Debug)]
struct Cli {
    /// Open config gui
    #[arg(short = 'c', long)]
    config: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.config {
        true => Ok(config_helper::run()?),
        false => {
            let config = Config::from_file("conf.toml")?;
            server::run(&config)
        }
    }
}
