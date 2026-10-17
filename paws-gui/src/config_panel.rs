use std::fs;
use std::path::PathBuf;

use eframe::egui;
use paws::Config;

#[derive(Default)]
enum SaveStatus {
    #[default]
    None,
    Saved,
    Error(String),
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
        let config = match Config::from_file(&config_path) {
            Ok(c) => c,
            Err(e) => {
                let mut panel = Self {
                    config: Config::default(),
                    saved_toml: String::new(),
                    config_path,
                    status: SaveStatus::Error(format!(
                        "Could not read config file, using defaults: {e}"
                    )),
                };
                panel.saved_toml = toml::to_string(&panel.config).unwrap_or_default();
                return panel;
            }
        };
        let saved_toml = toml::to_string(&config).unwrap_or_default();
        Self {
            config,
            saved_toml,
            config_path,
            status: SaveStatus::None,
        }
    }

    fn is_dirty(&self) -> bool {
        toml::to_string(&self.config).unwrap_or_default() != self.saved_toml
    }

    fn validation_error(&self) -> Option<String> {
        match self.config.port.trim().parse::<u16>() {
            Ok(p) if p > 0 => {}
            _ => return Some("Port must be a number between 1 and 65535".into()),
        }
        if self.config.interface.trim().is_empty() {
            return Some("Interface must not be empty".into());
        }
        if self.config.document_root.trim().is_empty() {
            return Some("Document root must not be empty".into());
        }
        None
    }

    fn save(&mut self) {
        let serialized = match toml::to_string(&self.config) {
            Ok(s) => s,
            Err(e) => {
                self.status = SaveStatus::Error(format!("Error serializing config: {e}"));
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
            Err(e) => self.status = SaveStatus::Error(format!("Error writing to file: {e}")),
        }
    }

    fn open_config_folder(&mut self) {
        let dir = self
            .config_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        if let Err(e) = open::that(&dir) {
            self.status = SaveStatus::Error(format!("Could not open {}: {e}", dir.display()));
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let error = self.validation_error();
        let dirty = self.is_dirty();

        ui.heading("PAWS config");
        ui.add_space(4.0);

        field(ui, "Interface", |ui| {
            ui.add_sized(
                [ui.available_width(), 20.0],
                egui::TextEdit::singleline(&mut self.config.interface),
            );
        });

        field(ui, "Port", |ui| {
            ui.add_sized(
                [ui.available_width(), 20.0],
                egui::TextEdit::singleline(&mut self.config.port),
            );
        });

        field(ui, "Document root", |ui| {
            let browse_width = 70.0;
            let field_width = (ui.available_width() - browse_width - 6.0).max(80.0);
            ui.add_sized(
                [field_width, 20.0],
                egui::TextEdit::singleline(&mut self.config.document_root),
            );
            if ui.button("Browse...").clicked() {
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
        ui.checkbox(&mut self.config.php, "PHP (bundled php-fpm/php-cgi)");
        ui.small("Restart the server to apply changes.");

        ui.add_space(8.0);

        if let Some(err) = &error {
            ui.colored_label(egui::Color32::LIGHT_RED, err);
        }

        ui.horizontal(|ui| {
            if ui
                .add_enabled(dirty && error.is_none(), egui::Button::new("Save"))
                .clicked()
            {
                self.save();
            }
            if dirty {
                ui.label(egui::RichText::new("unsaved changes").italics());
            }
        });

        match &self.status {
            SaveStatus::None => {}
            SaveStatus::Saved => {
                ui.colored_label(egui::Color32::LIGHT_GREEN, "Saved");
            }
            SaveStatus::Error(e) => {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Config file:");
            ui.add(
                egui::Label::new(
                    egui::RichText::new(self.config_path.display().to_string()).monospace(),
                )
                .truncate(),
            )
            .on_hover_text(self.config_path.display().to_string());
            if ui.button("Open folder").clicked() {
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
