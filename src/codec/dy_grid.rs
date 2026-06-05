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

#[derive(Debug, Clone, Copy)]
struct ReservedAreas<'a> {
    finders: &'a [DyFinder; 3],
    badge: Option<DyBadge>,
    badge_style: DyBadgeStyle,
    logo: Option<DyLogo>,
    has_border: bool,
}

impl DyGrid {
    pub fn code_ring_count(&self) -> u8 {
        self.rings.len() as u8
    }

    pub fn ring_count(&self) -> u8 {
        self.code_ring_count() + self.decorative_rings.len() as u8
    }

    pub fn sample(&self, ring: u32, point: u32) -> bool {
        self.samples[(ring * self.points_per_ring + point) as usize]
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

    let geometry = dy_geometry(finders)?;
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
    let center_logo = source
        .and_then(|source| detect_center_logo(source, &geometry))
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
    let candidate_rings = ring_specs(
        &geometry,
        DyParams {
            ring_count: candidate_ring_count,
            ..params
        },
    );
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
        best_theta_offset(bin, &geometry, &candidate_rings, params.points_per_ring)
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
    let black_threshold = if params.has_border { 0.34 } else { 0.26 };
    let mut samples = Vec::with_capacity(rings.len() * params.points_per_ring as usize);
    let mut ratios = Vec::with_capacity(rings.len() * params.points_per_ring as usize);

    for ring_idx in 0..rings.len() as u32 {
        let ring = &rings[ring_idx as usize];
        for point in 0..params.points_per_ring {
            let reserved = is_reserved_cell(
                ring,
                ring_idx,
                point,
                params.points_per_ring,
                theta_offset,
                &geometry,
                &reserved_areas,
            );
            let ratio = sample_cell_black_ratio(
                bin,
                &geometry,
                ring,
                params.points_per_ring,
                theta_offset,
                point,
            );
            let black = !reserved && ratio >= black_threshold;
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
        Vec::new()
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
    }

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
    let thickness = (geometry.r_max - geometry.r_min) / ring_count as f64;
    (0..ring_count)
        .map(|ring| RingSpec {
            r_inner: geometry.r_max - (ring as f64 + 1.0) * thickness,
            r_outer: geometry.r_max - ring as f64 * thickness,
            is_decoration: ring == 0 || ring == 2,
        })
        .collect()
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

fn candidate_grid_score(
    bin: &BinaryImage,
    geometry: &DyGeometry,
    rings: &[RingSpec],
    points_per_ring: u32,
    theta_offset: f64,
) -> f64 {
    let mut uncertainty = 0.0;
    let mut black = 0_u32;
    let mut total = 0_u32;
    for ring in rings {
        for point in 0..points_per_ring {
            let ratio =
                sample_cell_black_ratio(bin, geometry, ring, points_per_ring, theta_offset, point);
            uncertainty += ratio.min(1.0 - ratio);
            if ratio >= 0.26 {
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

fn is_reserved_cell(
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
            distance(point_xy, (finder.cx, finder.cy)) <= finder.outer_radius() * 1.30
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

fn badge_code_skip_scale(has_border: bool, points_per_ring: u32, badge_style: DyBadgeStyle) -> f64 {
    match (has_border, points_per_ring, badge_style) {
        (true, 72, _) => BLACK_BORDER_BADGE_CODE_SKIP_SCALE_72,
        (true, _, DyBadgeStyle::Bullseye | DyBadgeStyle::DouyinLogo) => {
            BLACK_BORDER_BADGE_CODE_SKIP_SCALE_120
        }
        (false, _, _) => 1.04,
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

fn detect_center_logo(source: &DynamicImage, geometry: &DyGeometry) -> Option<DyLogo> {
    let rgba = source.to_rgba8();
    let mut visited = vec![false; (rgba.width() * rgba.height()) as usize];
    let mut best: Option<(u32, DyLogo)> = None;

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
            if distance((logo.cx, logo.cy), geometry.center) > geometry.r_min * 0.75 {
                continue;
            }
            if logo.radius < geometry.r_min * 0.10 || logo.radius > geometry.r_min * 0.95 {
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

    best.map(|(_, logo)| logo).or(Some(DyLogo {
        cx: geometry.center.0,
        cy: geometry.center.1,
        radius: geometry.r_min * 0.72,
    }))
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

    #[test]
    #[ignore]
    fn debug_black_border_optional_ring_scores() {
        for path in douyin_sample_paths()
            .into_iter()
            .map(|(path, _, _)| path)
            .chain(
                [
                    "黑框版4.jpg",
                    "黑框版5.jpg",
                    "黑框版6.png",
                    "黑框版8.png",
                    "黑框版9.png",
                    "黑框版10.png",
                    "黑框版11.jpg",
                ]
                .into_iter()
                .map(std::path::PathBuf::from),
            )
        {
            if !path.exists()
                || !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("黑框版"))
            {
                continue;
            }

            let img = image::open(&path).unwrap();
            let bin = preprocess(&img);
            let finders = find_dy_finders(&bin);
            let selected = select_dy_finders(&finders)
                .unwrap_or_else(|| panic!("failed to select dy finders for {}", path.display()));
            let params = detect_dy_params(&bin, &selected).unwrap();
            let geometry = dy_geometry(&selected).unwrap();
            let badge = estimate_black_border_badge_from_finders(&selected);
            let badge_style =
                detect_black_border_badge_style(&img, badge).unwrap_or(DyBadgeStyle::DouyinLogo);
            let rings = black_border_ring_specs(&geometry, BLACK_BORDER_CODE_RINGS.len() as u8);
            let theta_offset = black_border_standard_code_theta_offset(
                &selected,
                params.points_per_ring,
                badge_style,
            )
            .unwrap();
            let center_logo = detect_center_logo(&img, &geometry);
            let reserved = ReservedAreas {
                finders: &selected,
                badge,
                badge_style,
                logo: center_logo,
                has_border: true,
            };
            let count = detect_black_border_code_ring_count(
                &bin,
                &geometry,
                &rings,
                params.points_per_ring,
                theta_offset,
                &reserved,
            );
            eprintln!(
                "{} points={} visible_rings={}",
                path.display(),
                params.points_per_ring,
                count + 2
            );
            for ring_idx in BLACK_BORDER_BASE_CODE_RINGS as usize..rings.len() {
                let score = black_border_optional_ring_score(
                    &bin,
                    &geometry,
                    &rings,
                    ring_idx,
                    params.points_per_ring,
                    theta_offset,
                    &reserved,
                );
                let density = if score.usable_points == 0 {
                    0.0
                } else {
                    score.black_points as f64 / score.usable_points as f64
                };
                eprintln!(
                    "  candidate_ring={} black={} usable={} density={density:.3} runs={} max_run={} present={}",
                    ring_idx,
                    score.black_points,
                    score.usable_points,
                    score.black_runs,
                    score.max_run_len,
                    black_border_optional_ring_is_present(score, params.points_per_ring)
                );
            }
        }
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
