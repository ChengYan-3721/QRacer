// QRacerApp：应用状态 + egui 主循环回调。
//
// immediate-mode 的核心思想：
//   - 每一帧（约 60fps，空闲时按需）调用 update()
//   - 在 update() 里描述"这一帧的界面长什么样"
//   - 按钮的 .clicked() 返回 bool，用 if 立即处理
//   - 状态变化（粘贴图、选文件）只改 self 的字段，下一帧 UI 自动跟着变

use crate::code_kind::CodeKind;
use crate::codec::data_matrix_grid::{
    DATA_MATRIX_SYMBOLS, DataMatrixGrid, DataMatrixSymbol, sample_data_matrix_grid,
    sample_data_matrix_grid_for_symbols,
};
use crate::codec::dy_grid::{DyGrid, detect_dy_params, sample_dy, sample_dy_with_logos};
use crate::codec::qr::{QrDecoded, QrEcc, QrMatrix, decode_qr, regenerate_qr};
use crate::codec::qr_grid::{infer_qr_version, sample_qr_grid};
use crate::codec::wx_grid::{WxGrid, detect_wx_version, sample_wx, sample_wx_with_badge};
use crate::detect;
use crate::detect::finder_dm::{DataMatrixCandidate, find_data_matrix_candidates};
use crate::detect::finder_dy::{DyFinder, find_dy_finders, select_dy_finders_raw};
use crate::detect::finder_qr::{QrFinder, find_qr_finders, select_qr_finder_triplet};
use crate::detect::finder_wx::{
    WxFinder, find_wx_finders, select_wx_finders_raw, select_wx_finders_raw_with_badge,
};
use crate::image_io;
use crate::pipeline::perspective::{
    WxUprightAnchor, correct_dy_to_upright, detect_wx_badge_anchor, dy_upright_target_finders,
    warp_corners_to_image, warp_dy_to_upright_binary, warp_image_corners_to_square,
    warp_qr_to_square_image, warp_wx_to_upright_binary, warp_wx_to_upright_image,
    wx_upright_target_finders,
};
use crate::pipeline::preprocess::{BinaryImage, preprocess};
use crate::screen_capture;
use crate::ui;
use crate::vector::diff::{DiffResult, compute_matrix_diff};
use crate::vector::svg::{
    QrAppearance, data_matrix_grid_to_diff_preview_image, data_matrix_grid_to_preview_image,
    data_matrix_grid_to_svg, dy_grid_to_diff_preview_image, dy_grid_to_preview_image,
    dy_grid_to_svg, qr_matrix_to_preview_image, qr_matrix_to_svg, qr_matrix_to_svg_with_appearance,
    wx_grid_to_diff_preview_image, wx_grid_to_preview_image, wx_grid_to_svg,
};
use eframe::egui;
use image::{DynamicImage, GrayImage};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};
#[cfg(not(windows))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(windows)]
use windows_sys::Win32::Foundation::SYSTEMTIME;
#[cfg(windows)]
use windows_sys::Win32::System::SystemInformation::GetLocalTime;

const PREVIEW_SIZE: u32 = 1024;
const QR_GRID_REFERENCE_SNAP_RATIO: usize = 10;
const QR_LOGO_IGNORE_RATIO: usize = 3;
const QR_LOGO_IGNORE_MIN_MODULES: usize = 9;
const LOADING_REPAINT_INTERVAL: Duration = Duration::from_millis(50);
const SCREEN_CAPTURE_HIDE_DELAY: Duration = Duration::from_millis(180);
const DY_MANUAL_NO_BORDER_LOCATOR_DISTANCE: f64 = 240.529442688416;
const DY_MANUAL_NO_BORDER_LOCATOR_RADII: [f64; 3] = [8.13, 18.43, 29.01];
const DY_MANUAL_BLACK_BORDER_LOCATOR_DISTANCE: f64 = 261.452;
const DY_MANUAL_BLACK_BORDER_LOCATOR_RADII: [f64; 3] = [8.05, 18.24, 28.71];

/// 一张加载好的图像 + 上传到 GPU 的纹理句柄。
///
/// texture 用 lazy 加载：首次显示时调用 ctx.load_texture()，缓存在这里。
/// 如果图像替换，把 texture 设回 None，下一帧会重新上传。
pub struct LoadedImage {
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
    /// 手动指定码类型；None 表示继续使用自动识别结果。
    pub code_kind_override: Option<CodeKind>,
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
    /// Visual style used for QR SVG output and generated preview.
    pub qr_appearance: QrAppearance,
    /// Stage 3 last successful QR decode result.
    pub last_decoded: Option<QrDecoded>,
    /// Last inferred QR version, available even when payload decoding fails.
    pub qr_version: Option<u8>,
    /// Stage 3 last regenerated QR matrix.
    pub last_matrix: Option<QrMatrix>,
    /// Stage 3 QR matrix sampled from the corrected image for preview diffs.
    pub qr_reference_matrix: Option<QrMatrix>,
    /// QR mask that matches outside the center logo area.
    pub matched_mask: Option<u8>,
    /// Last sampled Data Matrix grid.
    pub last_data_matrix_grid: Option<DataMatrixGrid>,
    /// Stage 5 last sampled mini-program radial grid.
    pub last_wx_grid: Option<WxGrid>,
    /// Stage 6 last sampled Douyin radial grid.
    pub last_dy_grid: Option<DyGrid>,
    /// Stage 3 last generated SVG payload.
    pub last_svg: Option<String>,
    /// Stage 3 module difference count against the corrected QR reference matrix.
    pub last_diff_count: Option<u32>,
    /// Controls whether red/blue module differences are drawn in the preview.
    pub show_diff_overlay: bool,
    paste_shortcut_was_down: bool,
    processing_job: Option<ProcessingJob>,
    capture_job: Option<CaptureJob>,
    support_dialog: ui::support_dialog::SupportDialog,
    pub manual_calibration: ui::manual_calibration::ManualCalibrationState,
    next_job_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskChoice {
    Mask(u8),
    GridFallback,
}

struct ProcessingJob {
    receiver: Receiver<ProcessResult>,
    started_at: Instant,
}

struct CaptureJob {
    receiver: Receiver<anyhow::Result<DynamicImage>>,
    started_at: Instant,
}

struct ProcessResult {
    code_kind: CodeKind,
    status: String,
    binary: Option<BinaryImage>,
    finders: Option<Vec<QrFinder>>,
    warped: Option<BinaryImage>,
    mask_choice: MaskChoice,
    qr_appearance: QrAppearance,
    last_decoded: Option<QrDecoded>,
    qr_version: Option<u8>,
    last_matrix: Option<QrMatrix>,
    qr_reference_matrix: Option<QrMatrix>,
    matched_mask: Option<u8>,
    last_data_matrix_grid: Option<DataMatrixGrid>,
    last_wx_grid: Option<WxGrid>,
    last_dy_grid: Option<DyGrid>,
    last_svg: Option<String>,
    last_diff_count: Option<u32>,
    preview: Option<(String, DynamicImage)>,
}

impl QRacerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_cjk_font(&cc.egui_ctx);

