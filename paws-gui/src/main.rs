#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::time::Duration;

mod config_panel;
mod process;

use config_panel::ConfigPanel;
use process::{ServerProcess, Status};

#[derive(PartialEq, Eq, Clone, Copy)]
enum Tab {
    Server,
    Config,
}

struct App {
    tab: Tab,
    server: ServerProcess,
    config: ConfigPanel,
    autoscroll: bool,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Server, "Server");
            ui.selectable_value(&mut self.tab, Tab::Config, "Config");
        });
        ui.separator();

        match self.tab {
            Tab::Server => self.server_ui(ui),
            Tab::Config => self.config.ui(ui),
        }
    }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            tab: Tab::Server,
            server: ServerProcess::new(cc.egui_ctx.clone()),
            config: ConfigPanel::new(),
            autoscroll: true,
        }
    }

    fn server_ui(&mut self, ui: &mut egui::Ui) {
        self.server.poll();

        ui.horizontal(|ui| {
            let stopping = matches!(self.server.status, Status::Stopping { .. });
            if self.server.is_running() {
                if ui
                    .add_enabled(!stopping, egui::Button::new("Stop"))
                    .clicked()
                {
                    self.server.request_stop();
                }
                if ui.button("Restart").clicked() {
                    self.server.request_restart();
                }
            } else if ui.button("Start").clicked() {
                self.server.start();
            }

            ui.separator();
            status_indicator(ui, &self.server.status);
        });

        ui.add_space(2.0);

        ui.horizontal(|ui| {
            if ui.button("Open in browser").clicked() {
                let url = self.server_url();
                if let Err(e) = open::that(&url) {
                    self.server.note(format!("could not open {url}: {e}"));
                }
            }
            if ui.button("Open document root").clicked() {
                let root = self.config.config().document_root.trim().to_string();
                if root.is_empty() {
                    self.server.note("document root is empty".into());
                } else if let Err(e) = std::fs::create_dir_all(&root) {
                    self.server.note(format!("could not create {root}: {e}"));
                } else if let Err(e) = open::that(&root) {
                    self.server.note(format!("could not open {root}: {e}"));
                }
            }
            if ui.button("Clear logs").clicked() {
                self.server.logs.clear();
            }
            if ui.button("Copy logs").clicked() {
                let text = self
                    .server
                    .logs
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                ui.ctx().copy_text(text);
            }
            ui.checkbox(&mut self.autoscroll, "Autoscroll");
        });

        ui.separator();

        egui::ScrollArea::vertical()
            .stick_to_bottom(self.autoscroll)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for line in &self.server.logs {
                    let color = if line.stderr {
                        Some(egui::Color32::LIGHT_RED)
                    } else if line.text.contains("WARN") || line.text.contains("Warning") {
                        Some(egui::Color32::YELLOW)
                    } else {
                        None
                    };
                    let mut text = egui::RichText::new(&line.text).monospace();
                    if let Some(color) = color {
                        text = text.color(color);
                    }
                    ui.label(text);
                }
            });

        if self.server.is_running() {
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
    }

    fn server_url(&self) -> String {
        let config = self.config.config();
        let port = config.port.trim();
        let port = if port.parse::<u16>().is_ok() {
            port
        } else {
            "80"
        };
        let interface = config.interface.trim();
        let host = if interface.is_empty() || interface == "0.0.0.0" || interface == "::" {
            "localhost"
        } else {
            interface
        };
        format!("http://{host}:{port}/")
    }
}

fn status_indicator(ui: &mut egui::Ui, status: &Status) {
    let (color, label) = match status {
        Status::Stopped => (egui::Color32::GRAY, "Stopped".to_string()),
        Status::Running { pid } => (
            egui::Color32::from_rgb(0x4c, 0xaf, 0x50),
            format!("Running (pid {pid})"),
        ),
        Status::Stopping { .. } => (egui::Color32::YELLOW, "Stopping...".to_string()),
        Status::Exited { code: Some(0) } => (egui::Color32::GRAY, "Stopped (exit 0)".to_string()),
        Status::Exited { code: Some(c) } => {
            (egui::Color32::LIGHT_RED, format!("Exited (code {c})"))
        }
        Status::Exited { code: None } => (egui::Color32::LIGHT_RED, "Exited (killed)".to_string()),
        Status::Error(e) => (egui::Color32::LIGHT_RED, format!("Error: {e}")),
    };
    ui.colored_label(color, "\u{25cf}");
    ui.label(label);
}

fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../packaging/icons/256x256.png"
    )))
    .expect("embedded window icon is a valid PNG")
}

fn main() -> eframe::Result {
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([820.0, 620.0])
            .with_min_inner_size([560.0, 420.0])
            .with_icon(window_icon()),
        ..Default::default()
    };

    eframe::run_native("PAWS", native, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
