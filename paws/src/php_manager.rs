use anyhow::{Context, Result, bail};
use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const START_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(windows)]
const PHP_FILE: &str = "php-cgi.exe";
#[cfg(not(windows))]
const PHP_FILE: &str = "php-fpm";

#[cfg(windows)]
const PHP_NAME: &str = "php-cgi";
#[cfg(not(windows))]
const PHP_NAME: &str = "php-fpm";

pub struct PhpServer {
    pub addr: String,
    child: Child,
    conf: Option<PathBuf>,
}

impl Drop for PhpServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(conf) = self.conf.take() {
            let _ = fs::remove_file(conf);
        }
    }
}

pub fn spawn(php: bool, docroot: &Path) -> Result<Option<PhpServer>> {
    if !php {
        return Ok(None);
    }

    let binary = find_binary().context("failed to find PHP binary")?;
    let port = free_port()?;
    let addr = format!("127.0.0.1:{port}");
    #[cfg(windows)]
    let (mut child, conf) = (spawn_cgi(&binary, port)?, None);
    #[cfg(not(windows))]
    let (mut child, conf) = {
        let (c, f) = spawn_fpm(&binary, port, docroot)?;
        (c, Some(f))
    };
    wait_until_listening(&mut child, &addr)
        .with_context(|| format!("failed to start PHP: {}", binary.display()))?;
    Ok(Some(PhpServer { addr, child, conf }))
}

fn find_binary() -> Result<PathBuf> {
    for candidate in candidates() {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    if let Some(p) = find_in_path(PHP_FILE).or_else(|| find_in_path(PHP_NAME)) {
        return Ok(p);
    }
    bail!(
        "php = true but no {PHP_NAME} found (checked next to executable, ../Resources/php, ../lib/paws/php, APPDIR, PATH)"
    )
}

fn search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        roots.push(dir.to_path_buf());
        if let Some(parent) = dir.parent() {
            roots.push(parent.to_path_buf());
            roots.push(parent.join("Resources"));
            roots.push(parent.join("lib").join("paws"));
            roots.push(parent.join("lib").join("PAWS"));
        }
    }
    if let Ok(appdir) = std::env::var("APPDIR") {
        roots.push(PathBuf::from(appdir).join("usr").join("lib").join("paws"));
    }
    roots
}

fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for root in search_roots() {
        paths.push(root.join(PHP_FILE));
        paths.push(root.join("php").join(PHP_FILE));
    }
    paths
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PATH") {
        for p in std::env::split_paths(&path) {
            let candidate = p.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn spawn_fpm(binary: &Path, port: u16, docroot: &Path) -> Result<(Child, PathBuf)> {
    let conf = std::env::temp_dir().join(format!("paws-php-fpm-{port}.conf"));
    let docroot_s = docroot.display();
    let text = format!(
        "[global]\nerror_log = /dev/stderr\ndaemonize = no\n[www]\nlisten = 127.0.0.1:{port}\npm = dynamic\npm.max_children = 4\npm.start_servers = 2\npm.min_spare_servers = 1\npm.max_spare_servers = 3\ncatch_workers_output = yes\nphp_admin_value[open_basedir] = {docroot_s}:/tmp\n"
    );
    fs::write(&conf, text).with_context(|| format!("failed to write {}", conf.display()))?;
    let mut cmd = Command::new(binary);
    cmd.arg("-F").arg("-y").arg(&conf).arg("-R");
    apply_php_ini(&mut cmd, binary);
    let child = cmd
        .spawn()
        .with_context(|| format!("failed to start {}", binary.display()))?;
    Ok((child, conf))
}

#[cfg(windows)]
fn spawn_cgi(binary: &Path, port: u16) -> Result<Child> {
    let mut cmd = Command::new(binary);
    cmd.arg("-b").arg(format!("127.0.0.1:{port}"));
    if let Some(dir) = binary.parent().map(|p| p.join("ext")) {
        if dir.is_dir() {
            cmd.arg("-d")
                .arg(format!("extension_dir={}", dir.display()));
        }
    }
    apply_php_ini(&mut cmd, binary);
    cmd.spawn()
        .with_context(|| format!("failed to start {}", binary.display()))
}

fn apply_php_ini(cmd: &mut Command, binary: &Path) {
    if let Some(parent) = binary.parent() {
        let ini = parent.join("php.ini");
        if ini.is_file() {
            cmd.env("PHPRC", parent);
        }
    }
}

fn free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("failed to bind temp port")?;
    let port = listener
        .local_addr()
        .context("failed to get temp port")?
        .port();
    drop(listener);
    Ok(port)
}

fn wait_until_listening(child: &mut Child, addr: &str) -> Result<()> {
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        if TcpStream::connect(addr).is_ok() {
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            bail!("PHP exited during startup ({status})");
        }
        if Instant::now() >= deadline {
            bail!(
                "PHP did not listen on {addr} within {}s",
                START_TIMEOUT.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
