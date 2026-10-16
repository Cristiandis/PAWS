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
        }
    }

    fn server_ui(&mut self, ui: &mut egui::Ui) {
        self.server.poll();

        ui.horizontal(|ui| {
            let stopping = matches!(self.server.status, Status::Stopping { .. });
            if stopping {
                ui.add_enabled(false, egui::Button::new("Stopping..."));
            } else if self.server.is_running() {
                if ui.button("Stop").clicked() {
                    self.server.request_stop();
                }
            } else if ui.button("Start").clicked() {
                self.server.start();
            }
            ui.label(status_label(&self.server.status));
        });

        ui.separator();

        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for line in &self.server.logs {
                    let text = egui::RichText::new(&line.text);
                    ui.label(if line.stderr {
                        text.color(egui::Color32::LIGHT_RED)
                    } else {
                        text
                    });
                }
            });

        if self.server.is_running() {
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
    }
}

fn status_label(status: &Status) -> String {
    match status {
        Status::Stopped => "Stopped".into(),
        Status::Running { pid } => format!("Running (pid {pid})"),
        Status::Stopping { .. } => "Stopping...".into(),
        Status::Exited { code: Some(0) } => "Stopped (exit 0)".into(),
        Status::Exited { code: Some(c) } => format!("Exited (code {c})"),
        Status::Exited { code: None } => "Exited (killed)".into(),
        Status::Error(e) => format!("Error: {e}"),
    }
}

fn main() -> eframe::Result {
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Webserver GUI",
        native,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
