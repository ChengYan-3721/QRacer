use crate::app::QRacerApp;
use crate::code_kind::CodeKind;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut QRacerApp) {
    if app.code_kind != CodeKind::Douyin {
        return;
    }

    ui.horizontal_wrapped(|ui| {
        ui.label("抖音码：同心圆采样");
        if ui.button("重新采样").clicked() {
            app.resample_dy();
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

        if let Some(grid) = app.last_dy_grid.as_ref() {
            ui.label(format!(
                "{} 环 / 编码每环 {} 点 / {}",
                grid.ring_count(),
                grid.points_per_ring,
                if grid.has_border {
                    "黑框版"
                } else {
                    "无框版"
                }
            ));
        }
    });
}
