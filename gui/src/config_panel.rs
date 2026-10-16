use std::fs;

use eframe::egui;
use webserver::Config;

const CONFIG_PATH: &str = "conf.toml";

pub struct ConfigPanel {
    config: Config,
    php_fpm_text: String,
    status: String,
}

impl ConfigPanel {
    pub fn new() -> Self {
        let (config, status) = match Config::from_file(CONFIG_PATH) {
            Ok(c) => (c, String::new()),
            Err(e) => (
                Config::default(),
                format!("WARN: Could not read config file, using defaults: {e}"),
            ),
        };
        let php_fpm_text = config.php_fpm.clone().unwrap_or_default();
        Self {
            config,
            php_fpm_text,
            status,
        }
    }

    fn save(&mut self) {
        let php_fpm = self.php_fpm_text.trim();
        self.config.php_fpm = if php_fpm.is_empty() {
            None
        } else {
            Some(php_fpm.to_string())
        };

        self.status = match toml::to_string(&self.config) {
            Ok(s) => match fs::write(CONFIG_PATH, s) {
                Ok(()) => "Salvato".into(),
                Err(e) => format!("Error writing to file: {e}"),
            },
            Err(e) => format!("Error serializing config: {e}"),
        };
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.heading("Webserver config");
            ui.horizontal(|ui| {
                ui.label("Interface: ");
                ui.text_edit_singleline(&mut self.config.interface);
            });
            ui.horizontal(|ui| {
                ui.label("Port: ");
                ui.text_edit_singleline(&mut self.config.port);
            });
            ui.horizontal(|ui| {
                ui.label("Php-fpm interface: ");
                ui.text_edit_singleline(&mut self.php_fpm_text);
            });
            ui.horizontal(|ui| {
                ui.label("DocumentRoot: ");
                ui.text_edit_singleline(&mut self.config.document_root);
            });

            if ui.button("Save").clicked() {
                self.save();
            }
            if !self.status.is_empty() {
                ui.label(&self.status);
            }
        });
    }
}
