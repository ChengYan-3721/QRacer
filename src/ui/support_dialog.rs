use crate::app::LoadedImage;
use eframe::egui;

const GITHUB_URL: &str = "https://github.com/ChengYan-3721/QRacer";
const GITEE_URL: &str = "https://gitee.com/bilibili3721/qracer";
const AUTHOR_EMAIL: &str = "389004101@qq.com";
const SUPPORT_WINDOW_WIDTH: f32 = 380.0;
const SUPPORT_CONTENT_WIDTH: f32 = 350.0;
const QR_PREVIEW_SIZE: f32 = 136.0;
const QR_LARGE_SIZE: f32 = 380.0;
const LINK_ROW_WIDTH: f32 = 206.0;
const CLOSE_BUTTON_WIDTH: f32 = 48.0;

pub struct SupportDialog {
    open: bool,
    center_on_next_show: bool,
    wechat: Option<LoadedImage>,
    alipay: Option<LoadedImage>,
    enlarged_code: Option<SupportCode>,
    center_enlarged_on_next_show: bool,
}

#[derive(Clone, Copy)]
enum SupportCode {
    Wechat,
    Alipay,
}

impl SupportDialog {
    pub fn new() -> Self {
        Self {
            open: false,
            center_on_next_show: false,
            wechat: load_support_image(include_bytes!("../../wechat.png"), "support-wechat"),
            alipay: load_support_image(include_bytes!("../../alipay.jpg"), "support-alipay"),
            enlarged_code: None,
            center_enlarged_on_next_show: false,
        }
    }

    pub fn open(&mut self) {
        self.open = true;
        self.center_on_next_show = true;
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if self.open {
            self.show_support_window(ctx);
        }
        self.show_enlarged_code(ctx);
    }

    fn show_support_window(&mut self, ctx: &egui::Context) {
        let mut open = self.open;
        let mut close_requested = false;

        let mut window = egui::Window::new("支持")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(SUPPORT_WINDOW_WIDTH);
        if self.center_on_next_show {
            window = window.anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
        }

        window.show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.set_max_width(SUPPORT_CONTENT_WIDTH);

                centered_label(
                    ui,
                    egui::RichText::new("本软件完全开源免费！严禁倒卖！")
                        .strong()
                        .color(egui::Color32::from_rgb(210, 50, 45)),
                );
                ui.add_space(8.0);

                centered_row(ui, LINK_ROW_WIDTH, |ui| {
                    ui.label("开源地址：");
                    ui.hyperlink_to("GitHub", GITHUB_URL);
                    ui.separator();
                    ui.hyperlink_to("Gitee", GITEE_URL);
                });
                centered_row(ui, LINK_ROW_WIDTH, |ui| {
                    ui.label("联系作者：");
                    ui.hyperlink_to(AUTHOR_EMAIL, format!("mailto:{AUTHOR_EMAIL}"));
                });

                ui.add_space(8.0);
                ui.label("若本软件对您有所帮助，欢迎分享！");
                ui.label("开发不易，但我坚持开源免费，");
                ui.label("打赏可以让我感到您的认可，");
                ui.label("并且可以激励我做出更好的作品。");

                if self.wechat.is_some() || self.alipay.is_some() {
                    ui.add_space(12.0);
                    ui.columns(2, |columns| {
                        if support_image(&mut columns[0], ctx, "微信", self.wechat.as_mut()) {
                            self.enlarged_code = Some(SupportCode::Wechat);
                            self.center_enlarged_on_next_show = true;
                        }
                        if support_image(&mut columns[1], ctx, "支付宝", self.alipay.as_mut()) {
                            self.enlarged_code = Some(SupportCode::Alipay);
                            self.center_enlarged_on_next_show = true;
                        }
                    });
                }

                ui.add_space(12.0);
                centered_row(ui, CLOSE_BUTTON_WIDTH, |ui| {
                    if ui.button("关闭").clicked() {
                        close_requested = true;
                    }
                });
            });
        });
        self.center_on_next_show = false;

        if close_requested {
            open = false;
        }
        self.open = open;
    }

    fn show_enlarged_code(&mut self, ctx: &egui::Context) {
        let Some(code) = self.enlarged_code else {
            return;
        };

        let mut open = true;
        let title = match code {
            SupportCode::Wechat => "微信收款码",
            SupportCode::Alipay => "支付宝收款码",
        };

        let mut window = egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(QR_LARGE_SIZE + 48.0);
        if self.center_enlarged_on_next_show {
            window = window.anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
        }

        window.show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                image_box(
                    ui,
                    ctx,
                    support_code_image_mut(code, &mut self.wechat, &mut self.alipay),
                    egui::vec2(QR_LARGE_SIZE, QR_LARGE_SIZE),
                    false,
                );
            });
        });
        self.center_enlarged_on_next_show = false;

        if !open {
            self.enlarged_code = None;
            self.center_enlarged_on_next_show = false;
        }
    }
}

fn load_support_image(bytes: &[u8], texture_name: &str) -> Option<LoadedImage> {
    let image = image::load_from_memory(bytes).ok()?;
    Some(LoadedImage::from_dynamic(texture_name, image))
}

fn support_code_image_mut<'a>(
    code: SupportCode,
    wechat: &'a mut Option<LoadedImage>,
    alipay: &'a mut Option<LoadedImage>,
) -> Option<&'a mut LoadedImage> {
    match code {
        SupportCode::Wechat => wechat.as_mut(),
        SupportCode::Alipay => alipay.as_mut(),
    }
}

fn centered_label(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 0.0),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.add(
                egui::Label::new(text)
                    .halign(egui::Align::Center)
                    .wrap()
                    .selectable(false)
                    .sense(egui::Sense::hover()),
            );
        },
    );
}

fn centered_row<R>(
    ui: &mut egui::Ui,
    estimated_content_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        let space = (ui.available_width() - estimated_content_width).max(0.0) * 0.5;
        ui.add_space(space);
        add_contents(ui)
    })
    .inner
}

fn support_image(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    label: &str,
    image: Option<&mut LoadedImage>,
) -> bool {
    let mut clicked = false;

    ui.vertical_centered(|ui| {
        ui.label(label);
        let response = image_box(
            ui,
            ctx,
            image,
            egui::vec2(QR_PREVIEW_SIZE, QR_PREVIEW_SIZE),
            true,
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("点击放大");
        clicked = response.clicked();
    });

    clicked
}

fn image_box(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    image: Option<&mut LoadedImage>,
    max_size: egui::Vec2,
    clickable: bool,
) -> egui::Response {
    let sense = if clickable {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(max_size, sense);

    match image {
        Some(image) => {
            let texture = image.texture(ctx);
            let source_size = texture.size_vec2();
            let scale = (max_size.x / source_size.x).min(max_size.y / source_size.y);
            let display_size = source_size * scale;
            let image_rect = egui::Align2::CENTER_CENTER.align_size_within_rect(display_size, rect);
            ui.put(
                image_rect,
                egui::Image::from_texture(texture).fit_to_exact_size(display_size),
            );
        }
        None => {
            ui.put(
                rect,
                egui::Label::new(egui::RichText::new("未找到图片").color(egui::Color32::DARK_GRAY)),
            );
        }
    }

    response
}