        Self {
            original: None,
            preview: None,
            code_kind: CodeKind::Unknown,
            code_kind_override: None,
            status: String::from("粘贴截图（Ctrl+V）或点击 [打开...] 开始"),
            binary: None,
            finders: None,
            warped: None,
            mask_choice: MaskChoice::Mask(0),
            qr_appearance: QrAppearance::Standard,
            last_decoded: None,
            qr_version: None,
            last_matrix: None,
            qr_reference_matrix: None,
            matched_mask: None,
            last_data_matrix_grid: None,
            last_wx_grid: None,
            last_dy_grid: None,
            last_svg: None,
            last_diff_count: None,
            show_diff_overlay: true,
            paste_shortcut_was_down: false,
            processing_job: None,
            capture_job: None,
            support_dialog: ui::support_dialog::SupportDialog::new(),
            manual_calibration: ui::manual_calibration::ManualCalibrationState::new(),
            next_job_id: 0,
        }
    }

    pub fn open_support(&mut self) {
        self.support_dialog.open();
    }

    pub fn current_forced_or_detected_kind(&self) -> Option<CodeKind> {
        self.code_kind_override
            .or_else(|| self.code_kind.can_process().then_some(self.code_kind))
    }

    pub fn set_code_kind_override(&mut self, kind: Option<CodeKind>) {
        let kind = kind.filter(|kind| kind.can_process());
        if self.code_kind_override == kind {
            return;
        }

        self.code_kind_override = kind;
        if let Some(kind) = kind {
            self.code_kind = kind;
            if kind.can_manual_calibrate() {
                self.manual_calibration.set_kind(kind);
            }
        }

        if self.manual_calibration.open {
            return;
        }

        let Some(source) = self.original.as_ref().map(|loaded| loaded.source.clone()) else {
            self.status = match kind {
                Some(kind) => format!("已选择码类型：{}", kind.label()),
                None => String::from("已切回自动识别"),
            };
            return;
        };
        let label = if kind.is_some() {
            "图像（手动码类型）"
        } else {
            "图像（自动识别）"
        };
        self.begin_processing(source, label);
    }

    pub fn open_manual_calibration(&mut self) {
        let Some(original) = self.original.as_ref() else {
            self.status = String::from("没有可用于手动校准的原图");
            return;
        };

        let kind = self
            .current_forced_or_detected_kind()
            .filter(|kind| kind.can_manual_calibrate())
            .unwrap_or(CodeKind::Douyin);
        self.manual_calibration
            .open_for(kind, (original.source.width(), original.source.height()));
    }

    pub fn apply_manual_calibration(&mut self) {
        let kind = self.manual_calibration.kind;
        if !kind.can_manual_calibrate() {
            self.status = String::from("当前码类型不需要手动校准");
            return;
        }

        let Some(source) = self.original.as_ref().map(|loaded| loaded.source.clone()) else {
            self.status = String::from("没有可用于手动校准的原图");
            return;
        };

        let target_size = source
            .width()
            .max(source.height())
            .clamp(PREVIEW_SIZE, 1600);
        let destination_corners = self.manual_calibration.output_corners(target_size);
        let corrected_source =
            warp_image_corners_to_square(&source, &destination_corners, target_size);
        let dy_border_hint = self
            .last_dy_grid
            .as_ref()
            .is_some_and(|grid| grid.has_border);

        self.processing_job = None;
        self.code_kind_override = Some(kind);
        self.code_kind = kind;
        self.clear_recognition_artifacts();

        let result = match kind {
            CodeKind::WxMiniprogram => self.process_manual_wx(corrected_source),
            CodeKind::Douyin => self.process_manual_dy(corrected_source, dy_border_hint),
            _ => Err(String::from("当前码类型不需要手动校准")),
        };

        match result {
            Ok(()) => {
                self.manual_calibration.open = false;
            }
            Err(error) => {
                self.status = error;
            }
        }
    }

    /// 把图像装载为原图，并把识别/校正/矢量化放到后台线程。
    pub fn set_original(&mut self, img: DynamicImage) {
        self.begin_processing(img, "图像");
    }

    fn begin_processing(&mut self, img: DynamicImage, source_label: &str) {
        self.next_job_id = self.next_job_id.wrapping_add(1);
        let job_id = self.next_job_id;
        let show_diff_overlay = self.show_diff_overlay;
        let code_kind_override = self.code_kind_override;

        self.processing_job = None;
        self.manual_calibration.close_for_new_image();
        self.original = Some(LoadedImage::from_dynamic(
            format!("original-{job_id}"),
            img.clone(),
        ));
        self.preview = None;
        self.code_kind = CodeKind::Unknown;
        self.binary = None;
        self.finders = None;
        self.warped = None;
        self.mask_choice = MaskChoice::Mask(0);
        self.qr_appearance = QrAppearance::Standard;
        self.last_decoded = None;
        self.qr_version = None;
        self.last_matrix = None;
        self.qr_reference_matrix = None;
        self.matched_mask = None;
        self.last_data_matrix_grid = None;
        self.last_wx_grid = None;
        self.last_dy_grid = None;
        self.last_svg = None;
        self.last_diff_count = None;
        self.status = format!("{source_label}已载入，正在识别和校正...");

        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = process_image(img, job_id, show_diff_overlay, code_kind_override);
            let _ = sender.send(result);
        });
        self.processing_job = Some(ProcessingJob {
            receiver,
            started_at: Instant::now(),
        });
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

    pub fn try_capture_screen(&mut self, ctx: &egui::Context) {
        if self.capture_job.is_some() {
            self.status = String::from("截屏框选已经在进行中");
            return;
        }

        self.status = String::from("正在最小化窗口并启动截屏框选...");
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        let (sender, receiver) = mpsc::channel();
        let repaint_ctx = ctx.clone();
        thread::spawn(move || {
            thread::sleep(SCREEN_CAPTURE_HIDE_DELAY);
            let result = screen_capture::select_screen_region();
            screen_capture::restore_main_window();
            let _ = sender.send(result);
            repaint_ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            repaint_ctx.request_repaint();
        });
        self.capture_job = Some(CaptureJob {
            receiver,
            started_at: Instant::now(),
        });
        ctx.request_repaint_after(LOADING_REPAINT_INTERVAL);
    }

    pub fn loading_message(&self) -> Option<String> {
        if self.processing_job.is_some() {
            Some(String::from("正在识别、校正并生成预览..."))
        } else if self.capture_job.is_some() {
            Some(String::from("正在进行截屏框选..."))
        } else {
            None
        }
    }

    fn loading_progress(&self) -> Option<f32> {
        let started_at = self
            .processing_job
            .as_ref()
            .map(|job| job.started_at)
            .or_else(|| self.capture_job.as_ref().map(|job| job.started_at))?;
        Some((started_at.elapsed().as_secs_f32() * 0.35).fract())
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

        let Some(reference_matrix) = self.ensure_qr_reference_matrix() else {
            self.status = String::from("还没有可用于对比的校正采样矩阵");
            return;
        };

        let Some(candidate) = choose_matching_qr_mask(&decoded, &reference_matrix) else {
            self.matched_mask = None;
            self.use_grid_fallback();
            return;
        };

        self.mask_choice = MaskChoice::Mask(candidate.mask);
        self.matched_mask = Some(candidate.mask);
        let svg = self.qr_svg_for_matrix(&candidate.matrix);
        self.set_generated_artifacts(candidate.matrix, svg);
    }

    fn ensure_qr_reference_matrix(&mut self) -> Option<QrMatrix> {
        if let Some(matrix) = self.qr_reference_matrix.clone() {
            return Some(matrix);
        }

        let warped = self.warped.as_ref()?;
        let version = self.qr_version.or_else(|| infer_qr_version(warped).ok())?;
        let matrix = sample_qr_grid(warped, version).ok()?;
        self.qr_version = Some(version);
        self.qr_reference_matrix = Some(matrix.clone());
        Some(matrix)
    }

    pub fn use_grid_fallback(&mut self) {
        let Some(warped) = self.warped.as_ref() else {
            self.status = String::from("还没有可用于网格像素匹配的校正图");
            return;
        };

        let version = match self.qr_version.or_else(|| infer_qr_version(warped).ok()) {
            Some(version) => version,
            None => {
                self.status = String::from("网格像素匹配失败：无法推断 QR 版本");
                return;
            }
        };

        match sample_qr_grid(warped, version) {
            Ok(matrix) => {
                self.qr_reference_matrix = Some(matrix.clone());
                let matrix = self.stabilize_grid_matrix(matrix);
                self.qr_version = Some(version);
                self.mask_choice = MaskChoice::GridFallback;
                let svg = self.qr_svg_for_matrix(&matrix);
                self.set_generated_artifacts(matrix, svg);
            }
            Err(error) => {
                self.status = format!("网格像素匹配失败：{error}");
            }
        }
    }

    fn stabilize_grid_matrix(&self, matrix: QrMatrix) -> QrMatrix {
        let Some(decoded) = self.last_decoded.as_ref() else {
            return matrix;
        };
        if self.matched_mask.is_none() {
            return matrix;
        }
        let mask = decoded.original_mask.unwrap_or(0);
        let Ok(reference) = regenerate_qr(decoded, mask) else {
            return matrix;
        };
        let Some(diff) = qr_matrix_diff_count(&matrix, &reference) else {
            return matrix;
        };
        let modules = matrix.len().max(1);
        let max_snap_diff = (modules * modules / QR_GRID_REFERENCE_SNAP_RATIO).max(24);
        if diff <= max_snap_diff {
            reference
        } else {
            matrix
        }
    }

    #[allow(dead_code)]
    pub fn resample_wx(&mut self) {
        if self.code_kind != CodeKind::WxMiniprogram {
            self.status = String::from("当前图像不是小程序码");
            return;
        }

        let Some(binary) = self.binary.clone() else {
            self.status = String::from("还没有可用于采样的二值图");
            return;
        };

        let source = self.original.as_ref().map(|loaded| loaded.source.clone());
        self.process_wx(&binary, source.as_ref());
    }

    #[allow(dead_code)]
    pub fn resample_dy(&mut self) {
        if self.code_kind != CodeKind::Douyin {
            self.status = String::from("当前图像不是抖音码");
            return;
        }

        let Some(binary) = self.binary.clone() else {
            self.status = String::from("还没有可用于采样的二值图");
            return;
        };

        let source = self.original.as_ref().map(|loaded| loaded.source.clone());
        self.process_dy(&binary, source.as_ref());
    }

    pub fn set_show_diff_overlay(&mut self, show: bool) {
        if self.show_diff_overlay == show {
            return;
        }
        self.show_diff_overlay = show;

        if self.last_matrix.is_some() {
            self.refresh_generated_preview();
        } else if self.last_data_matrix_grid.is_some() {
            self.refresh_data_matrix_preview();
        } else if self.last_wx_grid.is_some() {
            self.refresh_wx_preview();
        } else if self.last_dy_grid.is_some() {
            self.refresh_dy_preview();
        }
    }

    pub fn try_export_svg(&mut self) {
        let Some(svg) = self.last_svg.as_ref() else {
            self.status = String::from("没有可导出的 SVG");
            return;
        };

        let Some(path) = rfd::FileDialog::new()
            .add_filter("SVG", &["svg"])
            .set_file_name(export_svg_file_name())
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
            Ok(()) => {
                self.status = format!(
                    "已导出 SVG：{}。本软件不承诺采样结果的正确性，请谨慎用于生产！",
                    path.display()
                )
            }
            Err(error) => self.status = format!("导出 SVG 失败：{error}"),
        }
    }

    pub fn can_copy_vector(&self) -> bool {
        self.last_svg.is_some()
    }

    pub fn try_copy_vector(&mut self) {
        let Some(svg) = self.last_svg.clone() else {
            self.status = String::from("没有可复制的 SVG");
            return;
        };

        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(svg)) {
            Ok(()) => {
                self.status = String::from(
                    "已复制 SVG 代码到剪贴板。本软件不承诺采样结果的正确性，请谨慎用于生产！",
                )
            }
            Err(error) => self.status = format!("复制 SVG 失败：{error}"),
        }
    }

    fn clear_recognition_artifacts(&mut self) {
        self.preview = None;
        self.binary = None;
        self.finders = None;
        self.warped = None;
        self.mask_choice = MaskChoice::Mask(0);
        self.qr_appearance = QrAppearance::Standard;
        self.last_decoded = None;
        self.qr_version = None;
        self.last_matrix = None;
        self.qr_reference_matrix = None;
        self.matched_mask = None;
        self.last_data_matrix_grid = None;
        self.last_wx_grid = None;
        self.last_dy_grid = None;
        self.last_svg = None;
        self.last_diff_count = None;
    }

    fn process_manual_wx(
        &mut self,
        corrected_source: DynamicImage,
    ) -> std::result::Result<(), String> {
        let corrected_binary = preprocess(&corrected_source);
        let selected = manual_wx_target_finders(corrected_binary.w);
        self.binary = Some(corrected_binary.clone());
        self.warped = Some(corrected_binary.clone());

        let preferred_version = detect_wx_version(&corrected_binary, &selected).ok();
        let mut best: Option<(u32, bool, WxGrid)> = None;
        let mut errors = Vec::new();
        for version in [36, 54, 72] {
            let grid = match sample_wx_with_badge(
                &corrected_binary,
                &corrected_source,
                &selected,
                version,
            ) {
                Ok(grid) => grid,
                Err(error) => {
                    errors.push(format!("{version} 线：{error}"));
                    continue;
                }
            };
            let (_, diff_count) =
                wx_grid_to_diff_preview_image(&grid, &corrected_binary, false, PREVIEW_SIZE);
            let preferred = preferred_version == Some(version);
            if best.as_ref().is_none_or(|(best_diff, best_preferred, _)| {
                diff_count < *best_diff
                    || (diff_count == *best_diff && preferred && !*best_preferred)
            }) {
                best = Some((diff_count, preferred, grid));
            }
        }

        let Some((_, _, grid)) = best else {
            return Err(if errors.is_empty() {
                String::from("手动校准小程序码失败：无可用候选")
            } else {
                format!("手动校准小程序码失败：{}", errors.join("；"))
            });
        };

        let svg = wx_grid_to_svg(&grid);
        self.set_wx_artifacts(grid, svg);
        Ok(())
    }

    fn process_manual_dy(
        &mut self,
        corrected_source: DynamicImage,
        border_hint: bool,
    ) -> std::result::Result<(), String> {
        let corrected_binary = preprocess(&corrected_source);
        let selected = manual_dy_target_finders(corrected_binary.w, border_hint);
        self.binary = Some(corrected_binary.clone());
        self.warped = Some(corrected_binary.clone());

        let params = detect_dy_params(&corrected_binary, &selected)
            .map_err(|error| format!("手动校准抖音码参数检测失败：{error}"))?;
        let grid = sample_dy_with_logos(&corrected_binary, &corrected_source, &selected, params)
            .map_err(|error| format!("手动校准抖音码环形采样失败：{error}"))?;
        let svg = dy_grid_to_svg(&grid);
        self.set_dy_artifacts(grid, svg);
        Ok(())
    }

    #[allow(dead_code)]
    fn process_wx(&mut self, binary: &BinaryImage, source: Option<&DynamicImage>) {
        let finders = find_wx_finders(binary);
        let badge_anchor = source.and_then(detect_wx_badge_anchor);
        let raw_selected = badge_anchor
            .and_then(|badge| select_wx_finders_raw_with_badge(&finders, badge))
            .or_else(|| select_wx_finders_raw(&finders));
        let Some(raw_selected) = raw_selected else {
            self.status = format!(
                "已识别小程序码，但无法从 {} 个候选中选出三牛眼定位点",
                finders.len()
            );
            return;
        };

        let correction_size = source
            .map(|source| source.width().max(source.height()))
            .unwrap_or_else(|| binary.w.max(binary.h))
            .clamp(PREVIEW_SIZE, 1600);
        let anchor = badge_anchor.map(WxUprightAnchor::Badge);
        let corrected_source = source
            .map(|source| warp_wx_to_upright_image(source, &raw_selected, anchor, correction_size));
        let corrected_binary = corrected_source
            .as_ref()
            .map(preprocess)
            .unwrap_or_else(|| {
                warp_wx_to_upright_binary(binary, &raw_selected, anchor, correction_size)
            });
        let selected = wx_upright_target_finders(&raw_selected, correction_size);
        self.warped = Some(corrected_binary.clone());

        let preferred_version = detect_wx_version(&corrected_binary, &selected).ok();
        let mut best: Option<(u32, bool, WxGrid)> = None;
        let mut errors = Vec::new();
        for version in [36, 54, 72] {
            let sampled = match corrected_source.as_ref() {
                Some(source) => sample_wx_with_badge(&corrected_binary, source, &selected, version),
                None => sample_wx(&corrected_binary, &selected, version),
            };
            let grid = match sampled {
                Ok(grid) => grid,
                Err(error) => {
                    errors.push(format!("{version} 线：{error}"));
                    continue;
                }
            };
            let (_, diff_count) =
                wx_grid_to_diff_preview_image(&grid, &corrected_binary, false, PREVIEW_SIZE);
            let preferred = preferred_version == Some(version);
            if best.as_ref().is_none_or(|(best_diff, best_preferred, _)| {
                diff_count < *best_diff
                    || (diff_count == *best_diff && preferred && !*best_preferred)
            }) {
                best = Some((diff_count, preferred, grid));
            }
        }

        match best {
            Some((_, _, grid)) => {
                let svg = wx_grid_to_svg(&grid);
                self.set_wx_artifacts(grid, svg);
            }
            None => {
                self.status = if errors.is_empty() {
                    String::from("小程序码径向采样失败：无可用候选")
                } else {
                    format!("小程序码径向采样失败：{}", errors.join("；"))
                };
            }
        }
    }

    #[allow(dead_code)]
    fn process_dy(&mut self, binary: &BinaryImage, source: Option<&DynamicImage>) {
        let finders = find_dy_finders(binary);
        let Some(raw_selected) = select_dy_finders_raw(&finders) else {
            self.status = format!(
                "已识别抖音码，但无法从 {} 个候选中选出三同心圆定位点",
                finders.len()
            );
            return;
        };

        let (corrected_source, corrected_binary, selected) = match source {
            Some(source) => {
                let corrected = correct_dy_to_upright(source, binary, &raw_selected);
                (Some(corrected.source), corrected.binary, corrected.finders)
            }
            None => {
                let correction_size = binary.w.max(binary.h).clamp(PREVIEW_SIZE, 1600);
                (
                    None,
                    warp_dy_to_upright_binary(binary, &raw_selected, correction_size),
                    dy_upright_target_finders(&raw_selected, correction_size),
                )
            }
        };
        self.warped = Some(corrected_binary.clone());

        let params = match detect_dy_params(&corrected_binary, &selected) {
            Ok(params) => params,
            Err(error) => {
                self.status = format!("抖音码参数检测失败：{error}");
                return;
            }
        };
        let sampled = match corrected_source.as_ref() {
            Some(source) => sample_dy_with_logos(&corrected_binary, source, &selected, params),
            None => sample_dy(&corrected_binary, &selected, params),
        };

        match sampled {
            Ok(grid) => {
                let svg = dy_grid_to_svg(&grid);
                self.set_dy_artifacts(grid, svg);
            }
            Err(error) => {
                self.status = format!("抖音码环形采样失败：{error}");
            }
        }
    }

    fn apply_current_mask(&mut self) {
        let MaskChoice::Mask(mask) = self.mask_choice else {
            self.status = String::from("网格像素匹配将在阶段 4 接入");
            return;
        };
        let Some(decoded) = self.last_decoded.clone() else {
            self.status = String::from("掩膜重生成需要先解码 QR；可使用网格像素匹配");
            return;
        };
        let _ = self.ensure_qr_reference_matrix();

        match regenerate_qr(&decoded, mask) {
            Ok(matrix) => {
                let svg = self.qr_svg_for_matrix(&matrix);
                self.set_generated_artifacts(matrix, svg);
            }
            Err(error) => {
                self.last_matrix = None;
                self.qr_reference_matrix = None;
                self.last_svg = None;
                self.last_diff_count = None;
                self.status = format!("QR 重生成失败：{error}");
            }
        }
    }

    pub fn can_switch_qr_appearance(&self) -> bool {
        self.code_kind == CodeKind::Qr && self.last_matrix.is_some()
    }

    pub fn set_qr_appearance(&mut self, appearance: QrAppearance) {
        if self.qr_appearance == appearance {
            return;
        }
        self.qr_appearance = appearance;
        if let Some(matrix) = self.last_matrix.clone() {
            self.last_svg = Some(self.qr_svg_for_matrix(&matrix));
            self.refresh_generated_preview();
        }
    }

    fn qr_svg_for_matrix(&self, matrix: &QrMatrix) -> String {
        match self.qr_appearance {
            QrAppearance::Standard => qr_matrix_to_svg(matrix, 1.0),
            appearance => qr_matrix_to_svg_with_appearance(matrix, 1.0, appearance),
        }
    }

    fn set_generated_artifacts(&mut self, matrix: QrMatrix, svg: String) {
        self.last_data_matrix_grid = None;
        self.last_wx_grid = None;
        self.last_dy_grid = None;
        self.last_matrix = Some(matrix);
        self.last_svg = Some(svg);
        self.refresh_generated_preview();
    }

    fn set_wx_artifacts(&mut self, grid: WxGrid, svg: String) {
        self.last_matrix = None;
        self.qr_reference_matrix = None;
        self.matched_mask = None;
        self.last_data_matrix_grid = None;
        self.last_dy_grid = None;
        self.last_wx_grid = Some(grid);
        self.last_svg = Some(svg);
        self.refresh_wx_preview();
    }

    fn set_dy_artifacts(&mut self, grid: DyGrid, svg: String) {
        self.last_matrix = None;
        self.qr_reference_matrix = None;
        self.matched_mask = None;
        self.last_data_matrix_grid = None;
        self.last_wx_grid = None;
        self.last_dy_grid = Some(grid);
        self.last_svg = Some(svg);
        self.refresh_dy_preview();
    }

    fn refresh_data_matrix_preview(&mut self) {
        let Some(grid) = self.last_data_matrix_grid.as_ref() else {
            return;
        };

        let diff_source = self.warped.as_ref().or(self.binary.as_ref());
        let (preview, diff_count) = match diff_source {
            Some(binary) => data_matrix_grid_to_diff_preview_image(
                grid,
                binary,
                self.show_diff_overlay,
                PREVIEW_SIZE,
            ),
            None => (data_matrix_grid_to_preview_image(grid, PREVIEW_SIZE), 0),
        };
        self.preview = Some(LoadedImage::from_dynamic(
            format!(
                "preview-data-matrix-{}x{}-{}",
                grid.cols, grid.rows, self.show_diff_overlay
            ),
            preview,
        ));
        self.last_diff_count = Some(diff_count);
        self.status = format!(
            "已识别 Data Matrix：{} x {} 模块；差异 {diff_count} 个模块（红色=原图有生成图没有，蓝色=原图没有生成图有）",
            grid.cols, grid.rows
        );
    }

    fn refresh_wx_preview(&mut self) {
        let Some(grid) = self.last_wx_grid.as_ref() else {
            return;
        };

        let diff_source = self.warped.as_ref().or(self.binary.as_ref());
        let (preview, diff_count) = match diff_source {
            Some(binary) => {
                wx_grid_to_diff_preview_image(grid, binary, self.show_diff_overlay, PREVIEW_SIZE)
            }
            None => (wx_grid_to_preview_image(grid, PREVIEW_SIZE), 0),
        };
        self.preview = Some(LoadedImage::from_dynamic(
            format!(
                "preview-wx-{}-{}-{}",
                grid.lines, grid.points_per_line, self.show_diff_overlay
            ),
            preview,
        ));
        self.last_diff_count = Some(diff_count);
        self.status = format!(
            "已识别小程序码：{} 线，每线 {} 点；差异 {diff_count} 个像素（红色=原图有生成图没有，蓝色=原图没有生成图有）",
            grid.lines, grid.points_per_line
        );
    }

    fn refresh_dy_preview(&mut self) {
        let Some(grid) = self.last_dy_grid.as_ref() else {
            return;
        };

        let diff_source = self.warped.as_ref().or(self.binary.as_ref());
        let (preview, diff_count) = match diff_source {
            Some(binary) => {
                dy_grid_to_diff_preview_image(grid, binary, self.show_diff_overlay, PREVIEW_SIZE)
            }
            None => (dy_grid_to_preview_image(grid, PREVIEW_SIZE), 0),
        };
        self.preview = Some(LoadedImage::from_dynamic(
            format!(
                "preview-dy-{}-{}-{}",
                grid.ring_count(),
                grid.points_per_ring,
                self.show_diff_overlay
            ),
            preview,
        ));
        self.last_diff_count = Some(diff_count);
        self.status = format!(
            "已识别抖音码：{} 环，编码每环 {} 点，{}；差异 {diff_count} 个像素（红色=原图有生成图没有，蓝色=原图没有生成图有）",
            grid.ring_count(),
            grid.points_per_ring,
            if grid.has_border {
                "黑框版"
            } else {
                "无框版"
            }
        );
    }

    fn refresh_generated_preview(&mut self) {
        let Some(matrix) = self.last_matrix.as_ref() else {
            return;
        };
        let diff = self
            .qr_reference_matrix
            .as_ref()
            .and_then(|reference| compute_matrix_diff(reference, matrix));
        let diff_count = diff.as_ref().map(|diff| diff.diff_count).unwrap_or(0);
        let modules = matrix.len().max(1) as u32;
        let scale = (PREVIEW_SIZE / modules).max(2);
        let preview = qr_matrix_to_preview_image(
            matrix,
            self.qr_appearance,
            diff.as_ref(),
            self.show_diff_overlay,
            scale,
            0,
        );
        let preview_name = match self.mask_choice {
            MaskChoice::Mask(mask) => format!("preview-mask-{mask}-{:?}", self.qr_appearance),
            MaskChoice::GridFallback => format!("preview-grid-fallback-{:?}", self.qr_appearance),
        };
        self.preview = Some(LoadedImage::from_dynamic(preview_name, preview));
        self.last_diff_count = Some(diff_count);

        let mode_text = match self.mask_choice {
            MaskChoice::Mask(mask) => self
                .last_decoded
                .as_ref()
                .map(|decoded| {
                    let mask_text = if self.matched_mask == Some(mask) {
                        format!("原掩膜 {mask}")
                    } else if self.matched_mask.is_none() {
                        format!("当前掩膜 {mask}，无匹配掩膜")
                    } else {
                        format!("掩膜 {mask}")
                    };
                    format!(
                        "已解码 QR：V{} / ECC {} / {mask_text}",
                        decoded.version,
                        decoded.ecc.label()
                    )
                })
                .unwrap_or_else(|| format!("QR 掩膜重生成：掩膜 {mask}")),
            MaskChoice::GridFallback => self
                .qr_version
                .map(|version| format!("使用网格像素匹配：V{version}"))
                .unwrap_or_else(|| String::from("使用网格像素匹配")),
        };
        self.status = format!(
            "{mode_text}，差异 {diff_count} 个模块（红色=原图有生成图没有，蓝色=原图没有生成图有）"
        );
    }

    fn poll_background_jobs(&mut self, ctx: &egui::Context) {
        self.poll_capture_job(ctx);
        self.poll_processing_job();

        if self.processing_job.is_some() || self.capture_job.is_some() {
            ctx.request_repaint_after(LOADING_REPAINT_INTERVAL);
        }
    }

    fn poll_capture_job(&mut self, ctx: &egui::Context) {
        let result = match self.capture_job.as_ref() {
            Some(job) => match job.receiver.try_recv() {
                Ok(result) => Some(Ok(result)),
                Err(TryRecvError::Disconnected) => Some(Err(String::from("截屏线程已退出"))),
                Err(TryRecvError::Empty) => None,
            },
            None => None,
        };

        let Some(result) = result else {
            return;
        };
        self.capture_job = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));

        match result {
            Ok(Ok(img)) => self.begin_processing(img, "截屏图片"),
            Ok(Err(error)) => self.status = format!("截屏取消或失败：{error}"),
            Err(error) => self.status = format!("截屏失败：{error}"),
        }
    }

    fn poll_processing_job(&mut self) {
        let result = match self.processing_job.as_ref() {
            Some(job) => match job.receiver.try_recv() {
                Ok(result) => Some(Ok(result)),
                Err(TryRecvError::Disconnected) => Some(Err(String::from("处理线程已退出"))),
                Err(TryRecvError::Empty) => None,
            },
            None => None,
        };

        let Some(result) = result else {
            return;
        };
        self.processing_job = None;

        match result {
            Ok(result) => self.apply_process_result(result),
            Err(error) => self.status = format!("处理失败：{error}"),
        }
    }

    fn apply_process_result(&mut self, result: ProcessResult) {
        self.code_kind = result.code_kind;
        self.binary = result.binary;
        self.finders = result.finders;
        self.warped = result.warped;
        self.mask_choice = result.mask_choice;
        self.qr_appearance = result.qr_appearance;
        self.last_decoded = result.last_decoded;
        self.qr_version = result.qr_version;
        self.last_matrix = result.last_matrix;
        self.qr_reference_matrix = result.qr_reference_matrix;
        self.matched_mask = result.matched_mask;
        self.last_data_matrix_grid = result.last_data_matrix_grid;
        self.last_wx_grid = result.last_wx_grid;
        self.last_dy_grid = result.last_dy_grid;
        self.last_svg = result.last_svg;
        self.last_diff_count = result.last_diff_count;
        self.preview = result
            .preview
            .map(|(name, image)| LoadedImage::from_dynamic(name, image));
        self.status = result.status;
    }
}

