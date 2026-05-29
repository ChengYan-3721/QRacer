// 中央对比视图：左右两栏分别显示"原图"和"矢量预览"。
//
// 阶段 1：两侧都用原图（preview 字段在 set_original 时被赋值为同一张图的拷贝）。
// 阶段 3+ 后右侧会被真实生成的矢量光栅化结果替代。

use crate::app::{LoadedImage, QRacerApp};
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut QRacerApp, ctx: &egui::Context) {
    let available = ui.available_size();
    let pane_w = (available.x - 16.0) * 0.5;

    ui.horizontal_top(|ui| {
        ui.allocate_ui(egui::vec2(pane_w, available.y), |ui| {
            pane(ui, ctx, "原图", app.original.as_mut());
        });

        ui.separator();

        ui.allocate_ui(egui::vec2(pane_w, available.y), |ui| {
            pane(ui, ctx, "校正预览", app.preview.as_mut());
        });
    });
}

fn pane(ui: &mut egui::Ui, ctx: &egui::Context, title: &str, image: Option<&mut LoadedImage>) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(title).heading());
        ui.separator();

        match image {
            Some(img) => {
                let tex = img.texture(ctx);
                // 按可用空间等比缩放显示
                let size = tex.size_vec2();
                let max = ui.available_size();
                let scale = (max.x / size.x).min(max.y / size.y).min(1.0);
                let display = size * scale;
                ui.add(egui::Image::from_texture(tex).fit_to_exact_size(display));
            }
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("（无图像）").color(egui::Color32::DARK_GRAY));
                });
            }
        }
    });
}
