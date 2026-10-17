#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::time::Duration;

mod config_panel;
mod process;

use config_panel::ConfigPanel;
use process::{ServerProcess, Status};

const LANGUAGES: [(&str, &str); 2] = [("en-US", "English"), ("it-IT", "Italiano")];
const DEFAULT_LANG: &str = "en-US";
const LANG_STORAGE_KEY: &str = "paws-gui-language";

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
    lang: &'static str,
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, LANG_STORAGE_KEY, &self.lang.to_string());
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Server, egui_i18n::tr!("tab-server"));
            ui.selectable_value(&mut self.tab, Tab::Config, egui_i18n::tr!("tab-config"));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut selected = self.lang;
                egui::ComboBox::from_id_salt("language")
                    .selected_text(language_name(selected))
                    .show_ui(ui, |ui| {
                        for (code, name) in LANGUAGES {
                            ui.selectable_value(&mut selected, code, name);
                        }
                    });
                if selected != self.lang {
                    self.lang = selected;
                    egui_i18n::set_language(selected);
                }
                ui.label(egui_i18n::tr!("cfg-lang"));
            });
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
        let lang = cc
            .storage
            .and_then(|storage| eframe::get_value::<String>(storage, LANG_STORAGE_KEY))
            .and_then(|code| LANGUAGES.iter().find(|(c, _)| *c == code).map(|(c, _)| *c))
            .unwrap_or_else(current_language);
        egui_i18n::set_language(lang);

        Self {
            tab: Tab::Server,
            server: ServerProcess::new(cc.egui_ctx.clone()),
            config: ConfigPanel::new(),
            autoscroll: true,
            lang,
        }
    }

    fn server_ui(&mut self, ui: &mut egui::Ui) {
        self.server.poll();

        ui.horizontal(|ui| {
            let stopping = matches!(self.server.status, Status::Stopping { .. });
            if self.server.is_running() {
                if ui
                    .add_enabled(!stopping, egui::Button::new(egui_i18n::tr!("btn-stop")))
                    .clicked()
                {
                    self.server.request_stop();
                }
                if ui.button(egui_i18n::tr!("btn-restart")).clicked() {
                    self.server.request_restart();
                }
            } else if ui.button(egui_i18n::tr!("btn-start")).clicked() {
                self.server.start();
            }

            ui.separator();
            status_indicator(ui, &self.server.status);
        });

        ui.add_space(2.0);

        ui.horizontal(|ui| {
            if ui.button(egui_i18n::tr!("btn-open-browser")).clicked() {
                let url = self.server_url();
                if let Err(e) = open::that(&url) {
                    self.server.note(egui_i18n::tr!("err-could-not-open", {
                        path: &url,
                        error: &e.to_string()
                    }));
                }
            }
            if ui.button(egui_i18n::tr!("btn-open-docroot")).clicked() {
                let root = self.config.config().document_root.trim().to_string();
                if root.is_empty() {
                    self.server.note(egui_i18n::tr!("err-docroot-empty"));
                } else if let Err(e) = std::fs::create_dir_all(&root) {
                    self.server.note(egui_i18n::tr!("err-could-not-create", {
                        path: &root,
                        error: &e.to_string()
                    }));
                } else if let Err(e) = open::that(&root) {
                    self.server.note(egui_i18n::tr!("err-could-not-open", {
                        path: &root,
                        error: &e.to_string()
                    }));
                }
            }
            if ui.button(egui_i18n::tr!("btn-clear-logs")).clicked() {
                self.server.logs.clear();
            }
            if ui.button(egui_i18n::tr!("btn-copy-logs")).clicked() {
                let text = self
                    .server
                    .logs
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                ui.ctx().copy_text(text);
            }
            ui.checkbox(&mut self.autoscroll, egui_i18n::tr!("chk-autoscroll"));
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
            "127.0.0.1"
        } else {
            interface
        };
        format!("http://{host}:{port}/")
    }
}

fn language_name(code: &str) -> &'static str {
    LANGUAGES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| *name)
        .unwrap_or(DEFAULT_LANG)
}

fn current_language() -> &'static str {
    let current = egui_i18n::get_language();
    LANGUAGES
        .iter()
        .find(|(c, _)| *c == current)
        .map(|(c, _)| *c)
        .unwrap_or(DEFAULT_LANG)
}

fn init_i18n() {
    egui_i18n::load_translations_from_text("en-US", include_str!("../i18n/en-US.egl"))
        .expect("en-US translations are valid");
    egui_i18n::load_translations_from_text("it-IT", include_str!("../i18n/it-IT.egl"))
        .expect("it-IT translations are valid");
    egui_i18n::set_fallback("en-US");
    egui_i18n::set_language(&detect_language());
}

fn detect_language() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(var) {
            let value = value.to_ascii_lowercase();
            if value.starts_with("it") {
                return "it-IT".into();
            }
            if value.starts_with("en") {
                return "en-US".into();
            }
        }
    }
    DEFAULT_LANG.into()
}

fn status_indicator(ui: &mut egui::Ui, status: &Status) {
    let (color, label) = match status {
        Status::Stopped => (egui::Color32::GRAY, egui_i18n::tr!("status-stopped")),
        Status::Running { pid } => (
            egui::Color32::from_rgb(0x4c, 0xaf, 0x50),
            egui_i18n::tr!("status-running", { pid: pid }),
        ),
        Status::Stopping { .. } => (egui::Color32::YELLOW, egui_i18n::tr!("status-stopping")),
        Status::Exited { code: Some(0) } => {
            (egui::Color32::GRAY, egui_i18n::tr!("status-exited-ok"))
        }
        Status::Exited { code: Some(c) } => (
            egui::Color32::LIGHT_RED,
            egui_i18n::tr!("status-exited-code", { code: c }),
        ),
        Status::Exited { code: None } => (
            egui::Color32::LIGHT_RED,
            egui_i18n::tr!("status-exited-killed"),
        ),
        Status::Error(e) => (
            egui::Color32::LIGHT_RED,
            egui_i18n::tr!("status-error", { error: e }),
        ),
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
    init_i18n();

    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([820.0, 620.0])
            .with_min_inner_size([560.0, 420.0])
            .with_icon(window_icon()),
        ..Default::default()
    };

    eframe::run_native("PAWS", native, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