fn manual_wx_target_finders(target_size: u32) -> [WxFinder; 3] {
    let max = target_size.saturating_sub(1) as f64;
    let margin = max * 0.23;
    let far = max - margin;
    let target_leg = far - margin;
    let radius = (target_leg * 0.0786).max(1.0);

    [
        WxFinder {
            cx: margin,
            cy: margin,
            r_outer: radius,
        },
        WxFinder {
            cx: far,
            cy: margin,
            r_outer: radius,
        },
        WxFinder {
            cx: margin,
            cy: far,
            r_outer: radius,
        },
    ]
}

fn manual_dy_target_finders(target_size: u32, has_border_hint: bool) -> [DyFinder; 3] {
    let max = target_size.saturating_sub(1) as f64;
    let margin = max * 0.23;
    let far = max - margin;
    let locator_distance = (far - margin) / std::f64::consts::SQRT_2;
    let (standard_distance, standard_radii) = if has_border_hint {
        (
            DY_MANUAL_BLACK_BORDER_LOCATOR_DISTANCE,
            DY_MANUAL_BLACK_BORDER_LOCATOR_RADII,
        )
    } else {
        (
            DY_MANUAL_NO_BORDER_LOCATOR_DISTANCE,
            DY_MANUAL_NO_BORDER_LOCATOR_RADII,
        )
    };
    let scale = (locator_distance / standard_distance).max(0.01);
    let rings = standard_radii
        .iter()
        .map(|radius| (radius * scale).max(1.0))
        .collect::<Vec<_>>();

    [
        DyFinder {
            cx: margin,
            cy: margin,
            rings: rings.clone(),
        },
        DyFinder {
            cx: margin,
            cy: far,
            rings: rings.clone(),
        },
        DyFinder {
            cx: far,
            cy: far,
            rings,
        },
    ]
}

