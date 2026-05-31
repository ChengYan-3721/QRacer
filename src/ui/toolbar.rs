// 顶部工具栏：粘贴 / 打开 / 导出 / 复制 + 码类型显示。
//
// 阶段 1：粘贴和打开真实工作；导出/复制按钮渲染但禁用。

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
        ui.add_enabled(false, egui::Button::new("复制矢量"));

        ui.separator();

        ui.label("码类型：");
        ui.label(
            egui::RichText::new(app.code_kind.label())
                .strong()
                .color(egui::Color32::from_rgb(60, 130, 220)),
        );
    });
}
