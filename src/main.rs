#![windows_subsystem = "windows"]

mod app;
mod code_kind;
mod codec;
mod detect;
mod error;
mod image_io;
mod pipeline;
mod screen_capture;
mod ui;
mod vector;

use app::QRacerApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([860.0, 520.0])
            .with_min_inner_size([860.0, 520.0])
            .with_title("QRacer"),
        ..Default::default()
    };

    eframe::run_native(
        "QRacer",
        native_options,
        Box::new(|cc| Ok(Box::new(QRacerApp::new(cc)))),
    )
}
