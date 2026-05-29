// QRacerApp：应用状态 + egui 主循环回调。
//
// immediate-mode 的核心思想：
//   - 每一帧（约 60fps，空闲时按需）调用 update()
//   - 在 update() 里描述"这一帧的界面长什么样"
//   - 按钮的 .clicked() 返回 bool，用 if 立即处理
//   - 状态变化（粘贴图、选文件）只改 self 的字段，下一帧 UI 自动跟着变

use crate::code_kind::CodeKind;
use crate::detect;
use crate::detect::finder_qr::{QrFinder, find_qr_finders, select_qr_finder_triplet};
use crate::image_io;
use crate::pipeline::perspective::warp_qr_to_square;
use crate::pipeline::preprocess::{BinaryImage, preprocess};
use crate::ui;
use eframe::egui;
use image::DynamicImage;

/// 一张加载好的图像 + 上传到 GPU 的纹理句柄。
///
/// texture 用 lazy 加载：首次显示时调用 ctx.load_texture()，缓存在这里。
/// 如果图像替换，把 texture 设回 None，下一帧会重新上传。
pub struct LoadedImage {
    #[allow(dead_code)] // 阶段 2+ 会被算法读取
    pub source: DynamicImage,
    pub texture: Option<egui::TextureHandle>,
    /// 给纹理的稳定名字（egui 用它做缓存键）
    name: String,
    /// 缓存一份 ColorImage，避免每次重新转换
    color: egui::ColorImage,
}

impl LoadedImage {
    pub fn from_dynamic(name: impl Into<String>, img: DynamicImage) -> Self {
        let color = image_io::to_color_image(&img);
        Self {
            source: img,
            texture: None,
            name: name.into(),
            color,
        }
    }

    pub fn from_binary(name: impl Into<String>, bin: &BinaryImage) -> Self {
        Self::from_dynamic(name, bin.to_dynamic_image())
    }

    /// 取出纹理用于显示。第一次调用会上传到 GPU。
    pub fn texture(&mut self, ctx: &egui::Context) -> &egui::TextureHandle {
        self.texture.get_or_insert_with(|| {
            ctx.load_texture(
                self.name.clone(),
                self.color.clone(),
                egui::TextureOptions::LINEAR,
            )
        })
    }
}

pub struct QRacerApp {
    /// 用户粘贴/打开的原图
    pub original: Option<LoadedImage>,
    /// 矢量化后的预览（阶段 1 暂时直接复制 original 作为占位）
    pub preview: Option<LoadedImage>,
    /// 当前识别到的码类型（阶段 1 始终 Unknown）
    pub code_kind: CodeKind,
    /// 给用户看的状态文字
    pub status: String,
    /// Stage 2 binary preprocessing result.
    pub binary: Option<BinaryImage>,
    /// Stage 2 QR finder candidates.
    pub finders: Option<Vec<QrFinder>>,
    /// Stage 2 perspective-corrected QR preview.
    pub warped: Option<BinaryImage>,
    paste_shortcut_was_down: bool,
}

impl QRacerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_cjk_font(&cc.egui_ctx);

        Self {
            original: None,
            preview: None,
            code_kind: CodeKind::Unknown,
            status: String::from("粘贴截图（Ctrl+V）或点击 [打开...] 开始"),
            binary: None,
            finders: None,
            warped: None,
            paste_shortcut_was_down: false,
        }
    }

    /// 把图像装载为原图 + 占位预览。阶段 2+ 会触发识别管线。
    pub fn set_original(&mut self, img: DynamicImage) {
        let binary = preprocess(&img);
        let code_kind = detect::detect_kind(&binary);

        self.original = Some(LoadedImage::from_dynamic("original", img));
        self.preview = None;
        self.code_kind = code_kind;
        self.binary = Some(binary.clone());
        self.finders = None;
        self.warped = None;

        if code_kind != CodeKind::Qr {
            self.status = String::from("图像已加载；未识别到支持的码类型");
            return;
        }

        let finders = find_qr_finders(&binary);

        let Some(selected) = select_qr_finder_triplet(&binary, &finders) else {
            self.status = format!(
                "已识别 QR，但无法从 {} 个候选中选出三角定位点",
                finders.len()
            );
            self.finders = Some(finders);
            return;
        };

        let warped = warp_qr_to_square(&binary, &selected, 512);
        self.preview = Some(LoadedImage::from_binary("preview", &warped));
        self.warped = Some(warped);
        self.finders = Some(finders);
        self.status = String::from("已识别 QR，并生成透视校正后的二值预览");
    }

    pub fn try_paste_from_clipboard(&mut self) {
        match image_io::read_clipboard_image() {
            Ok(img) => self.set_original(img),
            Err(e) => self.status = format!("粘贴失败：{e}"),
        }
    }

    pub fn try_open_file(&mut self) {
        match image_io::open_image_dialog() {
            Ok(Some(img)) => self.set_original(img),
            Ok(None) => {} // 用户取消，不改状态
            Err(e) => self.status = format!("打开失败：{e}"),
        }
    }
}

