use eframe::egui;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const STOP_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_LOG_LINES: usize = 1000;

pub struct LogLine {
    pub stderr: bool,
    pub text: String,
}

pub enum Status {
    Stopped,
    Running { pid: u32 },
    Stopping { since: Instant },
    Exited { code: Option<i32> },
    Error(String),
}

pub struct ServerProcess {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    tx: Sender<LogLine>,
    rx: Receiver<LogLine>,
    ctx: egui::Context,
    restart_pending: bool,
    pub status: Status,
    pub logs: Vec<LogLine>,
}

impl ServerProcess {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            child: None,
            stdin: None,
            tx,
            rx,
            ctx,
            restart_pending: false,
            status: Status::Stopped,
            logs: Vec::new(),
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(
            self.status,
            Status::Running { .. } | Status::Stopping { .. }
        )
    }

    pub fn start(&mut self) {
        if self.is_running() {
            return;
        }

        let path = match server_binary() {
            Ok(p) => p,
            Err(e) => {
                self.status = Status::Error(e);
                return;
            }
        };

        let mut cmd = Command::new(&path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);

        match cmd.spawn() {
            Ok(mut child) => {
                let stdout = child.stdout.take().expect("stdout was piped");
                let stderr = child.stderr.take().expect("stderr was piped");
                self.stdin = child.stdin.take();

                spawn_reader(stdout, false, self.tx.clone(), self.ctx.clone());
                spawn_reader(stderr, true, self.tx.clone(), self.ctx.clone());

                let pid = child.id();
                self.child = Some(child);
                self.status = Status::Running { pid };
                let display = path.display().to_string();
                self.push_system(egui_i18n::tr!("log-started", { path: &display }));
            }
            Err(e) => {
                let display = path.display().to_string();
                self.status = Status::Error(
                    egui_i18n::tr!("err-start-failed", { path: &display, error: &e }),
                );
            }
        }
    }

    pub fn request_stop(&mut self) {
        if !matches!(self.status, Status::Running { .. }) {
            return;
        }

        self.restart_pending = false;

        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"quit\n");
            let _ = stdin.flush();
        }

        self.status = Status::Stopping {
            since: Instant::now(),
        };
    }

    /// Stop the server if it is running, then start it again once it exits. If
    /// it is not running, start it immediately.
    pub fn request_restart(&mut self) {
        if self.is_running() {
            self.restart_pending = true;
            if matches!(self.status, Status::Running { .. }) {
                if let Some(mut stdin) = self.stdin.take() {
                    let _ = stdin.write_all(b"quit\n");
                    let _ = stdin.flush();
                }
                self.status = Status::Stopping {
                    since: Instant::now(),
                };
            }
        } else {
            self.start();
        }
    }

    pub fn poll(&mut self) {
        while let Ok(line) = self.rx.try_recv() {
            self.logs.push(line);
        }
        let over = self.logs.len().saturating_sub(MAX_LOG_LINES);
        if over > 0 {
            self.logs.drain(..over);
        }

        let stopping_since = match &self.status {
            Status::Stopping { since } => Some(*since),
            _ => None,
        };

        let Some(child) = self.child.as_mut() else {
            return;
        };

        match child.try_wait() {
            Ok(Some(exit)) => {
                let code = exit.code();
                self.child = None;
                if self.restart_pending {
                    self.restart_pending = false;
                    self.push_system(egui_i18n::tr!("log-restarting"));
                    self.status = Status::Stopped;
                    self.start();
                } else {
                    let code = code_str(code);
                    self.push_system(egui_i18n::tr!("log-server-exited", { code: &code }));
                    self.status = Status::Exited { code: exit.code() };
                }
            }
            Ok(None) => {
                if let Some(since) = stopping_since
                    && since.elapsed() >= STOP_TIMEOUT
                {
                    let _ = child.kill();
                    let code = child.wait().map(|s| s.code()).unwrap_or(None);
                    self.child = None;
                    if self.restart_pending {
                        self.restart_pending = false;
                        self.push_system(egui_i18n::tr!("log-stop-timeout-restart"));
                        self.status = Status::Stopped;
                        self.start();
                    } else {
                        self.push_system(egui_i18n::tr!("log-stop-timeout"));
                        self.status = Status::Exited { code };
                    }
                }
            }
            Err(e) => {
                self.status =
                    Status::Error(egui_i18n::tr!("err-wait-failed", { error: &e.to_string() }));
                self.child = None;
            }
        }
    }

    fn push_system(&mut self, text: String) {
        self.logs.push(LogLine {
            stderr: false,
            text,
        });
    }

    pub fn note(&mut self, text: String) {
        self.push_system(text);
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        drop(self.stdin.take());

        let Some(child) = self.child.as_mut() else {
            return;
        };
        for _ in 0..15 {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn server_binary() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| {
        egui_i18n::tr!("err-locate-gui", {
            error: &e.to_string()
        })
    })?;
    let dir = exe
        .parent()
        .ok_or_else(|| egui_i18n::tr!("err-no-parent"))?;
    let path = dir.join(format!("paws{}", std::env::consts::EXE_SUFFIX));
    if path.exists() {
        Ok(path)
    } else {
        let display = dir.display().to_string();
        Err(egui_i18n::tr!("err-paws-not-found", { dir: &display }))
    }
}

fn spawn_reader<R>(stream: R, stderr: bool, tx: Sender<LogLine>, ctx: egui::Context)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let mut text = String::from_utf8_lossy(&buf).into_owned();
                    while text.ends_with('\n') || text.ends_with('\r') {
                        text.pop();
                    }
                    if tx.send(LogLine { stderr, text }).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            }
        }
        ctx.request_repaint();
    });
}

fn code_str(code: Option<i32>) -> String {
    match code {
        Some(0) => egui_i18n::tr!("code-ok"),
        Some(_) => egui_i18n::tr!("code-error"),
        None => egui_i18n::tr!("code-killed"),
    }
}
