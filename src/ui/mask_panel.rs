use crate::app::{MaskChoice, QRacerApp};
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut QRacerApp) {
    if app.warped.is_none() || app.qr_version.is_none() {
        return;
    }

    ui.horizontal_wrapped(|ui| {
        let has_decode = app.last_decoded.is_some();
        ui.label("掩膜：");

        for mask in 0..=7 {
            let selected = app.mask_choice == MaskChoice::Mask(mask);
            if ui
                .add_enabled(
                    has_decode,
                    egui::RadioButton::new(selected, mask.to_string()),
                )
                .clicked()
            {
                app.set_mask(mask);
            }
        }

        ui.separator();

        if ui
            .add_enabled(has_decode, egui::Button::new("自动选最佳"))
            .clicked()
        {
            app.auto_select_best_mask();
        }

        if ui
            .button("网格像素匹配")
            .on_hover_text("直接按校正图模块采样")
            .clicked()
        {
            app.use_grid_fallback();
        }

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

        if let Some(decoded) = app.last_decoded.as_ref() {
            let mask_text = app
                .matched_mask
                .map(|mask| format!("原掩膜 {mask}"))
                .unwrap_or_else(|| "无匹配掩膜".to_owned());
            ui.label(format!(
                "V{} / ECC {} / {}",
                decoded.version,
                decoded.ecc.label(),
                mask_text
            ));
        } else if let Some(version) = app.qr_version {
            ui.label(format!("V{version} / 未解码 / 可网格兜底"));
        }
    });
}
