use std::fs;
use std::path::PathBuf;

use eframe::egui;
use paws::Config;

enum SaveStatus {
    None,
    Saved,
    ReadError(String),
    SerializeError(String),
    WriteError(String),
    OpenFolderError(String),
}

pub struct ConfigPanel {
    config: Config,
    saved_toml: String,
    config_path: PathBuf,
    status: SaveStatus,
}

impl ConfigPanel {
    pub fn new() -> Self {
        let config_path = Config::config_path();
        let (config, status) = match Config::from_file(&config_path) {
            Ok(c) => (c, SaveStatus::None),
            Err(e) => (Config::default(), SaveStatus::ReadError(format!("{e}"))),
        };
        let saved_toml = toml::to_string(&config).unwrap_or_default();
        Self {
            config,
            saved_toml,
            config_path,
            status,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    fn is_dirty(&self) -> bool {
        toml::to_string(&self.config).unwrap_or_default() != self.saved_toml
    }

    fn validation_error(&self) -> Option<&'static str> {
        match self.config.port.trim().parse::<u16>() {
            Ok(p) if p > 0 => {}
            _ => return Some("err-port"),
        }
        if self.config.interface.trim().is_empty() {
            return Some("err-interface-empty");
        }
        if self.config.document_root.trim().is_empty() {
            return Some("err-docroot-required");
        }
        if self.config.threads < 1 {
            return Some("err-threads");
        }
        None
    }

    fn save(&mut self) {
        let serialized = match toml::to_string(&self.config) {
            Ok(s) => s,
            Err(e) => {
                self.status = SaveStatus::SerializeError(format!("{e}"));
                return;
            }
        };
        if let Some(parent) = self.config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match fs::write(&self.config_path, &serialized) {
            Ok(()) => {
                self.saved_toml = serialized;
                self.status = SaveStatus::Saved;
            }
            Err(e) => self.status = SaveStatus::WriteError(format!("{e}")),
        }
    }

    fn open_config_folder(&mut self) {
        let dir = self
            .config_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        if let Err(e) = open::that(&dir) {
            self.status = SaveStatus::OpenFolderError(format!("{e}"));
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let error = self.validation_error();
        let dirty = self.is_dirty();

        ui.heading(egui_i18n::tr!("cfg-title"));
        ui.add_space(4.0);

        field(ui, &egui_i18n::tr!("cfg-interface"), |ui| {
            ui.add_sized(
                [ui.available_width(), 20.0],
                egui::TextEdit::singleline(&mut self.config.interface),
            );
        });

        field(ui, &egui_i18n::tr!("cfg-port"), |ui| {
            ui.add_sized(
                [ui.available_width(), 20.0],
                egui::TextEdit::singleline(&mut self.config.port),
            );
        });

        field(ui, &egui_i18n::tr!("cfg-threads"), |ui| {
            ui.add_sized(
                [ui.available_width(), 20.0],
                egui::DragValue::new(&mut self.config.threads).range(1..=256),
            );
        });

        field(ui, &egui_i18n::tr!("cfg-docroot"), |ui| {
            let browse_width = 90.0;
            let field_width = (ui.available_width() - browse_width - 6.0).max(80.0);
            ui.add_sized(
                [field_width, 20.0],
                egui::TextEdit::singleline(&mut self.config.document_root),
            );
            if ui.button(egui_i18n::tr!("cfg-browse")).clicked() {
                let start = PathBuf::from(self.config.document_root.trim());
                let mut dialog = rfd::FileDialog::new();
                if start.is_dir() {
                    dialog = dialog.set_directory(&start);
                }
                if let Some(folder) = dialog.pick_folder() {
                    self.config.document_root = folder.to_string_lossy().into_owned();
                }
            }
        });

        ui.add_space(4.0);
        ui.checkbox(&mut self.config.php, egui_i18n::tr!("cfg-php"));
        ui.small(egui_i18n::tr!("cfg-restart-hint"));

        ui.add_space(8.0);

        if let Some(key) = error {
            ui.colored_label(egui::Color32::LIGHT_RED, egui_i18n::tr!(key));
        }

        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    dirty && error.is_none(),
                    egui::Button::new(egui_i18n::tr!("cfg-save")),
                )
                .clicked()
            {
                self.save();
            }
            if dirty {
                ui.label(egui::RichText::new(egui_i18n::tr!("cfg-unsaved")).italics());
            }
        });

        match &self.status {
            SaveStatus::None => {}
            SaveStatus::Saved => {
                ui.colored_label(egui::Color32::LIGHT_GREEN, egui_i18n::tr!("cfg-saved"));
            }
            SaveStatus::ReadError(e) => {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    egui_i18n::tr!("err-read-config", { error: e }),
                );
            }
            SaveStatus::SerializeError(e) => {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    egui_i18n::tr!("err-serialize", { error: e }),
                );
            }
            SaveStatus::WriteError(e) => {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    egui_i18n::tr!("err-write-file", { error: e }),
                );
            }
            SaveStatus::OpenFolderError(e) => {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    egui_i18n::tr!("err-open-config-folder", { error: e }),
                );
            }
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui_i18n::tr!("cfg-file-label"));
            ui.add(
                egui::Label::new(
                    egui::RichText::new(self.config_path.display().to_string()).monospace(),
                )
                .truncate(),
            )
            .on_hover_text(self.config_path.display().to_string());
            if ui.button(egui_i18n::tr!("cfg-open-folder")).clicked() {
                self.open_config_folder();
            }
        });
    }
}

fn field(ui: &mut egui::Ui, label: &str, contents: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(label);
        contents(ui);
    });
}