fn install_cjk_font(ctx: &egui::Context) {
    let Some((font_name, font_bytes)) = load_system_cjk_font() else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        font_name.clone(),
        egui::FontData::from_owned(font_bytes).into(),
    );

    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(fonts_for_family) = fonts.families.get_mut(&family) {
            fonts_for_family.insert(0, font_name.clone());
        }
    }

    ctx.set_fonts(fonts);
}

fn load_system_cjk_font() -> Option<(String, Vec<u8>)> {
    const CANDIDATES: [(&str, &str); 5] = [
        ("Microsoft YaHei", "C:\\Windows\\Fonts\\msyh.ttc"),
        ("Microsoft YaHei UI", "C:\\Windows\\Fonts\\msyhbd.ttc"),
        ("SimHei", "C:\\Windows\\Fonts\\simhei.ttf"),
        ("SimSun", "C:\\Windows\\Fonts\\simsun.ttc"),
        ("DengXian", "C:\\Windows\\Fonts\\Deng.ttf"),
    ];

    for (name, path) in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            return Some((name.to_owned(), bytes));
        }
    }

    None
}

fn paste_shortcut_down(ctx: &egui::Context) -> bool {
    egui_paste_shortcut_down(ctx) || platform_paste_shortcut_down(ctx)
}

fn egui_paste_shortcut_down(ctx: &egui::Context) -> bool {
    ctx.input(|input| {
        if !input.focused {
            return false;
        }

        let shortcut_down = ((input.modifiers.ctrl || input.modifiers.command)
            && input.key_down(egui::Key::V)
            && !input.modifiers.alt)
            || (input.modifiers.shift && input.key_down(egui::Key::Insert))
            || input.key_down(egui::Key::Paste);

        shortcut_down
            || input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Paste(_)))
    })
}

#[cfg(target_os = "windows")]
fn platform_paste_shortcut_down(ctx: &egui::Context) -> bool {
    if !ctx.input(|input| input.focused) {
        return false;
    }

    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_INSERT, VK_MENU, VK_SHIFT, VK_V,
    };

    fn key_down(key: VIRTUAL_KEY) -> bool {
        // GetAsyncKeyState only reads the current state of a Windows virtual key.
        let state = unsafe { GetAsyncKeyState(key as i32) };
        (state as u16 & 0x8000) != 0
    }

    let ctrl_v = key_down(VK_CONTROL) && key_down(VK_V) && !key_down(VK_MENU);
    let shift_insert = key_down(VK_SHIFT) && key_down(VK_INSERT);
    ctrl_v || shift_insert
}

#[cfg(not(target_os = "windows"))]
fn platform_paste_shortcut_down(_ctx: &egui::Context) -> bool {
    false
}

impl eframe::App for QRacerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 全局快捷键：Ctrl+V 粘贴
        let shortcut_is_down = paste_shortcut_down(ctx);
        let paste_pressed = shortcut_is_down && !self.paste_shortcut_was_down;
        self.paste_shortcut_was_down = shortcut_is_down;

        if paste_pressed {
            self.try_paste_from_clipboard();
        }
        // 注：try_paste_from_clipboard / try_open_file 都是 pub，
        // toolbar 里的按钮也会调用它们

        // 顶部工具栏
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui_| {
            ui::toolbar::show(ui_, self);
        });

        // 底部状态栏
        egui::TopBottomPanel::bottom("statusbar").show(ctx, |ui_| {
            ui_.horizontal(|ui_| {
                ui_.label(format!("状态：{}", self.status));
            });
        });

        // 中央：左右对比预览
        egui::CentralPanel::default().show(ctx, |ui_| {
            ui::compare_view::show(ui_, self, ctx);
        });
    }
}
