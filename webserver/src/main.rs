use anyhow::Result;
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use webserver::{Config, server};

fn main() -> Result<()> {
    let config = Config::from_file("conf.toml")?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&shutdown);
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut line = String::new();
        loop {
            line.clear();
            match stdin.lock().read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) if line.trim() == "quit" => break,
                Ok(_) => {}
            }
        }
        flag.store(true, Ordering::SeqCst);
    });

    server::run(&config, &shutdown)
}
