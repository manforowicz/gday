//! TODO: COMMENT
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod app;
mod logic;

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([200.0, 200.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Gday GUI",
        native_options,
        Box::new(|cc| Ok(Box::new(app::GdayApp::new(cc)))),
    )
}
