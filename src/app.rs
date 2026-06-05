// QRacerApp：应用状态 + egui 主循环回调。
//
// immediate-mode 的核心思想：
//   - 每一帧（约 60fps，空闲时按需）调用 update()
//   - 在 update() 里描述"这一帧的界面长什么样"
//   - 按钮的 .clicked() 返回 bool，用 if 立即处理
//   - 状态变化（粘贴图、选文件）只改 self 的字段，下一帧 UI 自动跟着变

use crate::code_kind::CodeKind;
use crate::codec::dy_grid::{DyGrid, detect_dy_params, sample_dy, sample_dy_with_logos};
use crate::codec::qr::{QrDecoded, QrMatrix, decode_qr, regenerate_qr};
use crate::codec::qr_grid::{infer_qr_version, sample_qr_grid};
use crate::codec::wx_grid::{WxGrid, detect_wx_version, sample_wx, sample_wx_with_badge};
use crate::detect;
use crate::detect::finder_dy::{find_dy_finders, select_dy_finders_raw};
use crate::detect::finder_qr::{QrFinder, find_qr_finders, select_qr_finder_triplet};
use crate::detect::finder_wx::{
    find_wx_finders, select_wx_finders_raw, select_wx_finders_raw_with_badge,
};
use crate::image_io;
use crate::pipeline::perspective::{
    WxUprightAnchor, detect_dy_badge_anchor, detect_wx_badge_anchor, dy_upright_target_finders,
    warp_dy_to_upright_binary, warp_dy_to_upright_image_with_top_right, warp_qr_to_square_image,
    warp_wx_to_upright_binary, warp_wx_to_upright_image, wx_upright_target_finders,
};
use crate::pipeline::preprocess::{BinaryImage, preprocess};
use crate::screen_capture;
use crate::ui;
use crate::vector::diff::{DiffResult, compute_matrix_diff, render_qr_diff_preview};
use crate::vector::svg::{
    dy_grid_to_diff_preview_image, dy_grid_to_preview_image, dy_grid_to_svg, qr_matrix_to_svg,
    wx_grid_to_diff_preview_image, wx_grid_to_preview_image, wx_grid_to_svg,
};
use eframe::egui;
use image::DynamicImage;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