fn export_svg_file_name() -> String {
    format!("{}.svg", current_export_timestamp())
}

#[cfg(windows)]
fn current_export_timestamp() -> String {
    let mut now = SYSTEMTIME::default();
    unsafe {
        GetLocalTime(&mut now);
    }

    format_export_timestamp(
        now.wYear.into(),
        now.wMonth.into(),
        now.wDay.into(),
        now.wHour.into(),
        now.wMinute.into(),
        now.wSecond.into(),
    )
}

#[cfg(not(windows))]
fn current_export_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_unix_days(days);

    format_export_timestamp(
        year,
        month,
        day,
        (seconds_of_day / 3_600) as u32,
        ((seconds_of_day % 3_600) / 60) as u32,
        (seconds_of_day % 60) as u32,
    )
}

fn format_export_timestamp(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> String {
    format!("{year:04}{month:02}{day:02}{hour:02}{minute:02}{second:02}")
}

#[cfg(not(windows))]
fn civil_from_unix_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };

    (year as i32, month as u32, day as u32)
}

fn process_image(
    img: DynamicImage,
    job_id: u64,
    show_diff_overlay: bool,
    code_kind_override: Option<CodeKind>,
) -> ProcessResult {
    let binary = preprocess(&img);
    let code_kind =
        code_kind_override.unwrap_or_else(|| detect::detect_kind_with_image(&img, &binary));
    let mut result = ProcessResult {
        code_kind,
        status: String::from("图像已加载；未识别到支持的码类型"),
        binary: Some(binary.clone()),
        finders: None,
        warped: None,
        mask_choice: MaskChoice::Mask(0),
        qr_appearance: QrAppearance::Standard,
        last_decoded: None,
        qr_version: None,
        last_matrix: None,
        qr_reference_matrix: None,
        matched_mask: None,
        last_data_matrix_grid: None,
        last_wx_grid: None,
        last_dy_grid: None,
        last_svg: None,
        last_diff_count: None,
        preview: None,
    };

    match code_kind {
        CodeKind::Qr => process_qr_image(&img, &binary, job_id, show_diff_overlay, &mut result),
        CodeKind::DataMatrix => {
            process_data_matrix_image(&img, &binary, job_id, show_diff_overlay, &mut result)
        }
        CodeKind::WxMiniprogram => {
            process_wx_image(&img, &binary, job_id, show_diff_overlay, &mut result)
        }
        CodeKind::Douyin => process_dy_image(&img, &binary, job_id, show_diff_overlay, &mut result),
        _ => {}
    }

    result
}

fn process_qr_image(
    img: &DynamicImage,
    binary: &BinaryImage,
    job_id: u64,
    show_diff_overlay: bool,
    result: &mut ProcessResult,
) {
    let finders = find_qr_finders(binary);

    let Some(selected) = select_qr_finder_triplet(binary, &finders) else {
        result.status = format!(
            "已识别 QR，但无法从 {} 个候选中选出三角定位点",
            finders.len()
        );
        result.finders = Some(finders);
        return;
    };
    let corrected_source = warp_qr_to_square_image(img, binary, &selected, PREVIEW_SIZE);
    let warped = preprocess(&corrected_source);
    result.qr_appearance = infer_qr_appearance(img, &selected, &warped);
    result.preview = Some((
        format!("preview-qr-corrected-{job_id}"),
        warped.to_dynamic_image(),
    ));
    result.warped = Some(warped.clone());
    result.finders = Some(finders);
    result.qr_version = infer_qr_version(&warped).ok();

    match decode_qr(img, Some(&warped)) {
        Ok(decoded) => {
            result.qr_version = Some(decoded.version);
            let reference_matrix = sample_qr_grid(&warped, decoded.version).ok();
            result.qr_reference_matrix = reference_matrix.clone();

            if let Some(candidate) = reference_matrix
                .as_ref()
                .and_then(|reference| choose_matching_qr_mask(&decoded, reference))
            {
                let mut decoded = decoded.clone();
                decoded.ecc = candidate.ecc;
                let mask = candidate.mask;
                result.last_decoded = Some(decoded.clone());
                result.mask_choice = MaskChoice::Mask(mask);
                result.matched_mask = Some(mask);
                let svg =
                    qr_matrix_to_svg_with_appearance(&candidate.matrix, 1.0, result.qr_appearance);
                let (preview_name, preview, diff_count) = qr_preview_for_matrix(
                    &candidate.matrix,
                    reference_matrix.as_ref(),
                    result.mask_choice,
                    result.qr_appearance,
                    show_diff_overlay,
                    job_id,
                );
                result.last_matrix = Some(candidate.matrix);
                result.last_svg = Some(svg);
                result.last_diff_count = Some(diff_count);
                result.preview = Some((preview_name, preview));
                result.status = qr_mask_status_text(&decoded, mask, Some(mask), diff_count, 0);
            } else {
                result.last_decoded = Some(decoded.clone());
                result.matched_mask = None;
                result.mask_choice = MaskChoice::GridFallback;
                match reference_matrix {
                    Some(matrix) => {
                        let svg =
                            qr_matrix_to_svg_with_appearance(&matrix, 1.0, result.qr_appearance);
                        let (preview_name, preview, diff_count) = qr_preview_for_matrix(
                            &matrix,
                            result.qr_reference_matrix.as_ref(),
                            result.mask_choice,
                            result.qr_appearance,
                            show_diff_overlay,
                            job_id,
                        );
                        result.last_matrix = Some(matrix);
                        result.last_svg = Some(svg);
                        result.last_diff_count = Some(diff_count);
                        result.preview = Some((preview_name, preview));
                        result.status = format!(
                            "已解码 QR：V{} / ECC {} / 无匹配掩膜，已自动使用网格像素匹配，差异 {diff_count} 个模块（红色=原图有生成图没有，蓝色=原图没有生成图有）",
                            decoded.version,
                            decoded.ecc.label(),
                        );
                    }
                    None => {
                        result.status = String::from(
                            "已解码 QR，但 8 种掩膜均不匹配，且网格像素匹配失败：无法采样校正图",
                        );
                    }
                }
            }
        }
        Err(error) => {
            if let Ok(version) = try_use_decodeless_qr_grid_fallback(
                &warped,
                &selected,
                result,
                show_diff_overlay,
                job_id,
            ) {
                result.status = format!(
                    "已完成 QR 校正并推断版本 V{version}，但解码失败：{error}；已自动使用网格像素匹配"
                );
                return;
            }

            result.status = if let Some(version) = result.qr_version {
                format!(
                    "已完成 QR 校正并推断版本 V{version}，但解码失败：{error}；可使用网格像素匹配"
                )
            } else {
                format!("已完成 QR 校正，但解码失败：{error}")
            };
        }
    }
}

fn try_use_decodeless_qr_grid_fallback(
    warped: &BinaryImage,
    finders: &[QrFinder; 3],
    result: &mut ProcessResult,
    show_diff_overlay: bool,
    job_id: u64,
) -> Result<u8, String> {
    let version = result
        .qr_version
        .or_else(|| infer_qr_version(warped).ok())
        .or_else(|| estimate_qr_modules_from_finders(finders).and_then(qr_version_for_modules))
        .ok_or_else(|| String::from("无法推断 QR 版本"))?;
    let matrix = sample_qr_grid(warped, version).map_err(|error| error.to_string())?;

    result.qr_version = Some(version);
    result.qr_reference_matrix = Some(matrix.clone());
    result.matched_mask = None;
    result.mask_choice = MaskChoice::GridFallback;
    let svg = qr_matrix_to_svg_with_appearance(&matrix, 1.0, result.qr_appearance);
    let (preview_name, preview, diff_count) = qr_preview_for_matrix(
        &matrix,
        result.qr_reference_matrix.as_ref(),
        result.mask_choice,
        result.qr_appearance,
        show_diff_overlay,
        job_id,
    );
    result.last_matrix = Some(matrix);
    result.last_svg = Some(svg);
    result.last_diff_count = Some(diff_count);
    result.preview = Some((preview_name, preview));

    Ok(version)
}

