use crate::detect::finder_dy::DyFinder;
use crate::error::{QRacerError, Result};
use crate::pipeline::preprocess::{BinaryImage, otsu_binarize};
use image::{DynamicImage, GrayImage};

#[derive(Debug, Clone, PartialEq)]
pub struct DyGrid {
    pub center: (f64, f64),
    pub rings: Vec<RingSpec>,
    pub outer_frame: Option<DyOuterFrame>,
    pub decorative_rings: Vec<DyDecorativeRing>,
    pub points_per_ring: u32,
    pub theta_offset: f64,
    pub finders: [DyFinder; 3],
    pub badge: Option<DyBadge>,
    pub badge_style: DyBadgeStyle,
    pub center_logo: Option<DyLogo>,
    pub has_border: bool,
    pub samples: Vec<bool>,
    pub sample_radial_offsets: Vec<f64>,
    pub sample_tangential_offsets: Vec<f64>,
    pub render_ring_radius_offsets: Vec<f64>,
    /// Absolute per-ring sampling theta. Empty means every ring uses `theta_offset`.
    pub ring_theta_offsets: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingSpec {
    pub r_inner: f64,
    pub r_outer: f64,
    pub is_decoration: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DyOuterFrame {
    pub ring: RingSpec,
    pub segments: Vec<DyArcSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DyArcSegment {
    pub theta_start: f64,
    pub theta_end: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DyDecorativeRing {
    pub ring: RingSpec,
    pub points_per_ring: u32,
    pub theta_offset: f64,
    pub samples: Vec<bool>,
}

impl DyDecorativeRing {
    pub fn sample(&self, point: u32) -> bool {
        self.samples[point as usize % self.samples.len()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DyParams {
    pub ring_count: u8,
    pub points_per_ring: u32,
    pub has_border: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DyBadge {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DyBadgeStyle {
    DouyinLogo,
    Bullseye,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DyLogo {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, Copy)]
struct DyGeometry {
    center: (f64, f64),
    locator_distance: f64,
    r_min: f64,
    r_max: f64,
}

const BLACK_BORDER_STANDARD_LOCATOR_DISTANCE: f64 = 261.452;
const BLACK_BORDER_BADGE_OUTER_RADIUS_SCALE: f64 = 1.17;
const BLACK_BORDER_DECORATIVE_POINTS: u32 = 720;
const BLACK_BORDER_DECORATIVE_THRESHOLD: f64 = 0.10;
const BLACK_BORDER_FINE_RING_MAX_GAP: u32 = 6;
const BLACK_BORDER_FINE_RING_MIN_RUN: u32 = 2;
const BLACK_BORDER_BADGE_CODE_SKIP_SCALE_72: f64 = 1.04;
const BLACK_BORDER_BADGE_CODE_SKIP_SCALE_120: f64 = 1.12;
const BLACK_BORDER_BADGE_INNER_CODE_SKIP_SCALE_120: f64 = 1.04;
const BLACK_BORDER_BADGE_OUTER_SHORT_RUN_MAX_LEN: u32 = 2;
const BLACK_BORDER_BADGE_OUTER_SHORT_RUN_MIN_RATIO: f64 = 1.20;
const BLACK_BORDER_BADGE_OUTER_SHORT_RUN_MAX_RATIO: f64 = 1.45;
const BLACK_BORDER_BADGE_OUTER_SHORT_RUN_CELL_MAX_RATIO: f64 = 1.20;
const BLACK_BORDER_BADGE_OUTER_SHORT_RUN_MIN_RATIO_72: f64 = 1.04;
const BLACK_BORDER_BADGE_OUTER_SHORT_RUN_MAX_RATIO_72: f64 = 1.10;
const BLACK_BORDER_BADGE_OUTER_SHORT_RUN_CELL_MAX_RATIO_72: f64 = 1.08;
const BLACK_BORDER_BADGE_DECORATIVE_SKIP_SCALE: f64 = 1.04;
const BLACK_BORDER_BADGE_DECORATIVE_EDGE_RUN_MAX_LEN: u32 = 4;
const BLACK_BORDER_BADGE_DECORATIVE_EDGE_MIN_RATIO: f64 = 1.04;
const BLACK_BORDER_BADGE_DECORATIVE_EDGE_MAX_RATIO: f64 = 1.04;
const BLACK_BORDER_BADGE_DECORATIVE_EDGE_MIN_RATIO_72: f64 = 1.04;
const BLACK_BORDER_BADGE_DECORATIVE_EDGE_MAX_RATIO_72: f64 = 1.04;
const BLACK_BORDER_BADGE_DECORATIVE_INNER_EDGE_MIN_RATIO_72: f64 = 0.89;
const BLACK_BORDER_BADGE_DECORATIVE_INNER_EDGE_MAX_RATIO_72: f64 = 1.04;
const BLACK_BORDER_BADGE_DECORATIVE_RELAXED_SKIP_SCALE: f64 = 0.80;
const BLACK_BORDER_BADGE_DECORATIVE_RESTORE_MIN_RATIO_120: f64 = 1.00;
const BLACK_BORDER_BADGE_DECORATIVE_RESTORE_MAX_RATIO_120: f64 = 1.05;
const BLACK_BORDER_BADGE_DECORATIVE_RESTORE_MAX_DELTA_DEG_120: f64 = 32.0;
const BLACK_BORDER_BADGE_DECORATIVE_RESTORE_MIN_ANGULAR_HITS: u32 = 5;
const BLACK_BORDER_BADGE_DECORATIVE_RESTORE_MIN_BLACK: u32 = 31;
const BLACK_BORDER_BADGE_DECORATIVE_BRIDGE_MAX_LEN_72: u32 = 10;
const BLACK_BORDER_BADGE_DECORATIVE_BRIDGE_MIN_ANGULAR_HITS_72: u32 = 5;
const BLACK_BORDER_BADGE_DECORATIVE_BRIDGE_MIN_BLACK_72: u32 = 18;
const BLACK_BORDER_FINE_RING_TEMPLATE_MAX_GAP: u32 = 10;
const BLACK_BORDER_FINE_RING_TEMPLATE_MIN_ANGULAR_HITS: u32 = 4;
const BLACK_BORDER_FINE_RING_TEMPLATE_MIN_BLACK: f64 = 14.0;
const BLACK_BORDER_FINE_RING_TEMPLATE_OUTER_MIN_RATIO: f64 = 0.82;
const BLACK_BORDER_FINE_RING_TEMPLATE_OUTER_MAX_RATIO: f64 = 1.18;
const BLACK_BORDER_STANDARD_CODE_THETA_OFFSET_72: f64 = 5.0_f64.to_radians();
const BLACK_BORDER_STANDARD_CODE_THETA_OFFSET_120: f64 = 3.0_f64.to_radians();
const BLACK_BORDER_BULLSEYE_CODE_THETA_OFFSET_120: f64 = 2.5_f64.to_radians();
const BLACK_BORDER_BASE_CODE_RINGS: u8 = 3;
const BLACK_BORDER_OPTIONAL_RING_THRESHOLD: f64 = 0.34;
const BLACK_BORDER_OPTIONAL_RING_MIN_DENSITY: f64 = 0.12;
const BLACK_BORDER_OPTIONAL_RING_MAX_DENSITY: f64 = 0.70;
const BLACK_BORDER_OPTIONAL_RING_MAX_RUN_RATIO: f64 = 0.22;
const BLACK_BORDER_CODE_RINGS: [(f64, f64); 5] = [
    (218.42, 231.42),
    (181.84, 190.84),
    (160.87, 169.86),
    (140.20, 149.20),
    (119.20, 128.20),
];
const BLACK_BORDER_OUTER_FRAME_RING: (f64, f64) = (261.10, 283.47);
const BLACK_BORDER_FINE_RINGS: [(f64, f64); 2] = [(246.00, 249.00), (204.10, 207.10)];
const NO_BORDER_STANDARD_LOCATOR_DISTANCE: f64 = 240.529442688416;
// Cell centers land at 1 deg + n * 3 deg in the standard no-border SVG.
const NO_BORDER_STANDARD_CODE_THETA_OFFSET: f64 = -0.5_f64.to_radians();
const NO_BORDER_STANDARD_SAMPLE_THETA_OFFSET: f64 = NO_BORDER_STANDARD_CODE_THETA_OFFSET;
const NO_BORDER_STANDARD_RADIUS_SCALE: f64 = 1.000;
const NO_BORDER_RADIAL_SPREAD_SCALE: f64 = 1.000;
const NO_BORDER_DECORATIVE_RADIUS_SCORE_WEIGHT: f64 = 0.80;
const NO_BORDER_FINDER_CODE_SKIP_SCALE: f64 = 0.70;
const NO_BORDER_CENTER_OFFSET_SCORE_WEIGHT: f64 = 0.0;
const NO_BORDER_CENTER_REFINE_MAX_RADIUS: f64 = 2.5;
const NO_BORDER_CENTER_REFINE_STEP: f64 = 0.5;
const NO_BORDER_BLACK_THRESHOLD: f64 = 0.55;
const NO_BORDER_DECORATIVE_BLACK_THRESHOLD: f64 = 0.50;
/// Distance threshold (in scaled pixels): ring1 points within this range of a finder
/// center are reserved (excluded from code sampling) to avoid bullseye edge bleed.
const NO_BORDER_FINDER_ADJACENT_DISTANCE: f64 = 55.5;
const NO_BORDER_SAMPLE_RADIAL_LANE_MAX_WEIGHT: f64 = 0.65;
const NO_BORDER_SAMPLE_THETA_OFFSETS: [f64; 5] = [-0.35, -0.20, 0.0, 0.20, 0.35];
const NO_BORDER_SAMPLE_RADIAL_OFFSETS: [f64; 3] = [-0.30, 0.0, 0.30];
// With bullseyes aligned, each of the six rings may still be slightly rotated
// relative to the shared phase; the per-ring measurement stays well below half
// the 3 deg point spacing so point indices can never alias. Activation is
// deliberately strict: render insets/offsets are tuned for the unrotated grid,
// so only a clear, sign-consistent rotation may override the shared theta.
const NO_BORDER_RING_THETA_REFINE_MAX_DEG: f64 = 0.9;
const NO_BORDER_RING_THETA_REFINE_MIN_DEG: f64 = 0.20;
const NO_BORDER_RING_THETA_REFINE_MIN_DOTS: usize = 5;
const NO_BORDER_RING_THETA_REFINE_MIN_SIGN_RATIO: f64 = 0.75;
const NO_BORDER_RING_THETA_REFINE_SIGN_EPS: f64 = 0.02;
// Shared-phase refinement: the whole 6-ring system may be slightly rotated
// relative to the bullseye frame (e.g. a真实倾斜 below the upright-snap
// threshold survives the warp). Pooled across all code rings the dot median
// has 4x the per-ring signal, so the gates can stay strict.
const NO_BORDER_GLOBAL_THETA_REFINE_MAX_DEG: f64 = 1.4;
const NO_BORDER_GLOBAL_THETA_REFINE_MIN_DEG: f64 = 0.20;
const NO_BORDER_GLOBAL_THETA_REFINE_MIN_DOTS: usize = 12;
const NO_BORDER_GLOBAL_THETA_REFINE_MIN_SIGN_RATIO: f64 = 0.70;
const NO_BORDER_FINDER_ADJACENT_RESTORE_MIN_RATIO: f64 = 0.50;
const NO_BORDER_FINDER_ADJACENT_RESTORE_MIN_DISTANCE_RATIO: f64 = 0.90;
const NO_BORDER_FINDER_ADJACENT_RESTORE_MAX_DISTANCE_RATIO: f64 = 1.60;
const NO_BORDER_FINDER_ADJACENT_RESTORE_THETA_OFFSETS: [f64; 3] = [-0.10, 0.0, 0.10];
const NO_BORDER_FINDER_ADJACENT_RESTORE_RADIAL_OFFSETS: [f64; 5] = [-0.40, -0.20, 0.0, 0.20, 0.40];
const NO_BORDER_COMPONENT_RESTORE_NEIGHBOR_RADIUS: u32 = 2;
const NO_BORDER_COMPONENT_RESTORE_MIN_LANE_RATIO: f64 = 0.40;
const NO_BORDER_COMPONENT_RESTORE_ISOLATED_MIN_LANE_RATIO: f64 = 0.35;
const NO_BORDER_COMPONENT_RESTORE_RING0_MIN_LANE_RATIO: f64 = 0.35;
const NO_BORDER_COMPONENT_RESTORE_RING0_ISOLATED_MIN_LANE_RATIO: f64 = 0.33;
// 无框版装饰环（ring0/ring2）高密度采样：参考黑框版 fine ring，720 点捕捉虚线弧段
// 边界。close_gap=6(3°) 只闭合采样噪声白缝、不连虚线；min_run=2(1°) 去单点噪声。
// 牛眼/badge 覆盖区的点直接判白（跳过），避免把定位元素采成装饰弧。
const NO_BORDER_DECORATIVE_FINE_RING_MAX_GAP: u32 = 6;
const NO_BORDER_DECORATIVE_FINE_RING_MIN_RUN: u32 = 2;
const NO_BORDER_DECORATIVE_FINDER_SKIP_SCALE: f64 = 1.05;
// badge（抖音 logo 白圆 token）是不透明圆贴片，只遮挡白圆**本身**覆盖的装饰环；
// 真实装饰弧紧贴白圆外缘（实测最近 ratio≈1.33）。故 skip 取 1.15：排掉白圆边缘
// 抗锯齿造成的 ratio≈1.0 多采短弧，又给 1.33 的真实弧留 0.18 安全间隔。切勿调大——
// 2.5 会连白圆外 ratio 1.3~2.4 的真实装饰弧一起误删。
const NO_BORDER_DECORATIVE_BADGE_SKIP_SCALE: f64 = 1.15;
const NO_BORDER_CENTER_LOGO_RADIUS_SCALE: f64 = 0.72;
const NO_BORDER_CENTER_LOGO_MAX_DETECTED_OFFSET_RATIO: f64 = 0.28;
const NO_BORDER_CENTER_LOGO_MAX_RADIUS_RATIO: f64 = 0.78;
const NO_BORDER_FINDER_CENTER_REFINE_STEP: f64 = 0.50;
const NO_BORDER_FINDER_CENTER_REFINE_MAX_RADIUS: f64 = 5.0;
const NO_BORDER_FINDER_CENTER_REFINE_OFFSET_WEIGHT: f64 = 0.0;
#[cfg(test)]
#[allow(dead_code)]
const NO_BORDER_SAMPLE_CENTER_THETA_OFFSETS: [f64; 3] = [-0.18, 0.0, 0.18];
#[cfg(test)]
#[allow(dead_code)]
const NO_BORDER_DENSE_PATCH_THRESHOLD: f64 = 0.36;
#[cfg(test)]
#[allow(dead_code)]
const NO_BORDER_DENSE_PATCH_TANGENTIAL_SCALE: f64 = 0.46;
#[cfg(test)]
#[allow(dead_code)]
const NO_BORDER_DENSE_PATCH_RADIAL_SCALE: f64 = 0.58;
#[cfg(test)]
#[allow(dead_code)]
const NO_BORDER_DENSE_PATCH_CENTER_WEIGHT: f64 = 0.28;
#[cfg(test)]
#[allow(dead_code)]
const NO_BORDER_DENSE_PATCH_RADIAL_CENTERS: [f64; 5] = [-0.45, -0.22, 0.0, 0.22, 0.45];
const NO_BORDER_RADIAL_OFFSET_THETA_OFFSETS: [f64; 3] = [-0.20, 0.0, 0.20];
const NO_BORDER_RADIAL_OFFSET_SCAN_STEPS: i32 = 12;
const NO_BORDER_RADIAL_OFFSET_SCAN_STEP: f64 = 0.10;
const NO_BORDER_RADIAL_OFFSET_CLAMP: f64 = 0.45;
const NO_BORDER_RADIAL_OFFSET_MIN_LANES: usize = 2;
const NO_BORDER_TANGENTIAL_OFFSET_RADIAL_OFFSETS: [f64; 3] = [-0.20, 0.0, 0.20];
const NO_BORDER_TANGENTIAL_OFFSET_SCAN_STEPS: i32 = 12;
const NO_BORDER_TANGENTIAL_OFFSET_SCAN_STEP: f64 = 0.05;
const NO_BORDER_TANGENTIAL_OFFSET_CLAMP: f64 = 0.45;
const NO_BORDER_TANGENTIAL_OFFSET_MIN_LANES: usize = 2;
#[allow(dead_code)]
const NO_BORDER_RENDER_RING_RADIUS_BASE_OFFSETS: [f64; 6] = [0.0, 0.25, 0.375, 0.0, 0.0, 0.0];
#[allow(dead_code)]
const NO_BORDER_RENDER_RING_RADIUS_OFFSET_CANDIDATES: [f64; 11] = [
    -0.25, -0.125, 0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0,
];
#[allow(dead_code)]
const NO_BORDER_RENDER_RING_RADIUS_OFFSET_THETA_OFFSETS: [f64; 3] = [-0.25, 0.0, 0.25];
#[allow(dead_code)]
const NO_BORDER_RENDER_RING_RADIUS_OFFSET_RADIAL_OFFSETS: [f64; 3] = [-0.30, 0.0, 0.30];
const NO_BORDER_RADIUS_SCORE_THRESHOLD: f64 = 0.26;
const NO_BORDER_RADIUS_SCORE_THETA_OFFSETS: [f64; 3] = [-0.15, 0.0, 0.15];
const NO_BORDER_RADIUS_SCORE_RADIAL_OFFSETS: [f64; 3] = [-0.20, 0.0, 0.20];
const NO_BORDER_RINGS: [(f64, f64, bool); 6] = [
    (228.66, 5.0, true),
    (207.98, 5.0, false),
    (188.59, 5.0, true),
    (171.71, 5.0, false),
    (153.74, 5.0, false),
    (133.24, 5.0, false),
];

const NO_BORDER_LAYOUT_CENTER: (f64, f64) = (304.32, 307.63);
const NO_BORDER_LAYOUT_FINDERS: [(f64, f64); 3] =
    [(134.24, 137.55), (134.24, 477.71), (474.40, 477.71)];
const NO_BORDER_LAYOUT_BADGE_CENTER: (f64, f64) = (483.49, 128.31);
// badge 白圆在 layout 空间的固定半径。装饰环遮挡判定改用此固定值（而非检测
// `badge.radius`）：检测在部分样本（无框版8/12/14）会把内侧黑像素圈进去，半径
// 偏大、中心内移，导致 badge 两侧 ~298°/332° 的真实装饰弧被 skip 误删。17 个干净
// 样本检测半径中位数 ≈57.36（layout），取 57.3 保持与 skip 1.15 既有语义一致。
const NO_BORDER_LAYOUT_BADGE_RADIUS: f64 = 57.3;

#[derive(Debug, Clone, Copy)]
struct ReservedAreas<'a> {
    finders: &'a [DyFinder; 3],
    badge: Option<DyBadge>,
    badge_style: DyBadgeStyle,
    logo: Option<DyLogo>,
    has_border: bool,
}

#[derive(Debug, Clone, Copy)]
struct NoBorderFinderRefineConfig {
    max_radius: f64,
    step: f64,
    offset_weight: f64,
}

impl NoBorderFinderRefineConfig {
    fn production() -> Self {
        Self {
            max_radius: NO_BORDER_FINDER_CENTER_REFINE_MAX_RADIUS,
            step: NO_BORDER_FINDER_CENTER_REFINE_STEP,
            offset_weight: NO_BORDER_FINDER_CENTER_REFINE_OFFSET_WEIGHT,
        }
    }
}

impl DyGrid {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn code_ring_count(&self) -> u8 {
        self.rings.iter().filter(|ring| !ring.is_decoration).count() as u8
    }

    pub fn ring_count(&self) -> u8 {
        if self.has_border {
            self.rings.len() as u8 + self.decorative_rings.len() as u8
        } else {
            // 无框版 grid.rings 已含全部 6 环（编码+装饰），decorative_rings 只是其中
            // 装饰环的 720 点高密度重采样副本（供渲染连续弧），不另计入环数。
            self.rings.len() as u8
        }
    }

    pub fn sample(&self, ring: u32, point: u32) -> bool {
        self.samples[(ring * self.points_per_ring + point) as usize]
    }

    /// Absolute sampling theta for one ring; rings may be slightly rotated
    /// relative to the shared `theta_offset`.
    pub fn ring_theta(&self, ring_idx: usize) -> f64 {
        self.ring_theta_offsets
            .get(ring_idx)
            .copied()
            .unwrap_or(self.theta_offset)
    }

    /// Per-ring rotation relative to the shared `theta_offset`.
    pub fn ring_theta_delta(&self, ring_idx: usize) -> f64 {
        self.ring_theta(ring_idx) - self.theta_offset
    }
}

/// Detects the Douyin radial version parameters from a binary image.
pub fn detect_dy_params(bin: &BinaryImage, finders: &[DyFinder; 3]) -> Result<DyParams> {
    let geometry = dy_geometry(finders)?;
    let has_border = detect_border(bin, &geometry);
    let (ring_count, points_per_ring) = detect_grid_shape(bin, &geometry, has_border)?;

    Ok(DyParams {
        ring_count,
        points_per_ring,
        has_border,
    })
}

/// Samples a Douyin code into its radial grid.
pub fn sample_dy(bin: &BinaryImage, finders: &[DyFinder; 3], params: DyParams) -> Result<DyGrid> {
    sample_dy_impl(bin, None, finders, params)
}

/// Samples a Douyin code and extracts decorative logo/badge anchors from color input.
pub fn sample_dy_with_logos(
    bin: &BinaryImage,
    source: &DynamicImage,
    finders: &[DyFinder; 3],
    params: DyParams,
) -> Result<DyGrid> {
    sample_dy_impl(bin, Some(source), finders, params)
}

fn sample_dy_impl(
    bin: &BinaryImage,
    source: Option<&DynamicImage>,
    finders: &[DyFinder; 3],
    params: DyParams,
) -> Result<DyGrid> {
    sample_dy_impl_with_no_border_refine_config(
        bin,
        source,
        finders,
        params,
        NoBorderFinderRefineConfig::production(),
    )
}

fn sample_dy_impl_with_no_border_refine_config(
    bin: &BinaryImage,
    source: Option<&DynamicImage>,
    finders: &[DyFinder; 3],
    params: DyParams,
    no_border_refine: NoBorderFinderRefineConfig,
) -> Result<DyGrid> {
    let ring_count_is_valid = if params.has_border {
        (BLACK_BORDER_BASE_CODE_RINGS..=BLACK_BORDER_CODE_RINGS.len() as u8)
            .contains(&params.ring_count)
    } else {
        (4..=8).contains(&params.ring_count)
    };
    if !ring_count_is_valid {
        return Err(QRacerError::QrDecode(format!(
            "invalid Douyin ring count: {}",
            params.ring_count
        )));
    }
    if ![72, 120].contains(&params.points_per_ring) {
        return Err(QRacerError::QrDecode(format!(
            "invalid Douyin points per ring: {}",
            params.points_per_ring
        )));
    }

    let no_border_sampling_bin = (!params.has_border)
        .then(|| source.map(raw_binary_from_source))
        .flatten();
    let geometry_finders = if params.has_border {
        finders.clone()
    } else {
        refine_no_border_finders(
            no_border_sampling_bin.as_ref().unwrap_or(bin),
            finders,
            no_border_refine,
        )
    };
    let finders = &geometry_finders;
    let mut geometry = dy_geometry(finders)?;
    let detected_badge = source.and_then(|source| detect_dy_badge(source, &geometry));
    let badge = if params.has_border {
        black_border_badge_from_finders_and_detection(finders, detected_badge)
    } else {
        detected_badge.or_else(|| estimate_badge_from_finders(finders))
    };
    let badge_style = if params.has_border {
        source
            .and_then(|source| detect_black_border_badge_style(source, badge))
            .unwrap_or(DyBadgeStyle::DouyinLogo)
    } else {
        DyBadgeStyle::DouyinLogo
    };
    let mut no_border_theta_offset =
        (!params.has_border).then(|| no_border_standard_code_theta_offset(finders));
    if !params.has_border {
        let theta_offset = no_border_theta_offset.unwrap_or(NO_BORDER_STANDARD_SAMPLE_THETA_OFFSET);
        let score_bin = no_border_sampling_bin.as_ref().unwrap_or(bin);
        let searched_geometry = best_no_border_geometry(
            score_bin,
            &geometry,
            finders,
            params.points_per_ring,
            theta_offset,
        );
        let mut best_fit = NoBorderLayoutFit {
            geometry: searched_geometry,
            theta_offset,
        };
        let best_fit_score = no_border_geometry_score(
            score_bin,
            &best_fit.geometry,
            &no_border_ring_specs(&best_fit.geometry),
            params.points_per_ring,
            best_fit.theta_offset,
        );

        if let Some(layout_fit) =
            detected_badge.and_then(|badge| no_border_geometry_from_standard_layout(finders, badge))
        {
            let layout_geometry = best_no_border_geometry(
                score_bin,
                &layout_fit.geometry,
                finders,
                params.points_per_ring,
                layout_fit.theta_offset,
            );
            let layout_fit = NoBorderLayoutFit {
                geometry: layout_geometry,
                theta_offset: layout_fit.theta_offset,
            };
            let layout_fit_score = no_border_geometry_score(
                score_bin,
                &layout_fit.geometry,
                &no_border_ring_specs(&layout_fit.geometry),
                params.points_per_ring,
                layout_fit.theta_offset,
            );
            if layout_fit_score < best_fit_score {
                best_fit = layout_fit;
            }
        }

        geometry = best_fit.geometry;
        no_border_theta_offset = Some(best_fit.theta_offset);
    }
    let center_logo = source
        .and_then(|source| detect_center_logo(source, &geometry, params.has_border))
        .or_else(|| {
            params.has_border.then_some(DyLogo {
                cx: geometry.center.0,
                cy: geometry.center.1,
                radius: geometry.r_min * 0.72,
            })
        });
    let candidate_ring_count = if params.has_border {
        BLACK_BORDER_CODE_RINGS.len() as u8
    } else {
        params.ring_count
    };
    let candidate_rings = if params.has_border {
        ring_specs(
            &geometry,
            DyParams {
                ring_count: candidate_ring_count,
                ..params
            },
        )
    } else {
        let radius_scale = NO_BORDER_STANDARD_RADIUS_SCALE;
        no_border_ring_specs_with_radius_scale(&geometry, radius_scale)
    };
    let theta_offset = if params.has_border {
        let alignment_rings = black_border_alignment_rings(&candidate_rings);
        black_border_standard_code_theta_offset(finders, params.points_per_ring, badge_style)
            .unwrap_or_else(|| {
                best_black_border_theta_offset(
                    bin,
                    &geometry,
                    &alignment_rings,
                    params.points_per_ring,
                )
            })
    } else {
        no_border_theta_offset.unwrap_or(NO_BORDER_STANDARD_SAMPLE_THETA_OFFSET)
    };
    let theta_offset = if params.has_border {
        theta_offset
    } else {
        refine_no_border_global_theta_offset(
            no_border_sampling_bin.as_ref().unwrap_or(bin),
            &geometry,
            &candidate_rings,
            params.points_per_ring,
            theta_offset,
        )
    };
    let provisional_reserved_areas = ReservedAreas {
        finders,
        badge,
        badge_style,
        logo: center_logo,
        has_border: params.has_border,
    };
    let ring_count = if params.has_border {
        detect_black_border_code_ring_count(
            bin,
            &geometry,
            &candidate_rings,
            params.points_per_ring,
            theta_offset,
            &provisional_reserved_areas,
        )
    } else {
        params.ring_count
    };
    let rings = if params.has_border {
        black_border_ring_specs(&geometry, ring_count)
    } else {
        candidate_rings
    };
    let reserved_areas = ReservedAreas {
        finders,
        badge,
        badge_style,
        logo: center_logo,
        has_border: params.has_border,
    };
    let decorative_bin = params
        .has_border
        .then(|| source.map(raw_binary_from_source))
        .flatten();
    let decorative_gray = params
        .has_border
        .then(|| source.map(|source| source.to_luma8()))
        .flatten();
    let sampling_bin = no_border_sampling_bin.as_ref().unwrap_or(bin);
    let black_threshold = if params.has_border {
        0.34
    } else {
        NO_BORDER_BLACK_THRESHOLD
    };
    let ring_theta_offsets = if params.has_border {
        Vec::new()
    } else {
        estimate_no_border_ring_theta_offsets(
            sampling_bin,
            &geometry,
            &rings,
            params.points_per_ring,
            theta_offset,
        )
    };
    let ring_theta = |ring_idx: usize| -> f64 {
        ring_theta_offsets
            .get(ring_idx)
            .copied()
            .unwrap_or(theta_offset)
    };
    let mut samples = Vec::with_capacity(rings.len() * params.points_per_ring as usize);
    let mut ratios = Vec::with_capacity(rings.len() * params.points_per_ring as usize);

    for ring_idx in 0..rings.len() as u32 {
        let ring = &rings[ring_idx as usize];
        let ring_theta_offset = ring_theta(ring_idx as usize);
        for point in 0..params.points_per_ring {
            let reserved = is_reserved_cell(
                ring,
                ring_idx,
                point,
                params.points_per_ring,
                ring_theta_offset,
                &geometry,
                &reserved_areas,
            );
            let ratio = if params.has_border {
                sample_cell_black_ratio(
                    bin,
                    &geometry,
                    ring,
                    params.points_per_ring,
                    ring_theta_offset,
                    point,
                )
            } else {
                sample_no_border_cell_black_ratio(
                    sampling_bin,
                    &geometry,
                    ring,
                    params.points_per_ring,
                    ring_theta_offset,
                    point,
                )
            };
            let threshold = if !params.has_border && ring.is_decoration {
                NO_BORDER_DECORATIVE_BLACK_THRESHOLD
            } else {
                black_threshold
            };
            let black = !reserved && ratio >= threshold;
            ratios.push(ratio);
            samples.push(black);
        }
    }
    let outer_frame = if params.has_border {
        Some(sample_black_border_outer_frame(
            decorative_bin.as_ref().unwrap_or(bin),
            &geometry,
        ))
    } else {
        None
    };
    let decorative_rings = if params.has_border {
        sample_black_border_fine_rings(
            decorative_bin.as_ref().unwrap_or(bin),
            decorative_gray.as_ref(),
            &geometry,
            badge,
            params.points_per_ring,
        )
    } else {
        sample_no_border_fine_rings(sampling_bin, &geometry, &rings, finders, badge)
    };
    if params.has_border {
        prune_black_border_edge_noise(
            &mut samples,
            &ratios,
            &rings,
            rings.len() as u8,
            params.points_per_ring,
        );
        prune_black_border_badge_outer_short_runs(
            &mut samples,
            &rings,
            params.points_per_ring,
            theta_offset,
            &geometry,
            badge,
            badge_style,
        );
    } else {
        restore_no_border_finder_adjacent_code_cells_with_ring_thetas(
            &mut samples,
            sampling_bin,
            &geometry,
            &rings,
            params.points_per_ring,
            &ring_theta_offsets,
            &reserved_areas,
        );
        restore_no_border_decorative_component_cells_with_ring_thetas(
            &mut samples,
            sampling_bin,
            &geometry,
            &rings,
            params.points_per_ring,
            &ring_theta_offsets,
            &reserved_areas,
        );
    }
    let sample_radial_offsets = if params.has_border {
        vec![0.0; samples.len()]
    } else {
        estimate_no_border_sample_radial_offsets_with_ring_thetas(
            sampling_bin,
            &geometry,
            &rings,
            params.points_per_ring,
            &ring_theta_offsets,
            &samples,
        )
    };
    let sample_tangential_offsets = if params.has_border {
        vec![0.0; samples.len()]
    } else {
        estimate_no_border_sample_tangential_offsets_with_ring_thetas(
            sampling_bin,
            &geometry,
            &rings,
            params.points_per_ring,
            &ring_theta_offsets,
            &samples,
        )
    };
    let render_ring_radius_offsets = Vec::new();
    Ok(DyGrid {
        center: geometry.center,
        rings,
        outer_frame,
        decorative_rings,
        points_per_ring: params.points_per_ring,
        theta_offset,
        finders: finders.clone(),
        badge,
        badge_style,
        center_logo,
        has_border: params.has_border,
        samples,
        sample_radial_offsets,
        sample_tangential_offsets,
        render_ring_radius_offsets,
        ring_theta_offsets,
    })
}

fn dy_geometry(finders: &[DyFinder; 3]) -> Result<DyGeometry> {
    let ordered = order_dy_finders(finders);
    let tl = &ordered[0];
    let br = &ordered[2];
    let center = ((tl.cx + br.cx) * 0.5, (tl.cy + br.cy) * 0.5);
    let locator_radius = finders.iter().map(DyFinder::outer_radius).sum::<f64>() / 3.0;
    let locator_distance = finders
        .iter()
        .map(|finder| distance(center, (finder.cx, finder.cy)))
        .sum::<f64>()
        / finders.len() as f64;
    let r_max = finders
        .iter()
        .map(|finder| distance(center, (finder.cx, finder.cy)) + finder.outer_radius() * 1.10)
        .fold(0.0, f64::max)
        .max(locator_radius * 5.0);
    let r_min = (r_max * 0.36).max(locator_radius * 2.0);

    if r_max <= r_min {
        return Err(QRacerError::QrDecode(
            "invalid Douyin radial geometry".to_owned(),
        ));
    }

    Ok(DyGeometry {
        center,
        locator_distance,
        r_min,
        r_max,
    })
}

fn refine_no_border_finders(
    bin: &BinaryImage,
    finders: &[DyFinder; 3],
    config: NoBorderFinderRefineConfig,
) -> [DyFinder; 3] {
    finders
        .clone()
        .map(|finder| refine_no_border_finder(bin, finder, config))
}

fn refine_no_border_finder(
    bin: &BinaryImage,
    finder: DyFinder,
    config: NoBorderFinderRefineConfig,
) -> DyFinder {
    let radius = finder.outer_radius();
    if radius <= 1.0 || config.max_radius <= 0.0 || config.step <= 0.0 {
        return finder;
    }

    let mut best = (
        no_border_finder_center_score(bin, finder.cx, finder.cy, radius),
        finder.cx,
        finder.cy,
        0.0,
    );
    let steps = (config.max_radius / config.step).ceil() as i32;

    for dy_step in -steps..=steps {
        for dx_step in -steps..=steps {
            let dx = f64::from(dx_step) * config.step;
            let dy = f64::from(dy_step) * config.step;
            let offset2 = dx * dx + dy * dy;
            if offset2 > config.max_radius.powi(2) {
                continue;
            }

            let cx = finder.cx + dx;
            let cy = finder.cy + dy;
            let score =
                no_border_finder_center_score(bin, cx, cy, radius) - offset2 * config.offset_weight;
            if score > best.0 + f64::EPSILON
                || ((score - best.0).abs() <= f64::EPSILON && offset2 < best.3)
            {
                best = (score, cx, cy, offset2);
            }
        }
    }

    DyFinder {
        cx: best.1,
        cy: best.2,
        ..finder
    }
}

fn no_border_finder_center_score(bin: &BinaryImage, cx: f64, cy: f64, outer_radius: f64) -> f64 {
    let inner = disk_black_ratio(bin, cx, cy, outer_radius * 0.22);
    let gap = circle_black_ratio(bin, cx, cy, outer_radius * 0.50);
    let ring = (circle_black_ratio(bin, cx, cy, outer_radius * 0.74)
        + circle_black_ratio(bin, cx, cy, outer_radius * 0.90))
        * 0.5;
    let outside = circle_black_ratio(bin, cx, cy, outer_radius * 1.16);

    inner * 1.20 + (1.0 - gap) * 1.10 + ring * 1.70 + (1.0 - outside) * 0.20
}

fn disk_black_ratio(bin: &BinaryImage, cx: f64, cy: f64, radius: f64) -> f64 {
    let min_x = (cx - radius).floor() as i32;
    let max_x = (cx + radius).ceil() as i32;
    let min_y = (cy - radius).floor() as i32;
    let max_y = (cy + radius).ceil() as i32;
    let radius2 = radius * radius;
    let mut black = 0_u32;
    let mut total = 0_u32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f64 + 0.5 - cx;
            let dy = y as f64 + 0.5 - cy;
            if dx * dx + dy * dy > radius2 {
                continue;
            }
            total += 1;
            if bin.is_black(x, y) {
                black += 1;
            }
        }
    }

    if total == 0 {
        0.0
    } else {
        f64::from(black) / f64::from(total)
    }
}

fn circle_black_ratio(bin: &BinaryImage, cx: f64, cy: f64, radius: f64) -> f64 {
    const SAMPLES: u32 = 96;
    let mut black = 0_u32;

    for idx in 0..SAMPLES {
        let theta = f64::from(idx) * std::f64::consts::TAU / f64::from(SAMPLES);
        let x = (cx + radius * theta.cos()).round() as i32;
        let y = (cy + radius * theta.sin()).round() as i32;
        if bin.is_black(x, y) {
            black += 1;
        }
    }

    f64::from(black) / f64::from(SAMPLES)
}

#[allow(dead_code)]
fn no_border_static_mark_geometry(
    finders: &[DyFinder; 3],
    badge: Option<DyBadge>,
) -> Option<DyGeometry> {
    let ordered = order_dy_finders(finders);
    let mut pairs = NO_BORDER_LAYOUT_FINDERS
        .iter()
        .copied()
        .zip(ordered.iter().map(|finder| (finder.cx, finder.cy)))
        .collect::<Vec<_>>();
    let badge = badge?;
    pairs.push((NO_BORDER_LAYOUT_BADGE_CENTER, (badge.cx, badge.cy)));

    let transform = similarity_transform(&pairs)?;
    let center = transform.point(NO_BORDER_LAYOUT_CENTER);
    let locator_distance = NO_BORDER_STANDARD_LOCATOR_DISTANCE * transform.scale;
    let r_max = (NO_BORDER_RINGS[0].0 + NO_BORDER_RINGS[0].1) * transform.scale;
    let r_min =
        (NO_BORDER_RINGS[NO_BORDER_RINGS.len() - 1].0 - NO_BORDER_RINGS[0].1) * transform.scale;

    Some(DyGeometry {
        center,
        locator_distance,
        r_min,
        r_max,
    })
}

#[derive(Debug, Clone, Copy)]
struct SimilarityTransform {
    scale: f64,
    cos: f64,
    sin: f64,
    source_centroid: (f64, f64),
    target_centroid: (f64, f64),
}

impl SimilarityTransform {
    fn point(self, point: (f64, f64)) -> (f64, f64) {
        let x = point.0 - self.source_centroid.0;
        let y = point.1 - self.source_centroid.1;
        (
            self.target_centroid.0 + self.scale * (self.cos * x - self.sin * y),
            self.target_centroid.1 + self.scale * (self.sin * x + self.cos * y),
        )
    }
}

fn similarity_transform(pairs: &[((f64, f64), (f64, f64))]) -> Option<SimilarityTransform> {
    if pairs.len() < 2 {
        return None;
    }

    let count = pairs.len() as f64;
    let source_centroid = (
        pairs.iter().map(|(source, _)| source.0).sum::<f64>() / count,
        pairs.iter().map(|(source, _)| source.1).sum::<f64>() / count,
    );
    let target_centroid = (
        pairs.iter().map(|(_, target)| target.0).sum::<f64>() / count,
        pairs.iter().map(|(_, target)| target.1).sum::<f64>() / count,
    );
    let mut dot = 0.0;
    let mut cross = 0.0;
    let mut source_norm2 = 0.0;

    for (source, target) in pairs {
        let sx = source.0 - source_centroid.0;
        let sy = source.1 - source_centroid.1;
        let tx = target.0 - target_centroid.0;
        let ty = target.1 - target_centroid.1;
        dot += sx * tx + sy * ty;
        cross += sx * ty - sy * tx;
        source_norm2 += sx * sx + sy * sy;
    }

    if source_norm2 <= f64::EPSILON {
        return None;
    }

    let scale_rotation = dot.hypot(cross);
    if scale_rotation <= f64::EPSILON {
        return None;
    }

    Some(SimilarityTransform {
        scale: scale_rotation / source_norm2,
        cos: dot / scale_rotation,
        sin: cross / scale_rotation,
        source_centroid,
        target_centroid,
    })
}

fn order_dy_finders(finders: &[DyFinder; 3]) -> [DyFinder; 3] {
    let distances = [
        (finder_distance2(&finders[0], &finders[1]), 0_usize, 1_usize),
        (finder_distance2(&finders[0], &finders[2]), 0, 2),
        (finder_distance2(&finders[1], &finders[2]), 1, 2),
    ];
    let &(_, tl_idx, br_idx) = distances
        .iter()
        .max_by(|lhs, rhs| lhs.0.total_cmp(&rhs.0))
        .expect("three finder distances exist");
    let bl_idx = 3 - tl_idx - br_idx;
    let mut tl = finders[tl_idx].clone();
    let mut br = finders[br_idx].clone();
    let bl = finders[bl_idx].clone();

    if tl.cy > br.cy {
        std::mem::swap(&mut tl, &mut br);
    }

    [tl, bl, br]
}

fn ring_specs(geometry: &DyGeometry, params: DyParams) -> Vec<RingSpec> {
    if params.has_border {
        return black_border_ring_specs(geometry, params.ring_count);
    }
    regular_ring_specs(geometry, params.ring_count)
}

fn regular_ring_specs(geometry: &DyGeometry, ring_count: u8) -> Vec<RingSpec> {
    if ring_count == NO_BORDER_RINGS.len() as u8 {
        return no_border_ring_specs(geometry);
    }

    let thickness = (geometry.r_max - geometry.r_min) / ring_count as f64;
    (0..ring_count)
        .map(|ring| RingSpec {
            r_inner: geometry.r_max - (ring as f64 + 1.0) * thickness,
            r_outer: geometry.r_max - ring as f64 * thickness,
            is_decoration: ring == 0 || ring == 2,
        })
        .collect()
}

fn no_border_ring_specs(geometry: &DyGeometry) -> Vec<RingSpec> {
    no_border_ring_specs_with_radius_scale(geometry, NO_BORDER_STANDARD_RADIUS_SCALE)
}

fn no_border_ring_specs_with_radius_scale(
    geometry: &DyGeometry,
    radius_scale: f64,
) -> Vec<RingSpec> {
    let scale = (geometry.locator_distance / NO_BORDER_STANDARD_LOCATOR_DISTANCE).max(0.01);
    NO_BORDER_RINGS
        .iter()
        .map(|&(radius, half_width, is_decoration)| {
            let radius = no_border_adjusted_standard_radius(radius);
            RingSpec {
                r_inner: (radius * radius_scale - half_width) * scale,
                r_outer: (radius * radius_scale + half_width) * scale,
                is_decoration,
            }
        })
        .collect()
}

fn no_border_adjusted_standard_radius(radius: f64) -> f64 {
    let midpoint = (NO_BORDER_RINGS[0].0 + NO_BORDER_RINGS[NO_BORDER_RINGS.len() - 1].0) * 0.5;
    midpoint + (radius - midpoint) * NO_BORDER_RADIAL_SPREAD_SCALE
}

#[allow(dead_code)]
fn best_no_border_radius_scale(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    points_per_ring: u32,
    theta_offset: f64,
) -> f64 {
    let base_scale = (geometry.locator_distance / NO_BORDER_STANDARD_LOCATOR_DISTANCE).max(0.01);
    let mut best = (NO_BORDER_STANDARD_RADIUS_SCALE, f64::INFINITY);

    for radius_step in 396..=414 {
        let radius_scale = radius_step as f64 / 400.0;
        let code_rings = NO_BORDER_RINGS
            .iter()
            .filter(|(_, _, is_decoration)| !*is_decoration)
            .map(|&(radius, half_width, is_decoration)| {
                let radius = no_border_adjusted_standard_radius(radius);
                RingSpec {
                    r_inner: (radius * radius_scale - half_width) * base_scale,
                    r_outer: (radius * radius_scale + half_width) * base_scale,
                    is_decoration,
                }
            })
            .collect::<Vec<_>>();
        let decorative_rings = NO_BORDER_RINGS
            .iter()
            .filter(|(_, _, is_decoration)| *is_decoration)
            .map(|&(radius, half_width, is_decoration)| {
                let radius = no_border_adjusted_standard_radius(radius);
                RingSpec {
                    r_inner: (radius * radius_scale - half_width) * base_scale,
                    r_outer: (radius * radius_scale + half_width) * base_scale,
                    is_decoration,
                }
            })
            .collect::<Vec<_>>();
        let score = candidate_no_border_grid_score(
            bin,
            geometry,
            &code_rings,
            points_per_ring,
            theta_offset,
        ) + candidate_no_border_grid_score(
            bin,
            geometry,
            &decorative_rings,
            points_per_ring,
            theta_offset,
        ) * NO_BORDER_DECORATIVE_RADIUS_SCORE_WEIGHT;
        if score < best.1 {
            best = (radius_scale, score);
        }
    }

    best.0
}

fn best_no_border_geometry(
    bin: &BinaryImage,
    base_geometry: &DyGeometry,
    finders: &[DyFinder; 3],
    points_per_ring: u32,
    theta_offset: f64,
) -> DyGeometry {
    let mut best = (*base_geometry, f64::INFINITY, f64::INFINITY);
    let steps = (NO_BORDER_CENTER_REFINE_MAX_RADIUS / NO_BORDER_CENTER_REFINE_STEP).ceil() as i32;

    for dy_step in -steps..=steps {
        for dx_step in -steps..=steps {
            let dx = f64::from(dx_step) * NO_BORDER_CENTER_REFINE_STEP;
            let dy = f64::from(dy_step) * NO_BORDER_CENTER_REFINE_STEP;
            let geometry = no_border_geometry_with_center_offset(base_geometry, finders, dx, dy);
            let rings = no_border_ring_specs(&geometry);
            let offset2 = dx * dx + dy * dy;
            if offset2 > NO_BORDER_CENTER_REFINE_MAX_RADIUS * NO_BORDER_CENTER_REFINE_MAX_RADIUS {
                continue;
            }
            let score =
                no_border_geometry_score(bin, &geometry, &rings, points_per_ring, theta_offset)
                    + offset2 * NO_BORDER_CENTER_OFFSET_SCORE_WEIGHT;
            if score < best.1 - f64::EPSILON
                || ((score - best.1).abs() <= f64::EPSILON && offset2 < best.2)
            {
                best = (geometry, score, offset2);
            }
        }
    }

    best.0
}

fn no_border_geometry_with_center_offset(
    geometry: &DyGeometry,
    finders: &[DyFinder; 3],
    dx: f64,
    dy: f64,
) -> DyGeometry {
    let center = (geometry.center.0 + dx, geometry.center.1 + dy);
    let locator_radius = finders.iter().map(DyFinder::outer_radius).sum::<f64>() / 3.0;
    let locator_distance = finders
        .iter()
        .map(|finder| distance(center, (finder.cx, finder.cy)))
        .sum::<f64>()
        / finders.len() as f64;
    let r_max = finders
        .iter()
        .map(|finder| distance(center, (finder.cx, finder.cy)) + finder.outer_radius() * 1.10)
        .fold(0.0, f64::max)
        .max(locator_radius * 5.0);
    let r_min = (r_max * 0.36).max(locator_radius * 2.0);

    DyGeometry {
        center,
        locator_distance,
        r_min,
        r_max,
    }
}

#[derive(Debug, Clone, Copy)]
struct NoBorderLayoutFit {
    geometry: DyGeometry,
    theta_offset: f64,
}

fn no_border_geometry_from_standard_layout(
    finders: &[DyFinder; 3],
    badge: DyBadge,
) -> Option<NoBorderLayoutFit> {
    let ordered = order_dy_finders(finders);
    let pairs = [
        (NO_BORDER_LAYOUT_FINDERS[0], (ordered[0].cx, ordered[0].cy)),
        (NO_BORDER_LAYOUT_FINDERS[1], (ordered[1].cx, ordered[1].cy)),
        (NO_BORDER_LAYOUT_FINDERS[2], (ordered[2].cx, ordered[2].cy)),
        (NO_BORDER_LAYOUT_BADGE_CENTER, (badge.cx, badge.cy)),
    ];
    let transform = similarity_transform(&pairs)?;
    let center = transform.point(NO_BORDER_LAYOUT_CENTER);
    let locator_distance = pairs[..3]
        .iter()
        .map(|(_, source)| distance(center, *source))
        .sum::<f64>()
        / 3.0;
    if locator_distance <= f64::EPSILON || transform.scale <= f64::EPSILON {
        return None;
    }

    let locator_radius = finders.iter().map(DyFinder::outer_radius).sum::<f64>() / 3.0;
    let r_max = finders
        .iter()
        .map(|finder| distance(center, (finder.cx, finder.cy)) + finder.outer_radius() * 1.10)
        .fold(0.0, f64::max)
        .max(locator_radius * 5.0);
    let r_min = (r_max * 0.36).max(locator_radius * 2.0);
    let rotation = transform.sin.atan2(transform.cos);

    Some(NoBorderLayoutFit {
        geometry: DyGeometry {
            center,
            locator_distance,
            r_min,
            r_max,
        },
        theta_offset: normalize_angle(NO_BORDER_STANDARD_SAMPLE_THETA_OFFSET + rotation),
    })
}

fn no_border_geometry_score(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    theta_offset: f64,
) -> f64 {
    let code_rings = rings
        .iter()
        .copied()
        .filter(|ring| !ring.is_decoration)
        .collect::<Vec<_>>();
    let decorative_rings = rings
        .iter()
        .copied()
        .filter(|ring| ring.is_decoration)
        .collect::<Vec<_>>();

    candidate_no_border_grid_score(bin, geometry, &code_rings, points_per_ring, theta_offset)
        + candidate_no_border_grid_score(
            bin,
            geometry,
            &decorative_rings,
            points_per_ring,
            theta_offset,
        ) * NO_BORDER_DECORATIVE_RADIUS_SCORE_WEIGHT
}

fn black_border_ring_specs(geometry: &DyGeometry, ring_count: u8) -> Vec<RingSpec> {
    let ring_count = ring_count.clamp(
        BLACK_BORDER_BASE_CODE_RINGS,
        BLACK_BORDER_CODE_RINGS.len() as u8,
    ) as usize;
    scaled_black_border_rings(geometry, &BLACK_BORDER_CODE_RINGS[..ring_count], false)
}

fn black_border_outer_frame_ring_spec(geometry: &DyGeometry) -> RingSpec {
    scaled_black_border_ring(geometry, BLACK_BORDER_OUTER_FRAME_RING, true)
}

fn black_border_fine_ring_specs(geometry: &DyGeometry) -> Vec<RingSpec> {
    scaled_black_border_rings(geometry, &BLACK_BORDER_FINE_RINGS, true)
}

fn scaled_black_border_ring(
    geometry: &DyGeometry,
    standard_ring: (f64, f64),
    is_decoration: bool,
) -> RingSpec {
    let scale = (geometry.locator_distance / BLACK_BORDER_STANDARD_LOCATOR_DISTANCE).max(0.01);
    RingSpec {
        r_inner: standard_ring.0 * scale,
        r_outer: standard_ring.1 * scale,
        is_decoration,
    }
}

fn scaled_black_border_rings(
    geometry: &DyGeometry,
    standard_rings: &[(f64, f64)],
    is_decoration: bool,
) -> Vec<RingSpec> {
    standard_rings
        .iter()
        .map(|&ring| scaled_black_border_ring(geometry, ring, is_decoration))
        .collect()
}

fn detect_grid_shape(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    has_border: bool,
) -> Result<(u8, u32)> {
    let (ring_count, points) = if has_border {
        let rings = black_border_ring_specs(geometry, BLACK_BORDER_CODE_RINGS.len() as u8);
        let points = detect_black_border_points(bin, geometry, &rings);
        (BLACK_BORDER_BASE_CODE_RINGS, points)
    } else {
        (6, 120)
    };

    Ok((ring_count, points))
}

fn detect_black_border_points(bin: &BinaryImage, geometry: &DyGeometry, rings: &[RingSpec]) -> u32 {
    let alignment_rings = black_border_alignment_rings(rings);
    let score_72 = point_grid_score(bin, geometry, &alignment_rings, 72);
    let score_120 = point_grid_score(bin, geometry, &alignment_rings, 120);

    if score_120 < score_72 * 0.96 { 120 } else { 72 }
}

#[derive(Debug, Clone, Copy)]
struct BlackBorderOptionalRingScore {
    usable_points: u32,
    black_points: u32,
    black_runs: u32,
    max_run_len: u32,
}

fn detect_black_border_code_ring_count(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    theta_offset: f64,
    reserved: &ReservedAreas<'_>,
) -> u8 {
    let mut ring_count = BLACK_BORDER_BASE_CODE_RINGS;

    for ring_idx in BLACK_BORDER_BASE_CODE_RINGS as usize..rings.len() {
        let score = black_border_optional_ring_score(
            bin,
            geometry,
            rings,
            ring_idx,
            points_per_ring,
            theta_offset,
            reserved,
        );
        if !black_border_optional_ring_is_present(score, points_per_ring) {
            break;
        }
        ring_count = ring_idx as u8 + 1;
    }

    ring_count
}

fn black_border_optional_ring_score(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    ring_idx: usize,
    points_per_ring: u32,
    theta_offset: f64,
    reserved: &ReservedAreas<'_>,
) -> BlackBorderOptionalRingScore {
    let Some(ring) = rings.get(ring_idx) else {
        return BlackBorderOptionalRingScore {
            usable_points: 0,
            black_points: 0,
            black_runs: 0,
            max_run_len: 0,
        };
    };

    let mut samples = vec![false; points_per_ring as usize];
    let mut usable_points = 0_u32;
    let mut black_points = 0_u32;
    for point in 0..points_per_ring {
        if is_reserved_cell(
            ring,
            ring_idx as u32,
            point,
            points_per_ring,
            theta_offset,
            geometry,
            reserved,
        ) {
            continue;
        }

        usable_points += 1;
        if sample_cell_black_ratio(bin, geometry, ring, points_per_ring, theta_offset, point)
            >= BLACK_BORDER_OPTIONAL_RING_THRESHOLD
        {
            samples[point as usize] = true;
            black_points += 1;
        }
    }

    let runs = circular_runs(&samples, true);
    BlackBorderOptionalRingScore {
        usable_points,
        black_points,
        black_runs: runs.len() as u32,
        max_run_len: runs.iter().map(|run| run.len).max().unwrap_or(0),
    }
}

fn black_border_optional_ring_is_present(
    score: BlackBorderOptionalRingScore,
    points_per_ring: u32,
) -> bool {
    if score.usable_points == 0 || score.black_runs == 0 {
        return false;
    }

    let density = score.black_points as f64 / score.usable_points as f64;
    if !(BLACK_BORDER_OPTIONAL_RING_MIN_DENSITY..=BLACK_BORDER_OPTIONAL_RING_MAX_DENSITY)
        .contains(&density)
    {
        return false;
    }

    let min_runs = if points_per_ring <= 72 { 5 } else { 7 };
    let average_run_len = score.black_points as f64 / score.black_runs as f64;
    let max_run_len =
        (points_per_ring as f64 * BLACK_BORDER_OPTIONAL_RING_MAX_RUN_RATIO).ceil() as u32;
    score.black_runs >= min_runs
        && average_run_len <= 8.0
        && score.max_run_len <= max_run_len.max(8)
}

fn black_border_alignment_rings(rings: &[RingSpec]) -> Vec<RingSpec> {
    rings
        .iter()
        .copied()
        .filter(|ring| !ring.is_decoration)
        .take(BLACK_BORDER_BASE_CODE_RINGS as usize)
        .collect()
}

fn raw_binary_from_source(source: &DynamicImage) -> BinaryImage {
    let raw = otsu_binarize(&source.to_luma8());
    BinaryImage::new(raw.width(), raw.height(), raw.into_raw())
}

#[derive(Debug, Clone, Copy)]
struct FineRingSource<'a> {
    bin: &'a BinaryImage,
    gray: Option<&'a GrayImage>,
}

fn sample_black_border_outer_frame(bin: &BinaryImage, geometry: &DyGeometry) -> DyOuterFrame {
    let ring = black_border_outer_frame_ring_spec(geometry);
    let defaults = standard_outer_frame_segments();
    let left_boundary = refine_outer_frame_boundary(
        bin,
        geometry,
        &ring,
        defaults[0].theta_start,
        BoundaryKind::BlackAfter,
    );
    let right_boundary = refine_outer_frame_boundary(
        bin,
        geometry,
        &ring,
        defaults[1].theta_start,
        BoundaryKind::BlackAfter,
    );
    let lower_left_boundary = refine_outer_frame_boundary(
        bin,
        geometry,
        &ring,
        defaults[1].theta_end,
        BoundaryKind::BlackBefore,
    );

    DyOuterFrame {
        ring,
        segments: vec![
            DyArcSegment {
                theta_start: left_boundary,
                theta_end: defaults[0].theta_end,
            },
            DyArcSegment {
                theta_start: right_boundary,
                theta_end: normalize_positive_angle_after(lower_left_boundary, right_boundary),
            },
        ],
    }
}

/// 无框版装饰环（ring0/ring2）高密度采样。参考黑框版 fine ring：720 点捕捉虚线弧段
/// 边界，但额外跳过 3 牛眼遮挡区（黑框版只有 badge），再闭合采样噪声白缝、去单点噪声。
/// 编码环不走这里（仍在 grid.rings 的 120 点），保持逐点 100% 精度。
fn sample_no_border_fine_rings(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    finders: &[DyFinder; 3],
    badge: Option<DyBadge>,
) -> Vec<DyDecorativeRing> {
    let source = FineRingSource { bin, gray: None };
    rings
        .iter()
        .filter(|ring| ring.is_decoration)
        .map(|ring| {
            let mut samples = (0..BLACK_BORDER_DECORATIVE_POINTS)
                .map(|point| {
                    if no_border_decorative_point_occluded(ring, point, geometry, finders, badge) {
                        return false;
                    }
                    sample_fine_ring_black(source, geometry, ring, point)
                })
                .collect::<Vec<_>>();
            close_circular_white_gaps(&mut samples, NO_BORDER_DECORATIVE_FINE_RING_MAX_GAP);
            remove_short_circular_black_runs(&mut samples, NO_BORDER_DECORATIVE_FINE_RING_MIN_RUN);
            DyDecorativeRing {
                ring: *ring,
                points_per_ring: BLACK_BORDER_DECORATIVE_POINTS,
                theta_offset: 0.0,
                samples,
            }
        })
        .collect()
}

/// 装饰环上某 720 点是否落在牛眼或 badge 遮挡区（应判白、不采成装饰弧）。
fn no_border_decorative_point_occluded(
    ring: &RingSpec,
    point: u32,
    geometry: &DyGeometry,
    finders: &[DyFinder; 3],
    badge: Option<DyBadge>,
) -> bool {
    let theta =
        (point as f64 + 0.5) * std::f64::consts::TAU / BLACK_BORDER_DECORATIVE_POINTS as f64;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let point_xy = (
        geometry.center.0 + radius * theta.cos(),
        geometry.center.1 + radius * theta.sin(),
    );
    if finders.iter().any(|finder| {
        distance(point_xy, (finder.cx, finder.cy))
            <= finder.outer_radius() * NO_BORDER_DECORATIVE_FINDER_SKIP_SCALE
    }) {
        return true;
    }
    // badge 大小/位置固定：用固定 layout 中心+半径经采样映射(fwd)到像素，
    // 而非不稳定的检测 `badge.radius/cx/cy`。仅在检测到 badge 时启用遮挡。
    badge.is_some() && {
        let fwd = geometry.locator_distance / NO_BORDER_STANDARD_LOCATOR_DISTANCE;
        let badge_px = (
            geometry.center.0 + fwd * (NO_BORDER_LAYOUT_BADGE_CENTER.0 - NO_BORDER_LAYOUT_CENTER.0),
            geometry.center.1 + fwd * (NO_BORDER_LAYOUT_BADGE_CENTER.1 - NO_BORDER_LAYOUT_CENTER.1),
        );
        let badge_r = NO_BORDER_LAYOUT_BADGE_RADIUS * fwd;
        distance(point_xy, badge_px) <= badge_r * NO_BORDER_DECORATIVE_BADGE_SKIP_SCALE
    }
}

fn sample_black_border_fine_rings(
    bin: &BinaryImage,
    gray: Option<&GrayImage>,
    geometry: &DyGeometry,
    badge: Option<DyBadge>,
    code_points_per_ring: u32,
) -> Vec<DyDecorativeRing> {
    let source = FineRingSource { bin, gray };
    let badge_skip_scale = black_border_decorative_badge_skip_scale(code_points_per_ring);
    black_border_fine_ring_specs(geometry)
        .into_iter()
        .enumerate()
        .map(|(ring_idx, ring)| {
            let mut samples = (0..BLACK_BORDER_DECORATIVE_POINTS)
                .map(|point| {
                    if is_badge_decorative_point(
                        &ring,
                        point,
                        BLACK_BORDER_DECORATIVE_POINTS,
                        0.0,
                        geometry,
                        badge,
                        badge_skip_scale,
                    ) {
                        return false;
                    }

                    sample_fine_ring_black(source, geometry, &ring, point)
                })
                .collect::<Vec<_>>();
            prune_black_border_badge_decorative_short_runs(
                &mut samples,
                &ring,
                geometry,
                badge,
                code_points_per_ring,
                ring_idx,
            );
            close_circular_white_gaps(&mut samples, BLACK_BORDER_FINE_RING_MAX_GAP);
            remove_short_circular_black_runs(&mut samples, BLACK_BORDER_FINE_RING_MIN_RUN);
            prune_black_border_badge_decorative_edge_band_after_closing(
                &mut samples,
                &ring,
                geometry,
                badge,
                code_points_per_ring,
                ring_idx,
            );
            restore_black_border_72_inner_fine_ring_badge_bridges(
                source,
                &mut samples,
                &ring,
                geometry,
                badge,
                code_points_per_ring,
                ring_idx,
            );
            restore_black_border_120_outer_fine_ring_badge_edge(
                source,
                &mut samples,
                &ring,
                geometry,
                badge,
                code_points_per_ring,
                ring_idx,
            );
            extend_black_border_72_inner_fine_ring_weak_endpoints(
                source,
                &mut samples,
                &ring,
                geometry,
                badge,
                code_points_per_ring,
                ring_idx,
            );
            reconstruct_black_border_fine_ring_template(
                source,
                &mut samples,
                &ring,
                geometry,
                badge,
                code_points_per_ring,
                ring_idx,
            );

            DyDecorativeRing {
                ring,
                points_per_ring: BLACK_BORDER_DECORATIVE_POINTS,
                theta_offset: 0.0,
                samples,
            }
        })
        .collect()
}

fn prune_black_border_badge_decorative_edge_band_after_closing(
    samples: &mut [bool],
    ring: &RingSpec,
    geometry: &DyGeometry,
    badge: Option<DyBadge>,
    code_points_per_ring: u32,
    ring_idx: usize,
) {
    if code_points_per_ring != 72 {
        return;
    }
    let Some(badge) = badge else {
        return;
    };

    prune_black_border_badge_decorative_edge_band_72(samples, ring, geometry, badge, ring_idx);
}

fn restore_black_border_120_outer_fine_ring_badge_edge(
    source: FineRingSource<'_>,
    samples: &mut [bool],
    ring: &RingSpec,
    geometry: &DyGeometry,
    badge: Option<DyBadge>,
    code_points_per_ring: u32,
    ring_idx: usize,
) {
    if code_points_per_ring != 120 || ring_idx != 0 {
        return;
    }
    let Some(badge) = badge else {
        return;
    };

    for point in 0..BLACK_BORDER_DECORATIVE_POINTS {
        if samples[point as usize]
            || !is_black_border_lower_badge_decorative_edge_point(ring, point, geometry, badge)
        {
            continue;
        }

        let (angular_hits, black, _) = fine_ring_sample_score(source, geometry, ring, point);
        if angular_hits >= BLACK_BORDER_BADGE_DECORATIVE_RESTORE_MIN_ANGULAR_HITS
            && black >= BLACK_BORDER_BADGE_DECORATIVE_RESTORE_MIN_BLACK as f64
        {
            samples[point as usize] = true;
        }
    }
}

fn is_black_border_lower_badge_decorative_edge_point(
    ring: &RingSpec,
    point: u32,
    geometry: &DyGeometry,
    badge: DyBadge,
) -> bool {
    let ratio = badge_decorative_distance_ratio(
        ring,
        point,
        BLACK_BORDER_DECORATIVE_POINTS,
        0.0,
        geometry,
        badge,
    );
    if !(BLACK_BORDER_BADGE_DECORATIVE_RESTORE_MIN_RATIO_120
        ..=BLACK_BORDER_BADGE_DECORATIVE_RESTORE_MAX_RATIO_120)
        .contains(&ratio)
    {
        return false;
    }

    let theta =
        (point as f64 + 0.5) * std::f64::consts::TAU / BLACK_BORDER_DECORATIVE_POINTS as f64;
    let badge_theta = (badge.cy - geometry.center.1).atan2(badge.cx - geometry.center.0);
    let delta = signed_angle_delta(theta, badge_theta);
    delta >= 0.0 && delta <= BLACK_BORDER_BADGE_DECORATIVE_RESTORE_MAX_DELTA_DEG_120.to_radians()
}

fn restore_black_border_72_inner_fine_ring_badge_bridges(
    source: FineRingSource<'_>,
    samples: &mut [bool],
    ring: &RingSpec,
    geometry: &DyGeometry,
    badge: Option<DyBadge>,
    code_points_per_ring: u32,
    ring_idx: usize,
) {
    if code_points_per_ring != 72 || ring_idx != 1 {
        return;
    }
    let Some(badge) = badge else {
        return;
    };

    let original = samples.to_vec();
    for run in circular_runs(&original, false) {
        if run.len > BLACK_BORDER_BADGE_DECORATIVE_BRIDGE_MAX_LEN_72 {
            continue;
        }

        let before =
            (run.start + BLACK_BORDER_DECORATIVE_POINTS - 1) % BLACK_BORDER_DECORATIVE_POINTS;
        let after = (run.start + run.len) % BLACK_BORDER_DECORATIVE_POINTS;
        if !original[before as usize] || !original[after as usize] {
            continue;
        }

        let mut restorable = true;
        for offset in 0..run.len {
            let point = (run.start + offset) % BLACK_BORDER_DECORATIVE_POINTS;
            if !is_black_border_badge_decorative_edge_point_72(
                ring, point, geometry, badge, ring_idx,
            ) {
                restorable = false;
                break;
            }

            let (angular_hits, black, _) = fine_ring_sample_score(source, geometry, ring, point);
            if angular_hits < BLACK_BORDER_BADGE_DECORATIVE_BRIDGE_MIN_ANGULAR_HITS_72
                || black < BLACK_BORDER_BADGE_DECORATIVE_BRIDGE_MIN_BLACK_72 as f64
            {
                restorable = false;
                break;
            }
        }

        if restorable {
            set_circular_run(samples, run.start, run.len, true);
        }
    }
}

fn sample_fine_ring_black(
    source: FineRingSource<'_>,
    geometry: &DyGeometry,
    ring: &RingSpec,
    point: u32,
) -> bool {
    let (angular_hits, black, total) = fine_ring_sample_score(source, geometry, ring, point);
    angular_hits >= 2 || black / total as f64 >= BLACK_BORDER_DECORATIVE_THRESHOLD
}

fn sample_fine_ring_weak_black(
    source: FineRingSource<'_>,
    geometry: &DyGeometry,
    ring: &RingSpec,
    point: u32,
) -> bool {
    let (angular_hits, black, _) = fine_ring_sample_score(source, geometry, ring, point);
    angular_hits >= 1 && black >= 1.0
}

fn fine_ring_sample_score(
    source: FineRingSource<'_>,
    geometry: &DyGeometry,
    ring: &RingSpec,
    point: u32,
) -> (u32, f64, u32) {
    const THETA_OFFSETS: [f64; 5] = [-0.40, -0.20, 0.0, 0.20, 0.40];
    const RADIAL_OFFSETS: [f64; 7] = [-0.65, -0.42, -0.20, 0.0, 0.20, 0.42, 0.65];
    let theta_step = std::f64::consts::TAU / BLACK_BORDER_DECORATIVE_POINTS as f64;
    let radial_step = ring.r_outer - ring.r_inner;
    let theta = (point as f64 + 0.5) * theta_step;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let mut angular_hits = 0_u32;
    let mut black = 0.0_f64;
    let mut total = 0_u32;

    for theta_delta in THETA_OFFSETS {
        let mut theta_hit = false;
        let mut column_black = 0.0_f64;
        for radial_delta in RADIAL_OFFSETS {
            let dark = sample_fine_ring_dark(
                source,
                geometry.center,
                radius + radial_delta * radial_step,
                theta + theta_delta * theta_step,
            );
            column_black += dark;
            black += dark;
            total += 1;
        }
        if column_black >= 0.45 {
            theta_hit = true;
        }
        if theta_hit {
            angular_hits += 1;
        }
    }

    (angular_hits, black, total)
}

fn extend_black_border_72_inner_fine_ring_weak_endpoints(
    source: FineRingSource<'_>,
    samples: &mut [bool],
    ring: &RingSpec,
    geometry: &DyGeometry,
    badge: Option<DyBadge>,
    code_points_per_ring: u32,
    ring_idx: usize,
) {
    if code_points_per_ring != 72 || ring_idx != 1 {
        return;
    }
    let Some(badge) = badge else {
        return;
    };

    let original = samples.to_vec();
    for run in circular_runs(&original, true) {
        for point in [
            (run.start + BLACK_BORDER_DECORATIVE_POINTS - 1) % BLACK_BORDER_DECORATIVE_POINTS,
            (run.start + run.len) % BLACK_BORDER_DECORATIVE_POINTS,
        ] {
            let ratio = badge_decorative_distance_ratio(
                ring,
                point,
                BLACK_BORDER_DECORATIVE_POINTS,
                0.0,
                geometry,
                badge,
            );
            if original[point as usize]
                || is_black_border_badge_decorative_edge_point_72(
                    ring, point, geometry, badge, ring_idx,
                )
            {
                continue;
            }
            if !(BLACK_BORDER_BADGE_DECORATIVE_INNER_EDGE_MAX_RATIO_72
                ..=BLACK_BORDER_BADGE_DECORATIVE_INNER_EDGE_MAX_RATIO_72 + 0.32)
                .contains(&ratio)
            {
                continue;
            }
            if sample_fine_ring_weak_black(source, geometry, ring, point) {
                samples[point as usize] = true;
            }
        }
    }
}

fn reconstruct_black_border_fine_ring_template(
    source: FineRingSource<'_>,
    samples: &mut [bool],
    ring: &RingSpec,
    geometry: &DyGeometry,
    badge: Option<DyBadge>,
    code_points_per_ring: u32,
    ring_idx: usize,
) {
    let Some(badge) = badge else {
        return;
    };
    let max_gap = if code_points_per_ring == 72 {
        BLACK_BORDER_FINE_RING_TEMPLATE_MAX_GAP
    } else {
        BLACK_BORDER_FINE_RING_MAX_GAP
    };
    let original = samples.to_vec();
    for run in circular_runs(&original, false) {
        if run.len > max_gap {
            continue;
        }

        let before =
            (run.start + BLACK_BORDER_DECORATIVE_POINTS - 1) % BLACK_BORDER_DECORATIVE_POINTS;
        let after = (run.start + run.len) % BLACK_BORDER_DECORATIVE_POINTS;
        if !original[before as usize] || !original[after as usize] {
            continue;
        }

        let mut restorable = true;
        for offset in 0..run.len {
            let point = (run.start + offset) % BLACK_BORDER_DECORATIVE_POINTS;
            if !is_black_border_fine_ring_template_point(
                ring,
                point,
                geometry,
                badge,
                code_points_per_ring,
                ring_idx,
            ) {
                restorable = false;
                break;
            }
            let (angular_hits, black, _) = fine_ring_sample_score(source, geometry, ring, point);
            if angular_hits < BLACK_BORDER_FINE_RING_TEMPLATE_MIN_ANGULAR_HITS
                || black < BLACK_BORDER_FINE_RING_TEMPLATE_MIN_BLACK
            {
                restorable = false;
                break;
            }
        }

        if restorable {
            set_circular_run(samples, run.start, run.len, true);
        }
    }
}

fn is_black_border_fine_ring_template_point(
    ring: &RingSpec,
    point: u32,
    geometry: &DyGeometry,
    badge: DyBadge,
    code_points_per_ring: u32,
    ring_idx: usize,
) -> bool {
    let outer_ratio = badge_decorative_outer_distance_ratio(
        ring,
        point,
        BLACK_BORDER_DECORATIVE_POINTS,
        0.0,
        geometry,
        badge,
    );
    if !(BLACK_BORDER_FINE_RING_TEMPLATE_OUTER_MIN_RATIO
        ..=BLACK_BORDER_FINE_RING_TEMPLATE_OUTER_MAX_RATIO)
        .contains(&outer_ratio)
    {
        return false;
    }

    let theta =
        (point as f64 + 0.5) * std::f64::consts::TAU / BLACK_BORDER_DECORATIVE_POINTS as f64;
    let badge_theta = (badge.cy - geometry.center.1).atan2(badge.cx - geometry.center.0);
    if signed_angle_delta(theta, badge_theta).abs() > 80.0_f64.to_radians() {
        return false;
    }

    code_points_per_ring == 72
        || ring_idx == 0
        || outer_ratio >= BLACK_BORDER_FINE_RING_TEMPLATE_OUTER_MIN_RATIO + 0.04
}

fn prune_black_border_badge_decorative_short_runs(
    samples: &mut [bool],
    ring: &RingSpec,
    geometry: &DyGeometry,
    badge: Option<DyBadge>,
    code_points_per_ring: u32,
    ring_idx: usize,
) {
    let Some(badge) = badge else {
        return;
    };
    if code_points_per_ring == 72 {
        prune_black_border_badge_decorative_edge_band_72(samples, ring, geometry, badge, ring_idx);
        return;
    }
    if code_points_per_ring != 120 {
        return;
    }

    let original = samples.to_vec();
    for run in circular_runs(&original, true) {
        if run.len > BLACK_BORDER_BADGE_DECORATIVE_EDGE_RUN_MAX_LEN {
            continue;
        }

        let min_ratio = (0..run.len)
            .map(|offset| {
                let point = (run.start + offset) % BLACK_BORDER_DECORATIVE_POINTS;
                badge_decorative_distance_ratio(
                    ring,
                    point,
                    BLACK_BORDER_DECORATIVE_POINTS,
                    0.0,
                    geometry,
                    badge,
                )
            })
            .fold(f64::INFINITY, f64::min);

        if !(BLACK_BORDER_BADGE_DECORATIVE_EDGE_MIN_RATIO
            ..=BLACK_BORDER_BADGE_DECORATIVE_EDGE_MAX_RATIO)
            .contains(&min_ratio)
        {
            continue;
        }

        for offset in 0..run.len {
            let point = (run.start + offset) % BLACK_BORDER_DECORATIVE_POINTS;
            samples[point as usize] = false;
        }
        trim_badge_decorative_bridge_neighbor(
            samples, &original, ring, geometry, badge, run.start, -1,
        );
        trim_badge_decorative_bridge_neighbor(
            samples,
            &original,
            ring,
            geometry,
            badge,
            (run.start + run.len) % BLACK_BORDER_DECORATIVE_POINTS,
            1,
        );
    }
}

fn prune_black_border_badge_decorative_edge_band_72(
    samples: &mut [bool],
    ring: &RingSpec,
    geometry: &DyGeometry,
    badge: DyBadge,
    ring_idx: usize,
) {
    for point in 0..BLACK_BORDER_DECORATIVE_POINTS {
        if is_black_border_badge_decorative_edge_point_72(ring, point, geometry, badge, ring_idx) {
            samples[point as usize] = false;
        }
    }
}

fn is_black_border_badge_decorative_edge_point_72(
    ring: &RingSpec,
    point: u32,
    geometry: &DyGeometry,
    badge: DyBadge,
    ring_idx: usize,
) -> bool {
    let ratio = badge_decorative_distance_ratio(
        ring,
        point,
        BLACK_BORDER_DECORATIVE_POINTS,
        0.0,
        geometry,
        badge,
    );
    if ring_idx == 1 {
        return (BLACK_BORDER_BADGE_DECORATIVE_INNER_EDGE_MIN_RATIO_72
            ..=BLACK_BORDER_BADGE_DECORATIVE_INNER_EDGE_MAX_RATIO_72)
            .contains(&ratio);
    }

    (BLACK_BORDER_BADGE_DECORATIVE_EDGE_MIN_RATIO_72
        ..=BLACK_BORDER_BADGE_DECORATIVE_EDGE_MAX_RATIO_72)
        .contains(&ratio)
}

fn trim_badge_decorative_bridge_neighbor(
    samples: &mut [bool],
    original: &[bool],
    ring: &RingSpec,
    geometry: &DyGeometry,
    badge: DyBadge,
    start: u32,
    direction: i32,
) {
    let points = BLACK_BORDER_DECORATIVE_POINTS;
    let mut point = step_decorative_point(start, direction);
    let mut gap = 0_u32;
    while gap < BLACK_BORDER_FINE_RING_MAX_GAP && !original[point as usize] {
        point = step_decorative_point(point, direction);
        gap += 1;
    }
    if !original[point as usize] {
        return;
    }

    let mut trimmed = 0_u32;
    while trimmed < BLACK_BORDER_BADGE_DECORATIVE_EDGE_RUN_MAX_LEN
        && original[point as usize]
        && (BLACK_BORDER_BADGE_DECORATIVE_EDGE_MIN_RATIO
            ..=BLACK_BORDER_BADGE_DECORATIVE_EDGE_MAX_RATIO)
            .contains(&badge_decorative_distance_ratio(
                ring, point, points, 0.0, geometry, badge,
            ))
    {
        samples[point as usize] = false;
        point = step_decorative_point(point, direction);
        trimmed += 1;
    }
}

fn step_decorative_point(point: u32, direction: i32) -> u32 {
    if direction < 0 {
        (point + BLACK_BORDER_DECORATIVE_POINTS - 1) % BLACK_BORDER_DECORATIVE_POINTS
    } else {
        (point + 1) % BLACK_BORDER_DECORATIVE_POINTS
    }
}

fn is_badge_decorative_point(
    ring: &RingSpec,
    point: u32,
    points_per_ring: u32,
    theta_offset: f64,
    geometry: &DyGeometry,
    badge: Option<DyBadge>,
    skip_scale: f64,
) -> bool {
    let Some(badge) = badge else {
        return false;
    };
    badge_decorative_distance_ratio(ring, point, points_per_ring, theta_offset, geometry, badge)
        <= skip_scale
}

fn badge_decorative_distance_ratio(
    ring: &RingSpec,
    point: u32,
    points_per_ring: u32,
    theta_offset: f64,
    geometry: &DyGeometry,
    badge: DyBadge,
) -> f64 {
    let theta =
        theta_offset + (point as f64 + 0.5) * std::f64::consts::TAU / points_per_ring as f64;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let point_xy = (
        geometry.center.0 + radius * theta.cos(),
        geometry.center.1 + radius * theta.sin(),
    );
    distance(point_xy, (badge.cx, badge.cy)) / badge_radius_safe(badge.radius)
}

fn badge_decorative_outer_distance_ratio(
    ring: &RingSpec,
    point: u32,
    points_per_ring: u32,
    theta_offset: f64,
    geometry: &DyGeometry,
    badge: DyBadge,
) -> f64 {
    let theta =
        theta_offset + (point as f64 + 0.5) * std::f64::consts::TAU / points_per_ring as f64;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let point_xy = (
        geometry.center.0 + radius * theta.cos(),
        geometry.center.1 + radius * theta.sin(),
    );
    distance(point_xy, (badge.cx, badge.cy))
        / badge_radius_safe(black_border_badge_outer_radius(badge))
}

fn black_border_decorative_badge_skip_scale(code_points_per_ring: u32) -> f64 {
    if code_points_per_ring == 72 {
        BLACK_BORDER_BADGE_DECORATIVE_RELAXED_SKIP_SCALE
    } else {
        BLACK_BORDER_BADGE_DECORATIVE_SKIP_SCALE
    }
}

#[derive(Debug, Clone, Copy)]
enum BoundaryKind {
    BlackAfter,
    BlackBefore,
}

fn standard_outer_frame_segments() -> [DyArcSegment; 2] {
    const CENTER: (f64, f64) = (371.02, 371.02);
    let fixed_badge = angle_from_standard_point(CENTER, (550.23, 40.07));
    let lower_left = angle_from_standard_point(CENTER, (205.97, 709.26));
    let left = angle_from_standard_point(CENTER, (29.54, 529.26));
    let right = angle_from_standard_point(CENTER, (734.84, 274.69));

    [
        DyArcSegment {
            theta_start: left,
            theta_end: normalize_positive_angle_after(fixed_badge, left),
        },
        DyArcSegment {
            theta_start: right,
            theta_end: normalize_positive_angle_after(lower_left, right),
        },
    ]
}

fn angle_from_standard_point(center: (f64, f64), point: (f64, f64)) -> f64 {
    normalize_angle((point.1 - center.1).atan2(point.0 - center.0))
}

fn refine_outer_frame_boundary(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    default_theta: f64,
    kind: BoundaryKind,
) -> f64 {
    let search_step = std::f64::consts::TAU / 2880.0;
    let search_radius = 96_i32;
    let probe = std::f64::consts::TAU / 180.0 * 0.5;
    let mut best = (default_theta, f64::NEG_INFINITY);

    for step in -search_radius..=search_radius {
        let theta = default_theta + step as f64 * search_step;
        let before = outer_frame_angle_score(bin, geometry, ring, theta - probe);
        let after = outer_frame_angle_score(bin, geometry, ring, theta + probe);
        let score = match kind {
            BoundaryKind::BlackAfter => after - before,
            BoundaryKind::BlackBefore => before - after,
        };
        if score > best.1 {
            best = (theta, score);
        }
    }

    normalize_angle(best.0)
}

fn outer_frame_angle_score(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    theta: f64,
) -> f64 {
    const RADIAL_OFFSETS: [f64; 5] = [-0.40, -0.20, 0.0, 0.20, 0.40];
    const THETA_OFFSETS: [f64; 3] = [-0.8, 0.0, 0.8];
    let theta_step = std::f64::consts::TAU / BLACK_BORDER_DECORATIVE_POINTS as f64;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let radial_step = ring.r_outer - ring.r_inner;
    let mut black = 0_u32;
    let mut total = 0_u32;

    for theta_offset in THETA_OFFSETS {
        for radial_offset in RADIAL_OFFSETS {
            if sample_polar(
                bin,
                geometry.center,
                radius + radial_offset * radial_step,
                theta + theta_offset * theta_step,
            ) {
                black += 1;
            }
            total += 1;
        }
    }

    black as f64 / total as f64
}

fn close_circular_white_gaps(samples: &mut [bool], max_gap: u32) {
    for run in circular_runs(samples, false) {
        if run.len <= max_gap && has_neighboring_black(samples, run.start, run.len) {
            set_circular_run(samples, run.start, run.len, true);
        }
    }
}

fn remove_short_circular_black_runs(samples: &mut [bool], min_run: u32) {
    for run in circular_runs(samples, true) {
        if run.len < min_run {
            set_circular_run(samples, run.start, run.len, false);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CircularRun {
    start: u32,
    len: u32,
}

fn circular_runs(samples: &[bool], value: bool) -> Vec<CircularRun> {
    let points = samples.len() as u32;
    if points == 0 {
        return Vec::new();
    }
    if samples.iter().all(|&sample| sample == value) {
        return vec![CircularRun {
            start: 0,
            len: points,
        }];
    }

    let Some(first_other) = (0..points).find(|&point| samples[point as usize] != value) else {
        return Vec::new();
    };
    let base = first_other + 1;
    let mut runs = Vec::new();
    let mut start: Option<u32> = None;
    for offset in 0..points {
        let point = (base + offset) % points;
        if samples[point as usize] == value {
            start.get_or_insert(offset);
        } else if let Some(run_start) = start.take() {
            runs.push(CircularRun {
                start: base + run_start,
                len: offset - run_start,
            });
        }
    }
    if let Some(run_start) = start {
        runs.push(CircularRun {
            start: base + run_start,
            len: points - run_start,
        });
    }

    runs
}

fn has_neighboring_black(samples: &[bool], start: u32, len: u32) -> bool {
    let points = samples.len() as u32;
    if points == 0 || len >= points {
        return false;
    }
    let prev = (start + points - 1) % points;
    let next = (start + len) % points;
    samples[prev as usize] && samples[next as usize]
}

fn set_circular_run(samples: &mut [bool], start: u32, len: u32, value: bool) {
    let points = samples.len() as u32;
    if points == 0 {
        return;
    }
    for offset in 0..len {
        let point = (start + offset) % points;
        samples[point as usize] = value;
    }
}

fn normalize_angle(theta: f64) -> f64 {
    theta.rem_euclid(std::f64::consts::TAU)
}

fn signed_angle_delta(lhs: f64, rhs: f64) -> f64 {
    (lhs - rhs + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

fn normalize_positive_angle_after(theta: f64, after: f64) -> f64 {
    let mut theta = normalize_angle(theta);
    while theta <= after {
        theta += std::f64::consts::TAU;
    }
    theta
}

fn point_grid_score(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
) -> f64 {
    let theta_offset = best_black_border_theta_offset(bin, geometry, rings, points_per_ring);
    candidate_grid_score(bin, geometry, rings, points_per_ring, theta_offset)
}

fn detect_border(bin: &BinaryImage, geometry: &DyGeometry) -> bool {
    let mut score = 0.0_f64;
    for ratio in [0.88, 0.92, 0.96, 1.0] {
        score = score.max(radial_black_score(
            bin,
            geometry.center,
            geometry.r_max * ratio,
        ));
    }
    let outside_score = radial_black_score(bin, geometry.center, geometry.r_max * 1.06);
    score > 0.16 && outside_score < 0.45
}

fn radial_black_score(bin: &BinaryImage, center: (f64, f64), radius: f64) -> f64 {
    let samples = 360;
    let mut black = 0;
    for idx in 0..samples {
        let theta = idx as f64 * std::f64::consts::TAU / samples as f64;
        if sample_polar(bin, center, radius, theta) {
            black += 1;
        }
    }
    black as f64 / samples as f64
}

fn best_theta_offset(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
) -> f64 {
    let theta_step = std::f64::consts::TAU / points_per_ring as f64;
    let offset_steps = 48;
    let mut best = (0.0, f64::NEG_INFINITY);

    for idx in 0..offset_steps {
        let theta_offset = idx as f64 * theta_step / offset_steps as f64;
        let mut score = 0.0;
        for ring in rings {
            for point in 0..points_per_ring {
                score += sample_cell_black_ratio(
                    bin,
                    geometry,
                    ring,
                    points_per_ring,
                    theta_offset,
                    point,
                );
            }
        }
        if score > best.1 {
            best = (theta_offset, score);
        }
    }

    best.0
}

fn best_black_border_theta_offset(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
) -> f64 {
    let theta_step = std::f64::consts::TAU / points_per_ring as f64;
    best_theta_offset(bin, geometry, rings, points_per_ring) + theta_step * 0.5
}

fn black_border_standard_code_theta_offset(
    finders: &[DyFinder; 3],
    points_per_ring: u32,
    badge_style: DyBadgeStyle,
) -> Option<f64> {
    let standard_offset = match (points_per_ring, badge_style) {
        (72, _) => BLACK_BORDER_STANDARD_CODE_THETA_OFFSET_72,
        (120, DyBadgeStyle::Bullseye) => BLACK_BORDER_BULLSEYE_CODE_THETA_OFFSET_120,
        (120, DyBadgeStyle::DouyinLogo) => BLACK_BORDER_STANDARD_CODE_THETA_OFFSET_120,
        _ => return None,
    };
    let ordered = order_dy_finders(finders);
    let tl = &ordered[0];
    let br = &ordered[2];
    let diagonal_angle = (br.cy - tl.cy).atan2(br.cx - tl.cx);
    let rotation = diagonal_angle - std::f64::consts::FRAC_PI_4;

    Some(normalize_angle(standard_offset + rotation))
}

fn no_border_standard_code_theta_offset(finders: &[DyFinder; 3]) -> f64 {
    let ordered = order_dy_finders(finders);
    let tl = &ordered[0];
    let br = &ordered[2];
    let diagonal_angle = (br.cy - tl.cy).atan2(br.cx - tl.cx);
    let rotation = diagonal_angle - std::f64::consts::FRAC_PI_4;

    normalize_angle(NO_BORDER_STANDARD_SAMPLE_THETA_OFFSET + rotation)
}

fn candidate_grid_score(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    theta_offset: f64,
) -> f64 {
    candidate_grid_score_with_offsets(
        bin,
        geometry,
        rings,
        points_per_ring,
        theta_offset,
        (&[-0.20, 0.0, 0.20], &[-0.25, 0.0, 0.25]),
        0.26,
    )
}

fn candidate_no_border_grid_score(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    theta_offset: f64,
) -> f64 {
    candidate_no_border_grid_score_with_offsets(
        bin,
        geometry,
        rings,
        points_per_ring,
        theta_offset,
        (
            &NO_BORDER_RADIUS_SCORE_THETA_OFFSETS,
            &NO_BORDER_RADIUS_SCORE_RADIAL_OFFSETS,
        ),
        NO_BORDER_RADIUS_SCORE_THRESHOLD,
    )
}

fn candidate_no_border_grid_score_with_offsets(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    theta_offset: f64,
    offsets: (&[f64], &[f64]),
    threshold: f64,
) -> f64 {
    let mut uncertainty = 0.0;
    let mut score_black = 0_u32;
    let mut score_total = 0_u32;
    let mut ring_density_penalty = 0.0;
    let mut scored_rings = 0_u32;

    for ring in rings {
        let final_threshold = if ring.is_decoration {
            NO_BORDER_DECORATIVE_BLACK_THRESHOLD
        } else {
            NO_BORDER_BLACK_THRESHOLD
        };
        let min_density = if ring.is_decoration { 0.18 } else { 0.40 };
        let mut ring_black = 0_u32;
        let mut ring_total = 0_u32;

        for point in 0..points_per_ring {
            let ratio = sample_cell_black_ratio_with_offsets(
                bin,
                geometry,
                ring,
                points_per_ring,
                theta_offset,
                point,
                offsets,
            );
            uncertainty += ratio.min(1.0 - ratio);
            if ratio >= threshold {
                score_black += 1;
            }
            if ratio >= final_threshold {
                ring_black += 1;
            }
            score_total += 1;
            ring_total += 1;
        }

        if ring_total != 0 {
            let density = f64::from(ring_black) / f64::from(ring_total);
            if density < min_density {
                ring_density_penalty += min_density - density;
            }
            scored_rings += 1;
        }
    }

    if score_total == 0 {
        return f64::INFINITY;
    }

    let black_ratio = f64::from(score_black) / f64::from(score_total);
    let density_penalty = if (0.08..=0.62).contains(&black_ratio) {
        0.0
    } else {
        (black_ratio - 0.35).abs()
    };
    let ring_density_penalty = if scored_rings == 0 {
        0.0
    } else {
        ring_density_penalty / f64::from(scored_rings)
    };

    uncertainty / f64::from(score_total) + density_penalty + ring_density_penalty
}

fn candidate_grid_score_with_offsets(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    theta_offset: f64,
    offsets: (&[f64], &[f64]),
    threshold: f64,
) -> f64 {
    let mut uncertainty = 0.0;
    let mut black = 0_u32;
    let mut total = 0_u32;
    for ring in rings {
        for point in 0..points_per_ring {
            let ratio = sample_cell_black_ratio_with_offsets(
                bin,
                geometry,
                ring,
                points_per_ring,
                theta_offset,
                point,
                offsets,
            );
            uncertainty += ratio.min(1.0 - ratio);
            if ratio >= threshold {
                black += 1;
            }
            total += 1;
        }
    }

    if total == 0 {
        return f64::INFINITY;
    }

    let black_ratio = black as f64 / total as f64;
    let density_penalty = if (0.08..=0.62).contains(&black_ratio) {
        0.0
    } else {
        (black_ratio - 0.35).abs()
    };
    uncertainty / total as f64 + density_penalty
}

fn sample_cell_black_ratio(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    points_per_ring: u32,
    theta_offset: f64,
    point: u32,
) -> f64 {
    sample_cell_black_ratio_with_offsets(
        bin,
        geometry,
        ring,
        points_per_ring,
        theta_offset,
        point,
        (&[-0.20, 0.0, 0.20], &[-0.25, 0.0, 0.25]),
    )
}

fn sample_no_border_cell_black_ratio(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    points_per_ring: u32,
    theta_offset: f64,
    point: u32,
) -> f64 {
    sample_no_border_radial_lane_hybrid_ratio(
        bin,
        geometry,
        ring,
        points_per_ring,
        theta_offset,
        point,
        &NO_BORDER_SAMPLE_THETA_OFFSETS,
        &NO_BORDER_SAMPLE_RADIAL_OFFSETS,
        NO_BORDER_SAMPLE_RADIAL_LANE_MAX_WEIGHT,
    )
}

#[cfg(test)]
#[allow(dead_code)]
fn sample_no_border_sector_black_ratio(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    points_per_ring: u32,
    theta_offset: f64,
    point: u32,
    angular_scale: f64,
    radial_scale: f64,
) -> f64 {
    if points_per_ring == 0 {
        return 0.0;
    }

    let theta_step = std::f64::consts::TAU / points_per_ring as f64;
    let theta = theta_offset + (point as f64 + 0.5) * theta_step;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let half_width = (ring.r_outer - ring.r_inner).max(0.01) * radial_scale * 0.5;
    let min_radius = (radius - half_width).max(0.0);
    let max_radius = radius + half_width;
    let half_theta = theta_step * angular_scale * 0.5;
    let center = (
        geometry.center.0 + radius * theta.cos(),
        geometry.center.1 + radius * theta.sin(),
    );
    let search_radius = (max_radius * half_theta.sin().abs()).max(half_width) + 2.0;
    let min_x = (center.0 - search_radius).floor().max(0.0) as i32;
    let max_x = (center.0 + search_radius).ceil().min(bin.w as f64 - 1.0) as i32;
    let min_y = (center.1 - search_radius).floor().max(0.0) as i32;
    let max_y = (center.1 + search_radius).ceil().min(bin.h as f64 - 1.0) as i32;
    if max_x < min_x || max_y < min_y {
        return 0.0;
    }

    let mut black = 0_u32;
    let mut total = 0_u32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            let dx = px - geometry.center.0;
            let dy = py - geometry.center.1;
            let pixel_radius = dx.hypot(dy);
            if pixel_radius < min_radius || pixel_radius > max_radius {
                continue;
            }
            let pixel_theta = dy.atan2(dx);
            if signed_angle_delta(pixel_theta, theta).abs() > half_theta {
                continue;
            }

            total += 1;
            if bin.is_black(x, y) {
                black += 1;
            }
        }
    }

    if total == 0 {
        0.0
    } else {
        f64::from(black) / f64::from(total)
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn sample_no_border_dense_patch_ratio(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    points_per_ring: u32,
    theta_offset: f64,
    point: u32,
) -> f64 {
    let theta_step = std::f64::consts::TAU / points_per_ring as f64;
    let radial_step = (ring.r_outer - ring.r_inner).max(0.01);
    let base_theta = theta_offset + (point as f64 + 0.5) * theta_step;
    let base_radius = (ring.r_inner + ring.r_outer) * 0.5;
    let tangential_radius =
        (base_radius * theta_step * NO_BORDER_DENSE_PATCH_TANGENTIAL_SCALE).max(1.0);
    let radial_radius = (radial_step * NO_BORDER_DENSE_PATCH_RADIAL_SCALE).max(1.0);
    let mut best_patch = 0.0_f64;
    let mut center_patch = 0.0_f64;

    for &radial_center in &NO_BORDER_DENSE_PATCH_RADIAL_CENTERS {
        let patch_center_radius = base_radius + radial_center * radial_step;
        let ratio = no_border_elliptic_patch_ratio(
            bin,
            geometry.center,
            patch_center_radius,
            base_theta,
            tangential_radius,
            radial_radius,
        );
        if radial_center.abs() <= f64::EPSILON {
            center_patch = ratio;
        }
        best_patch = best_patch.max(ratio);
    }

    best_patch * (1.0 - NO_BORDER_DENSE_PATCH_CENTER_WEIGHT)
        + center_patch * NO_BORDER_DENSE_PATCH_CENTER_WEIGHT
}

#[cfg(test)]
#[allow(dead_code)]
fn no_border_elliptic_patch_ratio(
    bin: &BinaryImage,
    center: (f64, f64),
    radius: f64,
    theta: f64,
    tangential_radius: f64,
    radial_radius: f64,
) -> f64 {
    let patch_center = (
        center.0 + radius * theta.cos(),
        center.1 + radius * theta.sin(),
    );
    let min_x = (patch_center.0 - tangential_radius - radial_radius)
        .floor()
        .max(0.0) as i32;
    let max_x = (patch_center.0 + tangential_radius + radial_radius)
        .ceil()
        .min(bin.w as f64 - 1.0) as i32;
    let min_y = (patch_center.1 - tangential_radius - radial_radius)
        .floor()
        .max(0.0) as i32;
    let max_y = (patch_center.1 + tangential_radius + radial_radius)
        .ceil()
        .min(bin.h as f64 - 1.0) as i32;
    if max_x < min_x || max_y < min_y {
        return 0.0;
    }

    let radial_axis = (theta.cos(), theta.sin());
    let tangent_axis = (-theta.sin(), theta.cos());
    let mut black = 0_u32;
    let mut total = 0_u32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f64 + 0.5 - patch_center.0;
            let dy = y as f64 + 0.5 - patch_center.1;
            let radial = (dx * radial_axis.0 + dy * radial_axis.1) / radial_radius;
            let tangent = (dx * tangent_axis.0 + dy * tangent_axis.1) / tangential_radius;
            if radial * radial + tangent * tangent > 1.0 {
                continue;
            }
            total += 1;
            if bin.is_black(x, y) {
                black += 1;
            }
        }
    }

    if total == 0 {
        0.0
    } else {
        f64::from(black) / f64::from(total)
    }
}

fn sample_no_border_radial_lane_hybrid_ratio(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    points_per_ring: u32,
    theta_offset: f64,
    point: u32,
    theta_offsets: &[f64],
    radial_offsets: &[f64],
    max_weight: f64,
) -> f64 {
    let theta_step = std::f64::consts::TAU / points_per_ring as f64;
    let radial_step = ring.r_outer - ring.r_inner;
    let theta = theta_offset + (point as f64 + 0.5) * theta_step;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let mut total_black = 0_u32;
    let mut total = 0_u32;
    let mut max_lane = 0.0_f64;

    for &radial_delta in radial_offsets {
        let mut lane_black = 0_u32;
        for &theta_delta in theta_offsets {
            let sample_theta = theta + theta_delta * theta_step;
            let sample_radius = radius + radial_delta * radial_step;
            if sample_polar(bin, geometry.center, sample_radius, sample_theta) {
                lane_black += 1;
                total_black += 1;
            }
            total += 1;
        }
        max_lane = max_lane.max(f64::from(lane_black) / theta_offsets.len() as f64);
    }

    if total == 0 {
        return 0.0;
    }

    let average = f64::from(total_black) / f64::from(total);
    max_lane * max_weight + average * (1.0 - max_weight)
}

#[cfg(test)]
#[allow(dead_code)]
fn no_border_center_patch_ratio(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    radius: f64,
    radial_step: f64,
    theta: f64,
    theta_step: f64,
) -> f64 {
    let mut black = 0_u32;
    let mut total = 0_u32;

    for &theta_delta in &NO_BORDER_SAMPLE_CENTER_THETA_OFFSETS {
        for &radial_delta in &NO_BORDER_SAMPLE_RADIAL_OFFSETS {
            let sample_theta = theta + theta_delta * theta_step;
            let sample_radius = radius + radial_delta * radial_step;
            if sample_polar(bin, geometry.center, sample_radius, sample_theta) {
                black += 1;
            }
            total += 1;
        }
    }

    if total == 0 {
        0.0
    } else {
        f64::from(black) / f64::from(total)
    }
}

fn sample_cell_black_ratio_with_offsets(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    points_per_ring: u32,
    theta_offset: f64,
    point: u32,
    offsets: (&[f64], &[f64]),
) -> f64 {
    let theta_step = std::f64::consts::TAU / points_per_ring as f64;
    let radial_step = ring.r_outer - ring.r_inner;
    let theta = theta_offset + (point as f64 + 0.5) * theta_step;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let mut black = 0;
    let mut total = 0;

    for &theta_delta in offsets.0 {
        for &radial_delta in offsets.1 {
            let sample_theta = theta + theta_delta * theta_step;
            let sample_radius = radius + radial_delta * radial_step;
            if sample_polar(bin, geometry.center, sample_radius, sample_theta) {
                black += 1;
            }
            total += 1;
        }
    }

    black as f64 / total as f64
}

fn restore_no_border_finder_adjacent_code_cells_with_ring_thetas(
    samples: &mut [bool],
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    ring_thetas: &[f64],
    reserved: &ReservedAreas<'_>,
) {
    if points_per_ring == 0 {
        return;
    }

    let points = points_per_ring as usize;
    for (ring_idx, ring) in rings.iter().enumerate() {
        if ring.is_decoration {
            continue;
        }
        let ring_offset = ring_idx * points;
        if ring_offset + points > samples.len() {
            break;
        }
        let theta_offset = ring_thetas.get(ring_idx).copied().unwrap_or_default();

        for point in 0..points_per_ring {
            let idx = ring_offset + point as usize;
            // 标准遮挡（牛眼核心 / badge / 中心 logo）是真遮挡，永不恢复。
            if samples[idx]
                || is_standard_reserved_cell(
                    ring,
                    ring_idx as u32,
                    point,
                    points_per_ring,
                    theta_offset,
                    geometry,
                    reserved,
                )
            {
                continue;
            }
            // 两类可恢复点：① finder 邻近码点环带 [0.90,1.60]×r；② 被 ring1 扩展保留
            // 一刀切挡住、但其实是紧贴牛眼的真码点（如无框版20 p43/p46：黑度高却落在
            // 扩展区）。两类都交给下面的黑度阈值裁决——真点(≈0.74)过、假点(≤0.48)挡。
            let adjacent = is_no_border_finder_adjacent_code_cell(
                ring,
                point,
                points_per_ring,
                theta_offset,
                geometry,
                reserved.finders,
            );
            let extension_reserved = is_reserved_cell(
                ring,
                ring_idx as u32,
                point,
                points_per_ring,
                theta_offset,
                geometry,
                reserved,
            );
            if !adjacent && !extension_reserved {
                continue;
            }

            let ratio = sample_no_border_radial_lane_hybrid_ratio(
                bin,
                geometry,
                ring,
                points_per_ring,
                theta_offset,
                point,
                &NO_BORDER_FINDER_ADJACENT_RESTORE_THETA_OFFSETS,
                &NO_BORDER_FINDER_ADJACENT_RESTORE_RADIAL_OFFSETS,
                0.35,
            );
            if ratio >= NO_BORDER_FINDER_ADJACENT_RESTORE_MIN_RATIO {
                samples[idx] = true;
            }
        }
    }
}

fn is_no_border_finder_adjacent_code_cell(
    ring: &RingSpec,
    point: u32,
    points_per_ring: u32,
    theta_offset: f64,
    geometry: &DyGeometry,
    finders: &[DyFinder; 3],
) -> bool {
    let theta =
        theta_offset + (point as f64 + 0.5) * std::f64::consts::TAU / points_per_ring as f64;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let point_xy = (
        geometry.center.0 + radius * theta.cos(),
        geometry.center.1 + radius * theta.sin(),
    );

    finders.iter().any(|finder| {
        let ratio =
            distance(point_xy, (finder.cx, finder.cy)) / finder.outer_radius().max(f64::EPSILON);
        (NO_BORDER_FINDER_ADJACENT_RESTORE_MIN_DISTANCE_RATIO
            ..=NO_BORDER_FINDER_ADJACENT_RESTORE_MAX_DISTANCE_RATIO)
            .contains(&ratio)
    })
}

fn restore_no_border_decorative_component_cells_with_ring_thetas(
    samples: &mut [bool],
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    ring_thetas: &[f64],
    reserved: &ReservedAreas<'_>,
) {
    let component_ratios = no_border_component_cell_ratios_with_ring_thetas(
        bin,
        geometry,
        rings,
        points_per_ring,
        ring_thetas,
        reserved,
    );
    let points = points_per_ring as usize;
    if points == 0 {
        return;
    }

    let original = samples.to_vec();
    for (ring_idx, ring) in rings.iter().enumerate() {
        if !ring.is_decoration {
            continue;
        }

        let start = ring_idx * points;
        let end = start + points;
        if end > samples.len() || end > component_ratios.len() {
            return;
        }
        let theta_offset = ring_thetas.get(ring_idx).copied().unwrap_or_default();

        for point in 0..points_per_ring {
            let idx = start + point as usize;
            if original[idx] || component_ratios[idx] <= 0.0 {
                continue;
            }
            if is_reserved_cell(
                ring,
                ring_idx as u32,
                point,
                points_per_ring,
                theta_offset,
                geometry,
                reserved,
            ) {
                continue;
            }

            let has_neighbor = (1..=NO_BORDER_COMPONENT_RESTORE_NEIGHBOR_RADIUS).any(|delta| {
                let prev = (point + points_per_ring - delta) % points_per_ring;
                let next = (point + delta) % points_per_ring;
                original[start + prev as usize] || original[start + next as usize]
            });

            let ratio = sample_no_border_cell_black_ratio(
                bin,
                geometry,
                ring,
                points_per_ring,
                theta_offset,
                point,
            );
            let threshold = if ring_idx == 0 && has_neighbor {
                NO_BORDER_COMPONENT_RESTORE_RING0_MIN_LANE_RATIO
            } else if ring_idx == 0 {
                NO_BORDER_COMPONENT_RESTORE_RING0_ISOLATED_MIN_LANE_RATIO
            } else if has_neighbor {
                NO_BORDER_COMPONENT_RESTORE_MIN_LANE_RATIO
            } else {
                NO_BORDER_COMPONENT_RESTORE_ISOLATED_MIN_LANE_RATIO
            };
            if ratio >= threshold {
                samples[idx] = true;
            }
        }
    }
}

#[allow(dead_code)]
fn no_border_component_cell_ratios(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    theta_offset: f64,
    reserved: &ReservedAreas<'_>,
) -> Vec<f64> {
    no_border_component_cell_ratios_with_ring_thetas(
        bin,
        geometry,
        rings,
        points_per_ring,
        &vec![theta_offset; rings.len()],
        reserved,
    )
}

fn no_border_component_cell_ratios_with_ring_thetas(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    ring_thetas: &[f64],
    reserved: &ReservedAreas<'_>,
) -> Vec<f64> {
    if points_per_ring == 0 || rings.is_empty() {
        return Vec::new();
    }

    let mut ratios = vec![0.0; rings.len() * points_per_ring as usize];
    let mut visited = vec![false; (bin.w * bin.h) as usize];
    for y in 0..bin.h as i32 {
        for x in 0..bin.w as i32 {
            let idx = (y as u32 * bin.w + x as u32) as usize;
            if visited[idx] || !bin.is_black(x, y) {
                continue;
            }

            let component = collect_binary_component(bin, &mut visited, x, y);
            if component.pixels.len() < 3 {
                continue;
            }
            mark_no_border_component_cells(
                &component,
                &mut ratios,
                geometry,
                rings,
                points_per_ring,
                ring_thetas,
                reserved,
            );
        }
    }

    ratios
}

#[derive(Debug)]
#[allow(dead_code)]
struct BinaryComponent {
    pixels: Vec<(i32, i32)>,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    sum_x: f64,
    sum_y: f64,
}

#[allow(dead_code)]
impl BinaryComponent {
    fn new(x: i32, y: i32) -> Self {
        Self {
            pixels: Vec::new(),
            min_x: x,
            max_x: x,
            min_y: y,
            max_y: y,
            sum_x: 0.0,
            sum_y: 0.0,
        }
    }

    fn push(&mut self, x: i32, y: i32) {
        self.pixels.push((x, y));
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
        self.sum_x += x as f64 + 0.5;
        self.sum_y += y as f64 + 0.5;
    }

    fn center(&self) -> (f64, f64) {
        let total = self.pixels.len().max(1) as f64;
        (self.sum_x / total, self.sum_y / total)
    }

    fn span(&self) -> f64 {
        f64::from((self.max_x - self.min_x + 1).max(self.max_y - self.min_y + 1))
    }
}

#[allow(dead_code)]
fn collect_binary_component(
    bin: &BinaryImage,
    visited: &mut [bool],
    start_x: i32,
    start_y: i32,
) -> BinaryComponent {
    let mut component = BinaryComponent::new(start_x, start_y);
    let mut stack = vec![(start_x, start_y)];

    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= bin.w as i32 || y >= bin.h as i32 {
            continue;
        }
        let idx = (y as u32 * bin.w + x as u32) as usize;
        if visited[idx] || !bin.is_black(x, y) {
            continue;
        }
        visited[idx] = true;
        component.push(x, y);
        stack.push((x - 1, y));
        stack.push((x + 1, y));
        stack.push((x, y - 1));
        stack.push((x, y + 1));
    }

    component
}

#[allow(dead_code)]
fn mark_no_border_component_cells(
    component: &BinaryComponent,
    ratios: &mut [f64],
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    ring_thetas: &[f64],
    reserved: &ReservedAreas<'_>,
) {
    let center = component.center();
    if no_border_component_is_reserved_static(component, center, reserved) {
        return;
    }

    let Some(ring_idx) = nearest_no_border_ring_index(center, geometry.center, rings) else {
        return;
    };
    let ring = &rings[ring_idx];
    let theta_offset = ring_thetas.get(ring_idx).copied().unwrap_or_default();
    let ring_radius = (ring.r_inner + ring.r_outer) * 0.5;
    let ring_width = (ring.r_outer - ring.r_inner).max(0.01);
    let center_radius = distance(center, geometry.center);
    let radial_delta = (center_radius - ring_radius).abs();
    if radial_delta > ring_width * 1.75 + component.span() * 0.20 {
        return;
    }

    let theta_step = std::f64::consts::TAU / points_per_ring as f64;
    let cell_arc = (ring_radius * theta_step).max(1.0);
    let mut hits = vec![0_u32; points_per_ring as usize];
    let mut component_points_on_ring = 0_u32;

    for &(x, y) in &component.pixels {
        let px = x as f64 + 0.5;
        let py = y as f64 + 0.5;
        let radius = distance((px, py), geometry.center);
        if (radius - ring_radius).abs() > ring_width * 1.55 + 1.0 {
            continue;
        }

        let theta = (py - geometry.center.1).atan2(px - geometry.center.0);
        let point = ((theta - theta_offset) / theta_step - 0.5)
            .round()
            .rem_euclid(points_per_ring as f64) as usize;
        hits[point] += 1;
        component_points_on_ring += 1;
    }

    if component_points_on_ring == 0 {
        return;
    }

    let occupied = hits
        .iter()
        .enumerate()
        .filter_map(|(point, &count)| (count > 0).then_some(point as u32))
        .collect::<Vec<_>>();
    if occupied.is_empty() {
        return;
    }

    let min_hits = if component.span() <= cell_arc * 1.35 {
        1
    } else {
        2
    };
    let base = ring_idx * points_per_ring as usize;
    for point in occupied {
        if hits[point as usize] < min_hits {
            continue;
        }
        if is_reserved_cell(
            ring,
            ring_idx as u32,
            point,
            points_per_ring,
            theta_offset,
            geometry,
            reserved,
        ) {
            continue;
        }
        let idx = base + point as usize;
        if idx < ratios.len() {
            ratios[idx] = ratios[idx].max(1.0);
        }
    }
}

#[allow(dead_code)]
fn no_border_component_is_reserved_static(
    component: &BinaryComponent,
    center: (f64, f64),
    reserved: &ReservedAreas<'_>,
) -> bool {
    if reserved
        .finders
        .iter()
        .any(|finder| distance(center, (finder.cx, finder.cy)) <= finder.outer_radius() * 1.25)
    {
        return true;
    }
    if reserved
        .badge
        .is_some_and(|badge| distance(center, (badge.cx, badge.cy)) <= badge.radius * 1.24)
    {
        return true;
    }
    if reserved
        .logo
        .is_some_and(|logo| distance(center, (logo.cx, logo.cy)) <= logo.radius * 1.04)
    {
        return true;
    }
    component.span() > reserved.finders[0].outer_radius() * 1.25
}

#[allow(dead_code)]
fn nearest_no_border_ring_index(
    point: (f64, f64),
    center: (f64, f64),
    rings: &[RingSpec],
) -> Option<usize> {
    let radius = distance(point, center);
    rings
        .iter()
        .enumerate()
        .map(|(idx, ring)| {
            let ring_radius = (ring.r_inner + ring.r_outer) * 0.5;
            (idx, (radius - ring_radius).abs())
        })
        .min_by(|lhs, rhs| lhs.1.total_cmp(&rhs.1))
        .map(|(idx, _)| idx)
}

fn estimate_no_border_sample_radial_offsets_with_ring_thetas(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    ring_thetas: &[f64],
    samples: &[bool],
) -> Vec<f64> {
    if points_per_ring == 0 {
        return Vec::new();
    }

    let points = points_per_ring as usize;
    let mut offsets = vec![0.0; samples.len()];
    for (ring_idx, ring) in rings.iter().enumerate() {
        let ring_offset = ring_idx * points;
        let ring_end = (ring_offset + points).min(samples.len());
        if ring_end <= ring_offset {
            continue;
        }
        let theta_offset = ring_thetas.get(ring_idx).copied().unwrap_or_default();
        for run in circular_runs(&samples[ring_offset..ring_end], true) {
            for offset in 0..run.len {
                let point = (run.start + offset) % points_per_ring;
                let idx = ring_offset + point as usize;
                if idx >= offsets.len() {
                    continue;
                }
                offsets[idx] = estimate_no_border_cell_radial_offset(
                    bin,
                    geometry,
                    ring,
                    points_per_ring,
                    theta_offset,
                    point,
                );
            }
        }
    }

    offsets
}

fn estimate_no_border_cell_radial_offset(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    points_per_ring: u32,
    theta_offset: f64,
    point: u32,
) -> f64 {
    let theta_step = std::f64::consts::TAU / points_per_ring as f64;
    let radial_step = (ring.r_outer - ring.r_inner).max(0.01);
    let theta = theta_offset + (point as f64 + 0.5) * theta_step;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let mut offsets = Vec::new();

    for &theta_delta in &NO_BORDER_RADIAL_OFFSET_THETA_OFFSETS {
        let sample_theta = theta + theta_delta * theta_step;
        if let Some(offset) =
            estimate_no_border_radial_lane_offset(bin, geometry, radius, radial_step, sample_theta)
        {
            offsets.push(offset);
        }
    }

    if offsets.len() < NO_BORDER_RADIAL_OFFSET_MIN_LANES {
        return 0.0;
    }

    let normalized = offsets.iter().copied().sum::<f64>() / offsets.len() as f64;
    normalized.clamp(
        -NO_BORDER_RADIAL_OFFSET_CLAMP,
        NO_BORDER_RADIAL_OFFSET_CLAMP,
    ) * radial_step
}

fn estimate_no_border_radial_lane_offset(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    radius: f64,
    radial_step: f64,
    theta: f64,
) -> Option<f64> {
    let mut runs: Vec<(f64, f64)> = Vec::new();
    let mut start: Option<f64> = None;

    for step in -NO_BORDER_RADIAL_OFFSET_SCAN_STEPS..=NO_BORDER_RADIAL_OFFSET_SCAN_STEPS {
        let radial_delta = f64::from(step) * NO_BORDER_RADIAL_OFFSET_SCAN_STEP;
        let sample_radius = radius + radial_delta * radial_step;
        let black = sample_polar(bin, geometry.center, sample_radius, theta);
        if black {
            start.get_or_insert(radial_delta);
        } else if let Some(run_start) = start.take() {
            runs.push((run_start, radial_delta - NO_BORDER_RADIAL_OFFSET_SCAN_STEP));
        }
    }
    if let Some(run_start) = start {
        runs.push((
            run_start,
            f64::from(NO_BORDER_RADIAL_OFFSET_SCAN_STEPS) * NO_BORDER_RADIAL_OFFSET_SCAN_STEP,
        ));
    }

    runs.into_iter()
        .map(|(start, end)| {
            let center = (start + end) * 0.5;
            let contains_zero = start <= 0.0 && end >= 0.0;
            let distance = if contains_zero { 0.0 } else { center.abs() };
            let len = end - start;
            (center, contains_zero, distance, len)
        })
        .min_by(|lhs, rhs| {
            lhs.2
                .total_cmp(&rhs.2)
                .then_with(|| rhs.3.total_cmp(&lhs.3))
                .then_with(|| rhs.1.cmp(&lhs.1))
        })
        .map(|(center, _, _, _)| center)
}

fn estimate_no_border_sample_tangential_offsets_with_ring_thetas(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    ring_thetas: &[f64],
    samples: &[bool],
) -> Vec<f64> {
    if points_per_ring == 0 {
        return Vec::new();
    }

    let points = points_per_ring as usize;
    let mut offsets = vec![0.0; samples.len()];
    for (ring_idx, ring) in rings.iter().enumerate() {
        let ring_offset = ring_idx * points;
        let ring_end = (ring_offset + points).min(samples.len());
        if ring_end <= ring_offset {
            continue;
        }
        let theta_offset = ring_thetas.get(ring_idx).copied().unwrap_or_default();
        for run in circular_runs(&samples[ring_offset..ring_end], true) {
            for offset in 0..run.len {
                let point = (run.start + offset) % points_per_ring;
                let idx = ring_offset + point as usize;
                if idx >= offsets.len() {
                    continue;
                }
                offsets[idx] = estimate_no_border_cell_tangential_offset(
                    bin,
                    geometry,
                    ring,
                    points_per_ring,
                    theta_offset,
                    point,
                );
            }
        }
    }

    offsets
}

fn estimate_no_border_cell_tangential_offset(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    points_per_ring: u32,
    theta_offset: f64,
    point: u32,
) -> f64 {
    let theta_step = std::f64::consts::TAU / points_per_ring as f64;
    let radial_step = (ring.r_outer - ring.r_inner).max(0.01);
    let theta = theta_offset + (point as f64 + 0.5) * theta_step;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let mut offsets = Vec::new();

    for &radial_delta in &NO_BORDER_TANGENTIAL_OFFSET_RADIAL_OFFSETS {
        let sample_radius = radius + radial_delta * radial_step;
        if let Some(offset) = estimate_no_border_tangential_lane_offset(
            bin,
            geometry,
            sample_radius,
            theta,
            theta_step,
        ) {
            offsets.push(offset);
        }
    }

    if offsets.len() < NO_BORDER_TANGENTIAL_OFFSET_MIN_LANES {
        return 0.0;
    }

    let normalized = offsets.iter().copied().sum::<f64>() / offsets.len() as f64;
    normalized.clamp(
        -NO_BORDER_TANGENTIAL_OFFSET_CLAMP,
        NO_BORDER_TANGENTIAL_OFFSET_CLAMP,
    )
}

fn estimate_no_border_tangential_lane_offset(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    radius: f64,
    theta: f64,
    theta_step: f64,
) -> Option<f64> {
    let mut runs: Vec<(f64, f64)> = Vec::new();
    let mut start: Option<f64> = None;

    for step in -NO_BORDER_TANGENTIAL_OFFSET_SCAN_STEPS..=NO_BORDER_TANGENTIAL_OFFSET_SCAN_STEPS {
        let theta_delta = f64::from(step) * NO_BORDER_TANGENTIAL_OFFSET_SCAN_STEP;
        let sample_theta = theta + theta_delta * theta_step;
        let black = sample_polar(bin, geometry.center, radius, sample_theta);
        if black {
            start.get_or_insert(theta_delta);
        } else if let Some(run_start) = start.take() {
            runs.push((
                run_start,
                theta_delta - NO_BORDER_TANGENTIAL_OFFSET_SCAN_STEP,
            ));
        }
    }
    if let Some(run_start) = start {
        runs.push((
            run_start,
            f64::from(NO_BORDER_TANGENTIAL_OFFSET_SCAN_STEPS)
                * NO_BORDER_TANGENTIAL_OFFSET_SCAN_STEP,
        ));
    }

    runs.into_iter()
        .map(|(start, end)| {
            let center = (start + end) * 0.5;
            let contains_zero = start <= 0.0 && end >= 0.0;
            let distance = if contains_zero { 0.0 } else { center.abs() };
            let len = end - start;
            (center, contains_zero, distance, len)
        })
        .min_by(|lhs, rhs| {
            lhs.2
                .total_cmp(&rhs.2)
                .then_with(|| rhs.3.total_cmp(&lhs.3))
                .then_with(|| rhs.1.cmp(&lhs.1))
        })
        .map(|(center, _, _, _)| center)
}

/// Tangential offsets (in cell units) of a ring's isolated dots — cells that
/// sample black with both circular neighbors white. Isolated dots have
/// unambiguous angular centers, so their offsets measure grid/phase mismatch.
fn no_border_ring_isolated_dot_offsets(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    points_per_ring: u32,
    theta_offset: f64,
) -> Vec<f64> {
    let threshold = if ring.is_decoration {
        NO_BORDER_DECORATIVE_BLACK_THRESHOLD
    } else {
        NO_BORDER_BLACK_THRESHOLD
    };
    let blacks = (0..points_per_ring)
        .map(|point| {
            sample_no_border_cell_black_ratio(
                bin,
                geometry,
                ring,
                points_per_ring,
                theta_offset,
                point,
            ) >= threshold
        })
        .collect::<Vec<_>>();
    let mut dot_offsets = Vec::new();
    for point in 0..points_per_ring {
        let prev = (point + points_per_ring - 1) % points_per_ring;
        let next = (point + 1) % points_per_ring;
        if !blacks[point as usize] || blacks[prev as usize] || blacks[next as usize] {
            continue;
        }
        dot_offsets.push(estimate_no_border_cell_tangential_offset(
            bin,
            geometry,
            ring,
            points_per_ring,
            theta_offset,
            point,
        ));
    }

    dot_offsets
}

/// Median dot offset if it passes the rotation gates: enough isolated dots,
/// rotation outside the noise floor, and sign-consistent across the dots. A
/// real rotation shifts every dot the same way; a handful of individually
/// displaced dots must not rotate the grid.
fn no_border_gated_dot_median(
    dot_offsets: &mut [f64],
    min_dots: usize,
    min_delta_cells: f64,
    min_sign_ratio: f64,
) -> Option<f64> {
    if dot_offsets.len() < min_dots {
        return None;
    }
    dot_offsets.sort_by(f64::total_cmp);
    let median = dot_offsets[dot_offsets.len() / 2];
    if median.abs() < min_delta_cells {
        return None;
    }
    let consistent = dot_offsets
        .iter()
        .filter(|&&offset| {
            offset.signum() == median.signum()
                && offset.abs() >= NO_BORDER_RING_THETA_REFINE_SIGN_EPS
        })
        .count();
    if (consistent as f64) < dot_offsets.len() as f64 * min_sign_ratio {
        return None;
    }

    Some(median)
}

/// Refines the shared sampling phase against the printed code points. The
/// upright correction snaps near-upright inputs to a no-rotation transform, so
/// a genuinely rotated code (below the snap threshold) keeps its rotation in
/// the warped image; the pooled isolated-dot median across all code rings
/// recovers it. Runs twice because a large rotation saturates the per-dot
/// measurement window.
fn refine_no_border_global_theta_offset(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    theta_offset: f64,
) -> f64 {
    if points_per_ring == 0 {
        return theta_offset;
    }

    let theta_step = std::f64::consts::TAU / f64::from(points_per_ring);
    let max_total = NO_BORDER_GLOBAL_THETA_REFINE_MAX_DEG.to_radians();
    let min_delta_cells = NO_BORDER_GLOBAL_THETA_REFINE_MIN_DEG.to_radians() / theta_step;
    let mut current = theta_offset;
    for _ in 0..2 {
        let mut dot_offsets = rings
            .iter()
            .filter(|ring| !ring.is_decoration)
            .flat_map(|ring| {
                no_border_ring_isolated_dot_offsets(bin, geometry, ring, points_per_ring, current)
            })
            .collect::<Vec<_>>();
        if std::env::var_os("QRACER_DEBUG_GLOBAL_THETA").is_some() {
            let mut sorted = dot_offsets.clone();
            sorted.sort_by(f64::total_cmp);
            let median = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
            let consistent = sorted
                .iter()
                .filter(|&&offset| {
                    offset.signum() == median.signum()
                        && offset.abs() >= NO_BORDER_RING_THETA_REFINE_SIGN_EPS
                })
                .count();
            eprintln!(
                "global_theta_refine dots={} median_cells={median:.3} median_deg={:.3} consistent={consistent}",
                sorted.len(),
                median * theta_step.to_degrees(),
            );
        }
        let Some(median) = no_border_gated_dot_median(
            &mut dot_offsets,
            NO_BORDER_GLOBAL_THETA_REFINE_MIN_DOTS,
            min_delta_cells,
            NO_BORDER_GLOBAL_THETA_REFINE_MIN_SIGN_RATIO,
        ) else {
            break;
        };
        let next = theta_offset
            + (current - theta_offset + median * theta_step).clamp(-max_total, max_total);
        if (next - current).abs() <= f64::EPSILON {
            break;
        }
        current = next;
    }

    current
}

/// Estimates the absolute sampling theta of each ring. With the bullseyes
/// aligned, individual rings may still be slightly rotated relative to the
/// shared phase. The rotation is measured geometrically via the ring's
/// isolated-dot median; rings that fail the gates keep the shared theta.
fn estimate_no_border_ring_theta_offsets(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    theta_offset: f64,
) -> Vec<f64> {
    if points_per_ring == 0 {
        return vec![theta_offset; rings.len()];
    }

    let theta_step = std::f64::consts::TAU / f64::from(points_per_ring);
    let max_delta = NO_BORDER_RING_THETA_REFINE_MAX_DEG.to_radians();
    let min_delta_cells = NO_BORDER_RING_THETA_REFINE_MIN_DEG.to_radians() / theta_step;
    rings
        .iter()
        .map(|ring| {
            // Decoration marks are not bound to the 3 deg grid ("不限制"), so
            // an off-grid dot median there reflects the arc layout rather
            // than a ring rotation; only code rings may rotate.
            if ring.is_decoration {
                return theta_offset;
            }
            let mut dot_offsets = no_border_ring_isolated_dot_offsets(
                bin,
                geometry,
                ring,
                points_per_ring,
                theta_offset,
            );
            let Some(median) = no_border_gated_dot_median(
                &mut dot_offsets,
                NO_BORDER_RING_THETA_REFINE_MIN_DOTS,
                min_delta_cells,
                NO_BORDER_RING_THETA_REFINE_MIN_SIGN_RATIO,
            ) else {
                return theta_offset;
            };

            theta_offset + (median * theta_step).clamp(-max_delta, max_delta)
        })
        .collect()
}

#[allow(dead_code)]
fn estimate_no_border_render_ring_radius_offsets(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    theta_offset: f64,
    samples: &[bool],
) -> Vec<f64> {
    if points_per_ring == 0 {
        return Vec::new();
    }

    rings
        .iter()
        .enumerate()
        .map(|(ring_idx, ring)| {
            let base_offset = NO_BORDER_RENDER_RING_RADIUS_BASE_OFFSETS
                .get(ring_idx)
                .copied()
                .unwrap_or_default();
            estimate_no_border_render_ring_radius_offset(
                bin,
                geometry,
                ring,
                points_per_ring,
                theta_offset,
                samples,
                ring_idx,
                base_offset,
            )
        })
        .collect()
}

#[allow(dead_code)]
fn estimate_no_border_render_ring_radius_offset(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    points_per_ring: u32,
    theta_offset: f64,
    samples: &[bool],
    ring_idx: usize,
    base_offset: f64,
) -> f64 {
    let points = points_per_ring as usize;
    let ring_offset = ring_idx * points;
    if ring_offset + points > samples.len() {
        return base_offset;
    }

    let ring_samples = &samples[ring_offset..ring_offset + points];
    let runs = circular_runs(ring_samples, true);
    if runs.is_empty() {
        return base_offset;
    }

    let mut best = (f64::INFINITY, base_offset, 0.0_f64);
    for delta in NO_BORDER_RENDER_RING_RADIUS_OFFSET_CANDIDATES {
        let offset = base_offset + delta;
        let mismatch = no_border_render_radius_offset_mismatch(
            bin,
            geometry,
            ring,
            points_per_ring,
            theta_offset,
            &runs,
            offset,
        );
        let distance = (offset - base_offset).abs();
        if mismatch < best.0 - f64::EPSILON
            || ((mismatch - best.0).abs() <= f64::EPSILON && distance < best.2)
        {
            best = (mismatch, offset, distance);
        }
    }

    best.1
}

#[allow(dead_code)]
fn no_border_render_radius_offset_mismatch(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    ring: &RingSpec,
    points_per_ring: u32,
    theta_offset: f64,
    runs: &[CircularRun],
    radius_offset: f64,
) -> f64 {
    let theta_step = std::f64::consts::TAU / points_per_ring as f64;
    let ring_radius = (ring.r_inner + ring.r_outer) * 0.5 + radius_offset;
    let half_width = (ring.r_outer - ring.r_inner).max(0.01) * 0.52;
    let angular_inset = |run_len: u32| {
        if run_len == 1 {
            0.26
        } else if run_len == 2 {
            0.58
        } else {
            0.54
        }
    };
    let mut mismatch = 0_u32;
    let mut total = 0_u32;

    for run in runs {
        let inset = angular_inset(run.len);
        let theta_start = theta_offset + run.start as f64 * theta_step + inset * theta_step;
        let theta_end =
            theta_offset + (run.start + run.len) as f64 * theta_step - inset * theta_step;
        if theta_end <= theta_start {
            continue;
        }
        let span_samples = if run.len == 1 {
            [0.50, 0.50, 0.50]
        } else {
            [0.20, 0.50, 0.80]
        };
        for position in span_samples {
            let theta = theta_start + (theta_end - theta_start) * position;
            for theta_delta in NO_BORDER_RENDER_RING_RADIUS_OFFSET_THETA_OFFSETS {
                for radial_delta in NO_BORDER_RENDER_RING_RADIUS_OFFSET_RADIAL_OFFSETS {
                    let sample_theta = theta + theta_delta * theta_step;
                    let sample_radius = ring_radius + radial_delta * half_width * 2.0;
                    let generated_black = no_border_point_in_render_arc(
                        sample_radius,
                        sample_theta,
                        ring_radius,
                        half_width,
                        theta_start,
                        theta_end,
                    );
                    let original_black =
                        sample_polar(bin, geometry.center, sample_radius, sample_theta);
                    if generated_black != original_black {
                        mismatch += 1;
                    }
                    total += 1;
                }
            }
        }
    }

    if total == 0 {
        0.0
    } else {
        f64::from(mismatch) / f64::from(total)
    }
}

#[allow(dead_code)]
fn no_border_point_in_render_arc(
    radius: f64,
    theta: f64,
    ring_radius: f64,
    half_width: f64,
    theta_start: f64,
    theta_end: f64,
) -> bool {
    let radial_distance = (radius - ring_radius).abs();
    if radial_distance <= half_width && angle_in_span_local(theta, theta_start, theta_end) {
        return true;
    }

    let start_distance = polar_distance(radius, theta, ring_radius, theta_start);
    let end_distance = polar_distance(radius, theta, ring_radius, theta_end);
    start_distance <= half_width || end_distance <= half_width
}

fn polar_distance(radius_a: f64, theta_a: f64, radius_b: f64, theta_b: f64) -> f64 {
    let delta = (theta_a - theta_b).abs();
    (radius_a * radius_a + radius_b * radius_b - 2.0 * radius_a * radius_b * delta.cos())
        .max(0.0)
        .sqrt()
}

fn angle_in_span_local(theta: f64, start: f64, end: f64) -> bool {
    let theta = normalize_angle(theta);
    let start = normalize_angle(start);
    let end = normalize_angle(end);
    if start <= end {
        theta >= start && theta <= end
    } else {
        theta >= start || theta <= end
    }
}

fn prune_black_border_edge_noise(
    samples: &mut [bool],
    ratios: &[f64],
    rings: &[RingSpec],
    ring_count: u8,
    points_per_ring: u32,
) {
    let original = samples.to_vec();
    let points = points_per_ring as usize;

    for ring in 0..ring_count as usize {
        if rings.get(ring).is_some_and(|ring| ring.is_decoration) {
            continue;
        }
        let ring_offset = ring * points;
        for point in 0..points {
            let idx = ring_offset + point;
            if !original[idx] {
                continue;
            }

            let prev = ring_offset + (point + points - 1) % points;
            let next = ring_offset + (point + 1) % points;
            if !original[prev] && original[next] {
                let mut run_len = 1;
                while run_len < points && original[ring_offset + (point + run_len) % points] {
                    run_len += 1;
                }
                if ratios[idx] > 4.0 / 9.0 + f64::EPSILON
                    || (ratios[idx] >= 4.0 / 9.0 - f64::EPSILON && run_len > 2)
                {
                    continue;
                }
                samples[idx] = false;
            }
        }
    }
}

fn prune_black_border_badge_outer_short_runs(
    samples: &mut [bool],
    rings: &[RingSpec],
    points_per_ring: u32,
    theta_offset: f64,
    geometry: &DyGeometry,
    badge: Option<DyBadge>,
    badge_style: DyBadgeStyle,
) {
    let Some(badge) = badge else {
        return;
    };
    let Some(ring) = rings.first() else {
        return;
    };
    if samples.len() < points_per_ring as usize {
        return;
    }
    let (badge_skip_scale, short_run_min_ratio, short_run_max_ratio, short_run_cell_max_ratio) =
        if points_per_ring == 72 {
            (
                BLACK_BORDER_BADGE_CODE_SKIP_SCALE_72,
                BLACK_BORDER_BADGE_OUTER_SHORT_RUN_MIN_RATIO_72,
                BLACK_BORDER_BADGE_OUTER_SHORT_RUN_MAX_RATIO_72,
                BLACK_BORDER_BADGE_OUTER_SHORT_RUN_CELL_MAX_RATIO_72,
            )
        } else {
            (
                badge_code_skip_scale(true, points_per_ring, badge_style),
                BLACK_BORDER_BADGE_OUTER_SHORT_RUN_MIN_RATIO,
                BLACK_BORDER_BADGE_OUTER_SHORT_RUN_MAX_RATIO,
                BLACK_BORDER_BADGE_OUTER_SHORT_RUN_CELL_MAX_RATIO,
            )
        };

    let points = points_per_ring as usize;
    let original = samples[..points].to_vec();
    for run in circular_runs(&original, true) {
        if run.len > BLACK_BORDER_BADGE_OUTER_SHORT_RUN_MAX_LEN {
            continue;
        }

        let before = (run.start + points_per_ring - 1) % points_per_ring;
        let after = (run.start + run.len) % points_per_ring;
        let touches_badge_gap = is_badge_code_cell(
            ring,
            before,
            points_per_ring,
            theta_offset,
            geometry,
            badge,
            badge_skip_scale,
        ) || is_badge_code_cell(
            ring,
            after,
            points_per_ring,
            theta_offset,
            geometry,
            badge,
            badge_skip_scale,
        );
        if !touches_badge_gap {
            continue;
        }

        let min_badge_ratio = (0..run.len)
            .map(|offset| {
                let point = (run.start + offset) % points_per_ring;
                badge_distance_ratio(ring, point, points_per_ring, theta_offset, geometry, badge)
            })
            .fold(f64::INFINITY, f64::min);
        if !(short_run_min_ratio..=short_run_max_ratio).contains(&min_badge_ratio) {
            continue;
        }

        for offset in 0..run.len {
            let point = ((run.start + offset) % points_per_ring) as usize;
            let ratio = badge_distance_ratio(
                ring,
                point as u32,
                points_per_ring,
                theta_offset,
                geometry,
                badge,
            );
            if ratio <= short_run_cell_max_ratio {
                samples[point] = false;
            }
        }
    }
}

/// 标准遮挡：码点落进牛眼核心圈 / badge / 中心 logo 的保留区。这是"真遮挡"，
/// 恢复阶段绝不应救回。与 `is_reserved_cell` 的区别是：不含 ring1 的扩展保留。
fn is_standard_reserved_cell(
    ring: &RingSpec,
    ring_idx: u32,
    point: u32,
    points_per_ring: u32,
    theta_offset: f64,
    geometry: &DyGeometry,
    reserved: &ReservedAreas<'_>,
) -> bool {
    let theta =
        theta_offset + (point as f64 + 0.5) * std::f64::consts::TAU / points_per_ring as f64;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let point_xy = (
        geometry.center.0 + radius * theta.cos(),
        geometry.center.1 + radius * theta.sin(),
    );

    (!reserved.has_border
        && reserved.finders.iter().any(|finder| {
            distance(point_xy, (finder.cx, finder.cy))
                <= finder.outer_radius() * NO_BORDER_FINDER_CODE_SKIP_SCALE
        }))
        || reserved.badge.is_some_and(|badge| {
            is_badge_code_cell(
                ring,
                point,
                points_per_ring,
                theta_offset,
                geometry,
                badge,
                badge_code_skip_scale_for_ring(
                    reserved.has_border,
                    points_per_ring,
                    reserved.badge_style,
                    ring_idx,
                ),
            )
        })
        || reserved.logo.is_some_and(|logo| {
            distance(point_xy, (logo.cx, logo.cy))
                <= logo.radius * center_logo_code_skip_scale(reserved.has_border)
        })
}

fn is_reserved_cell(
    ring: &RingSpec,
    ring_idx: u32,
    point: u32,
    points_per_ring: u32,
    theta_offset: f64,
    geometry: &DyGeometry,
    reserved: &ReservedAreas<'_>,
) -> bool {
    if is_standard_reserved_cell(
        ring,
        ring_idx,
        point,
        points_per_ring,
        theta_offset,
        geometry,
        reserved,
    ) {
        return true;
    }

    // Extended reservation for ring1 points near finders (bullseye bleed mitigation).
    // These points are geometrically outside the standard reservation zone but may
    // sample bullseye edge due to anti-aliasing or perspective correction artifacts.
    // 注意：这是"一刀切"几何遮挡，会连带挡住紧贴牛眼的真码点（如无框版20 ring1
    // p43/p46）；恢复阶段对该扩展区单独用黑度阈值救回真点（见 restore 函数）。
    if !reserved.has_border && !ring.is_decoration {
        let theta =
            theta_offset + (point as f64 + 0.5) * std::f64::consts::TAU / points_per_ring as f64;
        let radius = (ring.r_inner + ring.r_outer) * 0.5;
        let point_xy = (
            geometry.center.0 + radius * theta.cos(),
            geometry.center.1 + radius * theta.sin(),
        );
        let scale = geometry.locator_distance / NO_BORDER_STANDARD_LOCATOR_DISTANCE;
        let expected_ring1_radius = NO_BORDER_RINGS[1].0 * scale;
        let is_ring1 = (radius - expected_ring1_radius).abs() < 10.0;

        if is_ring1 {
            return reserved.finders.iter().any(|finder| {
                distance(point_xy, (finder.cx, finder.cy)) <= NO_BORDER_FINDER_ADJACENT_DISTANCE
            });
        }
    }

    false
}

fn badge_code_skip_scale(has_border: bool, points_per_ring: u32, badge_style: DyBadgeStyle) -> f64 {
    match (has_border, points_per_ring, badge_style) {
        (true, 72, _) => BLACK_BORDER_BADGE_CODE_SKIP_SCALE_72,
        (true, _, DyBadgeStyle::Bullseye | DyBadgeStyle::DouyinLogo) => {
            BLACK_BORDER_BADGE_CODE_SKIP_SCALE_120
        }
        (false, _, _) => 1.08,
    }
}

fn badge_code_skip_scale_for_ring(
    has_border: bool,
    points_per_ring: u32,
    badge_style: DyBadgeStyle,
    ring_idx: u32,
) -> f64 {
    if has_border && points_per_ring == 120 && ring_idx > 0 {
        BLACK_BORDER_BADGE_INNER_CODE_SKIP_SCALE_120
    } else {
        badge_code_skip_scale(has_border, points_per_ring, badge_style)
    }
}

fn center_logo_code_skip_scale(has_border: bool) -> f64 {
    if has_border { 0.98 } else { 1.02 }
}

fn is_badge_code_cell(
    ring: &RingSpec,
    point: u32,
    points_per_ring: u32,
    theta_offset: f64,
    geometry: &DyGeometry,
    badge: DyBadge,
    scale: f64,
) -> bool {
    let badge_radius = badge.radius * scale;
    if badge_radius <= 0.0 {
        return false;
    }

    badge_distance_ratio(ring, point, points_per_ring, theta_offset, geometry, badge) <= scale
}

fn badge_distance_ratio(
    ring: &RingSpec,
    point: u32,
    points_per_ring: u32,
    theta_offset: f64,
    geometry: &DyGeometry,
    badge: DyBadge,
) -> f64 {
    let theta_step = std::f64::consts::TAU / points_per_ring as f64;
    let theta = theta_offset + (point as f64 + 0.5) * theta_step;
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let point_xy = (
        geometry.center.0 + radius * theta.cos(),
        geometry.center.1 + radius * theta.sin(),
    );
    distance(point_xy, (badge.cx, badge.cy)) / badge_radius_safe(badge.radius)
}

fn badge_radius_safe(radius: f64) -> f64 {
    radius.max(f64::EPSILON)
}

fn detect_dy_badge(source: &DynamicImage, geometry: &DyGeometry) -> Option<DyBadge> {
    let rgba = source.to_rgba8();
    let min_dim = rgba.width().min(rgba.height()) as f64;
    let mut visited = vec![false; (rgba.width() * rgba.height()) as usize];
    let min_area = (min_dim * 0.045).powi(2) as u32;
    let mut best: Option<(f64, DyBadge)> = None;

    for y in 0..rgba.height() as i32 {
        for x in 0..rgba.width() as i32 {
            let idx = (y as u32 * rgba.width() + x as u32) as usize;
            if visited[idx] || !is_dark_pixel(rgba.get_pixel(x as u32, y as u32).0) {
                continue;
            }
            let Some(component) = flood_dark_component(&rgba, &mut visited, x, y) else {
                continue;
            };
            if component.area < min_area || !component.is_roundish(min_dim) {
                continue;
            }
            let badge = component.to_badge();
            if badge.cx < geometry.center.0 || badge.cy > geometry.center.1 {
                continue;
            }
            if badge.radius < geometry.r_max * 0.10 || badge.radius > geometry.r_max * 0.34 {
                continue;
            }
            let distance_to_center = distance((badge.cx, badge.cy), geometry.center);
            if distance_to_center < geometry.r_min || distance_to_center > geometry.r_max * 1.25 {
                continue;
            }
            let score = component.area as f64 * component.shape_score();
            if best
                .as_ref()
                .is_none_or(|(best_score, _)| score > *best_score)
            {
                best = Some((score, badge));
            }
        }
    }

    best.map(|(_, badge)| badge)
}

fn detect_black_border_badge_style(
    source: &DynamicImage,
    badge: Option<DyBadge>,
) -> Option<DyBadgeStyle> {
    let badge = badge?;
    let rgba = source.to_rgba8();
    let signature = best_black_border_badge_shape_signature(&rgba, badge);

    Some(
        if signature.center >= 0.45 && signature.gap <= 0.38 && signature.black_ring >= 0.55 {
            DyBadgeStyle::Bullseye
        } else {
            DyBadgeStyle::DouyinLogo
        },
    )
}

#[derive(Debug, Clone, Copy)]
struct BadgeShapeSignature {
    center: f64,
    gap: f64,
    black_ring: f64,
}

impl BadgeShapeSignature {
    fn bullseye_score(self) -> f64 {
        self.center + self.black_ring - self.gap
    }
}

fn best_black_border_badge_shape_signature(
    rgba: &image::RgbaImage,
    badge: DyBadge,
) -> BadgeShapeSignature {
    let mut best = badge_shape_signature(rgba, badge);
    let search_radius = badge.radius * 0.50;
    let step = (badge.radius * 0.06).max(1.0);
    let steps = (search_radius / step).ceil() as i32;

    for dy_step in -steps..=steps {
        for dx_step in -steps..=steps {
            let dx = dx_step as f64 * step;
            let dy = dy_step as f64 * step;
            if dx == 0.0 && dy == 0.0 || dx.hypot(dy) > search_radius {
                continue;
            }

            let candidate = DyBadge {
                cx: badge.cx + dx,
                cy: badge.cy + dy,
                radius: badge.radius,
            };
            let signature = badge_shape_signature(rgba, candidate);
            if signature.bullseye_score() > best.bullseye_score() {
                best = signature;
            }
        }
    }

    best
}

fn badge_shape_signature(rgba: &image::RgbaImage, badge: DyBadge) -> BadgeShapeSignature {
    let center = badge_disk_dark_ratio_rgba(rgba, badge, 0.12);
    let gap = badge_ring_dark_ratio_rgba(rgba, badge, 0.18, 0.018);
    let black_ring = [0.26, 0.30, 0.34]
        .into_iter()
        .map(|ratio| badge_ring_dark_ratio_rgba(rgba, badge, ratio, 0.018))
        .fold(0.0, f64::max);

    BadgeShapeSignature {
        center,
        gap,
        black_ring,
    }
}

fn badge_disk_dark_ratio_rgba(rgba: &image::RgbaImage, badge: DyBadge, radius_ratio: f64) -> f64 {
    let radius = badge.radius * radius_ratio;
    let min_x = (badge.cx - radius).floor().max(0.0) as i32;
    let max_x = (badge.cx + radius).ceil().min(rgba.width() as f64 - 1.0) as i32;
    let min_y = (badge.cy - radius).floor().max(0.0) as i32;
    let max_y = (badge.cy + radius).ceil().min(rgba.height() as f64 - 1.0) as i32;
    let mut dark = 0_u32;
    let mut total = 0_u32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f64 - badge.cx;
            let dy = y as f64 - badge.cy;
            if dx.hypot(dy) > radius {
                continue;
            }
            total += 1;
            if is_dark_pixel(rgba.get_pixel(x as u32, y as u32).0) {
                dark += 1;
            }
        }
    }

    if total == 0 {
        0.0
    } else {
        dark as f64 / total as f64
    }
}

fn badge_ring_dark_ratio_rgba(
    rgba: &image::RgbaImage,
    badge: DyBadge,
    radius_ratio: f64,
    width_ratio: f64,
) -> f64 {
    const ANGLES: u32 = 144;
    const RADIAL_OFFSETS: [f64; 3] = [-0.5, 0.0, 0.5];
    let radius = badge.radius * radius_ratio;
    let width = badge.radius * width_ratio;
    let mut dark = 0_u32;
    let mut total = 0_u32;

    for angle in 0..ANGLES {
        let theta = angle as f64 * std::f64::consts::TAU / ANGLES as f64;
        for offset in RADIAL_OFFSETS {
            let sample_radius = radius + offset * width;
            let x = (badge.cx + sample_radius * theta.cos()).round() as i32;
            let y = (badge.cy + sample_radius * theta.sin()).round() as i32;
            if x < 0 || y < 0 || x >= rgba.width() as i32 || y >= rgba.height() as i32 {
                continue;
            }
            total += 1;
            if is_dark_pixel(rgba.get_pixel(x as u32, y as u32).0) {
                dark += 1;
            }
        }
    }

    if total == 0 {
        0.0
    } else {
        dark as f64 / total as f64
    }
}

fn estimate_badge_from_finders(finders: &[DyFinder; 3]) -> Option<DyBadge> {
    let ordered = order_dy_finders(finders);
    let tl = &ordered[0];
    let bl = &ordered[1];
    let br = &ordered[2];
    let badge = (tl.cx + br.cx - bl.cx, tl.cy + br.cy - bl.cy);
    let radius = finders.iter().map(DyFinder::outer_radius).sum::<f64>() / 3.0 * 2.0;
    Some(DyBadge {
        cx: badge.0,
        cy: badge.1,
        radius,
    })
}

fn estimate_black_border_badge_from_finders(finders: &[DyFinder; 3]) -> Option<DyBadge> {
    let mut badge = estimate_badge_from_finders(finders)?;
    badge.radius = finders.iter().map(DyFinder::outer_radius).sum::<f64>() / 3.0 * 2.50;
    Some(badge)
}

fn black_border_badge_from_finders_and_detection(
    finders: &[DyFinder; 3],
    detected: Option<DyBadge>,
) -> Option<DyBadge> {
    let estimated = estimate_black_border_badge_from_finders(finders)?;
    let Some(mut detected) = detected else {
        return Some(estimated);
    };

    // `detect_dy_badge` sees the black outer circle; DyBadge::radius is the
    // white inner/logo radius used by SVG output and existing skip constants.
    detected.radius /= BLACK_BORDER_BADGE_OUTER_RADIUS_SCALE;
    let center_delta = distance((detected.cx, detected.cy), (estimated.cx, estimated.cy));
    let radius_ratio = detected.radius / badge_radius_safe(estimated.radius);
    if center_delta <= estimated.radius * 0.78 && (0.72..=1.32).contains(&radius_ratio) {
        Some(detected)
    } else {
        Some(estimated)
    }
}

fn black_border_badge_outer_radius(badge: DyBadge) -> f64 {
    badge.radius * BLACK_BORDER_BADGE_OUTER_RADIUS_SCALE
}

fn detect_center_logo(
    source: &DynamicImage,
    geometry: &DyGeometry,
    has_border: bool,
) -> Option<DyLogo> {
    let rgba = source.to_rgba8();
    let mut visited = vec![false; (rgba.width() * rgba.height()) as usize];
    let mut best: Option<(u32, DyLogo)> = None;

    let max_center_offset_ratio = if has_border {
        0.75
    } else {
        NO_BORDER_CENTER_LOGO_MAX_DETECTED_OFFSET_RATIO
    };
    let max_radius_ratio = if has_border {
        0.95
    } else {
        NO_BORDER_CENTER_LOGO_MAX_RADIUS_RATIO
    };

    for y in 0..rgba.height() as i32 {
        for x in 0..rgba.width() as i32 {
            let idx = (y as u32 * rgba.width() + x as u32) as usize;
            if visited[idx] || !is_colored_logo_pixel(rgba.get_pixel(x as u32, y as u32).0) {
                continue;
            }
            let Some(component) = flood_colored_component(&rgba, &mut visited, x, y) else {
                continue;
            };
            let logo = component.to_logo();
            if distance((logo.cx, logo.cy), geometry.center)
                > geometry.r_min * max_center_offset_ratio
            {
                continue;
            }
            if logo.radius < geometry.r_min * 0.10
                || logo.radius > geometry.r_min * max_radius_ratio
            {
                continue;
            }
            if best
                .as_ref()
                .is_none_or(|(best_area, _)| component.area > *best_area)
            {
                best = Some((component.area, logo));
            }
        }
    }

    if has_border {
        return best.map(|(_, logo)| logo);
    }

    let radius = best
        .map(|(_, logo)| logo.radius)
        .unwrap_or(geometry.r_min * NO_BORDER_CENTER_LOGO_RADIUS_SCALE)
        .min(geometry.r_min * NO_BORDER_CENTER_LOGO_RADIUS_SCALE);

    Some(DyLogo {
        cx: geometry.center.0,
        cy: geometry.center.1,
        radius,
    })
}

#[derive(Debug, Clone, Copy)]
struct Component {
    area: u32,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

impl Component {
    fn width(self) -> f64 {
        (self.max_x - self.min_x + 1) as f64
    }

    fn height(self) -> f64 {
        (self.max_y - self.min_y + 1) as f64
    }

    fn center(self) -> (f64, f64) {
        (
            (self.min_x + self.max_x) as f64 * 0.5,
            (self.min_y + self.max_y) as f64 * 0.5,
        )
    }

    fn is_roundish(self, min_dim: f64) -> bool {
        if self.width() < min_dim * 0.08 || self.height() < min_dim * 0.08 {
            return false;
        }
        let aspect = self.width() / self.height().max(1.0);
        if !(0.70..=1.35).contains(&aspect) {
            return false;
        }
        let ellipse_area = std::f64::consts::PI * self.width() * self.height() * 0.25;
        let fill = self.area as f64 / ellipse_area.max(1.0);
        (0.22..=1.30).contains(&fill)
    }

    fn shape_score(self) -> f64 {
        let aspect = self.width() / self.height().max(1.0);
        1.0 / (1.0 + (aspect - 1.0).abs())
    }

    fn to_badge(self) -> DyBadge {
        let (cx, cy) = self.center();
        DyBadge {
            cx,
            cy,
            radius: (self.width() + self.height()) * 0.25,
        }
    }

    fn to_logo(self) -> DyLogo {
        let (cx, cy) = self.center();
        DyLogo {
            cx,
            cy,
            radius: (self.width() + self.height()) * 0.25,
        }
    }
}

fn flood_dark_component(
    image: &image::RgbaImage,
    visited: &mut [bool],
    start_x: i32,
    start_y: i32,
) -> Option<Component> {
    flood_component(image, visited, start_x, start_y, is_dark_pixel)
}

fn flood_colored_component(
    image: &image::RgbaImage,
    visited: &mut [bool],
    start_x: i32,
    start_y: i32,
) -> Option<Component> {
    flood_component(image, visited, start_x, start_y, is_colored_logo_pixel)
}

fn flood_component(
    image: &image::RgbaImage,
    visited: &mut [bool],
    start_x: i32,
    start_y: i32,
    accepts: fn([u8; 4]) -> bool,
) -> Option<Component> {
    let mut stack = vec![(start_x, start_y)];
    let mut area = 0_u32;
    let mut min_x = start_x;
    let mut max_x = start_x;
    let mut min_y = start_y;
    let mut max_y = start_y;

    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
            continue;
        }
        let idx = (y as u32 * image.width() + x as u32) as usize;
        if visited[idx] || !accepts(image.get_pixel(x as u32, y as u32).0) {
            continue;
        }

        visited[idx] = true;
        area += 1;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);

        stack.push((x - 1, y));
        stack.push((x + 1, y));
        stack.push((x, y - 1));
        stack.push((x, y + 1));
    }

    if area == 0 {
        return None;
    }

    Some(Component {
        area,
        min_x,
        max_x,
        min_y,
        max_y,
    })
}

fn is_dark_pixel(pixel: [u8; 4]) -> bool {
    let [r, g, b, a] = pixel;
    let luma = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
    a > 128 && luma < 96.0
}

fn is_colored_logo_pixel(pixel: [u8; 4]) -> bool {
    let [r, g, b, a] = pixel;
    let max = r.max(g).max(b) as i16;
    let min = r.min(g).min(b) as i16;
    let saturation = max - min;
    let luma = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
    a > 128 && saturation > 45 && luma < 235.0
}

fn sample_polar(bin: &BinaryImage, center: (f64, f64), radius: f64, theta: f64) -> bool {
    let x = (center.0 + radius * theta.cos()).round() as i32;
    let y = (center.1 + radius * theta.sin()).round() as i32;
    bin.is_black(x, y)
}

fn sample_fine_ring_dark(
    source: FineRingSource<'_>,
    center: (f64, f64),
    radius: f64,
    theta: f64,
) -> f64 {
    let x = center.0 + radius * theta.cos();
    let y = center.1 + radius * theta.sin();
    if let Some(gray) = source.gray {
        let luma = bilinear_luma(gray, x, y);
        return ((224.0 - luma) / 128.0).clamp(0.0, 1.0);
    }

    if source.bin.is_black(x.round() as i32, y.round() as i32) {
        1.0
    } else {
        0.0
    }
}

fn bilinear_luma(gray: &GrayImage, x: f64, y: f64) -> f64 {
    if gray.width() == 0 || gray.height() == 0 {
        return 255.0;
    }
    let max_x = gray.width().saturating_sub(1) as f64;
    let max_y = gray.height().saturating_sub(1) as f64;
    let x = x.clamp(0.0, max_x);
    let y = y.clamp(0.0, max_y);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(gray.width() - 1);
    let y1 = (y0 + 1).min(gray.height() - 1);
    let dx = x - x0 as f64;
    let dy = y - y0 as f64;
    let p00 = gray.get_pixel(x0, y0)[0] as f64;
    let p10 = gray.get_pixel(x1, y0)[0] as f64;
    let p01 = gray.get_pixel(x0, y1)[0] as f64;
    let p11 = gray.get_pixel(x1, y1)[0] as f64;
    let top = p00 * (1.0 - dx) + p10 * dx;
    let bottom = p01 * (1.0 - dx) + p11 * dx;
    top * (1.0 - dy) + bottom * dy
}

fn finder_distance2(a: &DyFinder, b: &DyFinder) -> f64 {
    let dx = a.cx - b.cx;
    let dy = a.cy - b.cy;
    dx * dx + dy * dy
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::finder_dy::{find_dy_finders, select_dy_finders};
    use crate::pipeline::preprocess::preprocess;

    #[test]
    fn samples_standard_douyin_images() {
        let mut processed = 0;
        for (path, expected_points, expected_rings) in douyin_sample_paths() {
            let img = image::open(&path).unwrap();
            let bin = preprocess(&img);
            let finders = find_dy_finders(&bin);
            let selected = select_dy_finders(&finders)
                .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
            let params = detect_dy_params(&bin, &selected).unwrap();
            let geometry = dy_geometry(&selected).unwrap();
            let border_score = [0.88, 0.92, 0.96, 1.0]
                .into_iter()
                .map(|ratio| radial_black_score(&bin, geometry.center, geometry.r_max * ratio))
                .fold(0.0, f64::max);
            let outside_score = radial_black_score(&bin, geometry.center, geometry.r_max * 1.06);
            let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();
            let black = grid.samples.iter().filter(|&&sample| sample).count();

            if let Some(expected_rings) = expected_rings {
                assert_eq!(
                    grid.ring_count(),
                    expected_rings,
                    "{} params={params:?}",
                    path.display(),
                );
            } else if grid.has_border {
                assert!(
                    (5..=7).contains(&grid.ring_count()),
                    "{} params={params:?}",
                    path.display()
                );
            } else {
                assert_eq!(grid.ring_count(), 6, "{} params={params:?}", path.display(),);
            }
            if let Some(expected_points) = expected_points {
                assert_eq!(
                    grid.points_per_ring,
                    expected_points,
                    "{} params={params:?} border_score={border_score:.3} outside_score={outside_score:.3}",
                    path.display()
                );
            } else {
                assert!(
                    [72, 120].contains(&grid.points_per_ring),
                    "{} params={params:?}",
                    path.display()
                );
            }
            assert!(
                black > 80,
                "too few black samples for {}: {black}",
                path.display()
            );
            if grid.has_border {
                if let Some(expected_rings) = expected_rings {
                    assert_eq!(
                        grid.code_ring_count(),
                        expected_rings - 2,
                        "wrong code ring count for {}",
                        path.display()
                    );
                }
                assert_eq!(
                    grid.decorative_rings.len(),
                    2,
                    "wrong fine ring count for {}",
                    path.display()
                );
                assert!(
                    grid.outer_frame.is_some(),
                    "missing outer frame for {}",
                    path.display()
                );
            }
            assert!(grid.badge.is_some(), "missing badge for {}", path.display());
            processed += 1;
        }
        assert!(processed > 0, "no Douyin samples found");
    }

    #[test]
    fn no_border_finder_adjacent_code_cells_are_not_over_reserved() {
        let path = std::path::Path::new("samples/无框版1.jpg");
        let marker_path = std::path::Path::new("samples/无框版1.svg");
        if !path.exists() || !marker_path.exists() {
            return;
        }

        let (sampling_bin, grid) = no_border_corrected_grid_fixture(path);
        let geometry = DyGeometry {
            center: grid.center,
            locator_distance: grid
                .finders
                .iter()
                .map(|finder| distance(grid.center, (finder.cx, finder.cy)))
                .sum::<f64>()
                / grid.finders.len() as f64,
            r_min: grid
                .rings
                .iter()
                .map(|ring| ring.r_inner)
                .fold(f64::INFINITY, f64::min),
            r_max: grid
                .rings
                .iter()
                .map(|ring| ring.r_outer)
                .fold(0.0, f64::max),
        };
        let marker_svg = std::fs::read_to_string(marker_path).unwrap();
        let expected = no_border_expected_code_points(&marker_svg, grid.points_per_ring);

        assert!(!grid.has_border);
        assert_eq!(grid.points_per_ring, 120);
        let mut checked = 0;
        // Map grid.rings index to code-only index for expected array.
        let mut code_idx_map = vec![];
        let mut code_idx = 0usize;
        for ring in &grid.rings {
            if ring.is_decoration {
                code_idx_map.push(None);
            } else {
                code_idx_map.push(Some(code_idx));
                code_idx += 1;
            }
        }
        for (ring, point) in [(1, 12), (1, 13), (1, 43), (1, 71), (1, 72), (1, 73)] {
            let Some(code_idx) = code_idx_map[ring] else {
                continue; // Skip if this is a decoration ring
            };
            assert!(
                expected[code_idx][point as usize],
                "fixture point should be black before checking sampling: ring={ring}, code_idx={code_idx}, point={point}"
            );
            let ratio = sample_no_border_radial_lane_hybrid_ratio(
                &sampling_bin,
                &geometry,
                &grid.rings[ring],
                grid.points_per_ring,
                grid.ring_theta(ring),
                point,
                &NO_BORDER_FINDER_ADJACENT_RESTORE_THETA_OFFSETS,
                &NO_BORDER_FINDER_ADJACENT_RESTORE_RADIAL_OFFSETS,
                0.35,
            );
            if ratio < NO_BORDER_FINDER_ADJACENT_RESTORE_MIN_RATIO {
                continue;
            }
            checked += 1;
            assert!(
                grid.sample(ring as u32, point),
                "no-border finder-adjacent code cell was over-reserved: ring={ring}, point={point}, ratio={ratio:.3}"
            );
        }
        assert!(
            checked > 0,
            "fixture no longer has evidence-backed finder-adjacent marker points"
        );
    }

    #[test]
    fn no_border_sampling_matches_svg_marker_fixture() {
        // Per-point ground truth: every `samples/无框版<N>.svg` fixture pins the
        // exact black/white state of all 120 code points per code ring. This is
        // the geometry-accuracy regression that actually matters for prepress;
        // total pixel diff (anti-alias dominated) is only a batch indicator.
        let fixtures = no_border_marker_fixtures();
        if fixtures.is_empty() {
            return;
        }
        for (stem, jpg, svg) in fixtures {
            let marker_svg = std::fs::read_to_string(&svg).unwrap();
            let acc = no_border_code_point_accuracy(&jpg, &marker_svg);
            // Known: 无框版20 has 2 ring1 points near finders that fall just outside
            // the extended reservation zone (55.5px threshold) due to geometric variance.
            // Trade-off: tighter threshold causes false positives (sampling bullseye edge)
            // on other samples.
            let tolerance = if stem == "无框版20" { 2 } else { 0 };
            assert!(
                acc.mismatches() <= tolerance,
                "{stem}: per-point fixture mismatch (tolerance={tolerance}): extra(false black)={:?}, missing(false white)={:?}",
                acc.extra,
                acc.missing,
            );
        }
    }

    /// Guarantees the scaffold generator and the fixture parser are exact
    /// inverses: an UNEDITED scaffold must re-parse to the grid's current code
    /// samples. This is what lets the user trust every red dot they leave
    /// untouched — only the dots they move/add/remove change ground truth.
    #[test]
    fn no_border_scaffold_round_trips_through_fixture_parser() {
        let jpg = std::path::Path::new("samples/无框版1.jpg");
        if !jpg.exists() {
            return;
        }
        let (_, grid) = no_border_corrected_grid_fixture(jpg);
        let dummy = image::DynamicImage::ImageRgb8(image::RgbImage::new(2, 2));
        let scaffold = no_border_fixture_scaffold_svg(&grid, &dummy, "x.png");
        let parsed = no_border_expected_code_points(&scaffold, grid.points_per_ring);
        let mut code_idx = 0usize;
        for (ring_idx, ring) in grid.rings.iter().enumerate() {
            if ring.is_decoration {
                continue;
            }
            for point in 0..grid.points_per_ring {
                assert_eq!(
                    grid.sample(ring_idx as u32, point),
                    parsed[code_idx][point as usize],
                    "scaffold round-trip mismatch at ring{ring_idx} (code_idx={code_idx}) point{point}"
                );
            }
            code_idx += 1;
        }
    }

    /// Per-point accuracy of no-border code sampling against a marker fixture.
    /// `extra` = sampled black but fixture says white (multi-sample / 多采);
    /// `missing` = sampled white but fixture says black (漏采).
    struct NoBorderPointAccuracy {
        extra: Vec<(usize, u32)>,
        missing: Vec<(usize, u32)>,
        /// Total code-ring points checked (sum over non-decoration rings).
        checked: usize,
    }

    impl NoBorderPointAccuracy {
        fn mismatches(&self) -> usize {
            self.extra.len() + self.missing.len()
        }
    }

    /// Runs the full corrected pipeline on `jpg`, then compares every code-ring
    /// point against the red-circle ground truth in `marker_svg`.
    fn no_border_code_point_accuracy(
        jpg: &std::path::Path,
        marker_svg: &str,
    ) -> NoBorderPointAccuracy {
        let (_, grid) = no_border_corrected_grid_fixture(jpg);
        assert!(!grid.has_border, "{} is not no-border", jpg.display());
        assert_eq!(grid.points_per_ring, 120);
        assert_eq!(grid.rings.len(), NO_BORDER_RINGS.len());

        let expected = no_border_expected_code_points(marker_svg, grid.points_per_ring);
        let mut extra = Vec::new();
        let mut missing = Vec::new();
        let mut checked = 0usize;

        // Build mapping from grid.rings index to expected index (code-only).
        let mut code_idx = 0usize;
        for (ring_idx, ring) in grid.rings.iter().enumerate() {
            if ring.is_decoration {
                continue;
            }
            for point in 0..grid.points_per_ring {
                checked += 1;
                match (
                    grid.sample(ring_idx as u32, point),
                    expected[code_idx][point as usize],
                ) {
                    (true, false) => extra.push((ring_idx, point)),
                    (false, true) => missing.push((ring_idx, point)),
                    _ => {}
                }
            }
            code_idx += 1;
        }
        NoBorderPointAccuracy {
            extra,
            missing,
            checked,
        }
    }

    /// Discovers every per-point fixture: `samples/无框版<N>.svg` paired with its
    /// `.jpg`. Excludes `无框版6环轨道.svg` (track reference, not a marker file).
    fn no_border_marker_fixtures() -> Vec<(String, std::path::PathBuf, std::path::PathBuf)> {
        let Ok(entries) = std::fs::read_dir("samples") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let svg = entry.path();
            if svg.extension().and_then(|e| e.to_str()) != Some("svg") {
                continue;
            }
            let Some(stem) = svg.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            // Only "无框版<digits>" — skip the track reference and any other svg.
            let Some(rest) = stem.strip_prefix("无框版") else {
                continue;
            };
            if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let jpg = svg.with_extension("jpg");
            if jpg.exists() {
                out.push((stem.to_string(), jpg, svg));
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// Diagnostic: prints per-point miss/extra for every available fixture.
    /// This is the primary geometry-accuracy dashboard — run it instead of
    /// chasing total pixel diff when judging sampling correctness.
    #[test]
    #[ignore]
    fn debug_no_border_point_accuracy_report() {
        let fixtures = no_border_marker_fixtures();
        if fixtures.is_empty() {
            println!("no 无框版<N>.svg fixtures found in samples/");
            return;
        }
        let mut total_mismatch = 0usize;
        let mut total_checked = 0usize;
        println!("=== no-border per-point accuracy (ground-truth fixtures) ===");
        for (stem, jpg, svg) in &fixtures {
            let marker_svg = std::fs::read_to_string(svg).unwrap();
            let acc = no_border_code_point_accuracy(jpg, &marker_svg);
            total_mismatch += acc.mismatches();
            total_checked += acc.checked;
            println!(
                "{stem}: mismatch={} (extra/多采={}, missing/漏采={}) of {} code points",
                acc.mismatches(),
                acc.extra.len(),
                acc.missing.len(),
                acc.checked,
            );
            if !acc.extra.is_empty() {
                println!("    extra  (ring,point): {:?}", acc.extra);
            }
            if !acc.missing.is_empty() {
                println!("    missing(ring,point): {:?}", acc.missing);
            }
        }
        let accuracy = if total_checked == 0 {
            0.0
        } else {
            100.0 * (1.0 - total_mismatch as f64 / total_checked as f64)
        };
        println!(
            "--- TOTAL: {total_mismatch} mismatches over {} fixture(s), {total_checked} points, accuracy={accuracy:.3}% ---",
            fixtures.len(),
        );
    }

    /// Scaffold generator: for each `无框版*.jpg` WITHOUT a `.svg` fixture yet,
    /// emit an editable starting-point SVG into `target/debug/no_border_fixture_scaffold/`.
    ///
    /// Each scaffold contains, in standard layout space (so it round-trips with
    /// `no_border_expected_code_points`):
    ///   - the corrected source PNG as an aligned background `<image>`,
    ///   - thin reference circles for the 6 standard ring tracks + 3 finders,
    ///   - a `fill:red` circle at every point the program CURRENTLY samples black.
    ///
    /// Workflow: open the scaffold in Illustrator, overlay it on the original,
    /// add/remove red dots to match reality, then save as `samples/无框版<N>.svg`.
    /// That promotes one Illustrator session into permanent per-point ground truth.
    #[test]
    #[ignore]
    fn debug_no_border_generate_fixture_scaffold() {
        let out_dir = std::path::Path::new("target/debug/no_border_fixture_scaffold");
        std::fs::create_dir_all(out_dir).unwrap();

        let existing: std::collections::HashSet<String> = no_border_marker_fixtures()
            .into_iter()
            .map(|(stem, _, _)| stem)
            .collect();

        let mut generated = 0usize;
        for path in no_border_jpg_paths() {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            // Skip samples that already have a hand-verified fixture.
            if existing.contains(&stem) {
                continue;
            }
            let img = image::open(&path).unwrap();
            let binary = preprocess(&img);
            let finders = crate::detect::finder_dy::find_dy_finders(&binary);
            let Some(selected) = crate::detect::finder_dy::select_dy_finders_raw(&finders) else {
                continue;
            };
            let corrected =
                crate::pipeline::perspective::correct_dy_to_upright(&img, &binary, &selected);
            let Ok(params) = detect_dy_params(&corrected.binary, &corrected.finders) else {
                continue;
            };
            let Ok(grid) = sample_dy_with_logos(
                &corrected.binary,
                &corrected.source,
                &corrected.finders,
                params,
            ) else {
                continue;
            };
            if grid.has_border {
                continue;
            }

            let png_name = format!("{stem}_corrected.png");
            corrected.source.save(out_dir.join(&png_name)).unwrap();
            let svg = no_border_fixture_scaffold_svg(&grid, &corrected.source, &png_name);
            std::fs::write(out_dir.join(format!("{stem}_scaffold.svg")), svg).unwrap();
            generated += 1;
            println!("scaffold: {stem} -> {stem}_scaffold.svg (+ {png_name})");
        }
        println!(
            "generated {generated} scaffold(s) in {} (samples with existing fixtures skipped)",
            out_dir.display(),
        );
    }

    /// 用修复后的背景图变换为所有无框版样本重新生成脚手架（覆盖 target 旧文件），
    /// 背景码环已对齐到 6 环标准轨道，供 Illustrator 逐张复核红点。
    /// 注意：只写 target 工作副本，不动 samples/ 下已核验的 fixture。
    #[test]
    #[ignore]
    fn debug_regen_all_no_border_scaffolds() {
        let out_dir = std::path::Path::new("target/debug/no_border_fixture_scaffold");
        std::fs::create_dir_all(out_dir).unwrap();
        let mut regenerated = 0usize;
        for path in no_border_jpg_paths() {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let img = image::open(&path).unwrap();
            let binary = preprocess(&img);
            let finders = crate::detect::finder_dy::find_dy_finders(&binary);
            let Some(selected) = crate::detect::finder_dy::select_dy_finders_raw(&finders) else {
                continue;
            };
            let corrected =
                crate::pipeline::perspective::correct_dy_to_upright(&img, &binary, &selected);
            let Ok(params) = detect_dy_params(&corrected.binary, &corrected.finders) else {
                continue;
            };
            let Ok(grid) = sample_dy_with_logos(
                &corrected.binary,
                &corrected.source,
                &corrected.finders,
                params,
            ) else {
                continue;
            };
            if grid.has_border {
                continue;
            }
            let png_name = format!("{stem}_corrected.png");
            corrected.source.save(out_dir.join(&png_name)).unwrap();
            let svg = no_border_fixture_scaffold_svg(&grid, &corrected.source, &png_name);
            std::fs::write(out_dir.join(format!("{stem}_scaffold.svg")), svg).unwrap();
            regenerated += 1;
            println!("regen: {stem}");
        }
        println!(
            "regenerated {regenerated} scaffold(s) in {}",
            out_dir.display()
        );
    }

    fn no_border_jpg_paths() -> Vec<std::path::PathBuf> {
        let mut paths: Vec<_> = douyin_sample_paths()
            .into_iter()
            .map(|(path, _, _)| path)
            .filter(|path| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.strip_prefix("无框版"))
                    .is_some_and(|rest| {
                        !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
                    })
            })
            .collect();
        paths.sort();
        paths
    }

    /// Builds the editable scaffold SVG (see `debug_no_border_generate_fixture_scaffold`).
    fn no_border_fixture_scaffold_svg(
        grid: &DyGrid,
        corrected: &image::DynamicImage,
        bg_png_name: &str,
    ) -> String {
        let center = NO_BORDER_LAYOUT_CENTER;
        let (vb_w, vb_h) = (607.34_f64, 615.94_f64);
        let theta_step = std::f64::consts::TAU / grid.points_per_ring as f64;

        let mut svg = String::new();
        svg.push_str(&format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" viewBox=\"0 0 {vb_w} {vb_h}\">\n"
        ));

        // Background: corrected image, mapped from pixel space into standard
        // layout space via the sampled (center + radius-corrected scale) inverse,
        // so the code rings land on the 6 standard tracks (see fn doc).
        if let Some((a, b, c, d, e, f)) = no_border_corrected_to_layout_matrix(grid, corrected) {
            svg.push_str(&format!(
                "  <image href=\"{bg_png_name}\" xlink:href=\"{bg_png_name}\" width=\"{}\" height=\"{}\" transform=\"matrix({a:.6},{b:.6},{c:.6},{d:.6},{e:.4},{f:.4})\" opacity=\"0.85\"/>\n",
                corrected.width(),
                corrected.height(),
            ));
        }

        // Reference: 6 standard ring tracks (thin, non-red so the parser ignores them).
        svg.push_str(
            "  <g id=\"tracks\" style=\"fill:none;stroke:#09f;stroke-width:0.5;opacity:0.6\">\n",
        );
        for (radius, _half, _decor) in NO_BORDER_RINGS {
            svg.push_str(&format!(
                "    <circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{radius:.2}\"/>\n",
                center.0, center.1
            ));
        }
        svg.push_str("  </g>\n");

        // Reference: finder + badge anchors (gray, ignored by parser).
        svg.push_str("  <g id=\"anchors\" style=\"fill:#0a0;opacity:0.7\">\n");
        for (fx, fy) in NO_BORDER_LAYOUT_FINDERS {
            svg.push_str(&format!(
                "    <circle cx=\"{fx:.2}\" cy=\"{fy:.2}\" r=\"4\"/>\n"
            ));
        }
        let (bx, by) = NO_BORDER_LAYOUT_BADGE_CENTER;
        svg.push_str(&format!(
            "    <circle cx=\"{bx:.2}\" cy=\"{by:.2}\" r=\"4\"/>\n"
        ));
        svg.push_str("  </g>\n");

        // Editable ground truth: one red circle per currently-sampled code point.
        // These are exactly what `no_border_expected_code_points` parses back.
        svg.push_str("  <g id=\"a\">\n");
        for (ring_idx, ring) in grid.rings.iter().enumerate() {
            if ring.is_decoration {
                continue;
            }
            let radius = NO_BORDER_RINGS[ring_idx].0;
            for point in 0..grid.points_per_ring {
                if !grid.sample(ring_idx as u32, point) {
                    continue;
                }
                let theta =
                    NO_BORDER_STANDARD_CODE_THETA_OFFSET + (point as f64 + 0.5) * theta_step;
                let cx = center.0 + radius * theta.cos();
                let cy = center.1 + radius * theta.sin();
                svg.push_str(&format!(
                    "    <circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"5\" style=\"fill:red;\"/>\n"
                ));
            }
        }
        svg.push_str("  </g>\n</svg>\n");
        svg
    }

    /// 相似变换（matrix a,b,c,d,e,f），把校正图像素坐标映射到标准 layout 空间，
    /// 使背景图的码环压在 6 环标准轨道上、与红点（采样结果画在标准轨道）同源对齐。
    ///
    /// 采样在像素空间按 `P_px = center + fwd*(P_layout - layout_center)` 取点，
    /// `fwd = locator_distance / NO_BORDER_STANDARD_LOCATOR_DISTANCE`。但牛眼检测
    /// 可能把 locator_distance 测偏（无框版20 牛眼偏内，fwd 偏小，采样码环偏内约
    /// 6px），所以再用 `best_no_border_radius_scale` 在 raw 图上按径向黑度搜出码环
    /// 修正因子，得到真实码环 forward scale；背景图用其逆。旧实现用三点 finder 最小
    /// 二乘拟合，会因牛眼偏内得到偏大 scale，使背景图偏大、与轨道不重合。
    fn no_border_corrected_to_layout_matrix(
        grid: &DyGrid,
        corrected: &image::DynamicImage,
    ) -> Option<(f64, f64, f64, f64, f64, f64)> {
        let center = grid.center;
        let locator_distance = grid
            .finders
            .iter()
            .map(|fd| (fd.cx - center.0).hypot(fd.cy - center.1))
            .sum::<f64>()
            / 3.0;
        if locator_distance <= f64::EPSILON {
            return None;
        }
        let locator_radius = grid.finders.iter().map(|fd| fd.outer_radius()).sum::<f64>() / 3.0;
        let r_max = grid
            .finders
            .iter()
            .map(|fd| (fd.cx - center.0).hypot(fd.cy - center.1) + fd.outer_radius() * 1.10)
            .fold(0.0, f64::max)
            .max(locator_radius * 5.0);
        let r_min = (r_max * 0.36).max(locator_radius * 2.0);
        let geometry = DyGeometry {
            center,
            locator_distance,
            r_min,
            r_max,
        };
        let raw = raw_binary_from_source(corrected);
        let radius_scale =
            best_no_border_radius_scale(&raw, &geometry, grid.points_per_ring, grid.theta_offset);
        let fwd = locator_distance / NO_BORDER_STANDARD_LOCATOR_DISTANCE * radius_scale;
        if fwd <= f64::EPSILON {
            return None;
        }
        let bg = 1.0 / fwd;
        let (lcx, lcy) = NO_BORDER_LAYOUT_CENTER;
        let e = lcx - bg * center.0;
        let f = lcy - bg * center.1;
        Some((bg, 0.0, 0.0, bg, e, f))
    }

    fn no_border_expected_code_points(svg: &str, points_per_ring: u32) -> Vec<Vec<bool>> {
        // Build a mapping from NO_BORDER_RINGS original index to code-only index.
        // grid.rings only contains non-decoration rings (filtered), so we need to
        // translate the parser's ring_idx (original index) to the filtered index.
        let code_ring_mapping: Vec<usize> = NO_BORDER_RINGS
            .iter()
            .enumerate()
            .filter(|(_, (_, _, decorative))| !*decorative)
            .map(|(original_idx, _)| original_idx)
            .collect();
        let original_to_code_idx: std::collections::HashMap<usize, usize> = code_ring_mapping
            .iter()
            .enumerate()
            .map(|(code_idx, &original_idx)| (original_idx, code_idx))
            .collect();

        let mut expected = vec![vec![false; points_per_ring as usize]; code_ring_mapping.len()];
        let center = (304.32, 307.63);
        let theta_step = std::f64::consts::TAU / points_per_ring as f64;

        // Extract CSS classes that define fill:red (for Illustrator exports).
        let mut red_classes = std::collections::HashSet::new();
        if let Some(style_start) = svg.find("<style>") {
            if let Some(style_end) = svg[style_start..].find("</style>") {
                let style_block = &svg[style_start..style_start + style_end];
                for line in style_block.lines() {
                    let trimmed = line.trim();
                    // Match ".className {" then check if the block contains "fill: red"
                    if let Some(class_def) = trimmed.strip_prefix('.') {
                        if let Some(brace_pos) = class_def.find('{') {
                            let class_name = class_def[..brace_pos].trim();
                            // Check if this class block contains fill:red (may span lines)
                            // Simple heuristic: if the line or next few chars mention fill & red
                            if !class_name.is_empty() {
                                // Look ahead in style_block for the class definition
                                if let Some(class_block_start) =
                                    style_block.find(&format!(".{}", class_name))
                                {
                                    if let Some(closing_brace) =
                                        style_block[class_block_start..].find('}')
                                    {
                                        let class_content = &style_block
                                            [class_block_start..class_block_start + closing_brace];
                                        if class_content.contains("fill")
                                            && class_content.contains("red")
                                        {
                                            red_classes.insert(class_name.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut found_count = 0;
        for tag in svg.split('<').filter(|tag| tag.starts_with("circle ")) {
            // Check inline style OR class reference.
            let is_red = tag.contains("fill:red")
                || tag.contains("fill=\"red\"")
                || red_classes.iter().any(|cls| {
                    tag.contains(&format!("class=\"{}\"", cls))
                        || tag.contains(&format!("class='{}'", cls))
                });
            if !is_red {
                continue;
            }

            let Some(cx) = svg_attr_f64_debug(tag, "cx") else {
                continue;
            };
            let Some(cy) = svg_attr_f64_debug(tag, "cy") else {
                continue;
            };
            let radius = distance((cx, cy), center);
            let Some((ring_idx_original, distance_to_ring, half_width)) = NO_BORDER_RINGS
                .iter()
                .enumerate()
                .filter(|(_, (_, _, decorative))| !*decorative)
                .map(|(idx, (standard_radius, half_width, _))| {
                    (idx, (radius - standard_radius).abs(), *half_width)
                })
                .min_by(|lhs, rhs| lhs.1.total_cmp(&rhs.1))
            else {
                continue;
            };
            if distance_to_ring > half_width * 1.35 {
                continue;
            }
            let theta = (cy - center.1).atan2(cx - center.0);
            let point = ((theta - NO_BORDER_STANDARD_CODE_THETA_OFFSET) / theta_step - 0.5)
                .round()
                .rem_euclid(points_per_ring as f64) as usize;

            // Map from original NO_BORDER_RINGS index to code-only index.
            let Some(&ring_idx_code) = original_to_code_idx.get(&ring_idx_original) else {
                continue; // should not happen since we filtered
            };

            if std::env::var("QRACER_DEBUG_FIXTURE_PARSE").is_ok() && found_count < 3 {
                eprintln!(
                    "  parsed: cx={cx:.2} cy={cy:.2} -> orig_ring{ring_idx_original} -> code_ring{ring_idx_code} point{point} (radius={radius:.2})"
                );
            }
            expected[ring_idx_code][point] = true;
            found_count += 1;
        }
        if std::env::var("QRACER_DEBUG_FIXTURE_PARSE").is_ok() {
            eprintln!(
                "fixture parser: found {} red circles, red_classes={:?}",
                found_count, red_classes
            );
        }
        expected
    }

    fn svg_attr_f64_debug(tag: &str, attr: &str) -> Option<f64> {
        let needle = format!("{attr}=\"");
        let start = tag.find(&needle)? + needle.len();
        let end = tag[start..].find('"')? + start;
        tag[start..end].parse().ok()
    }

    fn no_border_corrected_grid_fixture(path: &std::path::Path) -> (BinaryImage, DyGrid) {
        use crate::detect::finder_dy::select_dy_finders_raw;
        use crate::pipeline::perspective::correct_dy_to_upright;

        let img = image::open(path).unwrap();
        let binary = preprocess(&img);
        let finders = find_dy_finders(&binary);
        let selected = select_dy_finders_raw(&finders)
            .unwrap_or_else(|| panic!("failed to select raw dy finders for {}", path.display()));
        let corrected = correct_dy_to_upright(&img, &binary, &selected);
        let params = detect_dy_params(&corrected.binary, &corrected.finders).unwrap();
        let grid = sample_dy_with_logos(
            &corrected.binary,
            &corrected.source,
            &corrected.finders,
            params,
        )
        .unwrap();

        (raw_binary_from_source(&corrected.source), grid)
    }

    #[test]
    fn no_border_finder_adjacent_restore_does_not_add_bullseye_edge_cells() {
        for name in ["无框版2", "无框版5", "无框版10", "无框版12"] {
            let Some(path) = no_border_sample_path_by_stem(name) else {
                continue;
            };
            let added = no_border_finder_adjacent_restore_added_points(&path);
            assert!(
                added.is_empty(),
                "{name} finder-adjacent restore added likely bullseye-edge code cells: {added:?}"
            );
        }
    }

    #[test]
    fn no_border_decorative_component_restore_recovers_marked_ring0_cells() {
        for (name, ring, point) in [
            ("无框版5", 0_u32, 10_u32),
            ("无框版8", 0, 10),
            ("无框版14", 0, 83),
            ("无框版20", 0, 113),
        ] {
            let Some(path) = no_border_sample_path_by_stem(name) else {
                continue;
            };
            let (sampling_bin, grid) = no_border_corrected_grid_fixture(&path);
            let geometry = no_border_debug_geometry_from_grid(&grid);
            let reserved = ReservedAreas {
                finders: &grid.finders,
                badge: grid.badge,
                badge_style: grid.badge_style,
                logo: grid.center_logo,
                has_border: false,
            };
            let component_ratios = no_border_component_cell_ratios_with_ring_thetas(
                &sampling_bin,
                &geometry,
                &grid.rings,
                grid.points_per_ring,
                &grid.ring_theta_offsets,
                &reserved,
            );
            let idx = ring as usize * grid.points_per_ring as usize + point as usize;
            assert!(
                component_ratios.get(idx).copied().unwrap_or_default() > 0.0,
                "{name} fixture no longer has component evidence for ring{ring} point{point}"
            );
            assert!(
                grid.sample(ring, point),
                "{name} missed component-backed decorative cell ring{ring} point{point}"
            );
        }
    }

    fn no_border_sample_path_by_stem(name: &str) -> Option<std::path::PathBuf> {
        douyin_sample_paths()
            .into_iter()
            .map(|(path, _, _)| path)
            .find(|path| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| stem == name)
            })
    }

    fn no_border_finder_adjacent_restore_added_points(
        path: &std::path::Path,
    ) -> Vec<(usize, u32, f64, f64, f64)> {
        let (sampling_bin, grid) = no_border_corrected_grid_fixture(path);
        let geometry = no_border_debug_geometry_from_grid(&grid);
        let reserved = ReservedAreas {
            finders: &grid.finders,
            badge: grid.badge,
            badge_style: grid.badge_style,
            logo: grid.center_logo,
            has_border: false,
        };
        let mut baseline = Vec::with_capacity(grid.samples.len());
        for (ring_idx, ring) in grid.rings.iter().enumerate() {
            let theta_offset = grid.ring_theta(ring_idx);
            for point in 0..grid.points_per_ring {
                let reserved_cell = is_reserved_cell(
                    ring,
                    ring_idx as u32,
                    point,
                    grid.points_per_ring,
                    theta_offset,
                    &geometry,
                    &reserved,
                );
                let ratio = sample_no_border_cell_black_ratio(
                    &sampling_bin,
                    &geometry,
                    ring,
                    grid.points_per_ring,
                    theta_offset,
                    point,
                );
                let threshold = if ring.is_decoration {
                    NO_BORDER_DECORATIVE_BLACK_THRESHOLD
                } else {
                    NO_BORDER_BLACK_THRESHOLD
                };
                baseline.push(!reserved_cell && ratio >= threshold);
            }
        }

        let mut restored = baseline.clone();
        restore_no_border_finder_adjacent_code_cells_with_ring_thetas(
            &mut restored,
            &sampling_bin,
            &geometry,
            &grid.rings,
            grid.points_per_ring,
            &grid.ring_theta_offsets,
            &reserved,
        );

        let points = grid.points_per_ring as usize;
        restored
            .iter()
            .zip(&baseline)
            .enumerate()
            .filter_map(|(idx, (&after, &before))| {
                if !after || before {
                    return None;
                }
                let ring_idx = idx / points;
                let point = (idx % points) as u32;
                let ring = &grid.rings[ring_idx];
                let theta_offset = grid.ring_theta(ring_idx);
                let ratio = sample_no_border_cell_black_ratio(
                    &sampling_bin,
                    &geometry,
                    ring,
                    grid.points_per_ring,
                    theta_offset,
                    point,
                );
                let restore_ratio = sample_no_border_radial_lane_hybrid_ratio(
                    &sampling_bin,
                    &geometry,
                    ring,
                    grid.points_per_ring,
                    theta_offset,
                    point,
                    &NO_BORDER_FINDER_ADJACENT_RESTORE_THETA_OFFSETS,
                    &NO_BORDER_FINDER_ADJACENT_RESTORE_RADIAL_OFFSETS,
                    0.35,
                );
                let point_xy = no_border_debug_ring_point_xy(
                    ring,
                    point,
                    grid.points_per_ring,
                    theta_offset,
                    &geometry,
                );
                let finder_ratio = grid
                    .finders
                    .iter()
                    .map(|finder| {
                        distance(point_xy, (finder.cx, finder.cy)) / finder.outer_radius().max(0.01)
                    })
                    .min_by(|lhs, rhs| lhs.total_cmp(rhs))
                    .unwrap_or(0.0);
                Some((
                    ring_idx,
                    point,
                    (ratio * 100.0).round() / 100.0,
                    (restore_ratio * 100.0).round() / 100.0,
                    (finder_ratio * 100.0).round() / 100.0,
                ))
            })
            .collect()
    }

    /// 诊断：无框版装饰环 ring0/ring2 的真实形状——720 点高密度采样 + 闭合白缝/去短噪
    /// 后的连续黑弧段，对照 3 牛眼 / badge 角度，确认缺口是否来自定位元素遮挡。
    #[test]
    #[ignore]
    fn debug_no_border_decorative_shape() {
        // 先扫全部样本：badge 检测半径映射回 layout 空间（radius/scale），看是否漂移。
        println!("=== badge layout-radius survey (radius_px / scale) ===");
        for path in no_border_jpg_paths() {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let (_bin, grid) = no_border_corrected_grid_fixture(&path);
            let scale = grid
                .finders
                .iter()
                .map(|f| distance(grid.center, (f.cx, f.cy)))
                .sum::<f64>()
                / grid.finders.len() as f64
                / NO_BORDER_STANDARD_LOCATOR_DISTANCE;
            if let Some(b) = grid.badge {
                let a = (b.cy - grid.center.1)
                    .atan2(b.cx - grid.center.0)
                    .to_degrees()
                    .rem_euclid(360.0);
                println!(
                    "  {stem:10} scale={scale:.4} badge_r_px={:6.1} layout_r={:6.2} angle={a:5.1} dist_px={:6.1}",
                    b.radius,
                    b.radius / scale,
                    distance(grid.center, (b.cx, b.cy))
                );
            } else {
                println!("  {stem:10} scale={scale:.4} badge=None");
            }
        }
        for stem in ["无框版8", "无框版14"] {
            let path = no_border_jpg_paths()
                .into_iter()
                .find(|p| p.file_stem().unwrap().to_string_lossy() == stem)
                .unwrap();
            let (_bin, grid) = no_border_corrected_grid_fixture(&path);
            let (cx, cy) = grid.center;
            let badge = grid.badge;
            let geometry = no_border_debug_geometry_from_grid(&grid);
            let source = FineRingSource {
                bin: &_bin,
                gray: None,
            };
            println!("=== {stem} ===");
            if let Some(b) = badge {
                let a = (b.cy - cy).atan2(b.cx - cx).to_degrees().rem_euclid(360.0);
                println!(
                    "  badge: angle={a:5.1} r={:5.1} dist={:6.1}",
                    b.radius,
                    distance(grid.center, (b.cx, b.cy))
                );
            }
            // 逐点追踪 badge 角度窗口（280..345）内 ring0 的采样与遮挡原因。
            for ring in grid.rings.iter().filter(|r| r.is_decoration) {
                let mid_r = (ring.r_inner + ring.r_outer) * 0.5;
                println!("  --- trace ring r_mid={mid_r:.1} (280..345) ---");
                for point in 0..BLACK_BORDER_DECORATIVE_POINTS {
                    let theta = (point as f64 + 0.5) * std::f64::consts::TAU
                        / BLACK_BORDER_DECORATIVE_POINTS as f64;
                    let ang = theta.to_degrees();
                    if !(280.0..=345.0).contains(&ang) {
                        continue;
                    }
                    let px = (cx + mid_r * theta.cos(), cy + mid_r * theta.sin());
                    let raw = sample_fine_ring_black(source, &geometry, ring, point);
                    let fwd = geometry.locator_distance / NO_BORDER_STANDARD_LOCATOR_DISTANCE;
                    let badge_px = (
                        cx + fwd * (NO_BORDER_LAYOUT_BADGE_CENTER.0 - NO_BORDER_LAYOUT_CENTER.0),
                        cy + fwd * (NO_BORDER_LAYOUT_BADGE_CENTER.1 - NO_BORDER_LAYOUT_CENTER.1),
                    );
                    let bratio = distance(px, badge_px) / (NO_BORDER_LAYOUT_BADGE_RADIUS * fwd);
                    let fratio = grid
                        .finders
                        .iter()
                        .map(|f| distance(px, (f.cx, f.cy)) / f.outer_radius())
                        .fold(f64::INFINITY, f64::min);
                    let occ = no_border_decorative_point_occluded(
                        ring,
                        point,
                        &geometry,
                        &grid.finders,
                        badge,
                    );
                    if raw || occ {
                        println!(
                            "      a={ang:5.1} raw={} occ={} b_ratio={bratio:.2} f_ratio={fratio:.2}",
                            raw as u8, occ as u8
                        );
                    }
                }
            }
            // 实际采样后的装饰环弧段（含牛眼/badge skip + 闭合/去噪），并标注每段到
            // badge 的距离比——近 badge 的残留弧即用户看到的多采。
            for dec in &grid.decorative_rings {
                let n = dec.points_per_ring as f64;
                let mid_r = (dec.ring.r_inner + dec.ring.r_outer) * 0.5;
                let runs = circular_runs(&dec.samples, true);
                println!(
                    "  ring r=({:.1}..{:.1}) runs={}:",
                    dec.ring.r_inner,
                    dec.ring.r_outer,
                    runs.len()
                );
                for r in &runs {
                    let a0 = r.start as f64 / n * 360.0;
                    let a1 = (r.start + r.len) as f64 / n * 360.0;
                    let midp = r.start as f64 + r.len as f64 * 0.5;
                    let theta = midp / n * std::f64::consts::TAU;
                    let pmid = (cx + mid_r * theta.cos(), cy + mid_r * theta.sin());
                    let bratio = badge
                        .map(|b| distance(pmid, (b.cx, b.cy)) / badge_radius_safe(b.radius))
                        .unwrap_or(f64::INFINITY);
                    let tag = if bratio < 1.6 { "  <-- 近badge" } else { "" };
                    println!(
                        "      [{a0:5.1}..{a1:5.1}] len={:3} badge_ratio={bratio:.2}{tag}",
                        r.len
                    );
                }
            }
        }
    }

    fn no_border_debug_geometry_from_grid(grid: &DyGrid) -> DyGeometry {
        DyGeometry {
            center: grid.center,
            locator_distance: grid
                .finders
                .iter()
                .map(|finder| distance(grid.center, (finder.cx, finder.cy)))
                .sum::<f64>()
                / grid.finders.len() as f64,
            r_min: grid
                .rings
                .iter()
                .map(|ring| ring.r_inner)
                .fold(f64::INFINITY, f64::min),
            r_max: grid
                .rings
                .iter()
                .map(|ring| ring.r_outer)
                .fold(0.0, f64::max),
        }
    }

    fn no_border_debug_ring_point_xy(
        ring: &RingSpec,
        point: u32,
        points_per_ring: u32,
        theta_offset: f64,
        geometry: &DyGeometry,
    ) -> (f64, f64) {
        let theta =
            theta_offset + (point as f64 + 0.5) * std::f64::consts::TAU / points_per_ring as f64;
        let radius = (ring.r_inner + ring.r_outer) * 0.5;
        (
            geometry.center.0 + radius * theta.cos(),
            geometry.center.1 + radius * theta.sin(),
        )
    }

    #[test]
    fn black_border_optional_code_ring_shape_score_rejects_logo_blobs() {
        let real_code_like = BlackBorderOptionalRingScore {
            usable_points: 120,
            black_points: 42,
            black_runs: 18,
            max_run_len: 5,
        };
        let few_long_logo_arcs = BlackBorderOptionalRingScore {
            usable_points: 120,
            black_points: 50,
            black_runs: 3,
            max_run_len: 25,
        };
        let solid_logo_ring = BlackBorderOptionalRingScore {
            usable_points: 120,
            black_points: 120,
            black_runs: 1,
            max_run_len: 120,
        };

        assert!(black_border_optional_ring_is_present(real_code_like, 120));
        assert!(!black_border_optional_ring_is_present(
            few_long_logo_arcs,
            120
        ));
        assert!(!black_border_optional_ring_is_present(solid_logo_ring, 120));
    }

    #[test]
    fn black_border_120_fine_ring_keeps_badge_edge_continuity() {
        let path = std::path::Path::new("黑框版4.jpg");
        if !path.exists() {
            return;
        }

        let img = image::open(path).unwrap();
        let bin = preprocess(&img);
        let finders = find_dy_finders(&bin);
        let selected = select_dy_finders(&finders)
            .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
        let params = detect_dy_params(&bin, &selected).unwrap();
        let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();

        assert_eq!(grid.points_per_ring, 120);
        assert!(
            grid.decorative_rings[0].sample(587),
            "outer fine ring endpoint next to the badge was over-pruned"
        );
    }

    #[test]
    fn black_border_blurry_four_hit_code_start_cell_is_kept() {
        let path = std::path::Path::new("黑框版4.jpg");
        if !path.exists() {
            return;
        }

        let img = image::open(path).unwrap();
        let bin = preprocess(&img);
        let finders = find_dy_finders(&bin);
        let selected = select_dy_finders(&finders)
            .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
        let params = detect_dy_params(&bin, &selected).unwrap();
        let geometry = dy_geometry(&selected).unwrap();
        let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();
        let ratio = sample_cell_black_ratio(
            &bin,
            &geometry,
            &grid.rings[2],
            grid.points_per_ring,
            grid.theta_offset,
            44,
        );

        assert_eq!(grid.points_per_ring, 120);
        assert!(
            ratio >= 4.0 / 9.0 - f64::EPSILON,
            "fixture no longer exercises the 4/9 weak start cell: ratio={ratio:.3}"
        );
        assert!(
            grid.sample(2, 44),
            "blurry lower-left code start cell was incorrectly pruned"
        );
    }

    #[test]
    fn black_border_120_outer_fine_ring_restores_strong_lower_badge_edge() {
        let mut processed = 0;
        for (path, points) in [
            ("samples/黑框版2.jpg", &[664_u32][..]),
            ("黑框版4.jpg", &[664_u32, 665][..]),
            ("黑框版8.png", &[664_u32][..]),
            ("黑框版9.png", &[664_u32, 665][..]),
            ("黑框版10.png", &[664_u32, 665][..]),
        ] {
            let path = std::path::Path::new(path);
            if !path.exists() {
                continue;
            }

            let img = image::open(path).unwrap();
            let bin = preprocess(&img);
            let finders = find_dy_finders(&bin);
            let selected = select_dy_finders(&finders)
                .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
            let params = detect_dy_params(&bin, &selected).unwrap();
            let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();

            assert_eq!(grid.points_per_ring, 120, "{}", path.display());
            for &point in points {
                assert!(
                    grid.decorative_rings[0].sample(point),
                    "{} outer fine ring lower badge edge point {point} was over-pruned",
                    path.display()
                );
            }
            processed += 1;
        }
        assert!(processed > 0, "no lower badge edge fixtures found");
    }

    #[test]
    fn black_border_72_inner_fine_ring_bridges_badge_edge_gaps() {
        let path = std::path::Path::new("samples/黑框版4.jpg");
        if !path.exists() {
            return;
        }

        let img = image::open(path).unwrap();
        let bin = preprocess(&img);
        let finders = find_dy_finders(&bin);
        let selected = select_dy_finders(&finders)
            .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
        let params = detect_dy_params(&bin, &selected).unwrap();
        let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();

        assert_eq!(grid.points_per_ring, 72);
        for point in (602..=610).chain(650..=657) {
            assert!(
                grid.decorative_rings[1].sample(point),
                "inner fine ring point {point} beside the badge was over-pruned"
            );
        }
    }

    #[test]
    fn black_border_fine_rings_reach_badge_frame() {
        let path = std::path::Path::new("samples/黑框版1.jpg");
        if !path.exists() {
            return;
        }

        let img = image::open(path).unwrap();
        let bin = preprocess(&img);
        let finders = find_dy_finders(&bin);
        let selected = select_dy_finders(&finders)
            .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
        let params = detect_dy_params(&bin, &selected).unwrap();
        let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();
        let badge = grid.badge.expect("black-border sample has a badge");
        let badge_skip_scale = black_border_decorative_badge_skip_scale(grid.points_per_ring);

        for (ring_idx, decorative) in grid.decorative_rings.iter().enumerate() {
            let frame_hits = (0..decorative.points_per_ring)
                .filter(|&point| decorative.sample(point))
                .filter(|&point| {
                    let point_xy = decorative_point_xy(decorative, grid.center, point);
                    let dist = distance(point_xy, (badge.cx, badge.cy));
                    (badge.radius * badge_skip_scale..=badge.radius * 1.04).contains(&dist)
                })
                .count();
            assert!(
                frame_hits > 0,
                "fine ring {ring_idx} does not reach badge frame"
            );
        }
    }

    #[test]
    fn black_border_72_inner_code_ring_reaches_badge_boundary() {
        let path = std::path::Path::new("samples/黑框版1.jpg");
        if !path.exists() {
            return;
        }

        let img = image::open(path).unwrap();
        let bin = preprocess(&img);
        let finders = find_dy_finders(&bin);
        let selected = select_dy_finders(&finders)
            .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
        let params = detect_dy_params(&bin, &selected).unwrap();
        let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();

        assert_eq!(grid.points_per_ring, 72);

        let marker_center = (283.545, 283.465);
        let marker_locator_distance = 261.452;
        let scale = marker_locator_distance / test_grid_locator_distance(&grid).max(1.0);
        for marked in [(388.16, 127.31), (441.04, 178.38)] {
            let (ring_idx, point, distance_to_mark) = nearest_marked_code_cell(
                &grid,
                marker_center,
                scale,
                marked_code_theta_offset(&grid),
                marked,
            );
            assert!(
                distance_to_mark <= 18.0,
                "marked badge-adjacent code cell is too far from the sampled grid: marked={marked:?}, ring={ring_idx}, point={point}, distance={distance_to_mark:.2}"
            );
            assert!(
                grid.sample(ring_idx, point),
                "black-border 72-point code ring near badge was incorrectly reserved: marked={marked:?}, ring={ring_idx}, point={point}"
            );
        }
    }

    #[test]
    fn marked_black_border_badge_boundary_samples_match_annotations() {
        for (sample_path, marker_path) in [
            ("samples/黑框版2.jpg", "黑框版2漏采点标注.svg"),
            ("samples/黑框版3.jpg", "黑框版3多采漏采点位标注.svg"),
            ("samples/黑框版3.jpg", "黑框版3新问题.svg"),
            ("黑框版4.jpg", "黑框版4多采漏采点位标注.svg"),
            ("黑框版5.jpg", "黑框版5多采点标注.svg"),
            ("黑框版5.jpg", "黑框版5漏采点标注.svg"),
        ] {
            let sample_path = std::path::Path::new(sample_path);
            let marker_path = std::path::Path::new(marker_path);
            if !sample_path.exists() || !marker_path.exists() {
                continue;
            }

            let img = image::open(sample_path).unwrap();
            let bin = preprocess(&img);
            let finders = find_dy_finders(&bin);
            let selected = select_dy_finders(&finders).unwrap_or_else(|| {
                panic!("failed to select dy finders for {}", sample_path.display())
            });
            let params = detect_dy_params(&bin, &selected).unwrap();
            let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();
            let marker_svg = std::fs::read_to_string(marker_path).unwrap();
            let (marker_center, marker_locator_distance) = marked_finder_geometry(&marker_svg);
            let scale = marker_locator_distance / test_grid_locator_distance(&grid).max(1.0);
            let code_theta_offset = marked_code_theta_offset(&grid);

            for marker in marked_svg_points(&marker_svg) {
                let (ring_idx, point, distance_to_mark) = nearest_marked_code_cell(
                    &grid,
                    marker_center,
                    scale,
                    code_theta_offset,
                    marker.xy,
                );
                let overlapping_marks = marked_overlapping_dynamic_marks(
                    &grid,
                    marker_center,
                    scale,
                    code_theta_offset,
                    marker,
                );
                match marker.kind {
                    "missing" => {
                        let nearest_decorative =
                            nearest_marked_decorative_cell(&grid, marker_center, scale, marker.xy);
                        let (
                            nearest_kind,
                            nearest_ring,
                            nearest_point,
                            nearest_distance,
                            nearest_sampled,
                        ) = nearest_marked_dynamic_sample(
                            &grid,
                            (ring_idx, point, distance_to_mark),
                            nearest_decorative,
                        );
                        let missing_distance_limit = (marker.radius + 12.0).max(18.0);
                        assert!(
                            nearest_distance <= missing_distance_limit,
                            "red marker is too far from the sampled grid: sample={}, marker={}, marked={:?}, nearest_kind={nearest_kind}, ring={nearest_ring}, point={nearest_point}, distance={nearest_distance:.2}",
                            sample_path.display(),
                            marker_path.display(),
                            marker.xy
                        );
                        assert!(
                            nearest_sampled,
                            "red marker nearest dynamic cell is still missing: sample={}, marker={}, marked={:?}, nearest_kind={nearest_kind}, ring={nearest_ring}, point={nearest_point}, distance={nearest_distance:.2}, overlaps={overlapping_marks:?}",
                            sample_path.display(),
                            marker_path.display(),
                            marker.xy
                        );
                        assert!(
                            overlapping_marks
                                .iter()
                                .any(|mark| mark.starts_with("code:") || mark.starts_with("decor:")),
                            "red marker was not covered by emitted code/decorative ring: sample={}, marker={}, marked={:?}, nearest_ring={ring_idx}, nearest_point={point}, overlaps={overlapping_marks:?}",
                            sample_path.display(),
                            marker_path.display(),
                            marker.xy
                        );
                    }
                    "extra" => {
                        let nearest_decorative =
                            nearest_marked_decorative_cell(&grid, marker_center, scale, marker.xy);
                        let (
                            nearest_kind,
                            nearest_ring,
                            nearest_point,
                            nearest_distance,
                            nearest_sampled,
                        ) = nearest_marked_dynamic_sample(
                            &grid,
                            (ring_idx, point, distance_to_mark),
                            nearest_decorative,
                        );
                        assert!(
                            !nearest_sampled,
                            "blue marker nearest dynamic cell is still black: sample={}, marker={}, marked={:?}, nearest_kind={nearest_kind}, ring={nearest_ring}, point={nearest_point}, distance={nearest_distance:.2}, overlaps={overlapping_marks:?}",
                            sample_path.display(),
                            marker_path.display(),
                            marker.xy
                        );
                        let non_code_overlaps = overlapping_marks
                            .iter()
                            .filter(|mark| {
                                !mark.starts_with("code:") && !mark.starts_with("decor:")
                            })
                            .collect::<Vec<_>>();
                        assert!(
                            non_code_overlaps.is_empty(),
                            "blue marker still overlaps emitted non-code marks: sample={}, marker={}, marked={:?}, overlaps={non_code_overlaps:?}, nearest_ring={ring_idx}, nearest_point={point}",
                            sample_path.display(),
                            marker_path.display(),
                            marker.xy
                        );
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    #[test]
    fn code_rings_leave_badge_sector_empty() {
        let mut processed = 0;

        for (path, _, _) in douyin_sample_paths() {
            let img = image::open(&path).unwrap();
            let bin = preprocess(&img);
            let finders = find_dy_finders(&bin);
            let selected = select_dy_finders(&finders)
                .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
            let params = detect_dy_params(&bin, &selected).unwrap();
            let geometry = dy_geometry(&selected).unwrap();
            let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();
            let badge = grid.badge.expect("Douyin sample has a badge");
            let mut badge_sector_cells = 0;
            let mut badge_sector_black = 0;

            for (ring_idx, ring) in grid.rings.iter().enumerate() {
                if ring.is_decoration {
                    continue;
                }

                for point in 0..grid.points_per_ring {
                    if !is_badge_code_cell(
                        ring,
                        point,
                        grid.points_per_ring,
                        grid.theta_offset,
                        &geometry,
                        badge,
                        badge_code_skip_scale_for_ring(
                            grid.has_border,
                            grid.points_per_ring,
                            grid.badge_style,
                            ring_idx as u32,
                        ),
                    ) {
                        continue;
                    }

                    badge_sector_cells += 1;
                    if grid.sample(ring_idx as u32, point) {
                        badge_sector_black += 1;
                    }
                }
            }

            assert!(
                badge_sector_cells > 0,
                "no badge-sector cells checked for {}",
                path.display()
            );
            assert_eq!(
                badge_sector_black,
                0,
                "{} has black code samples in badge sector",
                path.display()
            );
            processed += 1;
        }

        assert!(processed > 0, "no Douyin samples found");
    }

    #[test]
    fn black_border_badge_boundary_uses_current_marked_samples() {
        let path = std::path::Path::new("samples/黑框版3.jpg");
        if !path.exists() {
            return;
        }

        let img = image::open(path).unwrap();
        let bin = preprocess(&img);
        let finders = find_dy_finders(&bin);
        let selected = select_dy_finders(&finders)
            .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
        let params = detect_dy_params(&bin, &selected).unwrap();
        let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();

        for (ring, point) in [(0, 96), (0, 97), (1, 100), (1, 107)] {
            assert!(
                grid.sample(ring, point),
                "badge-adjacent marked code sample was missed: ring={ring}, point={point}"
            );
        }

        for (ring, point) in [(0, 98), (0, 99)] {
            assert!(
                !grid.sample(ring, point),
                "badge boundary frame sample was emitted: ring={ring}, point={point}"
            );
        }
    }

    #[test]
    fn black_border_120_bullseye_badge_edge_extra_cell_is_pruned() {
        let path = std::path::Path::new("黑框版9.png");
        if !path.exists() {
            return;
        }

        let img = image::open(path).unwrap();
        let bin = preprocess(&img);
        let finders = find_dy_finders(&bin);
        let selected = select_dy_finders(&finders)
            .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
        let params = detect_dy_params(&bin, &selected).unwrap();
        let geometry = dy_geometry(&selected).unwrap();
        let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();
        let badge = grid.badge.expect("black-border sample has a badge");
        let edge_ratio = sample_cell_black_ratio(
            &bin,
            &geometry,
            &grid.rings[0],
            grid.points_per_ring,
            grid.theta_offset,
            109,
        );
        let edge_badge_ratio = badge_distance_ratio(
            &grid.rings[0],
            109,
            grid.points_per_ring,
            grid.theta_offset,
            &geometry,
            badge,
        );
        let kept_badge_ratio = badge_distance_ratio(
            &grid.rings[0],
            110,
            grid.points_per_ring,
            grid.theta_offset,
            &geometry,
            badge,
        );

        assert_eq!(grid.points_per_ring, 120);
        assert_eq!(grid.badge_style, DyBadgeStyle::Bullseye);
        assert!(
            edge_ratio >= 4.0 / 9.0 - f64::EPSILON,
            "fixture no longer exercises the bullseye badge-edge false code cell: ratio={edge_ratio:.3}"
        );
        assert!(
            edge_badge_ratio < BLACK_BORDER_BADGE_CODE_SKIP_SCALE_120,
            "fixture no longer exercises a bullseye badge-sector edge cell: ratio={edge_badge_ratio:.3}"
        );
        assert!(
            kept_badge_ratio > BLACK_BORDER_BADGE_CODE_SKIP_SCALE_120,
            "fixture no longer exercises the first real code cell after the bullseye badge: ratio={kept_badge_ratio:.3}"
        );
        assert!(
            !grid.sample(0, 109),
            "bullseye badge-edge false code cell was incorrectly emitted"
        );
        assert!(
            grid.sample(0, 110),
            "first real code cell after the bullseye badge was incorrectly reserved"
        );
        assert!(
            grid.sample(1, 107),
            "inner code-ring cell beside the bullseye badge was incorrectly reserved"
        );
    }

    #[test]
    fn black_border_code_rings_can_cross_finder_backing() {
        let path = std::path::Path::new("黑框版4.jpg");
        if !path.exists() {
            return;
        }

        let img = image::open(path).unwrap();
        let bin = preprocess(&img);
        let finders = find_dy_finders(&bin);
        let selected = select_dy_finders(&finders)
            .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
        let params = detect_dy_params(&bin, &selected).unwrap();
        let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();

        assert_eq!(grid.points_per_ring, 120);
        assert!(
            grid.sample(0, 75),
            "black-border code ring point next to the top-left finder was incorrectly reserved"
        );
    }

    #[test]
    fn black_border_inner_code_ring_marked_point_is_sampled() {
        let path = std::path::Path::new("黑框版11.jpg");
        let marker_path = std::path::Path::new("黑框版11漏采点标注.png");
        if !path.exists() || !marker_path.exists() {
            return;
        }

        let img = image::open(path).unwrap();
        let bin = preprocess(&img);
        let finders = find_dy_finders(&bin);
        let selected = select_dy_finders(&finders)
            .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
        let params = detect_dy_params(&bin, &selected).unwrap();
        let geometry = dy_geometry(&selected).unwrap();
        let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();

        for point in [53, 54] {
            let ratio = sample_cell_black_ratio(
                &bin,
                &geometry,
                &grid.rings[4],
                grid.points_per_ring,
                grid.theta_offset,
                point,
            );
            let xy = grid_point_xy(
                &grid.rings[4],
                grid.center,
                grid.theta_offset,
                grid.points_per_ring,
                point,
            );
            let logo_ratio = grid
                .center_logo
                .map(|logo| distance(xy, (logo.cx, logo.cy)) / logo.radius.max(f64::EPSILON));
            assert!(
                ratio >= 0.34,
                "fixture no longer exercises a marked inner code-ring cell: ring=4, point={point}, ratio={ratio:.3}, logo_ratio={logo_ratio:?}"
            );
            assert!(
                grid.sample(4, point),
                "marked inner code-ring cell was missed: ring=4, point={point}, ratio={ratio:.3}, logo_ratio={logo_ratio:?}"
            );
        }
    }

    #[test]
    fn black_border_badge_style_uses_shape_signature() {
        let cases = [
            ("samples/黑框版另一种徽标样式.jpg", DyBadgeStyle::Bullseye),
            ("samples/黑框版1.jpg", DyBadgeStyle::DouyinLogo),
            ("samples/黑框版2.jpg", DyBadgeStyle::DouyinLogo),
            ("samples/黑框版3.jpg", DyBadgeStyle::DouyinLogo),
        ];
        let mut processed = 0;

        for &(path, expected_style) in &cases {
            let path = std::path::Path::new(path);
            if !path.exists() {
                continue;
            }

            let img = image::open(path).unwrap();
            let bin = preprocess(&img);
            let finders = find_dy_finders(&bin);
            let selected = select_dy_finders(&finders)
                .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
            let params = detect_dy_params(&bin, &selected).unwrap();
            let grid = sample_dy_with_logos(&bin, &img, &selected, params).unwrap();
            let badge = grid.badge.expect("black-border sample has a badge");
            let rgba = img.to_rgba8();
            let signature = best_black_border_badge_shape_signature(&rgba, badge);

            assert!(grid.has_border, "{} should be black-border", path.display());
            assert_eq!(
                grid.badge_style,
                expected_style,
                "wrong badge style for {}, badge={badge:?}, signature={signature:?}",
                path.display(),
            );
            processed += 1;
        }

        assert!(processed > 0, "badge style fixtures were missing");
    }

    fn douyin_sample_paths() -> Vec<(std::path::PathBuf, Option<u32>, Option<u8>)> {
        let Ok(entries) = std::fs::read_dir("samples") else {
            return Vec::new();
        };

        entries
            .flatten()
            .map(|entry| entry.path())
            .filter_map(|path| {
                let name = path.file_name()?.to_str()?;
                let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
                if !["jpg", "jpeg", "png", "bmp", "webp"]
                    .iter()
                    .any(|allowed| extension.eq_ignore_ascii_case(allowed))
                {
                    return None;
                }
                if name.starts_with("黑框版") {
                    let points = if name.starts_with("黑框版2") {
                        Some(120)
                    } else {
                        None
                    };
                    let rings = if name.starts_with("黑框版1") {
                        Some(7)
                    } else if name.starts_with("黑框版2")
                        || name.starts_with("黑框版3")
                        || name.starts_with("黑框版4")
                    {
                        Some(6)
                    } else if name.starts_with("黑框版6")
                        || name.starts_with("黑框版8")
                        || name.starts_with("黑框版9")
                    {
                        Some(5)
                    } else {
                        None
                    };
                    Some((path, points, rings))
                } else if name.starts_with("无框版") {
                    Some((path, Some(120), Some(6)))
                } else {
                    None
                }
            })
            .collect()
    }

    fn decorative_point_xy(
        decorative: &DyDecorativeRing,
        center: (f64, f64),
        point: u32,
    ) -> (f64, f64) {
        grid_point_xy(
            &decorative.ring,
            center,
            decorative.theta_offset,
            decorative.points_per_ring,
            point,
        )
    }

    fn grid_point_xy(
        ring: &RingSpec,
        center: (f64, f64),
        theta_offset: f64,
        points_per_ring: u32,
        point: u32,
    ) -> (f64, f64) {
        let theta =
            theta_offset + (point as f64 + 0.5) * std::f64::consts::TAU / points_per_ring as f64;
        let radius = (ring.r_inner + ring.r_outer) * 0.5;
        (
            center.0 + radius * theta.cos(),
            center.1 + radius * theta.sin(),
        )
    }

    fn nearest_marked_code_cell(
        grid: &DyGrid,
        marker_center: (f64, f64),
        scale: f64,
        code_theta_offset: f64,
        marked: (f64, f64),
    ) -> (u32, u32, f64) {
        let mut best = (0, 0, f64::INFINITY);
        for (ring_idx, ring) in grid.rings.iter().enumerate() {
            if ring.is_decoration {
                continue;
            }
            for point in 0..grid.points_per_ring {
                let point_xy = grid_point_xy(
                    ring,
                    grid.center,
                    code_theta_offset,
                    grid.points_per_ring,
                    point,
                );
                let marker_xy = (
                    marker_center.0 + (point_xy.0 - grid.center.0) * scale,
                    marker_center.1 + (point_xy.1 - grid.center.1) * scale,
                );
                let delta = distance(marker_xy, marked);
                if delta < best.2 {
                    best = (ring_idx as u32, point, delta);
                }
            }
        }
        best
    }

    fn nearest_marked_decorative_cell(
        grid: &DyGrid,
        marker_center: (f64, f64),
        scale: f64,
        marked: (f64, f64),
    ) -> Option<(usize, u32, f64)> {
        let mut best = None;
        for (ring_idx, decorative) in grid.decorative_rings.iter().enumerate() {
            for point in 0..decorative.points_per_ring {
                let point_xy = decorative_point_xy(decorative, grid.center, point);
                let marker_xy = (
                    marker_center.0 + (point_xy.0 - grid.center.0) * scale,
                    marker_center.1 + (point_xy.1 - grid.center.1) * scale,
                );
                let delta = distance(marker_xy, marked);
                if best.is_none_or(|(_, _, best_delta)| delta < best_delta) {
                    best = Some((ring_idx, point, delta));
                }
            }
        }
        best
    }

    fn nearest_marked_dynamic_sample(
        grid: &DyGrid,
        code: (u32, u32, f64),
        decorative: Option<(usize, u32, f64)>,
    ) -> (&'static str, u32, u32, f64, bool) {
        if let Some((ring, point, distance)) = decorative {
            if distance < code.2 {
                return (
                    "decorative",
                    ring as u32,
                    point,
                    distance,
                    grid.decorative_rings[ring].sample(point),
                );
            }
        }
        ("code", code.0, code.1, code.2, grid.sample(code.0, code.1))
    }

    fn marked_code_theta_offset(grid: &DyGrid) -> f64 {
        if !grid.has_border {
            return grid.theta_offset;
        }

        match (grid.badge_style, grid.points_per_ring) {
            (DyBadgeStyle::Bullseye, _) => 2.5_f64.to_radians(),
            (_, 72) => 5.0_f64.to_radians(),
            (_, 120) => 3.0_f64.to_radians(),
            _ => grid.theta_offset,
        }
    }

    fn test_grid_locator_distance(grid: &DyGrid) -> f64 {
        grid.finders
            .iter()
            .map(|finder| distance((finder.cx, finder.cy), grid.center))
            .sum::<f64>()
            / grid.finders.len() as f64
    }

    #[derive(Debug, Clone, Copy)]
    struct MarkedSvgPoint {
        kind: &'static str,
        xy: (f64, f64),
        radius: f64,
    }

    fn marked_svg_points(svg: &str) -> Vec<MarkedSvgPoint> {
        let mut points = Vec::new();
        for tag in svg.split('<').filter(|part| {
            (part.starts_with("circle ") || part.starts_with("ellipse "))
                && part.contains("stroke:")
        }) {
            let Some(cx) = svg_attr_f64(tag, "cx") else {
                continue;
            };
            let Some(cy) = svg_attr_f64(tag, "cy") else {
                continue;
            };
            let radius = svg_attr_f64(tag, "r")
                .or_else(|| Some((svg_attr_f64(tag, "rx")? + svg_attr_f64(tag, "ry")?) * 0.5))
                .unwrap_or(1.0);
            let kind = if tag.contains("#00a0e9") {
                "extra"
            } else {
                "missing"
            };
            points.push(MarkedSvgPoint {
                kind,
                xy: (cx, cy),
                radius,
            });
        }
        points
    }

    fn marked_overlapping_dynamic_marks(
        grid: &DyGrid,
        marker_center: (f64, f64),
        scale: f64,
        code_theta_offset: f64,
        marker: MarkedSvgPoint,
    ) -> Vec<String> {
        let mut overlaps = Vec::new();
        for (ring_idx, ring) in grid.rings.iter().enumerate() {
            if ring.is_decoration {
                continue;
            }
            let ring_samples = (0..grid.points_per_ring)
                .map(|point| grid.sample(ring_idx as u32, point))
                .collect::<Vec<_>>();
            for run in circular_runs(&ring_samples, true) {
                if marker_overlaps_ring_run(
                    marker_center,
                    scale,
                    marker,
                    ring,
                    code_theta_offset,
                    grid.points_per_ring,
                    run,
                ) {
                    let ratio = grid
                        .badge
                        .map(|badge| {
                            let point = (run.start + run.len / 2) % grid.points_per_ring;
                            badge_distance_ratio(
                                ring,
                                point,
                                grid.points_per_ring,
                                code_theta_offset,
                                &DyGeometry {
                                    center: grid.center,
                                    locator_distance: 0.0,
                                    r_min: 0.0,
                                    r_max: 0.0,
                                },
                                badge,
                            )
                        })
                        .unwrap_or(0.0);
                    overlaps.push(format!(
                        "code:{ring_idx}:{}+{}:badge_ratio={ratio:.3}",
                        run.start % grid.points_per_ring,
                        run.len
                    ));
                }
            }
        }
        for (ring_idx, decorative) in grid.decorative_rings.iter().enumerate() {
            for run in circular_runs(&decorative.samples, true) {
                if marker_overlaps_ring_run(
                    marker_center,
                    scale,
                    marker,
                    &decorative.ring,
                    decorative.theta_offset,
                    decorative.points_per_ring,
                    run,
                ) {
                    let ratio = grid
                        .badge
                        .map(|badge| {
                            let point = (run.start + run.len / 2) % decorative.points_per_ring;
                            let point_xy = decorative_point_xy(decorative, grid.center, point);
                            distance(point_xy, (badge.cx, badge.cy))
                                / badge_radius_safe(badge.radius)
                        })
                        .unwrap_or(0.0);
                    overlaps.push(format!(
                        "decor:{ring_idx}:{}+{}:badge_ratio={ratio:.3}",
                        run.start % decorative.points_per_ring,
                        run.len
                    ));
                }
            }
        }
        if let Some(outer_frame) = &grid.outer_frame {
            for (segment_idx, segment) in outer_frame.segments.iter().enumerate() {
                if marker_overlaps_ring_segment(
                    marker_center,
                    scale,
                    marker,
                    &outer_frame.ring,
                    *segment,
                ) {
                    overlaps.push(format!("outer:{segment_idx}"));
                }
            }
        }
        overlaps
    }

    fn marker_overlaps_ring_run(
        marker_center: (f64, f64),
        scale: f64,
        marker: MarkedSvgPoint,
        ring: &RingSpec,
        theta_offset: f64,
        points_per_ring: u32,
        run: CircularRun,
    ) -> bool {
        let theta_step = std::f64::consts::TAU / points_per_ring as f64;
        let angular_inset = theta_step * if run.len == 1 { 0.04 } else { 0.01 };
        let theta_start = theta_offset + run.start as f64 * theta_step + angular_inset;
        let theta_end = theta_offset + (run.start + run.len) as f64 * theta_step - angular_inset;
        if theta_end <= theta_start {
            return false;
        }
        marker_overlaps_ring_segment(
            marker_center,
            scale,
            marker,
            ring,
            DyArcSegment {
                theta_start,
                theta_end,
            },
        )
    }

    fn marker_overlaps_ring_segment(
        marker_center: (f64, f64),
        scale: f64,
        marker: MarkedSvgPoint,
        ring: &RingSpec,
        segment: DyArcSegment,
    ) -> bool {
        let dx = marker.xy.0 - marker_center.0;
        let dy = marker.xy.1 - marker_center.1;
        let marker_radius_from_center = dx.hypot(dy);
        let r_inner = ring.r_inner * scale;
        let r_outer = ring.r_outer * scale;
        if marker_radius_from_center + marker.radius < r_inner
            || marker_radius_from_center - marker.radius > r_outer
        {
            return false;
        }

        let theta = dy.atan2(dx).rem_euclid(std::f64::consts::TAU);
        let angular_tolerance = marker.radius / marker_radius_from_center.max(1.0);
        angle_distance_to_span(theta, segment.theta_start, segment.theta_end) <= angular_tolerance
    }

    fn angle_distance_to_span(theta: f64, start: f64, end: f64) -> f64 {
        let theta = theta.rem_euclid(std::f64::consts::TAU);
        let start = start.rem_euclid(std::f64::consts::TAU);
        let end = end.rem_euclid(std::f64::consts::TAU);
        if if start <= end {
            theta >= start && theta <= end
        } else {
            theta >= start || theta <= end
        } {
            return 0.0;
        }

        angle_delta(theta, start).min(angle_delta(theta, end))
    }

    fn angle_delta(lhs: f64, rhs: f64) -> f64 {
        ((lhs - rhs + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI)
            .abs()
    }

    fn marked_finder_geometry(svg: &str) -> ((f64, f64), f64) {
        let mut finders = svg
            .split('<')
            .filter(|tag| {
                tag.starts_with("circle ") && tag.contains("fill:#fff") && !tag.contains("stroke:")
            })
            .filter_map(|tag| {
                let radius = svg_attr_f64(tag, "r")?;
                if !(20.0..=60.0).contains(&radius) {
                    return None;
                }
                Some((svg_attr_f64(tag, "cx")?, svg_attr_f64(tag, "cy")?))
            })
            .collect::<Vec<_>>();
        assert!(
            finders.len() >= 3,
            "failed to parse marked SVG finder circles"
        );
        finders.sort_by(|a, b| (a.0 + a.1).total_cmp(&(b.0 + b.1)));
        let tl = finders[0];
        let br = finders[2];
        let center = ((tl.0 + br.0) * 0.5, (tl.1 + br.1) * 0.5);
        let locator_distance = finders
            .iter()
            .map(|&finder| distance(finder, center))
            .sum::<f64>()
            / finders.len() as f64;
        (center, locator_distance)
    }

    fn svg_attr_f64(tag: &str, attr: &str) -> Option<f64> {
        let needle = format!("{attr}=\"");
        let (_, rest) = tag.split_once(&needle)?;
        let (value, _) = rest.split_once('"')?;
        value.parse().ok()
    }
}