const PREVIEW_SIZE: u32 = 1024;
const QR_GRID_REFERENCE_SNAP_RATIO: usize = 10;
const QR_LOGO_IGNORE_RATIO: usize = 3;
const QR_LOGO_IGNORE_MIN_MODULES: usize = 9;
const LOADING_REPAINT_INTERVAL: Duration = Duration::from_millis(50);
const SCREEN_CAPTURE_HIDE_DELAY: Duration = Duration::from_millis(180);

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
    /// Stage 3 QR matrix sampled from the corrected image for preview diffs.
    pub qr_reference_matrix: Option<QrMatrix>,
    /// QR mask that matches outside the center logo area.
    pub matched_mask: Option<u8>,
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
    last_decoded: Option<QrDecoded>,
    qr_version: Option<u8>,
    last_matrix: Option<QrMatrix>,
    qr_reference_matrix: Option<QrMatrix>,
    matched_mask: Option<u8>,
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
            status: String::from("粘贴截图（Ctrl+V）或点击 [打开...] 开始"),
            binary: None,
            finders: None,
            warped: None,
            mask_choice: MaskChoice::Mask(0),
            last_decoded: None,
            qr_version: None,
            last_matrix: None,
            qr_reference_matrix: None,
            matched_mask: None,
            last_wx_grid: None,
            last_dy_grid: None,
            last_svg: None,
            last_diff_count: None,
            show_diff_overlay: true,
            paste_shortcut_was_down: false,
            processing_job: None,
            capture_job: None,
            next_job_id: 0,
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

        self.processing_job = None;
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
        self.last_decoded = None;
        self.qr_version = None;
        self.last_matrix = None;
        self.qr_reference_matrix = None;
        self.matched_mask = None;
        self.last_wx_grid = None;
        self.last_dy_grid = None;
        self.last_svg = None;
        self.last_diff_count = None;
        self.status = format!("{source_label}已载入，正在识别和校正...");

        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = process_image(img, job_id, show_diff_overlay);
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
        let svg = qr_matrix_to_svg(&candidate.matrix, 1.0);
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
                let svg = qr_matrix_to_svg(&matrix, 1.0);
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

    pub fn can_copy_vector(&self) -> bool {
        self.last_svg.is_some()
    }

    pub fn try_copy_vector(&mut self) {
        let Some(svg) = self.last_svg.clone() else {
            self.status = String::from("没有可复制的 SVG");
            return;
        };

        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(svg)) {
            Ok(()) => self.status = String::from("已复制 SVG 代码到剪贴板"),
            Err(error) => self.status = format!("复制 SVG 失败：{error}"),
        }
    }

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

    fn process_dy(&mut self, binary: &BinaryImage, source: Option<&DynamicImage>) {
        let finders = find_dy_finders(binary);
        let Some(raw_selected) = select_dy_finders_raw(&finders) else {
            self.status = format!(
                "已识别抖音码，但无法从 {} 个候选中选出三同心圆定位点",
                finders.len()
            );
            return;
        };

        let correction_size = source
            .map(|source| source.width().max(source.height()))
            .unwrap_or_else(|| binary.w.max(binary.h))
            .clamp(PREVIEW_SIZE, 1600);
        let top_right = source
            .and_then(|source| detect_dy_badge_anchor(source, &raw_selected))
            .map(|badge| (badge.cx, badge.cy));
        let corrected_source = source.map(|source| {
            warp_dy_to_upright_image_with_top_right(
                source,
                &raw_selected,
                top_right,
                correction_size,
            )
        });
        let corrected_binary = corrected_source
            .as_ref()
            .map(preprocess)
            .unwrap_or_else(|| warp_dy_to_upright_binary(binary, &raw_selected, correction_size));
        let selected = dy_upright_target_finders(&raw_selected, correction_size);
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
                let svg = qr_matrix_to_svg(&matrix, 1.0);
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

    fn set_generated_artifacts(&mut self, matrix: QrMatrix, svg: String) {
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
        self.last_dy_grid = None;
        self.last_wx_grid = Some(grid);
        self.last_svg = Some(svg);
        self.refresh_wx_preview();
    }

    fn set_dy_artifacts(&mut self, grid: DyGrid, svg: String) {
        self.last_matrix = None;
        self.qr_reference_matrix = None;
        self.matched_mask = None;
        self.last_wx_grid = None;
        self.last_dy_grid = Some(grid);
        self.last_svg = Some(svg);
        self.refresh_dy_preview();
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
        self.last_decoded = result.last_decoded;
        self.qr_version = result.qr_version;
        self.last_matrix = result.last_matrix;
        self.qr_reference_matrix = result.qr_reference_matrix;
        self.matched_mask = result.matched_mask;
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

fn process_image(img: DynamicImage, job_id: u64, show_diff_overlay: bool) -> ProcessResult {
    let binary = preprocess(&img);
    let code_kind = detect::detect_kind_with_image(&img, &binary);
    let mut result = ProcessResult {
        code_kind,
        status: String::from("图像已加载；未识别到支持的码类型"),
        binary: Some(binary.clone()),
        finders: None,
        warped: None,
        mask_choice: MaskChoice::Mask(0),
        last_decoded: None,
        qr_version: None,
        last_matrix: None,
        qr_reference_matrix: None,
        matched_mask: None,
        last_wx_grid: None,
        last_dy_grid: None,
        last_svg: None,
        last_diff_count: None,
        preview: None,
    };

    match code_kind {
        CodeKind::Qr => process_qr_image(&img, &binary, job_id, show_diff_overlay, &mut result),
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
            result.last_decoded = Some(decoded.clone());
            let reference_matrix = sample_qr_grid(&warped, decoded.version).ok();
            result.qr_reference_matrix = reference_matrix.clone();

            if let Some(candidate) = reference_matrix
                .as_ref()
                .and_then(|reference| choose_matching_qr_mask(&decoded, reference))
            {
                let mask = candidate.mask;
                result.mask_choice = MaskChoice::Mask(mask);
                result.matched_mask = Some(mask);
                let svg = qr_matrix_to_svg(&candidate.matrix, 1.0);
                let (preview_name, preview, diff_count) = qr_preview_for_matrix(
                    &candidate.matrix,
                    reference_matrix.as_ref(),
                    result.mask_choice,
                    show_diff_overlay,
                    job_id,
                );
                result.last_matrix = Some(candidate.matrix);
                result.last_svg = Some(svg);
                result.last_diff_count = Some(diff_count);
                result.preview = Some((preview_name, preview));
                result.status = qr_mask_status_text(&decoded, mask, Some(mask), diff_count, 0);
            } else {
                result.matched_mask = None;
                result.mask_choice = MaskChoice::GridFallback;
                match reference_matrix {
                    Some(matrix) => {
                        let svg = qr_matrix_to_svg(&matrix, 1.0);
                        let (preview_name, preview, diff_count) = qr_preview_for_matrix(
                            &matrix,
                            result.qr_reference_matrix.as_ref(),
                            result.mask_choice,
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

    let correction_size = img.width().max(img.height()).clamp(PREVIEW_SIZE, 1600);
    let top_right = detect_dy_badge_anchor(img, &raw_selected).map(|badge| (badge.cx, badge.cy));
    let corrected_source =
        warp_dy_to_upright_image_with_top_right(img, &raw_selected, top_right, correction_size);
    let corrected_binary = preprocess(&corrected_source);
    let selected = dy_upright_target_finders(&raw_selected, correction_size);
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

struct QrMaskCandidate {
    mask: u8,
    matrix: QrMatrix,
    total_diff: u32,
    outside_logo_diff: u32,
}

fn choose_matching_qr_mask(
    decoded: &QrDecoded,
    reference_matrix: &QrMatrix,
) -> Option<QrMaskCandidate> {
    let preferred = decoded.original_mask;
    let mut candidates = qr_mask_candidates(decoded, reference_matrix);
    let index = best_matching_qr_mask_index(&candidates, preferred)?;
    Some(candidates.swap_remove(index))
}

fn best_matching_qr_mask_index(
    candidates: &[QrMaskCandidate],
    preferred: Option<u8>,
) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.outside_logo_diff == 0)
        .min_by_key(|(_, candidate)| {
            (
                preferred != Some(candidate.mask),
                candidate.total_diff,
                candidate.mask,
            )
        })
        .map(|(index, _)| index)
}

fn qr_mask_candidates(decoded: &QrDecoded, reference_matrix: &QrMatrix) -> Vec<QrMaskCandidate> {
    (0..=7)
        .filter_map(|mask| {
            let matrix = regenerate_qr(decoded, mask).ok()?;
            let diff = compute_matrix_diff(reference_matrix, &matrix)?;
            let modules = matrix.len();
            Some(QrMaskCandidate {
                mask,
                matrix,
                total_diff: diff.diff_count,
                outside_logo_diff: qr_diff_outside_center_logo(&diff, modules),
            })
        })
        .collect()
}

fn qr_diff_outside_center_logo(diff: &DiffResult, modules: usize) -> u32 {
    let modules = modules.max(1);
    diff.diff_modules
        .iter()
        .filter(|&&(x, y)| !is_qr_center_logo_module(modules, x as usize, y as usize))
        .count() as u32
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
    show_diff_overlay: bool,
    job_id: u64,
) -> (String, DynamicImage, u32) {
    let diff = reference_matrix.and_then(|reference| compute_matrix_diff(reference, matrix));
    let diff_count = diff.as_ref().map(|diff| diff.diff_count).unwrap_or(0);
    let modules = matrix.len().max(1) as u32;
    let scale = (PREVIEW_SIZE / modules).max(2);
    let preview = render_qr_diff_preview(matrix, diff.as_ref(), show_diff_overlay, scale, 0);
    let name = match mask_choice {
        MaskChoice::Mask(mask) => format!("preview-mask-{mask}-{job_id}"),
        MaskChoice::GridFallback => format!("preview-grid-fallback-{job_id}"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_mask_matching_ignores_center_logo_region() {
        let diff = DiffResult {
            diff_modules: vec![(10, 10), (11, 10), (10, 11)],
            missing_in_generated: Vec::new(),
            extra_in_generated: Vec::new(),
            diff_count: 3,
        };

        assert_eq!(qr_diff_outside_center_logo(&diff, 29), 0);
    }

    #[test]
    fn qr_mask_matching_rejects_non_center_differences() {
        let diff = DiffResult {
            diff_modules: vec![(10, 10), (2, 18)],
            missing_in_generated: Vec::new(),
            extra_in_generated: Vec::new(),
            diff_count: 2,
        };

        assert_eq!(qr_diff_outside_center_logo(&diff, 29), 1);
    }
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
            ui::wx_panel::show(ui_, self);
            ui::dy_panel::show(ui_, self);
            ui_.separator();
            ui::compare_view::show(ui_, self, ctx);
        });
    }
}