fn process_data_matrix_image(
    img: &DynamicImage,
    binary: &BinaryImage,
    job_id: u64,
    show_diff_overlay: bool,
    result: &mut ProcessResult,
) {
    let candidates = find_data_matrix_candidates(binary);
    let Some(best) = best_data_matrix_processing_candidate(img, &candidates) else {
        result.status = format!(
            "已识别 Data Matrix 候选失败：无法从 {} 个候选中采样出合法网格",
            candidates.len()
        );
        return;
    };

    let svg = data_matrix_grid_to_svg(&best.grid);
    let (preview, diff_count) = data_matrix_grid_to_diff_preview_image(
        &best.grid,
        &best.binary,
        show_diff_overlay,
        PREVIEW_SIZE,
    );
    result.warped = Some(best.binary);
    result.last_data_matrix_grid = Some(best.grid.clone());
    result.last_svg = Some(svg);
    result.last_diff_count = Some(diff_count);
    result.preview = Some((
        format!(
            "preview-data-matrix-{}x{}-{job_id}",
            best.grid.cols, best.grid.rows
        ),
        preview,
    ));
    result.status = format!(
        "已识别 Data Matrix：{} x {} 模块；差异 {diff_count} 个模块（红色=原图有生成图没有，蓝色=原图没有生成图有）",
        best.grid.cols, best.grid.rows
    );
}

struct ProcessedDataMatrixCandidate {
    binary: BinaryImage,
    grid: DataMatrixGrid,
}

fn best_data_matrix_processing_candidate(
    img: &DynamicImage,
    candidates: &[DataMatrixCandidate],
) -> Option<ProcessedDataMatrixCandidate> {
    let mut best: Option<(f64, ProcessedDataMatrixCandidate)> = None;
    let max_side = candidates
        .iter()
        .copied()
        .map(data_matrix_candidate_side)
        .fold(0.0, f64::max);
    let min_side = if max_side >= 80.0 {
        max_side * 0.35
    } else {
        0.0
    };

    let mut processed_physical_candidates = Vec::new();

    for candidate in candidates.iter().take(8).copied() {
        let side = data_matrix_candidate_side(candidate);
        if side < min_side {
            continue;
        }
        if processed_physical_candidates
            .iter()
            .any(|existing| data_matrix_physical_candidates_overlap(*existing, candidate))
        {
            continue;
        }
        processed_physical_candidates.push(candidate);

        let corner_orders = if candidate.score >= 0.85 {
            vec![candidate.corners]
        } else {
            data_matrix_corner_orderings(candidate.corners).to_vec()
        };
        let corner_scales = [1.0, 1.04, 0.96];
        let symbol_hints = data_matrix_processing_symbol_hints(candidate, side, max_side);

        for ordered_corners in corner_orders {
            let mut order_has_confident_grid = false;
            for corner_scale in corner_scales {
                let mut scale_has_very_confident_grid = false;
                let corners = scaled_data_matrix_corners(&ordered_corners, corner_scale);
                let (width, height) = data_matrix_warp_size(candidate);
                let corrected_source = warp_corners_to_image(img, &corners, width, height);
                let corrected_binary = preprocess(&corrected_source);
                let corrected_binary = if candidate.score < 0.85 {
                    let (_, refined_binary) =
                        refine_data_matrix_corrected_source(corrected_source, corrected_binary);
                    refined_binary
                } else {
                    corrected_binary
                };

                let mut grids: Vec<DataMatrixGrid> = match sample_data_matrix_grid_for_symbols(
                    &corrected_binary,
                    symbol_hints.iter().copied(),
                ) {
                    Ok(grid) => vec![grid],
                    Err(_) => Vec::new(),
                };
                if grids.is_empty() {
                    if let Ok(grid) = sample_data_matrix_grid(&corrected_binary) {
                        grids.push(grid);
                    }
                }

                for grid in grids {
                    let side_ratio = if max_side > f64::EPSILON {
                        (side / max_side).min(1.0)
                    } else {
                        1.0
                    };
                    let corner_scale_penalty = (corner_scale - 1.0_f64).abs() * 0.22;
                    let undersampling_penalty =
                        data_matrix_square_undersampling_penalty(&grid, side);
                    let score = grid.score + candidate.score * 0.10 + side_ratio * 0.12
                        - corner_scale_penalty
                        - undersampling_penalty;
                    if is_confident_data_matrix_result(score, &grid, side_ratio) {
                        order_has_confident_grid = true;
                    }
                    if is_very_confident_data_matrix_result(score, &grid, side_ratio) {
                        scale_has_very_confident_grid = true;
                    }
                    if best
                        .as_ref()
                        .is_none_or(|(best_score, _)| score > *best_score)
                    {
                        best = Some((
                            score,
                            ProcessedDataMatrixCandidate {
                                binary: corrected_binary.clone(),
                                grid,
                            },
                        ));
                    }
                }
                if scale_has_very_confident_grid {
                    break;
                }
            }
            if order_has_confident_grid {
                break;
            }
        }
    }

    best.map(|(_, processed)| processed)
}

fn data_matrix_corner_orderings(corners: [(f64, f64); 4]) -> [[(f64, f64); 4]; 4] {
    let [a, b, c, d] = corners;
    [[a, b, c, d], [d, c, b, a], [b, d, a, c], [c, a, d, b]]
}

fn is_confident_data_matrix_result(score: f64, grid: &DataMatrixGrid, side_ratio: f64) -> bool {
    side_ratio >= 0.75 && grid.score >= 0.98 && score >= 1.12
}

fn is_very_confident_data_matrix_result(
    score: f64,
    grid: &DataMatrixGrid,
    side_ratio: f64,
) -> bool {
    side_ratio >= 0.75 && grid.score >= 1.03 && score >= 1.24
}

fn data_matrix_processing_symbol_hints(
    candidate: DataMatrixCandidate,
    side: f64,
    max_side: f64,
) -> Vec<DataMatrixSymbol> {
    let mut symbols = Vec::new();
    push_data_matrix_symbol_hint(&mut symbols, candidate.symbol);

    let top = data_matrix_corner_distance(candidate.corners[0], candidate.corners[1]);
    let bottom = data_matrix_corner_distance(candidate.corners[2], candidate.corners[3]);
    let left = data_matrix_corner_distance(candidate.corners[0], candidate.corners[2]);
    let right = data_matrix_corner_distance(candidate.corners[1], candidate.corners[3]);
    let width = (top + bottom) * 0.5;
    let height = (left + right) * 0.5;
    let aspect = width / height.max(f64::EPSILON);
    let is_large_square =
        side >= max_side * 0.75 && side >= 180.0 && (0.75..=1.33).contains(&aspect);

    if is_large_square && candidate.score < 0.85 {
        for modules in [22, 24, 26] {
            if let Some(symbol) = DATA_MATRIX_SYMBOLS
                .iter()
                .copied()
                .find(|symbol| symbol.rows == modules && symbol.cols == modules)
            {
                push_data_matrix_symbol_hint(&mut symbols, symbol);
            }
        }
    }

    symbols
}

fn push_data_matrix_symbol_hint(symbols: &mut Vec<DataMatrixSymbol>, symbol: DataMatrixSymbol) {
    if !symbols.contains(&symbol) {
        symbols.push(symbol);
    }
}

fn refine_data_matrix_corrected_source(
    source: DynamicImage,
    binary: BinaryImage,
) -> (DynamicImage, BinaryImage) {
    let Some(corners) = refined_data_matrix_corners(&binary) else {
        return (source, binary);
    };
    let width = source.width();
    let height = source.height();
    let refined_source = warp_corners_to_image(&source, &corners, width, height);
    let refined_binary = preprocess(&refined_source);
    (refined_source, refined_binary)
}

fn refined_data_matrix_corners(binary: &BinaryImage) -> Option<[(f64, f64); 4]> {
    if binary.w < 64 || binary.h < 64 {
        return None;
    }

    let left = fit_x_from_y(&collect_left_edge_points(binary)?)?;
    let right = fit_x_from_y(&collect_right_edge_points(binary)?)?;
    let top = fit_y_from_x(&collect_top_edge_points(binary)?)?;
    let bottom = fit_y_from_x(&collect_bottom_edge_points(binary)?)?;

    let corners = [
        intersect_x_from_y_with_y_from_x(left, top)?,
        intersect_x_from_y_with_y_from_x(right, top)?,
        intersect_x_from_y_with_y_from_x(left, bottom)?,
        intersect_x_from_y_with_y_from_x(right, bottom)?,
    ];

    let width = binary.w as f64;
    let height = binary.h as f64;
    let max_margin = width.max(height) * 0.08;
    if corners.iter().any(|&(x, y)| {
        x < -max_margin || y < -max_margin || x > width + max_margin || y > height + max_margin
    }) {
        return None;
    }

    let top_len = data_matrix_corner_distance(corners[0], corners[1]);
    let bottom_len = data_matrix_corner_distance(corners[2], corners[3]);
    let left_len = data_matrix_corner_distance(corners[0], corners[2]);
    let right_len = data_matrix_corner_distance(corners[1], corners[3]);
    let avg_w = (top_len + bottom_len) * 0.5;
    let avg_h = (left_len + right_len) * 0.5;
    if avg_w < width * 0.45 || avg_h < height * 0.45 {
        return None;
    }
    let aspect = avg_w / avg_h.max(f64::EPSILON);
    if !(0.50..=2.00).contains(&aspect) {
        return None;
    }

    Some(corners)
}

fn collect_left_edge_points(binary: &BinaryImage) -> Option<Vec<(f64, f64)>> {
    let mut points = Vec::new();
    let max_x = (binary.w as usize * 45 / 100).max(1);
    for y in 0..binary.h as usize {
        for x in 0..max_x.min(binary.w as usize) {
            if binary.is_black(x as i32, y as i32) {
                points.push((x as f64, y as f64));
                break;
            }
        }
    }
    (points.len() >= binary.h as usize / 5).then_some(points)
}

fn collect_right_edge_points(binary: &BinaryImage) -> Option<Vec<(f64, f64)>> {
    let mut points = Vec::new();
    let min_x = binary.w as usize * 55 / 100;
    for y in 0..binary.h as usize {
        for x in (min_x..binary.w as usize).rev() {
            if binary.is_black(x as i32, y as i32) {
                points.push((x as f64, y as f64));
                break;
            }
        }
    }
    (points.len() >= binary.h as usize / 8).then_some(points)
}

fn collect_top_edge_points(binary: &BinaryImage) -> Option<Vec<(f64, f64)>> {
    let mut points = Vec::new();
    let max_y = (binary.h as usize * 18 / 100).max(1);
    for x in 0..binary.w as usize {
        for y in 0..max_y.min(binary.h as usize) {
            if binary.is_black(x as i32, y as i32) {
                points.push((x as f64, y as f64));
                break;
            }
        }
    }
    (points.len() >= binary.w as usize / 8).then_some(points)
}

fn collect_bottom_edge_points(binary: &BinaryImage) -> Option<Vec<(f64, f64)>> {
    let mut points = Vec::new();
    let min_y = binary.h as usize * 55 / 100;
    for x in 0..binary.w as usize {
        for y in (min_y..binary.h as usize).rev() {
            if binary.is_black(x as i32, y as i32) {
                points.push((x as f64, y as f64));
                break;
            }
        }
    }
    (points.len() >= binary.w as usize / 5).then_some(points)
}

fn fit_x_from_y(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    fit_line(points, |(x, y)| (y, x))
}

fn fit_y_from_x(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    fit_line(points, |(x, y)| (x, y))
}

fn fit_line<F>(points: &[(f64, f64)], map: F) -> Option<(f64, f64)>
where
    F: Fn((f64, f64)) -> (f64, f64) + Copy,
{
    if points.len() < 8 {
        return None;
    }
    let mut mapped = points.iter().copied().map(map).collect::<Vec<_>>();
    let mut line = linear_regression(&mapped)?;

    for _ in 0..2 {
        let mut residuals = mapped
            .iter()
            .map(|&(input, output)| (output - (line.0 * input + line.1)).abs())
            .collect::<Vec<_>>();
        residuals.sort_by(|a, b| a.total_cmp(b));
        let cutoff = residuals[residuals.len() * 7 / 10].max(2.0);
        mapped.retain(|&(input, output)| (output - (line.0 * input + line.1)).abs() <= cutoff);
        line = linear_regression(&mapped)?;
    }

    Some(line)
}

