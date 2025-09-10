use eframe::egui;
use gday_file_transfer::LocalFileOffer;
use gday_hole_punch::{
    PeerCode,
    server_connector::{DEFAULT_SERVERS, ServerStream},
};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Gday GUI",
        options,
        Box::new(|_cc| Ok(Box::new(AppState::default()))),
    )
}

struct AppState {
    view: AppView,
    rt: tokio::runtime::Runtime,
}

enum AppView {
    Welcome {
        message: String,
    },
    Send {
        file_offer: Vec<LocalFileOffer>,
        server_stream: ServerStream,
        peer_code: PeerCode,
    },
    Receive {
        entered_code: String,
    },
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            view: AppView::Welcome {
                message: String::new(),
            },
            rt: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Couldn't start tokio runtime"),
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| match &mut self.view {
            AppView::Welcome { message } => {
                ui.heading("Gday GUI");
                ui.hyperlink("https://github.com/manforowicz/gday");

                ui.horizontal(|ui| {
                    if ui.button("Send files").clicked() {
                        // if let Some(files) = rfd::FileDialog::new().pick_files() {
                        //     gday_hole_punch::server_connector::connect_to_random_server(DEFAULT_SERVERS)
                        // } else {
                        //     *message = "Failed to pick files".to_string()
                        // }
                    }

                    if ui.button("Receive files").clicked() {}
                });

                ui.label("Note: the Gday command line tool has more features (custom server, custom code, etc.)");
            },

            AppView::Send { file_offer, server_stream, peer_code } => todo!(),

            AppView::Receive { entered_code } => todo!(),
        });
    }
}
