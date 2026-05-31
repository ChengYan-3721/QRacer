use crate::app::QRacerApp;
use crate::code_kind::CodeKind;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut QRacerApp) {
    if app.code_kind != CodeKind::WxMiniprogram {
        return;
    }

    ui.horizontal_wrapped(|ui| {
        ui.label("小程序码：径向采样");
        if ui.button("重新采样").clicked() {
            app.resample_wx();
        }

        let mut show_diff = app.show_diff_overlay;
        if ui.checkbox(&mut show_diff, "显示差异").changed() {
            app.set_show_diff_overlay(show_diff);
        }

        ui.separator();

        let diff_text = app
            .last_diff_count
            .map(|count| format!("差异：{count} 像素"))
            .unwrap_or_else(|| "差异：-".to_owned());
        ui.label(diff_text);
    });
}
