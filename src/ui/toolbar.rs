// 顶部工具栏：粘贴 / 打开 / 导出 / 复制 + 码类型显示。

use crate::app::QRacerApp;
use crate::code_kind::CodeKind;
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
        let selected_text = app
            .code_kind_override
            .map(|kind| format!("手动：{}", kind.label()))
            .unwrap_or_else(|| format!("自动：{}", app.code_kind.label()));
        egui::ComboBox::from_id_salt("code-kind-select")
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(app.code_kind_override.is_none(), "自动识别")
                    .clicked()
                {
                    app.set_code_kind_override(None);
                    ui.close();
                }
                for kind in CodeKind::PROCESSABLE {
                    if ui
                        .selectable_label(app.code_kind_override == Some(kind), kind.label())
                        .clicked()
                    {
                        app.set_code_kind_override(Some(kind));
                        ui.close();
                    }
                }
            });

        ui.separator();

        if ui.button("关于").clicked() {
            app.open_about();
        }
    });
}
