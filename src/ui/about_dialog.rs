use crate::app::LoadedImage;
use eframe::egui;

const GITHUB_URL: &str = "https://github.com/ChengYan-3721/QRacer";
const GITEE_URL: &str = "https://gitee.com/bilibili3721/qracer";
const AUTHOR_EMAIL: &str = "389004101@qq.com";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_DATE: &str = env!("QRACER_BUILD_DATE");
const ABOUT_WINDOW_WIDTH: f32 = 460.0;
const ABOUT_CONTENT_WIDTH: f32 = 440.0;
const ABOUT_LEFT_COLUMN_WIDTH: f32 = ABOUT_CONTENT_WIDTH / 3.0;
const ABOUT_RIGHT_COLUMN_WIDTH: f32 = ABOUT_CONTENT_WIDTH - ABOUT_LEFT_COLUMN_WIDTH;
const ABOUT_BODY_HEIGHT: f32 = 370.0;
const LOGO_SIZE: f32 = 96.0;
const LOGO_BLOCK_HEIGHT: f32 = 330.0;
const QR_PREVIEW_SIZE: f32 = 136.0;
const QR_LARGE_SIZE: f32 = 380.0;
const LINK_ROW_WIDTH: f32 = 206.0;

pub struct AboutDialog {
    open: bool,
    center_on_next_show: bool,
    wechat: Option<LoadedImage>,
    alipay: Option<LoadedImage>,
    enlarged_code: Option<AboutCode>,
    center_enlarged_on_next_show: bool,
}

#[derive(Clone, Copy)]
enum AboutCode {
    Wechat,
    Alipay,
}

impl AboutDialog {
    pub fn new() -> Self {
        Self {
            open: false,
            center_on_next_show: false,
            wechat: load_about_image(include_bytes!("../../assets/wechat.png"), "about-wechat"),
            alipay: load_about_image(include_bytes!("../../assets/alipay.jpg"), "about-alipay"),
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
            self.show_about_window(ctx);
        }
        self.show_enlarged_code(ctx);
    }

    fn show_about_window(&mut self, ctx: &egui::Context) {
        let mut open = self.open;
        let close_requested = false;

        let mut window = egui::Window::new("关于")
            .id(egui::Id::new("about-dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(ABOUT_WINDOW_WIDTH)
            .min_width(ABOUT_WINDOW_WIDTH)
            .max_width(ABOUT_WINDOW_WIDTH);
        if self.center_on_next_show {
            window = window.anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
        }

        window.show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.set_width(ABOUT_CONTENT_WIDTH);

                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ABOUT_LEFT_COLUMN_WIDTH, ABOUT_BODY_HEIGHT),
                            egui::Layout::top_down(egui::Align::Center),
                            about_logo_column,
                        );
                        ui.allocate_ui_with_layout(
                            egui::vec2(ABOUT_RIGHT_COLUMN_WIDTH, ABOUT_BODY_HEIGHT),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                about_text_content(ui);
                                if let Some(code) =
                                    about_payment_codes(ui, ctx, &mut self.wechat, &mut self.alipay)
                                {
                                    self.enlarged_code = Some(code);
                                    self.center_enlarged_on_next_show = true;
                                }
                            },
                        );
                    });
                });

                ui.add_space(12.0);
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
            AboutCode::Wechat => "微信收款码",
            AboutCode::Alipay => "支付宝收款码",
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
                    about_code_image_mut(code, &mut self.wechat, &mut self.alipay),
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

fn load_about_image(bytes: &[u8], texture_name: &str) -> Option<LoadedImage> {
    let image = image::load_from_memory(bytes).ok()?;
    Some(LoadedImage::from_dynamic(texture_name, image))
}

fn about_code_image_mut<'a>(
    code: AboutCode,
    wechat: &'a mut Option<LoadedImage>,
    alipay: &'a mut Option<LoadedImage>,
) -> Option<&'a mut LoadedImage> {
    match code {
        AboutCode::Wechat => wechat.as_mut(),
        AboutCode::Alipay => alipay.as_mut(),
    }
}

fn about_logo_column(ui: &mut egui::Ui) {
    ui.add_space(((ABOUT_BODY_HEIGHT - LOGO_BLOCK_HEIGHT) * 0.5).max(0.0));
    ui.vertical_centered(|ui| {
        ui.add(
            egui::Image::new(egui::include_image!("../../assets/logo.svg"))
                .fit_to_exact_size(egui::vec2(LOGO_SIZE, LOGO_SIZE)),
        );
        ui.add_space(6.0);
        ui.label(egui::RichText::new("QRacer 摹码"));
        ui.label(egui::RichText::new(format!("版本：{APP_VERSION}")).small());
        ui.label(egui::RichText::new(format!("构建日期：{BUILD_DATE}")).small());
    });
}

fn about_text_content(ui: &mut egui::Ui) {
    ui.label(
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
    centered_label(
        ui,
        "若本软件对您有所帮助，欢迎分享！\n开发不易，但我坚持开源免费，\n打赏可以让我感到您的认可，\n并且可以激励我做出更好的作品。",
    );
}

fn about_payment_codes(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    wechat: &mut Option<LoadedImage>,
    alipay: &mut Option<LoadedImage>,
) -> Option<AboutCode> {
    if wechat.is_none() && alipay.is_none() {
        return None;
    }

    let mut selected = None;
    ui.add_space(12.0);
    centered_row(ui, ABOUT_RIGHT_COLUMN_WIDTH * 0.8, |ui| {
        ui.columns(2, |columns| {
            if about_image(&mut columns[0], ctx, "微信", wechat.as_mut()) {
                selected = Some(AboutCode::Wechat);
            }
            if about_image(&mut columns[1], ctx, "支付宝", alipay.as_mut()) {
                selected = Some(AboutCode::Alipay);
            }
        });
    });
    selected
}

fn centered_label(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 0.0),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.add(egui::Label::new(text).halign(egui::Align::LEFT).wrap());
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

fn about_image(
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
