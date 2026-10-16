use std::fs;

use eframe::egui;
use paws::Config;

pub struct ConfigPanel {
    config: Config,
    status: String,
    config_path: std::path::PathBuf,
}

impl ConfigPanel {
    pub fn new() -> Self {
        let config_path = Config::config_path();
        let (config, status) = match Config::from_file(&config_path) {
            Ok(c) => (c, String::new()),
            Err(e) => (
                Config::default(),
                format!("WARN: Could not read config file, using defaults: {e}"),
            ),
        };
        Self {
            config,
            status,
            config_path,
        }
    }

    fn save(&mut self) {
        self.status = match toml::to_string(&self.config) {
            Ok(s) => match fs::write(&self.config_path, s) {
                Ok(()) => "Salvato".into(),
                Err(e) => format!("Error writing to file: {e}"),
            },
            Err(e) => format!("Error serializing config: {e}"),
        };
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.heading("PAWS config");
            ui.horizontal(|ui| {
                ui.label("Interface: ");
                ui.text_edit_singleline(&mut self.config.interface);
            });
            ui.horizontal(|ui| {
                ui.label("Port: ");
                ui.text_edit_singleline(&mut self.config.port);
            });
            ui.horizontal(|ui| {
                ui.label("DocumentRoot: ");
                ui.text_edit_singleline(&mut self.config.document_root);
            });
            ui.checkbox(&mut self.config.php, "PHP (bundled php-fpm)");

            if ui.button("Save").clicked() {
                self.save();
            }
            if !self.status.is_empty() {
                ui.label(&self.status);
            }
        });
    }
}
