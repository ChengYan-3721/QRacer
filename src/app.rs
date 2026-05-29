// QRacerApp：应用状态 + egui 主循环回调。
//
// immediate-mode 的核心思想：
//   - 每一帧（约 60fps，空闲时按需）调用 update()
//   - 在 update() 里描述"这一帧的界面长什么样"
//   - 按钮的 .clicked() 返回 bool，用 if 立即处理
//   - 状态变化（粘贴图、选文件）只改 self 的字段，下一帧 UI 自动跟着变

use crate::code_kind::CodeKind;
use crate::codec::qr::{QrDecoded, QrMatrix, decode_qr, regenerate_qr};
use crate::codec::qr_grid::{infer_qr_version, sample_qr_grid};
use crate::detect;
use crate::detect::finder_qr::{QrFinder, find_qr_finders, select_qr_finder_triplet};
use crate::image_io;
use crate::pipeline::perspective::warp_qr_to_square;
use crate::pipeline::preprocess::{BinaryImage, preprocess};
use crate::ui;
use crate::vector::diff::{compute_diff, render_qr_diff_preview};
use crate::vector::svg::qr_matrix_to_svg;
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
    /// Stage 3 selected QR mask regeneration mode.
    pub mask_choice: MaskChoice,
    /// Stage 3 last successful QR decode result.
    pub last_decoded: Option<QrDecoded>,
    /// Last inferred QR version, available even when payload decoding fails.
    pub qr_version: Option<u8>,
    /// Stage 3 last regenerated QR matrix.
    pub last_matrix: Option<QrMatrix>,
    /// Stage 3 last generated SVG payload.
    pub last_svg: Option<String>,
    /// Stage 3 module difference count against the corrected QR image.
    pub last_diff_count: Option<u32>,
    /// Controls whether red/blue module differences are drawn in the preview.
    pub show_diff_overlay: bool,
    paste_shortcut_was_down: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskChoice {
    Mask(u8),
    GridFallback,
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
            mask_choice: MaskChoice::Mask(0),
            last_decoded: None,
            qr_version: None,
            last_matrix: None,
            last_svg: None,
            last_diff_count: None,
            show_diff_overlay: true,
            paste_shortcut_was_down: false,
        }
    }

    /// 把图像装载为原图 + 占位预览。阶段 2+ 会触发识别管线。
    pub fn set_original(&mut self, img: DynamicImage) {
        let binary = preprocess(&img);
        let code_kind = detect::detect_kind(&binary);

        self.original = Some(LoadedImage::from_dynamic("original", img.clone()));
        self.preview = None;
        self.code_kind = code_kind;
        self.binary = Some(binary.clone());
        self.finders = None;
        self.warped = None;
        self.mask_choice = MaskChoice::Mask(0);
        self.last_decoded = None;
        self.qr_version = None;
        self.last_matrix = None;
        self.last_svg = None;
        self.last_diff_count = None;

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
        self.warped = Some(warped.clone());
        self.finders = Some(finders);
        self.qr_version = infer_qr_version(&warped).ok();

        match decode_qr(&img, Some(&warped)) {
            Ok(decoded) => {
                let mask = decoded.original_mask.unwrap_or(0);
                self.qr_version = Some(decoded.version);
                self.mask_choice = MaskChoice::Mask(mask);
                self.last_decoded = Some(decoded);
                self.apply_current_mask();
            }
            Err(error) => {
                self.status = if let Some(version) = self.qr_version {
                    format!(
                        "已完成 QR 校正并推断版本 V{version}，但解码失败：{error}；可使用网格兜底"
                    )
                } else {
                    format!("已完成 QR 校正，但解码失败：{error}")
                };
            }
        }
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

    pub fn set_mask(&mut self, mask: u8) {
        self.mask_choice = MaskChoice::Mask(mask.min(7));
        self.apply_current_mask();
    }

    pub fn auto_select_best_mask(&mut self) {
        let Some(decoded) = self.last_decoded.clone() else {
            self.status = String::from("还没有可用的 QR 解码结果");
            return;
        };
        let Some(warped) = self.warped.as_ref() else {
            self.status = String::from("还没有可用于对比的校正图");
            return;
        };

        let mut best: Option<(u32, u8, QrMatrix, String)> = None;
        for mask in 0..=7 {
            let Ok(matrix) = regenerate_qr(&decoded, mask) else {
                continue;
            };
            let diff = compute_diff(warped, &matrix).diff_count;
            let svg = qr_matrix_to_svg(&matrix, 1.0);
            if best
                .as_ref()
                .is_none_or(|(best_diff, _, _, _)| diff < *best_diff)
            {
                best = Some((diff, mask, matrix, svg));
            }
        }

        let Some((_, mask, matrix, svg)) = best else {
            self.status = String::from("自动选择掩膜失败：无法重生成 QR");
            return;
        };

        self.mask_choice = MaskChoice::Mask(mask);
        self.set_generated_artifacts(matrix, svg);
    }

    pub fn use_grid_fallback(&mut self) {
        let Some(warped) = self.warped.as_ref() else {
            self.status = String::from("还没有可用于网格兜底的校正图");
            return;
        };

        let version = match self.qr_version.or_else(|| infer_qr_version(warped).ok()) {
            Some(version) => version,
            None => {
                self.status = String::from("网格兜底失败：无法推断 QR 版本");
                return;
            }
        };

        match sample_qr_grid(warped, version) {
            Ok(matrix) => {
                self.qr_version = Some(version);
                self.mask_choice = MaskChoice::GridFallback;
                let svg = qr_matrix_to_svg(&matrix, 1.0);
                self.set_generated_artifacts(matrix, svg);
            }
            Err(error) => {
                self.status = format!("网格兜底失败：{error}");
            }
        }
    }

    pub fn set_show_diff_overlay(&mut self, show: bool) {
        if self.show_diff_overlay == show {
            return;
        }
        self.show_diff_overlay = show;

        if self.last_matrix.is_some() {
            self.refresh_generated_preview();
        }
    }

    pub fn try_export_svg(&mut self) {
        let Some(svg) = self.last_svg.as_ref() else {
            self.status = String::from("没有可导出的 SVG");
            return;
        };

        let Some(path) = rfd::FileDialog::new()
            .add_filter("SVG", &["svg"])
            .set_file_name("qracer.svg")
            .save_file()
        else {
            return;
        };
        let path = if path.extension().is_some() {
            path
        } else {
            path.with_extension("svg")
        };

        match std::fs::write(&path, svg) {
            Ok(()) => self.status = format!("已导出 SVG：{}", path.display()),
            Err(error) => self.status = format!("导出 SVG 失败：{error}"),
        }
    }

    fn apply_current_mask(&mut self) {
        let MaskChoice::Mask(mask) = self.mask_choice else {
            self.status = String::from("网格兜底将在阶段 4 接入");
            return;
        };
        let Some(decoded) = self.last_decoded.clone() else {
            self.status = String::from("掩膜重生成需要先解码 QR；可使用网格兜底");
            return;
        };

        match regenerate_qr(&decoded, mask) {
            Ok(matrix) => {
                let svg = qr_matrix_to_svg(&matrix, 1.0);
                self.set_generated_artifacts(matrix, svg);
            }
            Err(error) => {
                self.last_matrix = None;
                self.last_svg = None;
                self.last_diff_count = None;
                self.status = format!("QR 重生成失败：{error}");
            }
        }
    }

    fn set_generated_artifacts(&mut self, matrix: QrMatrix, svg: String) {
        self.last_matrix = Some(matrix);
        self.last_svg = Some(svg);
        self.refresh_generated_preview();
    }

    fn refresh_generated_preview(&mut self) {
        let Some(matrix) = self.last_matrix.as_ref() else {
            return;
        };
        let diff = self
            .warped
            .as_ref()
            .map(|warped| compute_diff(warped, matrix));
        let diff_count = diff.as_ref().map(|diff| diff.diff_count).unwrap_or(0);
        let modules = matrix.len().max(1) as u32;
        let scale = (512_u32 / modules).max(2);
        let preview =
            render_qr_diff_preview(matrix, diff.as_ref(), self.show_diff_overlay, scale, 0);
        let preview_name = match self.mask_choice {
            MaskChoice::Mask(mask) => format!("preview-mask-{mask}"),
            MaskChoice::GridFallback => String::from("preview-grid-fallback"),
        };
        self.preview = Some(LoadedImage::from_dynamic(preview_name, preview));
        self.last_diff_count = Some(diff_count);

        let mode_text = match self.mask_choice {
            MaskChoice::Mask(mask) => self
                .last_decoded
                .as_ref()
                .map(|decoded| {
                    format!(
                        "已解码 QR：V{} / ECC {} / 掩膜 {}",
                        decoded.version,
                        decoded.ecc.label(),
                        mask
                    )
                })
                .unwrap_or_else(|| format!("QR 掩膜重生成：掩膜 {mask}")),
            MaskChoice::GridFallback => self
                .qr_version
                .map(|version| format!("使用网格兜底：V{version}"))
                .unwrap_or_else(|| String::from("使用网格兜底")),
        };
        self.status = format!(
            "{mode_text}，差异 {diff_count} 个模块（红色=原图有生成图没有，蓝色=原图没有生成图有）"
        );
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
            ui::mask_panel::show(ui_, self);
            ui_.separator();
            ui::compare_view::show(ui_, self, ctx);
        });
    }
}
