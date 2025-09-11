use std::sync::{Arc, Mutex};

use bytesize::ByteSize;
use eframe::egui;
use egui::{Context, RichText, Ui};
use gday_encryption::EncryptedStream;
use gday_file_transfer::{FileOfferMsg, LocalFileOffer, TransferReport};
use gday_hole_punch::{FullContact, PeerCode};
use helpers::MyHandle;
use log::error;
use tokio::net::TcpStream;

use crate::{
    helpers::{receive1, receive2, send1, send2},
    logger::Logger,
};

mod helpers;
mod logger;

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
    logger: Logger,
}

#[derive(Default)]
enum AppView {
    #[default]
    Home,
    Send1 {
        handle: MyHandle<anyhow::Result<AppView>>,
    },
    Send2 {
        offer: LocalFileOffer,
        peer_code: PeerCode,
        peer_contact_handle: MyHandle<Result<(FullContact, FullContact), gday_hole_punch::Error>>,
    },
    Send3 {
        handle: MyHandle<anyhow::Result<()>>,
        transfer_report: Arc<Mutex<TransferReport>>,
    },
    Send4,
    Receive1 {
        entered_code: String,
    },
    Receive2 {
        handle: MyHandle<anyhow::Result<AppView>>,
    },
    Receive3 {
        peer_conn: EncryptedStream<TcpStream>,
        offer: FileOfferMsg,
    },
    Receive4 {
        handle: MyHandle<anyhow::Result<()>>,
        transfer_report: Arc<Mutex<TransferReport>>,
    },
    Receive5,
    ErrorScreen {
        message: String,
    },
}

impl Default for AppState {
    fn default() -> Self {
        let logger = Logger::init();
        Self {
            view: AppView::Home,
            rt: tokio::runtime::Runtime::new().unwrap(),
            logger,
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if !matches!(self.view, AppView::Home) && ui.button("⮪ Home").clicked() {
                self.view = AppView::Home;
                ui.separator();
            }

            self.main_ui(ctx, ui);
            ui.separator();

            ui.label("Log:");
            ui.group(|ui| {
                let scroll = egui::ScrollArea::vertical().id_salt("Log");
                scroll.show(ui, |ui| {
                    ui.label(self.logger.get_log().as_str())
                        .scroll_to_me(Some(egui::Align::BOTTOM));
                })
            })
        });
    }
}

