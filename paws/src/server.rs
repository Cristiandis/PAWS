use crate::{Config, ThreadPool, php};
use anyhow::{Context, Result, bail};
use percent_encoding::percent_decode_str;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{self, IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

const MAX_BODY: usize = 10 * 1024 * 1024;

struct Request {
    method: String,
    path: String,
    query: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

pub fn run(config: &Config, shutdown: &Arc<AtomicBool>) -> Result<()> {
    let listener = net::TcpListener::bind(format!("{}:{}", config.interface, config.port))
        .with_context(|| {
            format!(
                "failed to bind {}:{}; ports below 1024 need root or CAP_NET_BIND_SERVICE",
                config.interface, config.port
            )
        })?;
    let pool = ThreadPool::new(NonZeroUsize::new(4).unwrap())?;

    let docroot_path = if Path::new(&config.document_root).is_absolute() {
        PathBuf::from(&config.document_root)
    } else {
        std::env::current_dir()?.join(&config.document_root)
    };

    fs::create_dir_all(&docroot_path)
        .with_context(|| format!("Failed to create directory: {}", docroot_path.display()))?;

    let docroot = Arc::new(docroot_path.canonicalize().with_context(|| {
        format!(
            "Error: Cannot canonicalize path: {}",
            docroot_path.display()
        )
    })?);
    let php = crate::php_manager::spawn(config.php, &docroot)?;
    let php_addr = php.as_ref().map(|p| p.addr.clone());
    let config = Arc::new(config.clone());

    println!("Listening on: {}:{}", config.interface, config.port);
    println!("Document root: {}", docroot.display());
    match php_addr.as_deref() {
        Some(addr) => println!("PHP enabled via bundled PHP at {addr}"),
        None => println!("PHP disabled (php = false in conf.toml)"),
    }

    let mut wake_addr = listener.local_addr()?;
    match wake_addr.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => {
            wake_addr.set_ip(IpAddr::V4(Ipv4Addr::LOCALHOST));
        }
        IpAddr::V6(ip) if ip.is_unspecified() => {
            wake_addr.set_ip(IpAddr::V6(Ipv6Addr::LOCALHOST));
        }
        _ => {}
    }
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let waker = {
        let shutdown = Arc::clone(shutdown);
        std::thread::spawn(move || {
            let mut woken = false;
            loop {
                match done_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                if !woken && shutdown.load(Ordering::SeqCst) {
                    woken = TcpStream::connect(wake_addr).is_ok();
                }
            }
        })
    };

    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        match listener.accept() {
            Ok((stream, _)) => {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }

                let config = Arc::clone(&config);
                let docroot = Arc::clone(&docroot);
                let php_addr = php_addr.clone();

                if let Err(e) =
                    pool.execute(move || handle_connection_stream(stream, config, docroot, php_addr))
                {
                    eprintln!("could not schedule job: {e}");
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }

    let _ = done_tx.send(());
    let _ = waker.join();

    println!("Shutting down...");

    Ok(())
}

fn handle_connection_stream(mut stream: TcpStream, config: Arc<Config>, docroot: Arc<PathBuf>, php_addr: Option<String>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let peer = stream.peer_addr().ok();

    let response = match read_request(&stream) {
        Ok(Some(req)) => {
            println!("{} {}", req.method, req.path);
            route(&req, &config, &docroot, php_addr.as_deref(), peer)
        }
        Ok(None) => return,
        Err(e) => {
            if let Some(io) = e.downcast_ref::<std::io::Error>() {
                if matches!(io.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) {
                    return;
                }
            }
            eprintln!("bad request: {e}");
            status_only("400 Bad Request")
        }
    };

    let _ = stream.write_all(&response);
}

fn read_request(stream: &TcpStream) -> Result<Option<Request>> {
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().context("empty request")?.to_string();
    let target = parts.next().context("request has no target")?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (path, query) = (path.to_string(), query.to_string());

    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        reader.read_line(&mut h)?;
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }

    let len = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .unwrap_or(0);
    if len > MAX_BODY {
        bail!("body too large");
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;

    Ok(Some(Request {
        method,
        path,
        query,
        headers,
        body,
    }))
}

fn route(
    req: &Request,
    config: &Config,
    docroot: &Path,
    php_addr: Option<&str>,
    peer: Option<SocketAddr>,
) -> Vec<u8> {
    let Some(file) = resolve(docroot, &req.path, php_addr.is_some()) else {
        return status_only("404 Not Found");
    };

    let is_php = file.extension().is_some_and(|e| e == "php");

    if is_php {
        let Some(addr) = php_addr else {
            return status_only("404 Not Found");
        };
        let params = build_params(req, &file, docroot, config, peer);
        return match php::fastcgi_request(addr, &params, &req.body) {
            Ok(out) => php_response(&out),
            Err(e) => {
                eprintln!("php-fpm error: {e}");
                status_only("502 Bad Gateway")
            }
        };
    }

    match fs::read(&file) {
        Ok(contents) => {
            let mut r = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                content_type(&file),
                contents.len()
            )
            .into_bytes();
            r.extend_from_slice(&contents);
            r
        }
        Err(_) => status_only("404 Not Found"),
    }
}

fn resolve(docroot: &Path, url_path: &str, php: bool) -> Option<PathBuf> {
    let decoded = percent_decode_str(url_path).decode_utf8().ok()?;
    let mut full = docroot.join(decoded.trim_start_matches('/'));
    if full.is_dir() {
        let php_index = full.join("index.php");
        full = if php && php_index.exists() {
            php_index
        } else {
            full.join("index.html")
        };
    }
    let full = full.canonicalize().ok()?;
    full.starts_with(docroot).then_some(full)
}

fn build_params(
    req: &Request,
    script: &Path,
    docroot: &Path,
    config: &Config,
    peer: Option<SocketAddr>,
) -> Vec<(String, String)> {
    let script_name = script
        .strip_prefix(docroot)
        .map(|p| format!("/{}", p.display()))
        .unwrap_or_default();
    let uri = if req.query.is_empty() {
        req.path.clone()
    } else {
        format!("{}?{}", req.path, req.query)
    };

    let mut p = vec![
        ("GATEWAY_INTERFACE".to_string(), "CGI/1.1".to_string()),
        ("SERVER_PROTOCOL".to_string(), "HTTP/1.1".to_string()),
        ("REQUEST_METHOD".to_string(), req.method.clone()),
        ("SCRIPT_FILENAME".to_string(), script.display().to_string()),
        ("SCRIPT_NAME".to_string(), script_name),
        ("REQUEST_URI".to_string(), uri),
        ("QUERY_STRING".to_string(), req.query.clone()),
        ("DOCUMENT_ROOT".to_string(), docroot.display().to_string()),
        ("SERVER_NAME".to_string(), config.interface.clone()),
        ("SERVER_PORT".to_string(), config.port.to_string()),
        ("CONTENT_LENGTH".to_string(), req.body.len().to_string()),
    ];

    if let Some(peer) = peer {
        p.push(("REMOTE_ADDR".to_string(), peer.ip().to_string()));
    }

    for (k, v) in &req.headers {
        if k.eq_ignore_ascii_case("content-type") {
            p.push(("CONTENT_TYPE".to_string(), v.clone()));
        }
        if !k.eq_ignore_ascii_case("content-length") {
            p.push((
                format!("HTTP_{}", k.to_uppercase().replace('-', "_")),
                v.clone(),
            ));
        }
    }

    p
}

fn php_response(cgi: &[u8]) -> Vec<u8> {
    let (head, body) = match cgi.windows(4).position(|w| w == b"\r\n\r\n") {
        Some(i) => (&cgi[..i], &cgi[i + 4..]),
        None => (&cgi[..0], cgi),
    };

    let mut status = "200 OK".to_string();
    let mut headers = String::new();
    for line in String::from_utf8_lossy(head).lines() {
        if let Some(s) = line.strip_prefix("Status:") {
            status = s.trim().to_string();
        } else if !line.is_empty() {
            headers.push_str(line);
            headers.push_str("\r\n");
        }
    }

    let mut r = format!(
        "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    r.extend_from_slice(body);
    r
}

fn status_only(status: &str) -> Vec<u8> {
    format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()
}

fn content_type(p: &Path) -> &'static str {
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    match ext.as_deref() {
        Some("html" | "htm" | "php") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("txt") => "text/plain; charset=utf-8",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("mp4") => "video/mp4",
        Some("mp3") => "audio/mpeg",
        _ => "application/octet-stream",
    }
}
