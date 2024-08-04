use std::path::PathBuf;

use gday_hole_punch::server_connector;

use crate::logic::connect_to_peer;

/// TODO: COMMENT
#[derive(Debug, Default)]
pub struct GdayApp {
    view: View,
    paths_to_send: Vec<PathBuf>,
    send_code: Option<String>,

    receive_code: String,
    receive_path: Option<PathBuf>,
    custom_server_used: bool,
    custom_server: ServerConfig,
}

#[derive(Debug)]
enum View {
    Home,
    SendConfig,
    SendConnecting,
    SendTransfer,
    ReceiveConfig,
    ReceiveConnecting,
    ReceiveTransfer,
}

impl Default for View {
    fn default() -> Self {
        Self::Home
    }
}

#[derive(Debug)]
struct ServerConfig {
    /// Use a custom gday server with this domain name.
    server: String,

    /// Connect to a custom server port.
    port: String,

    /// Use TLS
    encrypted: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: String::from(""),
            port: server_connector::DEFAULT_PORT.to_string(),
            encrypted: true,
        }
    }
}

impl GdayApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Default::default()
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        ui.heading("Gday");
        ui.label("A tool to directly send files.");

        ui.horizontal(|ui| {
            if ui.button("Send files").clicked() {
                self.view = View::SendConfig
            }
            if ui.button("Receive files").clicked() {
                self.view = View::ReceiveConfig
            }
        });
    }

    fn send_config(&mut self, ui: &mut egui::Ui) {}

    fn send_connecting(&mut self, ui: &mut egui::Ui) {}

    fn send_transfer(&mut self, ui: &mut egui::Ui) {}

    fn receive_config(&mut self, ui: &mut egui::Ui) {
        if ui.button("⏴").clicked() {
            self.view = View::Home
        }

        ui.label("To receive files, enter the code your mate gave you.");

        ui.text_edit_singleline(&mut self.receive_code);

        ui.label("Example: 1.1C30.C71E.A");


        ui.checkbox(&mut self.custom_server_used, "Use a custom server");

        if self.custom_server_used {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Server address (example: example.com)");
                    ui.text_edit_singleline(&mut self.custom_server.server);
                });
                ui.horizontal(|ui| {
                    ui.label("Server port (default: 2311)");
                    ui.text_edit_singleline(&mut self.custom_server.port);
                });
                ui.checkbox(&mut self.custom_server.encrypted, "Encrypt using TLS? (default: Yes)");
            });
        }

        ui.separator();

        if ui.button("Receive").clicked() {
            self.view = View::ReceiveConnecting;

            if self.custom_server_used {
                // TODO
            }

            todo!();
        }
    }

    fn receive_connecting(&mut self, ui: &mut egui::Ui) {}

    fn receive_transfer(&mut self, ui: &mut egui::Ui) {}
}

impl eframe::App for GdayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| match self.view {
            View::Home => self.home(ui),
            View::SendConfig => self.send_config(ui),
            View::SendConnecting => self.send_connecting(ui),
            View::SendTransfer => self.send_transfer(ui),
            View::ReceiveConfig => self.receive_config(ui),
            View::ReceiveConnecting => self.receive_connecting(ui),
            View::ReceiveTransfer => self.receive_transfer(ui),
        });
    }
}