fn linear_regression(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    if points.len() < 8 {
        return None;
    }
    let n = points.len() as f64;
    let sum_x = points.iter().map(|point| point.0).sum::<f64>();
    let sum_y = points.iter().map(|point| point.1).sum::<f64>();
    let mean_x = sum_x / n;
    let mean_y = sum_y / n;
    let var_x = points
        .iter()
        .map(|point| {
            let dx = point.0 - mean_x;
            dx * dx
        })
        .sum::<f64>();
    if var_x <= f64::EPSILON {
        return None;
    }
    let cov_xy = points
        .iter()
        .map(|point| (point.0 - mean_x) * (point.1 - mean_y))
        .sum::<f64>();
    let slope = cov_xy / var_x;
    let intercept = mean_y - slope * mean_x;
    Some((slope, intercept))
}

fn intersect_x_from_y_with_y_from_x(
    x_from_y: (f64, f64),
    y_from_x: (f64, f64),
) -> Option<(f64, f64)> {
    let (mx, bx) = x_from_y;
    let (my, by) = y_from_x;
    let denom = 1.0 - mx * my;
    if denom.abs() <= 1e-6 {
        return None;
    }
    let y = (my * bx + by) / denom;
    let x = mx * y + bx;
    Some((x, y))
}

fn data_matrix_candidate_side(candidate: DataMatrixCandidate) -> f64 {
    let top = data_matrix_corner_distance(candidate.corners[0], candidate.corners[1]);
    let bottom = data_matrix_corner_distance(candidate.corners[2], candidate.corners[3]);
    let left = data_matrix_corner_distance(candidate.corners[0], candidate.corners[2]);
    let right = data_matrix_corner_distance(candidate.corners[1], candidate.corners[3]);
    ((top + bottom + left + right) * 0.25).max(1.0)
}

fn data_matrix_physical_candidates_overlap(a: DataMatrixCandidate, b: DataMatrixCandidate) -> bool {
    let ac = data_matrix_candidate_center(a);
    let bc = data_matrix_candidate_center(b);
    let side = data_matrix_candidate_side(a)
        .max(data_matrix_candidate_side(b))
        .max(1.0);
    let side_ratio = data_matrix_candidate_side(a).min(data_matrix_candidate_side(b)) / side;
    side_ratio >= 0.75 && (ac.0 - bc.0).hypot(ac.1 - bc.1) < side * 0.12
}

fn data_matrix_candidate_center(candidate: DataMatrixCandidate) -> (f64, f64) {
    let mut x = 0.0;
    let mut y = 0.0;
    for point in candidate.corners {
        x += point.0;
        y += point.1;
    }
    (x * 0.25, y * 0.25)
}

fn data_matrix_corner_distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

fn data_matrix_square_undersampling_penalty(grid: &DataMatrixGrid, side: f64) -> f64 {
    if grid.rows != grid.cols || grid.rows >= 24 || side < 300.0 {
        return 0.0;
    }
    (24 - grid.rows) as f64 * 0.13
}

fn scaled_data_matrix_corners(corners: &[(f64, f64); 4], scale: f64) -> [(f64, f64); 4] {
    if (scale - 1.0).abs() <= f64::EPSILON {
        return *corners;
    }
    let center = corners
        .iter()
        .fold((0.0, 0.0), |sum, point| (sum.0 + point.0, sum.1 + point.1));
    let center = (center.0 * 0.25, center.1 * 0.25);
    corners.map(|point| {
        (
            center.0 + (point.0 - center.0) * scale,
            center.1 + (point.1 - center.1) * scale,
        )
    })
}

fn data_matrix_warp_size(candidate: DataMatrixCandidate) -> (u32, u32) {
    let max_modules = candidate.rows().max(candidate.cols()).max(1) as u32;
    let module_px = (PREVIEW_SIZE / max_modules).clamp(4, 64);
    (
        candidate.cols() as u32 * module_px,
        candidate.rows() as u32 * module_px,
    )
}

fn process_wx_image(
    img: &DynamicImage,
    binary: &BinaryImage,
    job_id: u64,
    show_diff_overlay: bool,
    result: &mut ProcessResult,
) {
    let finders = find_wx_finders(binary);
    let badge_anchor = detect_wx_badge_anchor(img);
    let raw_selected = badge_anchor
        .and_then(|badge| select_wx_finders_raw_with_badge(&finders, badge))
        .or_else(|| select_wx_finders_raw(&finders));
    let Some(raw_selected) = raw_selected else {
        result.status = format!(
            "已识别小程序码，但无法从 {} 个候选中选出三牛眼定位点",
            finders.len()
        );
        return;
    };

    let correction_size = img.width().max(img.height()).clamp(PREVIEW_SIZE, 1600);
    let anchor = badge_anchor.map(WxUprightAnchor::Badge);
    let corrected_source = warp_wx_to_upright_image(img, &raw_selected, anchor, correction_size);
    let corrected_binary = preprocess(&corrected_source);
    let selected = wx_upright_target_finders(&raw_selected, correction_size);
    result.warped = Some(corrected_binary.clone());

    let preferred_version = detect_wx_version(&corrected_binary, &selected).ok();
    let mut best: Option<(u32, bool, WxGrid)> = None;
    let mut errors = Vec::new();
    for version in [36, 54, 72] {
        let grid =
            match sample_wx_with_badge(&corrected_binary, &corrected_source, &selected, version) {
                Ok(grid) => grid,
                Err(error) => {
                    errors.push(format!("{version} 线：{error}"));
                    continue;
                }
            };
        let (_, diff_count) =
            wx_grid_to_diff_preview_image(&grid, &corrected_binary, false, PREVIEW_SIZE);
        let preferred = preferred_version == Some(version);
        if best.as_ref().is_none_or(|(best_diff, best_preferred, _)| {
            diff_count < *best_diff || (diff_count == *best_diff && preferred && !*best_preferred)
        }) {
            best = Some((diff_count, preferred, grid));
        }
    }

    match best {
        Some((_, _, grid)) => {
            let svg = wx_grid_to_svg(&grid);
            let (preview, diff_count) = wx_grid_to_diff_preview_image(
                &grid,
                &corrected_binary,
                show_diff_overlay,
                PREVIEW_SIZE,
            );
            result.last_wx_grid = Some(grid.clone());
            result.last_svg = Some(svg);
            result.last_diff_count = Some(diff_count);
            result.preview = Some((format!("preview-wx-{}-{job_id}", grid.lines), preview));
            result.status = format!(
                "已识别小程序码：{} 线，每线 {} 点；差异 {diff_count} 个像素（红色=原图有生成图没有，蓝色=原图没有生成图有）",
                grid.lines, grid.points_per_line
            );
        }
        None => {
            result.status = if errors.is_empty() {
                String::from("小程序码径向采样失败：无可用候选")
            } else {
                format!("小程序码径向采样失败：{}", errors.join("；"))
            };
        }
    }
}

fn process_dy_image(
    img: &DynamicImage,
    binary: &BinaryImage,
    job_id: u64,
    show_diff_overlay: bool,
    result: &mut ProcessResult,
) {
    let finders = find_dy_finders(binary);
    let Some(raw_selected) = select_dy_finders_raw(&finders) else {
        result.status = format!(
            "已识别抖音码，但无法从 {} 个候选中选出三同心圆定位点",
            finders.len()
        );
        return;
    };

    let corrected = correct_dy_to_upright(img, binary, &raw_selected);
    let corrected_source = corrected.source;
    let corrected_binary = corrected.binary;
    let selected = corrected.finders;
    result.warped = Some(corrected_binary.clone());

    let params = match detect_dy_params(&corrected_binary, &selected) {
        Ok(params) => params,
        Err(error) => {
            result.status = format!("抖音码参数检测失败：{error}");
            return;
        }
    };

    match sample_dy_with_logos(&corrected_binary, &corrected_source, &selected, params) {
        Ok(grid) => {
            let svg = dy_grid_to_svg(&grid);
            let (preview, diff_count) = dy_grid_to_diff_preview_image(
                &grid,
                &corrected_binary,
                show_diff_overlay,
                PREVIEW_SIZE,
            );
            result.last_dy_grid = Some(grid.clone());
            result.last_svg = Some(svg);
            result.last_diff_count = Some(diff_count);
            result.preview = Some((
                format!(
                    "preview-dy-{}-{}-{job_id}",
                    grid.ring_count(),
                    grid.points_per_ring
                ),
                preview,
            ));
            result.status = format!(
                "已识别抖音码：{} 环，编码每环 {} 点，{}；差异 {diff_count} 个像素（红色=原图有生成图没有，蓝色=原图没有生成图有）",
                grid.ring_count(),
                grid.points_per_ring,
                if grid.has_border {
                    "黑框版"
                } else {
                    "无框版"
                }
            );
        }
        Err(error) => {
            result.status = format!("抖音码环形采样失败：{error}");
        }
    }
}

fn infer_qr_appearance(
    img: &DynamicImage,
    finders: &[QrFinder; 3],
    warped: &BinaryImage,
) -> QrAppearance {
    if has_wechat_center_badge(img, finders) {
        QrAppearance::Wechat
    } else if has_round_qr_finders(warped, finders) {
        QrAppearance::Xiaohongshu
    } else if has_enterprise_wechat_compact_modules(warped, finders) {
        QrAppearance::EnterpriseWechat
    } else {
        QrAppearance::Standard
    }
}

fn has_wechat_center_badge(img: &DynamicImage, finders: &[QrFinder; 3]) -> bool {
    let gray = img.to_luma8();
    let (center, qr_side) = qr_center_and_side_from_finders(finders);
    let core_side = (qr_side * 0.205).round().max(8.0) as i32;
    let ring_inner_side = (qr_side * 0.230).round().max(f64::from(core_side + 2)) as i32;
    let ring_outer_side = (qr_side * 0.275)
        .round()
        .max(f64::from(ring_inner_side + 2)) as i32;

    let (core_dark, core_total, longest_run) =
        centered_square_dark_stats(&gray, center, core_side, 112);
    let (ring_light, ring_total) =
        centered_square_ring_light_stats(&gray, center, ring_outer_side, ring_inner_side, 180);

    if core_total == 0 || ring_total == 0 {
        return false;
    }

    let core_dark_ratio = core_dark as f64 / core_total as f64;
    let core_run_ratio = longest_run as f64 / core_side.max(1) as f64;
    let ring_light_ratio = ring_light as f64 / ring_total as f64;
    core_dark_ratio > 0.56 && core_run_ratio > 0.68 && ring_light_ratio > 0.68
}

fn qr_center_and_side_from_finders(finders: &[QrFinder; 3]) -> ((f64, f64), f64) {
    let mut farthest = (finders[0], finders[1], 0.0_f64);
    for i in 0..finders.len() {
        for j in i + 1..finders.len() {
            let dx = finders[i].cx - finders[j].cx;
            let dy = finders[i].cy - finders[j].cy;
            let distance_sq = dx * dx + dy * dy;
            if distance_sq > farthest.2 {
                farthest = (finders[i], finders[j], distance_sq);
            }
        }
    }

    let center = (
        (farthest.0.cx + farthest.1.cx) * 0.5,
        (farthest.0.cy + farthest.1.cy) * 0.5,
    );
    let module = ((finders[0].module + finders[1].module + finders[2].module) / 3.0).max(1.0);
    let side = farthest.2.sqrt() / std::f64::consts::SQRT_2 + module * 7.0;
    (center, side.max(module * 21.0))
}

