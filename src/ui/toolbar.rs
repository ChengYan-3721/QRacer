// 顶部工具栏：粘贴 / 打开 / 导出 / 复制 + 码类型显示。

use crate::app::QRacerApp;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut QRacerApp, ctx: &egui::Context) {
    ui.horizontal_wrapped(|ui| {
        if ui.button("粘贴 (Ctrl+V)").clicked() {
            app.try_paste_from_clipboard();
        }
        if ui.button("打开...").clicked() {
            app.try_open_file();
        }
        if ui.button("截屏").clicked() {
            app.try_capture_screen(ctx);
        }

        ui.separator();

        if ui
            .add_enabled(app.last_svg.is_some(), egui::Button::new("导出 SVG"))
            .clicked()
        {
            app.try_export_svg();
        }
        if ui
            .add_enabled(app.can_copy_vector(), egui::Button::new("复制到剪贴板"))
            .clicked()
        {
            app.try_copy_vector();
        }

        ui.separator();

        ui.label("码类型：");
        ui.label(
            egui::RichText::new(app.code_kind.label())
                .strong()
                .color(egui::Color32::from_rgb(60, 130, 220)),
        );

        ui.separator();

        if ui.button("支持").clicked() {
            app.open_support();
        }
    });
}
