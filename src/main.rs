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
            .with_title("QRacer")
            .with_icon(load_app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "QRacer",
        native_options,
        Box::new(|cc| Ok(Box::new(QRacerApp::new(cc)))),
    )
}

fn load_app_icon() -> egui::IconData {
    let icon =
        image::load_from_memory_with_format(include_bytes!("../logo.ico"), image::ImageFormat::Ico)
            .expect("embedded logo.ico must be a valid ICO file")
            .into_rgba8();
    let (width, height) = icon.dimensions();

    egui::IconData {
        rgba: icon.into_raw(),
        width,
        height,
    }
}