fn centered_square_dark_stats(
    gray: &GrayImage,
    center: (f64, f64),
    side: i32,
    threshold: u8,
) -> (u32, u32, i32) {
    let width = gray.width() as i32;
    let height = gray.height() as i32;
    let x0 = (center.0 - f64::from(side) * 0.5).round() as i32;
    let y0 = (center.1 - f64::from(side) * 0.5).round() as i32;
    let mut dark = 0_u32;
    let mut total = 0_u32;
    let mut longest_run = 0_i32;

    for y in y0..y0 + side {
        let mut run = 0_i32;
        for x in x0..x0 + side {
            if x < 0 || y < 0 || x >= width || y >= height {
                continue;
            }
            total += 1;
            let is_dark = gray.get_pixel(x as u32, y as u32)[0] < threshold;
            if is_dark {
                dark += 1;
                run += 1;
                longest_run = longest_run.max(run);
            } else {
                run = 0;
            }
        }
    }

    (dark, total, longest_run)
}

fn centered_square_ring_light_stats(
    gray: &GrayImage,
    center: (f64, f64),
    outer_side: i32,
    inner_side: i32,
    threshold: u8,
) -> (u32, u32) {
    let width = gray.width() as i32;
    let height = gray.height() as i32;
    let outer_x0 = (center.0 - f64::from(outer_side) * 0.5).round() as i32;
    let outer_y0 = (center.1 - f64::from(outer_side) * 0.5).round() as i32;
    let inner_x0 = (center.0 - f64::from(inner_side) * 0.5).round() as i32;
    let inner_y0 = (center.1 - f64::from(inner_side) * 0.5).round() as i32;
    let inner_x1 = inner_x0 + inner_side;
    let inner_y1 = inner_y0 + inner_side;
    let mut light = 0_u32;
    let mut total = 0_u32;

    for y in outer_y0..outer_y0 + outer_side {
        for x in outer_x0..outer_x0 + outer_side {
            if x >= inner_x0 && x < inner_x1 && y >= inner_y0 && y < inner_y1 {
                continue;
            }
            if x < 0 || y < 0 || x >= width || y >= height {
                continue;
            }
            total += 1;
            if gray.get_pixel(x as u32, y as u32)[0] > threshold {
                light += 1;
            }
        }
    }

    (light, total)
}

fn has_round_qr_finders(warped: &BinaryImage, finders: &[QrFinder; 3]) -> bool {
    let modules = infer_qr_version(warped)
        .ok()
        .map(qr_modules_for_version)
        .or_else(|| estimate_qr_modules_from_finders(finders));
    modules.is_some_and(|modules| has_round_qr_finders_for_modules(warped, modules))
}

fn has_round_qr_finders_for_modules(warped: &BinaryImage, modules: usize) -> bool {
    if modules < 21 || warped.w == 0 || warped.h == 0 {
        return false;
    }

    let origins = [(0, 0), (modules - 7, 0), (0, modules - 7)];
    let rounded_finders = origins
        .into_iter()
        .filter(|&(origin_x, origin_y)| round_qr_finder_score(warped, modules, origin_x, origin_y))
        .count();

    rounded_finders >= 2
}

fn round_qr_finder_score(
    warped: &BinaryImage,
    modules: usize,
    origin_x: usize,
    origin_y: usize,
) -> bool {
    let corners = [
        (0.20, 0.20, 1.00, 1.00),
        (6.00, 0.20, 6.80, 1.00),
        (0.20, 6.00, 1.00, 6.80),
        (6.00, 6.00, 6.80, 6.80),
    ];
    let sides = [
        (2.45, 0.20, 4.55, 1.00),
        (0.20, 2.45, 1.00, 4.55),
        (6.00, 2.45, 6.80, 4.55),
        (2.45, 6.00, 4.55, 6.80),
    ];
    let centers = [(2.50, 2.50, 4.50, 4.50)];

    let corner_light_ratio =
        average_regions_ratio(warped, modules, origin_x, origin_y, &corners, false);
    let side_dark_ratio = average_regions_ratio(warped, modules, origin_x, origin_y, &sides, true);
    let center_dark_ratio =
        average_regions_ratio(warped, modules, origin_x, origin_y, &centers, true);

    (corner_light_ratio >= 0.75 && side_dark_ratio >= 0.42 && center_dark_ratio >= 0.45)
        || (corner_light_ratio >= 0.92 && center_dark_ratio >= 0.50)
}

fn has_enterprise_wechat_compact_modules(warped: &BinaryImage, finders: &[QrFinder; 3]) -> bool {
    let modules = infer_qr_version(warped)
        .ok()
        .map(qr_modules_for_version)
        .or_else(|| estimate_qr_modules_from_finders(finders));
    let Some(modules) = modules else {
        return false;
    };
    if modules < 21 || warped.w == 0 || warped.h == 0 {
        return false;
    }

    let edge_regions = [
        (0.04, 0.04, 0.20, 0.20),
        (0.80, 0.04, 0.96, 0.20),
        (0.04, 0.80, 0.20, 0.96),
        (0.80, 0.80, 0.96, 0.96),
        (0.32, 0.03, 0.68, 0.13),
        (0.32, 0.87, 0.68, 0.97),
        (0.03, 0.32, 0.13, 0.68),
        (0.87, 0.32, 0.97, 0.68),
    ];

    let mut candidates = 0_u32;
    let mut compact = 0_u32;
    let mut full = 0_u32;

    for y in 0..modules {
        for x in 0..modules {
            if is_enterprise_wechat_detection_ignored_module(modules, x, y) {
                continue;
            }

            let center_dark = module_region_ratio(
                warped,
                modules,
                x as f64 + 0.30,
                y as f64 + 0.30,
                x as f64 + 0.70,
                y as f64 + 0.70,
                true,
            );
            if center_dark < 0.68 {
                continue;
            }

            let edge_light = average_regions_ratio(warped, modules, x, y, &edge_regions, false);
            candidates += 1;
            if edge_light >= 0.64 {
                compact += 1;
            } else if edge_light <= 0.38 {
                full += 1;
            }
        }
    }

    candidates >= 16
        && compact as f64 / candidates as f64 >= 0.25
        && full as f64 / candidates as f64 <= 0.18
        && compact > full.saturating_mul(4)
}

fn is_enterprise_wechat_detection_ignored_module(modules: usize, x: usize, y: usize) -> bool {
    is_qr_finder_or_separator_module(modules, x, y)
        || is_qr_center_logo_module(modules, x, y)
        || qr_alignment_pattern_module(modules, x, y).is_some()
}

fn qr_alignment_pattern_module(modules: usize, x: usize, y: usize) -> Option<bool> {
    let version = qr_version_for_modules(modules)? as usize;
    if version == 1 {
        return None;
    }

    let centers = qr_alignment_pattern_centers(version, modules);
    for &cy in &centers {
        for &cx in &centers {
            if alignment_pattern_overlaps_finder(modules, cx, cy) {
                continue;
            }
            let dx = x.abs_diff(cx);
            let dy = y.abs_diff(cy);
            if dx <= 2 && dy <= 2 {
                let ring = dx.max(dy);
                return Some(ring == 2 || ring == 0);
            }
        }
    }
    None
}

fn qr_alignment_pattern_centers(version: usize, modules: usize) -> Vec<usize> {
    if version == 1 {
        return Vec::new();
    }

    let count = version / 7 + 2;
    let step = if version == 32 {
        26
    } else {
        ((version * 4 + count * 2 + 1) / (count * 2 - 2)) * 2
    };

    let mut centers = vec![0; count];
    centers[0] = 6;
    let mut pos = modules - 7;
    for index in (1..count).rev() {
        centers[index] = pos;
        pos = pos.saturating_sub(step);
    }
    centers
}

fn alignment_pattern_overlaps_finder(modules: usize, cx: usize, cy: usize) -> bool {
    let far = modules - 7;
    (cx == 6 && (cy == 6 || cy == far)) || (cx == far && cy == 6)
}

fn estimate_qr_modules_from_finders(finders: &[QrFinder; 3]) -> Option<usize> {
    let mut modules = [finders[0].module, finders[1].module, finders[2].module];
    modules.sort_by(f64::total_cmp);
    let avg_module = ((modules[0] + modules[1] + modules[2]) / 3.0).max(1.0);

    let mut distances = [
        finder_distance(&finders[0], &finders[1]),
        finder_distance(&finders[0], &finders[2]),
        finder_distance(&finders[1], &finders[2]),
    ];
    distances.sort_by(f64::total_cmp);

    let raw_modules = ((distances[0] + distances[1]) * 0.5 / avg_module + 7.0).round();
    if raw_modules < 21.0 {
        return None;
    }

    let version = ((raw_modules - 21.0) / 4.0).round().clamp(0.0, 39.0) as usize;
    Some(21 + version * 4)
}

fn finder_distance(a: &QrFinder, b: &QrFinder) -> f64 {
    (a.cx - b.cx).hypot(a.cy - b.cy)
}

fn average_regions_ratio(
    warped: &BinaryImage,
    modules: usize,
    origin_x: usize,
    origin_y: usize,
    regions: &[(f64, f64, f64, f64)],
    count_black: bool,
) -> f64 {
    if regions.is_empty() {
        return 0.0;
    }

    let total = regions
        .iter()
        .map(|&(x0, y0, x1, y1)| {
            module_region_ratio(
                warped,
                modules,
                origin_x as f64 + x0,
                origin_y as f64 + y0,
                origin_x as f64 + x1,
                origin_y as f64 + y1,
                count_black,
            )
        })
        .sum::<f64>();
    total / regions.len() as f64
}

fn module_region_ratio(
    warped: &BinaryImage,
    modules: usize,
    module_x0: f64,
    module_y0: f64,
    module_x1: f64,
    module_y1: f64,
    count_black: bool,
) -> f64 {
    const SAMPLES_PER_AXIS: usize = 5;

    if modules == 0 || warped.w == 0 || warped.h == 0 {
        return 0.0;
    }

    let cell_w = warped.w as f64 / modules as f64;
    let cell_h = warped.h as f64 / modules as f64;
    let mut matching = 0_u32;
    let mut total = 0_u32;

    for sample_y in 0..SAMPLES_PER_AXIS {
        let ty = (sample_y as f64 + 0.5) / SAMPLES_PER_AXIS as f64;
        let module_y = module_y0 + (module_y1 - module_y0) * ty;
        for sample_x in 0..SAMPLES_PER_AXIS {
            let tx = (sample_x as f64 + 0.5) / SAMPLES_PER_AXIS as f64;
            let module_x = module_x0 + (module_x1 - module_x0) * tx;
            let x = (module_x * cell_w)
                .round()
                .clamp(0.0, warped.w.saturating_sub(1) as f64) as i32;
            let y = (module_y * cell_h)
                .round()
                .clamp(0.0, warped.h.saturating_sub(1) as f64) as i32;
            if warped.is_black(x, y) == count_black {
                matching += 1;
            }
            total += 1;
        }
    }

    matching as f64 / total.max(1) as f64
}

fn qr_modules_for_version(version: u8) -> usize {
    (version as usize - 1) * 4 + 21
}

fn qr_version_for_modules(modules: usize) -> Option<u8> {
    if !(21..=177).contains(&modules) || !(modules - 21).is_multiple_of(4) {
        return None;
    }
    Some(((modules - 21) / 4 + 1) as u8)
}

struct QrMaskCandidate {
    ecc: QrEcc,
    mask: u8,
    matrix: QrMatrix,
    total_diff: u32,
    outside_ignored_diff: u32,
}

fn choose_matching_qr_mask(
    decoded: &QrDecoded,
    reference_matrix: &QrMatrix,
) -> Option<QrMaskCandidate> {
    let preferred = decoded.original_mask;
    let mut candidates = qr_mask_candidates(decoded, reference_matrix);
    let index = best_matching_qr_mask_index(&candidates, decoded.ecc, preferred)?;
    Some(candidates.swap_remove(index))
}

