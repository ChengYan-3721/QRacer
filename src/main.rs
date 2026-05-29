// QRacer 入口：启动 eframe，把窗口控制交给 QRacerApp。
//
// egui 是 immediate-mode GUI：没有"控件树"或"信号槽"，每帧重新执行
// QRacerApp::update() 把整个 UI 描述一遍。状态全部存在 QRacerApp 结构体里。

mod app;
mod code_kind;
mod detect;
mod error;
mod image_io;
mod pipeline;
mod ui;

use app::QRacerApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("QRacer"),
        ..Default::default()
    };

    eframe::run_native(
        "QRacer",
        native_options,
        Box::new(|cc| Ok(Box::new(QRacerApp::new(cc)))),
    )
}
