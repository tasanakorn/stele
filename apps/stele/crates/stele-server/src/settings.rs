use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub server: ServerSettings,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerSettings {
    #[serde(default = "default_bind_ip")]
    pub bind_ip: String,
    #[serde(default)]
    pub auth_key: Option<String>,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            bind_ip: default_bind_ip(),
            auth_key: None,
        }
    }
}

fn default_bind_ip() -> String {
    "127.0.0.1".to_string()
}

pub fn settings_path(db_path: &str) -> PathBuf {
    Path::new(db_path)
        .parent()
        .unwrap_or(Path::new("."))
        .join("config.toml")
}

pub fn load_settings(path: &Path) -> Settings {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Settings::default();
    };
    // Parse as a generic table so unknown top-level keys (e.g. stele-cli's
    // `default_profile` / `[profiles.*]`) coexist without being lost.
    let table: toml::Table = toml::from_str(&content).unwrap_or_default();
    match table.get("server") {
        Some(v) => v
            .clone()
            .try_into::<ServerSettings>()
            .map(|server| Settings { server })
            .unwrap_or_default(),
        None => Settings::default(),
    }
}

pub fn save_settings(path: &Path, settings: &Settings) -> Result<(), Box<dyn std::error::Error>> {
    // Read the existing file as a generic Table so unknown top-level keys
    // written by other binaries (e.g. stele-cli's `default_profile` and
    // `[profiles.*]`) are preserved across round-trips. On macOS the CLI and
    // server share the same config.toml path via case-insensitive APFS; on
    // Linux the files are distinct and this merge is a harmless no-op.
    let mut merged: toml::Table = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();

    let server_value = toml::Value::try_from(&settings.server)?;
    merged.insert("server".to_string(), server_value);

    let content = toml::to_string_pretty(&merged)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

// ── egui settings dialog (desktop only) ──

#[cfg(feature = "desktop")]
pub fn run_settings_dialog(db_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = settings_path(db_path);
    let current = load_settings(&config_path);

    let (mode, custom_ip) = match current.server.bind_ip.as_str() {
        "127.0.0.1" => (BindMode::Localhost, String::new()),
        "0.0.0.0" => (BindMode::AllInterfaces, String::new()),
        other => (BindMode::Custom, other.to_string()),
    };

    let app = SettingsApp {
        config_path,
        mode,
        custom_ip,
        error: None,
        status: None,
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([340.0, 220.0])
            .with_resizable(false)
            .with_title("Stele Settings"),
        ..Default::default()
    };

    eframe::run_native("Stele Settings", options, Box::new(|_cc| Ok(Box::new(app))))?;
    Ok(())
}

#[cfg(feature = "desktop")]
#[derive(Debug, Clone, PartialEq)]
enum BindMode {
    Localhost,
    AllInterfaces,
    Custom,
}

#[cfg(feature = "desktop")]
struct SettingsApp {
    config_path: PathBuf,
    mode: BindMode,
    custom_ip: String,
    error: Option<String>,
    status: Option<String>,
}

#[cfg(feature = "desktop")]
impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Bind Address");
            ui.add_space(8.0);

            ui.radio_value(
                &mut self.mode,
                BindMode::Localhost,
                "127.0.0.1 (localhost only)",
            );
            ui.radio_value(
                &mut self.mode,
                BindMode::AllInterfaces,
                "0.0.0.0 (all interfaces)",
            );

            ui.horizontal(|ui| {
                ui.radio_value(&mut self.mode, BindMode::Custom, "Custom:");
                let response = ui.text_edit_singleline(&mut self.custom_ip);
                if response.changed() {
                    self.mode = BindMode::Custom;
                    self.error = None;
                    self.status = None;
                }
            });

            ui.add_space(8.0);

            if let Some(err) = &self.error {
                ui.colored_label(eframe::egui::Color32::RED, err);
            }
            if let Some(status) = &self.status {
                ui.colored_label(eframe::egui::Color32::GREEN, status);
            }

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.with_layout(
                    eframe::egui::Layout::right_to_left(eframe::egui::Align::Center),
                    |ui| {
                        if ui.button("Apply").clicked() {
                            self.apply();
                        }
                        if ui.button("Cancel").clicked() {
                            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
                        }
                    },
                );
            });
        });
    }
}

#[cfg(feature = "desktop")]
impl SettingsApp {
    fn apply(&mut self) {
        let ip = match &self.mode {
            BindMode::Localhost => "127.0.0.1".to_string(),
            BindMode::AllInterfaces => "0.0.0.0".to_string(),
            BindMode::Custom => self.custom_ip.trim().to_string(),
        };

        if ip.parse::<std::net::IpAddr>().is_err() {
            self.error = Some("Invalid IP address".to_string());
            self.status = None;
            return;
        }

        let existing = load_settings(&self.config_path);
        let settings = Settings {
            server: ServerSettings {
                bind_ip: ip,
                auth_key: existing.server.auth_key,
            },
        };

        match save_settings(&self.config_path, &settings) {
            Ok(()) => {
                self.error = None;
                self.status = Some("Applied \u{2713}".to_string());
            }
            Err(e) => {
                self.error = Some(format!("Failed to save: {e}"));
                self.status = None;
            }
        }
    }
}