fn best_matching_qr_mask_index(
    candidates: &[QrMaskCandidate],
    preferred_ecc: QrEcc,
    preferred: Option<u8>,
) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.outside_ignored_diff == 0)
        .min_by_key(|(_, candidate)| {
            (
                candidate.ecc != preferred_ecc,
                preferred != Some(candidate.mask),
                candidate.total_diff,
                candidate.mask,
            )
        })
        .map(|(index, _)| index)
}

fn qr_mask_candidates(decoded: &QrDecoded, reference_matrix: &QrMatrix) -> Vec<QrMaskCandidate> {
    let mut candidates = Vec::new();
    let mut seen_ecc = Vec::new();
    for ecc in [decoded.ecc, QrEcc::L, QrEcc::M, QrEcc::Q, QrEcc::H] {
        if seen_ecc.contains(&ecc) {
            continue;
        }
        seen_ecc.push(ecc);

        let mut decoded = decoded.clone();
        decoded.ecc = ecc;
        for mask in 0..=7 {
            let Ok(matrix) = regenerate_qr(&decoded, mask) else {
                continue;
            };
            let Some(diff) = compute_matrix_diff(reference_matrix, &matrix) else {
                continue;
            };
            let modules = matrix.len();
            candidates.push(QrMaskCandidate {
                ecc,
                mask,
                matrix,
                total_diff: diff.diff_count,
                outside_ignored_diff: qr_diff_outside_ignored_qr_regions(&diff, modules),
            });
        }
    }
    candidates
}

fn qr_diff_outside_ignored_qr_regions(diff: &DiffResult, modules: usize) -> u32 {
    let modules = modules.max(1);
    diff.diff_modules
        .iter()
        .filter(|&&(x, y)| !is_ignored_qr_diff_module(modules, x as usize, y as usize))
        .count() as u32
}

fn is_ignored_qr_diff_module(modules: usize, x: usize, y: usize) -> bool {
    is_qr_center_logo_module(modules, x, y) || is_qr_finder_or_separator_module(modules, x, y)
}

fn is_qr_finder_or_separator_module(modules: usize, x: usize, y: usize) -> bool {
    if modules < 8 {
        return false;
    }
    let near_left = x <= 7;
    let near_top = y <= 7;
    let near_right = x + 8 >= modules;
    let near_bottom = y + 8 >= modules;

    (near_left && near_top) || (near_right && near_top) || (near_left && near_bottom)
}

fn is_qr_center_logo_module(modules: usize, x: usize, y: usize) -> bool {
    if modules == 0 {
        return false;
    }
    let side = (modules / QR_LOGO_IGNORE_RATIO)
        .max(QR_LOGO_IGNORE_MIN_MODULES)
        .min(modules);
    let start = (modules - side) / 2;
    let end = start + side;
    x >= start && x < end && y >= start && y < end
}

fn qr_mask_status_text(
    decoded: &QrDecoded,
    mask: u8,
    matched_mask: Option<u8>,
    diff_count: u32,
    outside_logo_diff: u32,
) -> String {
    let mask_text = if matched_mask == Some(mask) {
        format!("原掩膜 {mask}")
    } else {
        format!("当前掩膜 {mask}，无匹配掩膜（中心外差异 {outside_logo_diff}）")
    };
    format!(
        "已解码 QR：V{} / ECC {} / {}，差异 {diff_count} 个模块（红色=原图有生成图没有，蓝色=原图没有生成图有）",
        decoded.version,
        decoded.ecc.label(),
        mask_text
    )
}

fn qr_preview_for_matrix(
    matrix: &QrMatrix,
    reference_matrix: Option<&QrMatrix>,
    mask_choice: MaskChoice,
    appearance: QrAppearance,
    show_diff_overlay: bool,
    job_id: u64,
) -> (String, DynamicImage, u32) {
    let diff = reference_matrix.and_then(|reference| compute_matrix_diff(reference, matrix));
    let diff_count = diff.as_ref().map(|diff| diff.diff_count).unwrap_or(0);
    let modules = matrix.len().max(1) as u32;
    let scale = (PREVIEW_SIZE / modules).max(2);
    let preview = qr_matrix_to_preview_image(
        matrix,
        appearance,
        diff.as_ref(),
        show_diff_overlay,
        scale,
        0,
    );
    let name = match mask_choice {
        MaskChoice::Mask(mask) => format!("preview-mask-{mask}-{appearance:?}-{job_id}"),
        MaskChoice::GridFallback => format!("preview-grid-fallback-{appearance:?}-{job_id}"),
    };
    (name, preview, diff_count)
}

fn qr_matrix_diff_count(lhs: &QrMatrix, rhs: &QrMatrix) -> Option<usize> {
    if lhs.len() != rhs.len() || lhs.iter().zip(rhs).any(|(l, r)| l.len() != r.len()) {
        return None;
    }

    Some(
        lhs.iter()
            .zip(rhs)
            .map(|(lhs_row, rhs_row)| {
                lhs_row
                    .iter()
                    .zip(rhs_row)
                    .filter(|(lhs, rhs)| lhs != rhs)
                    .count()
            })
            .sum(),
    )
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
        self.poll_background_jobs(ctx);

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
            ui::toolbar::show(ui_, self, ctx);
        });
        self.support_dialog.show(ctx);
        ui::manual_calibration::show(ctx, self);

        // 底部状态栏
        egui::TopBottomPanel::bottom("statusbar").show(ctx, |ui_| {
            ui_.horizontal(|ui_| {
                if let Some(progress) = self.loading_progress() {
                    ui_.spinner();
                    ui_.add(
                        egui::ProgressBar::new(progress)
                            .desired_width(160.0)
                            .text("处理中"),
                    );
                }
                ui_.label(format!("状态：{}", self.status));
            });
        });

        // 中央：左右对比预览
        egui::CentralPanel::default().show(ctx, |ui_| {
            ui::mask_panel::show(ui_, self);
            ui::data_matrix_panel::show(ui_, self);
            ui::wx_panel::show(ui_, self);
            ui::dy_panel::show(ui_, self);
            ui_.separator();
            ui::compare_view::show(ui_, self, ctx);
        });
    }
}

#[cfg(test)]
mod data_matrix_sample_regression_tests {
    use super::*;
    use std::path::Path;

    const STANDARD_SAMPLE: &str = "samples/Data Matrix标准.jpg";
    const PHOTO_SAMPLES: [&str; 5] = [
        "samples/Data Matrix拍照1.jpg",
        "samples/Data Matrix拍照2.jpg",
        "samples/Data Matrix拍照3.jpg",
        "samples/Data Matrix拍照4.jpg",
        "samples/Data Matrix拍照5.jpg",
    ];

    #[test]
    #[ignore]
    fn print_data_matrix_photo_sample_diagnostics() {
        let standard =
            sample_data_matrix_path(Path::new(STANDARD_SAMPLE)).expect("standard sample");
        println!(
            "standard: candidates={} symbol={}x{} score={:.4}",
            standard.candidates, standard.grid.cols, standard.grid.rows, standard.grid.score
        );

        let only_sample = std::env::var("DM_SAMPLE").ok();
        let sample_paths = only_sample
            .as_deref()
            .map(|path| vec![path])
            .unwrap_or_else(|| PHOTO_SAMPLES.to_vec());

        for path in sample_paths {
            let image = image::open(path).expect(path);
            let binary = preprocess(&image);
            let candidates = find_data_matrix_candidates(&binary);
            println!("{path}: raw_candidates={}", candidates.len());
            for (idx, candidate) in candidates.iter().take(12).enumerate() {
                let width = ((candidate.corners[0].0 - candidate.corners[1].0)
                    .hypot(candidate.corners[0].1 - candidate.corners[1].1)
                    + (candidate.corners[2].0 - candidate.corners[3].0)
                        .hypot(candidate.corners[2].1 - candidate.corners[3].1))
                    * 0.5;
                let height = ((candidate.corners[0].0 - candidate.corners[2].0)
                    .hypot(candidate.corners[0].1 - candidate.corners[2].1)
                    + (candidate.corners[1].0 - candidate.corners[3].0)
                        .hypot(candidate.corners[1].1 - candidate.corners[3].1))
                    * 0.5;
                println!(
                    "  candidate {idx}: symbol={}x{} score={:.4} size={width:.1}x{height:.1} corners={:?}",
                    candidate.cols(),
                    candidate.rows(),
                    candidate.score,
                    candidate.corners
                );
            }

            match sample_data_matrix_image(&image, candidates.len(), &candidates) {
                Ok(sampled) => {
                    let diff = data_matrix_matrix_diff_count(&standard.grid, &sampled.grid)
                        .map(|count| count.to_string())
                        .unwrap_or_else(|| "symbol-size-mismatch".to_owned());
                    println!(
                        "  sampled: candidates={} symbol={}x{} score={:.4} diff={diff}",
                        sampled.candidates,
                        sampled.grid.cols,
                        sampled.grid.rows,
                        sampled.grid.score
                    );
                    let (_, preview_diff_count) = data_matrix_grid_to_diff_preview_image(
                        &sampled.grid,
                        &sampled.binary,
                        false,
                        PREVIEW_SIZE,
                    );
                    println!("  preview_diff_modules: {preview_diff_count}");
                    if let Some(points) =
                        data_matrix_matrix_diff_points(&standard.grid, &sampled.grid)
                    {
                        if !points.is_empty() {
                            println!("  diff points: {points:?}");
                            for &(x, y) in points.iter().take(12) {
                                println!(
                                    "    ({x},{y}) expected={} actual={}",
                                    standard.grid.matrix[y][x], sampled.grid.matrix[y][x]
                                );
                            }
                        }
                    }
                }
                Err(error) => {
                    println!("{path}: {error}");
                }
            }
        }
    }

    struct SampledDataMatrix {
        candidates: usize,
        grid: DataMatrixGrid,
        binary: BinaryImage,
    }

    fn sample_data_matrix_path(path: &Path) -> std::result::Result<SampledDataMatrix, String> {
        let image = image::open(path).map_err(|error| error.to_string())?;
        let binary = preprocess(&image);
        let candidates = find_data_matrix_candidates(&binary);
        sample_data_matrix_image(&image, candidates.len(), &candidates)
            .map_err(|error| format!("{error} in {}", path.display()))
    }

    fn sample_data_matrix_image(
        image: &DynamicImage,
        candidate_count: usize,
        candidates: &[DataMatrixCandidate],
    ) -> std::result::Result<SampledDataMatrix, String> {
        let processed = best_data_matrix_processing_candidate(image, candidates)
            .ok_or_else(|| "no sampleable Data Matrix candidate".to_owned())?;
        Ok(SampledDataMatrix {
            candidates: candidate_count,
            grid: processed.grid,
            binary: processed.binary,
        })
    }

    fn data_matrix_matrix_diff_count(
        expected: &DataMatrixGrid,
        actual: &DataMatrixGrid,
    ) -> Option<usize> {
        if expected.rows != actual.rows || expected.cols != actual.cols {
            return None;
        }
        Some(
            expected
                .matrix
                .iter()
                .zip(&actual.matrix)
                .flat_map(|(expected_row, actual_row)| expected_row.iter().zip(actual_row))
                .filter(|(expected, actual)| expected != actual)
                .count(),
        )
    }

    fn data_matrix_matrix_diff_points(
        expected: &DataMatrixGrid,
        actual: &DataMatrixGrid,
    ) -> Option<Vec<(usize, usize)>> {
        if expected.rows != actual.rows || expected.cols != actual.cols {
            return None;
        }
        let mut points = Vec::new();
        for y in 0..expected.rows {
            for x in 0..expected.cols {
                if expected.matrix[y][x] != actual.matrix[y][x] {
                    points.push((x, y));
                }
            }
        }
        Some(points)
    }
}
