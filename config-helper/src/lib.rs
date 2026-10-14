use std::fs;

use eframe::egui;
use webserver::Config;

const CONFIG_PATH: &str = "conf.toml";

pub fn run() -> eframe::Result {
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([480.0, 320.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Webserver config",
        native,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

struct App {
    config: Config,
    status: String,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (config, status) = match Config::from_file(CONFIG_PATH) {
            Ok(c) => (c, String::new()),
            Err(e) => (
                Config::default(),
                format!("WARN: Could not read config file, using defaults: {e}"),
            ),
        };
        Self { config, status }
    }

    fn save(&mut self) {
        if let Some(value) = &self.config.php_fpm {
            if value.is_empty() {
                self.config.php_fpm = None;
            }
        }

        self.status = match toml::to_string(&self.config) {
            Ok(s) => match fs::write(CONFIG_PATH, s) {
                Ok(()) => "Salvato".into(),
                Err(e) => format!("Error writing to file: {e}"),
            },
            Err(e) => format!("Error serializing config: {e}"),
        };
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.vertical_centered(|ui| {
            ui.heading("Webserver GUI Config");
            ui.horizontal(|ui| {
                ui.label("Interface: ");
                ui.text_edit_singleline(&mut self.config.interface);
            });
            ui.horizontal(|ui| {
                ui.label("Port: ");
                ui.text_edit_singleline(&mut self.config.port);
            });
            match &mut self.config.php_fpm {
                None => {
                    let mut option = "";
                    ui.horizontal(|ui| {
                        ui.label("Php-fpm interface: ");
                        ui.text_edit_singleline(&mut option);
                    });
                    self.config.php_fpm = Some(option.into())
                }
                Some(text) => {
                    ui.horizontal(|ui| {
                        ui.label("Php-fpm interface: ");
                        ui.text_edit_singleline(text);
                    });
                }
            }
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
