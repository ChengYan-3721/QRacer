use crate::app::QRacerApp;
use crate::code_kind::CodeKind;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut QRacerApp) {
    if app.code_kind != CodeKind::DataMatrix {
        return;
    }

    ui.horizontal_wrapped(|ui| {
        ui.label("Data Matrix码：矩形网格采样");
        let mut show_diff = app.show_diff_overlay;
        if ui.checkbox(&mut show_diff, "显示差异").changed() {
            app.set_show_diff_overlay(show_diff);
        }

        ui.separator();

        let diff_text = app
            .last_diff_count
            .map(|count| format!("差异：{count} 模块"))
            .unwrap_or_else(|| "差异：-".to_owned());
        ui.label(diff_text);

        if let Some(grid) = app.last_data_matrix_grid.as_ref() {
            ui.label(format!("{} x {} 模块", grid.cols, grid.rows));
        }
    });
}