impl AppState {
    fn main_ui(&mut self, ctx: &Context, ui: &mut Ui) {
        match &mut self.view {
            AppView::Home => {
                ui.heading("Gday GUI");
                ui.hyperlink("https://github.com/manforowicz/gday");

                ui.horizontal(|ui| {
                    if ui.button("Send files").clicked()
                        && let Some(paths) = rfd::FileDialog::new()
                            .set_title("Choose files to send")
                            .pick_files()
                    {
                        let handle = MyHandle(self.rt.spawn(async move { send1(&paths).await }));
                        self.view = AppView::Send1 { handle };
                    }

                    if ui.button("Receive files").clicked() {
                        self.view = AppView::Receive1 {
                            entered_code: String::new(),
                        };
                    }
                });

                ui.label(
                    "Note: the Gday command line tool has more \
                    features than the GUI (custom server, custom code, etc.)",
                );
            }
            AppView::Send1 { handle } => {
                ui.label("Connecting to server...");
                if handle.is_finished() {
                    match self.rt.block_on(handle).expect("Tokio join failed") {
                        Ok(view) => {
                            self.view = view;
                        }
                        Err(err) => {
                            self.view = AppView::ErrorScreen {
                                message: err.to_string(),
                            };
                        }
                    }
                }
                ctx.request_repaint();
            }
            AppView::Send2 {
                offer,
                peer_code,
                peer_contact_handle,
            } => {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("Tell your peer to receive with code: ");
                    });

                    ui.group(|ui| {
                        ui.label(RichText::new(format!(" {peer_code} ")).strong().monospace());
                    });
                });

                ui.group(|ui| {
                    ui.label("Files to send:");
                    ui.group(|ui| {
                        egui::ScrollArea::both().show(ui, |ui| {
                            for (path, meta) in &offer.offer.offer {
                                ui.label(format!("{} ({})", path.display(), ByteSize(meta.size)));
                            }
                        });
                    });
                });

                if peer_contact_handle.is_finished() {
                    let (my_contact, peer_contact) =
                        match self.rt.block_on(peer_contact_handle).expect("Tokio error") {
                            Ok(good) => good,
                            Err(err) => {
                                self.view = AppView::ErrorScreen {
                                    message: err.to_string(),
                                };
                                return;
                            }
                        };

                    let transfer_report = Arc::new(Mutex::new(TransferReport::default()));

                    let handle: MyHandle<anyhow::Result<()>> = MyHandle(self.rt.spawn(send2(
                        my_contact,
                        peer_contact,
                        peer_code.shared_secret().to_string(),
                        offer.clone(),
                        transfer_report.clone(),
                    )));
                    self.view = AppView::Send3 {
                        handle,
                        transfer_report,
                    };
                }
            }
            AppView::Send3 {
                handle,
                transfer_report,
            } => {
                let pr = transfer_report.lock().unwrap();
                let percentage = pr.processed_bytes as f32 / pr.total_bytes as f32;
                ui.add(egui::ProgressBar::new(percentage).text(format!(
                    "Sending {} ({} / {})",
                    pr.current_file.display(),
                    ByteSize(pr.processed_bytes),
                    ByteSize(pr.total_bytes),
                )));
                drop(pr);

                if handle.is_finished() {
                    match self.rt.block_on(handle).expect("Tokio error") {
                        Ok(()) => self.view = AppView::Send4,
                        Err(err) => {
                            self.view = AppView::ErrorScreen {
                                message: err.to_string(),
                            };
                        }
                    }
                }
            }

            AppView::Send4 => {
                ui.label("Transfer complete.");
            }

            AppView::Receive1 { entered_code } => {
                ui.label(
                    "To receive files, ask your peer to give you a code (such as 1.qmecyr.26h9aw)",
                );

                let mut clicked = false;

                ui.horizontal(|ui| {
                    ui.label("Code: ");
                    ui.text_edit_singleline(entered_code);
                    if ui.button("Enter").clicked() {
                        clicked = true;
                    }
                });

                if clicked {
                    let peer_code = PeerCode::try_from(entered_code.as_str());
                    match peer_code {
                        Ok(code) => {
                            let handle = MyHandle(self.rt.spawn(receive1(code)));
                            self.view = AppView::Receive2 { handle };
                        }
                        Err(err) => {
                            error!("Couldn't parse code: {err}");
                        }
                    }
                }
            }

            AppView::Receive2 { handle } => {
                ui.label("Connecting...");
                if handle.is_finished() {
                    match self.rt.block_on(handle).expect("Tokio error") {
                        Ok(view) => self.view = view,

                        Err(err) => {
                            self.view = AppView::ErrorScreen {
                                message: err.to_string(),
                            }
                        }
                    }
                }
            }

            AppView::Receive3 {
                peer_conn: _,
                offer,
            } => {
                ui.label("Would you like to receive these files?");
                ui.group(|ui| {
                    egui::ScrollArea::both().show(ui, |ui| {
                        for (path, meta) in &offer.offer {
                            ui.label(format!("{} ({})", path.display(), ByteSize(meta.size)));
                        }
                    });
                });

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.view = AppView::Home;
                    }

                    if ui.button("Proceed").clicked()
                        && let Some(save_loc) = rfd::FileDialog::new()
                            .set_title("Choose files to send")
                            .pick_folder()
                    {
                        let AppView::Receive3 { peer_conn, offer } = std::mem::take(&mut self.view)
                        else {
                            unreachable!()
                        };

                        let transfer_report = Arc::new(Mutex::new(TransferReport::default()));
                        let handle = MyHandle(self.rt.spawn(receive2(
                            peer_conn,
                            offer,
                            save_loc,
                            transfer_report.clone(),
                        )));
                        self.view = AppView::Receive4 {
                            handle,
                            transfer_report,
                        }
                    }
                });
            }

            AppView::Receive4 {
                handle,
                transfer_report,
            } => {
                let pr = transfer_report.lock().unwrap();
                let percentage = pr.processed_bytes as f32 / pr.total_bytes as f32;
                ui.add(egui::ProgressBar::new(percentage).text(format!(
                    "Receiving {} ({} / {})",
                    pr.current_file.display(),
                    ByteSize(pr.processed_bytes),
                    ByteSize(pr.total_bytes),
                )));
                drop(pr);

                if handle.is_finished() {
                    match self.rt.block_on(handle).expect("Tokio error") {
                        Ok(()) => {
                            self.view = AppView::Receive5;
                        }
                        Err(err) => {
                            self.view = AppView::ErrorScreen {
                                message: err.to_string(),
                            }
                        }
                    }
                }
            }

            AppView::Receive5 => {
                ui.label("Transfer complete.");
            }

            AppView::ErrorScreen { message } => {
                ui.label("Error:");
                ui.label(message.as_str());
            }
        }
    }
}
