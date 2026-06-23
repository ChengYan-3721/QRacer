use nalgebra::{DMatrix, DVector, Matrix3, SMatrix, SVector, Vector3};

use crate::detect::finder_dy::{
    DyFinder, refine_dy_finder_center, refine_dy_finder_center_from_center_dot,
};
use crate::detect::finder_qr::QrFinder;
use crate::detect::finder_wx::WxFinder;
use crate::pipeline::preprocess::{BinaryImage, preprocess};
use image::{DynamicImage, Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WxUprightAnchor {
    Badge((f64, f64)),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DyBadgeAnchor {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
}

const DY_BADGE_CENTER_DX_PER_LOCATOR_LEG: f64 = 0.0270;
const DY_BADGE_CENTER_DY_PER_LOCATOR_LEG: f64 = -0.0250;

pub fn warp_qr_to_square_image(
    image: &DynamicImage,
    bin: &BinaryImage,
    finders: &[QrFinder; 3],
    target_size: u32,
) -> DynamicImage {
    let size = target_size.max(1);
    let (tl, tr, bl) = order_qr_finders(finders);
    let src = qr_outer_corners(bin, tl, tr, bl);
    let max = size.saturating_sub(1) as f64;
    let dst = [(0.0, 0.0), (max, 0.0), (0.0, max), (max, max)];
    let h = homography_from_4pts(&src, &dst);
    let Some(inv) = h.try_inverse() else {
        return DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            size,
            size,
            Rgba([255, 255, 255, 255]),
        ));
    };

    let source = image.to_rgba8();
    let mut out = RgbaImage::from_pixel(size, size, Rgba([255, 255, 255, 255]));
    for y in 0..size {
        for x in 0..size {
            let p = inv * Vector3::new(x as f64, y as f64, 1.0);
            if p.z.abs() < f64::EPSILON {
                continue;
            }
            let sx = p.x / p.z;
            let sy = p.y / p.z;
            out.put_pixel(x, y, bilinear_sample_rgba(&source, sx, sy));
        }
    }

    DynamicImage::ImageRgba8(out)
}

pub fn warp_wx_to_upright_binary(
    bin: &BinaryImage,
    finders: &[WxFinder; 3],
    anchor: Option<WxUprightAnchor>,
    target_size: u32,
) -> BinaryImage {
    if target_size == 0 {
        return BinaryImage::new(0, 0, Vec::new());
    }

    let Some(inv) = wx_upright_to_source_homography(finders, anchor, target_size) else {
        return BinaryImage::new(
            target_size,
            target_size,
            vec![255; (target_size * target_size) as usize],
        );
    };

    let mut data = vec![255; (target_size * target_size) as usize];
    for y in 0..target_size {
        for x in 0..target_size {
            let p = inv * Vector3::new(x as f64, y as f64, 1.0);
            if p.z.abs() < f64::EPSILON {
                continue;
            }
            let sx = p.x / p.z;
            let sy = p.y / p.z;
            let value = bilinear_sample(bin, sx, sy);
            data[(y * target_size + x) as usize] = if value < 128.0 { 0 } else { 255 };
        }
    }

    BinaryImage::new(target_size, target_size, data)
}

pub fn warp_wx_to_upright_image(
    image: &DynamicImage,
    finders: &[WxFinder; 3],
    anchor: Option<WxUprightAnchor>,
    target_size: u32,
) -> DynamicImage {
    let size = target_size.max(1);
    let Some(inv) = wx_upright_to_source_homography(finders, anchor, size) else {
        return DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            size,
            size,
            Rgba([255, 255, 255, 255]),
        ));
    };

    let source = image.to_rgba8();
    let mut out = RgbaImage::from_pixel(size, size, Rgba([255, 255, 255, 255]));
    for y in 0..size {
        for x in 0..size {
            let p = inv * Vector3::new(x as f64, y as f64, 1.0);
            if p.z.abs() < f64::EPSILON {
                continue;
            }
            let sx = p.x / p.z;
            let sy = p.y / p.z;
            out.put_pixel(x, y, bilinear_sample_rgba(&source, sx, sy));
        }
    }

    DynamicImage::ImageRgba8(out)
}

pub fn detect_wx_badge_anchor(image: &DynamicImage) -> Option<(f64, f64)> {
    let rgba = image.to_rgba8();
    let mut visited = vec![false; (rgba.width() * rgba.height()) as usize];
    let min_dim = rgba.width().min(rgba.height()) as f64;
    let min_area = (min_dim * 0.045).powi(2) as u32;
    let mut best: Option<(f64, (f64, f64))> = None;

    for y in 0..rgba.height() as i32 {
        for x in 0..rgba.width() as i32 {
            let idx = (y as u32 * rgba.width() + x as u32) as usize;
            if visited[idx] || !is_wx_badge_shape_pixel(rgba.get_pixel(x as u32, y as u32).0) {
                continue;
            }

            let Some(component) = flood_badge_shape_component(&rgba, &mut visited, x, y) else {
                continue;
            };
            if component.area < min_area {
                continue;
            }
            let center = component.center();
            if center.0 < rgba.width() as f64 * 0.55 || center.1 < rgba.height() as f64 * 0.55 {
                continue;
            }
            if !component.is_badge_like(min_dim) {
                continue;
            }

            let score = component.area as f64 * component.shape_score();
            if best
                .as_ref()
                .is_none_or(|(best_score, _)| score > *best_score)
            {
                best = Some((score, center));
            }
        }
    }

    best.map(|(_, center)| center)
        .or_else(|| scan_badge_shape_anchor(&rgba))
}

pub fn wx_upright_target_finders(_finders: &[WxFinder; 3], target_size: u32) -> [WxFinder; 3] {
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

pub fn warp_dy_to_upright_binary(
    bin: &BinaryImage,
    finders: &[DyFinder; 3],
    target_size: u32,
) -> BinaryImage {
    warp_dy_to_upright_binary_with_top_right(bin, finders, None, target_size)
}

pub fn warp_dy_to_upright_binary_with_top_right(
    bin: &BinaryImage,
    finders: &[DyFinder; 3],
    top_right: Option<(f64, f64)>,
    target_size: u32,
) -> BinaryImage {
    if target_size == 0 {
        return BinaryImage::new(0, 0, Vec::new());
    }

    let Some(inv) = dy_upright_to_source_homography(finders, top_right, target_size) else {
        return BinaryImage::new(
            target_size,
            target_size,
            vec![255; (target_size * target_size) as usize],
        );
    };

    let mut data = vec![255; (target_size * target_size) as usize];
    for y in 0..target_size {
        for x in 0..target_size {
            let p = inv * Vector3::new(x as f64, y as f64, 1.0);
            if p.z.abs() < f64::EPSILON {
                continue;
            }
            let sx = p.x / p.z;
            let sy = p.y / p.z;
            let value = bilinear_sample(bin, sx, sy);
            data[(y * target_size + x) as usize] = if value < 128.0 { 0 } else { 255 };
        }
    }

    BinaryImage::new(target_size, target_size, data)
}

pub fn warp_dy_to_upright_image_with_top_right(
    image: &DynamicImage,
    finders: &[DyFinder; 3],
    top_right: Option<(f64, f64)>,
    target_size: u32,
) -> DynamicImage {
    let size = target_size.max(1);
    let Some(inv) = dy_upright_to_source_homography(finders, top_right, size) else {
        return DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            size,
            size,
            Rgba([255, 255, 255, 255]),
        ));
    };

    warp_image_with_inverse(image, &inv, size)
}

fn warp_image_with_inverse(image: &DynamicImage, inv: &Matrix3<f64>, size: u32) -> DynamicImage {
    let source = image.to_rgba8();
    let mut out = RgbaImage::from_pixel(size, size, Rgba([255, 255, 255, 255]));
    for y in 0..size {
        for x in 0..size {
            let p = inv * Vector3::new(x as f64, y as f64, 1.0);
            if p.z.abs() < f64::EPSILON {
                continue;
            }
            let sx = p.x / p.z;
            let sy = p.y / p.z;
            out.put_pixel(x, y, bilinear_sample_rgba(&source, sx, sy));
        }
    }

    DynamicImage::ImageRgba8(out)
}

pub struct DyUprightCorrection {
    pub source: DynamicImage,
    pub binary: BinaryImage,
    pub finders: [DyFinder; 3],
}

/// 抖音码统一校正管线：牛眼中心亚像素精修 + 透视校正到正立 + 重新二值化。
///
/// 主程序 `process_dy_image` 与所有无框版调参测试都必须经过该函数，
/// 保证 debug diff 输出与实际校正预览基于同一份采样输入。
///
/// 精修后若三枚定位点本来就接近轴对齐（两条腿倾角都小于
/// `DY_UPRIGHT_SNAP_MAX_TILT_DEG`），判定原图为标准正立图，改用无旋转的
/// 相似变换（均匀缩放 + 平移）：该变换不可能引入旋转或透视，检测误差只会
/// 化为亚像素级的平移/缩放残差，不会把正立原图校歪。只有真正倾斜或带
/// 透视的输入才走 badge 锚点 + 单应变换路径。
pub fn correct_dy_to_upright(
    image: &DynamicImage,
    raw_binary: &BinaryImage,
    raw_selected: &[DyFinder; 3],
) -> DyUprightCorrection {
    let refined = [
        refine_dy_finder_center(raw_binary, &raw_selected[0]),
        refine_dy_finder_center(raw_binary, &raw_selected[1]),
        refine_dy_finder_center(raw_binary, &raw_selected[2]),
    ];
    if false && is_standard_dy_upright_input(&refined) {
        if dy_direct_no_border_correction_candidate_allowed(image)
            && !dy_corrected_has_black_border(raw_binary, &refined)
        {
            let size = image.width().max(image.height()).clamp(1024, 1600);
            let correction = correct_dy_to_upright_with_refined(image, &refined, size);
            if !dy_corrected_has_black_border(&correction.binary, &correction.finders) {
                let direct_score = dy_corrected_no_border_score(raw_binary, &refined);
                let corrected_score =
                    dy_corrected_no_border_score(&correction.binary, &correction.finders);
                if corrected_score + DY_DIRECT_NO_BORDER_CORRECTION_SCORE_MARGIN < direct_score {
                    return correction;
                }
            }
        }
        return DyUprightCorrection {
            source: image.clone(),
            binary: raw_binary.clone(),
            finders: refined,
        };
    }

    let size = image.width().max(image.height()).clamp(1024, 1600);
    let mut correction = correct_dy_to_upright_with_refined(image, &refined, size);
    if false && !dy_corrected_has_black_border(&correction.binary, &correction.finders) {
        let shifted_correction = correct_dy_to_upright_with_refined_and_badge_shift(
            image,
            &refined,
            size,
            0.0,
            DY_NO_BORDER_BADGE_SOURCE_Y_SHIFT_SCALE,
        );
        if !dy_corrected_has_black_border(&shifted_correction.binary, &shifted_correction.finders)
            && dy_corrected_no_border_score(&shifted_correction.binary, &shifted_correction.finders)
                < dy_corrected_no_border_score(&correction.binary, &correction.finders)
        {
            correction = shifted_correction;
        }
        let weighted_correction = correct_dy_to_upright_with_weighted_badge(
            image,
            &refined,
            size,
            0.0,
            DY_NO_BORDER_BADGE_SOURCE_Y_SHIFT_SCALE,
            DY_NO_BORDER_BADGE_HOMOGRAPHY_WEIGHT,
        );
        if !dy_corrected_has_black_border(&weighted_correction.binary, &weighted_correction.finders)
            && dy_corrected_no_border_score(
                &weighted_correction.binary,
                &weighted_correction.finders,
            ) < dy_corrected_no_border_score(&correction.binary, &correction.finders)
        {
            correction = weighted_correction;
        }

        let dot_refined = [
            refine_dy_finder_center_from_center_dot(raw_binary, &raw_selected[0]),
            refine_dy_finder_center_from_center_dot(raw_binary, &raw_selected[1]),
            refine_dy_finder_center_from_center_dot(raw_binary, &raw_selected[2]),
        ];
        let dot_correction = correct_dy_to_upright_with_refined(image, &dot_refined, size);
        if !dy_corrected_has_black_border(&dot_correction.binary, &dot_correction.finders)
            && dy_corrected_no_border_score(&dot_correction.binary, &dot_correction.finders)
                < dy_corrected_no_border_score(&correction.binary, &correction.finders)
        {
            correction = dot_correction;
        }
        if dy_finder_max_axis_tilt_deg(&dot_refined) <= DY_NO_BORDER_SHIFTED_DOT_MAX_TILT_DEG {
            let shifted_dot_correction = correct_dy_to_upright_with_refined_and_badge_shift(
                image,
                &dot_refined,
                size,
                0.0,
                DY_NO_BORDER_BADGE_SOURCE_Y_SHIFT_SCALE,
            );
            if !dy_corrected_has_black_border(
                &shifted_dot_correction.binary,
                &shifted_dot_correction.finders,
            ) && dy_corrected_no_border_score(
                &shifted_dot_correction.binary,
                &shifted_dot_correction.finders,
            ) < dy_corrected_no_border_score(&correction.binary, &correction.finders)
            {
                correction = shifted_dot_correction;
            }
            let shifted_dot_left_correction = correct_dy_to_upright_with_refined_and_badge_shift(
                image,
                &dot_refined,
                size,
                DY_NO_BORDER_BADGE_SOURCE_X_SHIFT_SCALE,
                DY_NO_BORDER_BADGE_SOURCE_Y_SHIFT_SCALE,
            );
            if !dy_corrected_has_black_border(
                &shifted_dot_left_correction.binary,
                &shifted_dot_left_correction.finders,
            ) && dy_corrected_no_border_score(
                &shifted_dot_left_correction.binary,
                &shifted_dot_left_correction.finders,
            ) < dy_corrected_no_border_score(&correction.binary, &correction.finders)
            {
                correction = shifted_dot_left_correction;
            }
            let shifted_dot_small_correction = correct_dy_to_upright_with_refined_and_badge_shift(
                image,
                &dot_refined,
                size,
                -DY_NO_BORDER_BADGE_SOURCE_SMALL_SHIFT_SCALE,
                DY_NO_BORDER_BADGE_SOURCE_SMALL_SHIFT_SCALE,
            );
            if !dy_corrected_has_black_border(
                &shifted_dot_small_correction.binary,
                &shifted_dot_small_correction.finders,
            ) && dy_corrected_no_border_score(
                &shifted_dot_small_correction.binary,
                &shifted_dot_small_correction.finders,
            ) < dy_corrected_no_border_score(&correction.binary, &correction.finders)
            {
                correction = shifted_dot_small_correction;
            }
        }
    }

    let DyUprightCorrection {
        source,
        binary,
        finders,
    } = correction;
    DyUprightCorrection {
        source,
        binary,
        finders,
    }
}

fn correct_dy_to_upright_with_refined(
    image: &DynamicImage,
    refined: &[DyFinder; 3],
    size: u32,
) -> DyUprightCorrection {
    correct_dy_to_upright_with_refined_and_badge_shift(image, refined, size, 0.0, 0.0)
}

fn correct_dy_to_upright_with_refined_and_badge_shift(
    image: &DynamicImage,
    refined: &[DyFinder; 3],
    size: u32,
    badge_x_shift_scale: f64,
    badge_y_shift_scale: f64,
) -> DyUprightCorrection {
    let badge = detect_dy_badge_anchor(image, refined);
    let top_right = badge.map(|badge| {
        (
            badge.cx + badge.radius * badge_x_shift_scale,
            badge.cy + badge.radius * badge_y_shift_scale.max(0.0),
        )
    });
    let source = if let Some(top_right) = top_right {
        if let Some(inv) = dy_upright_badge_snap_inverse(refined, top_right, size) {
            warp_image_with_inverse(image, &inv, size)
        } else if let Some(inv) = dy_upright_snap_inverse(refined, size) {
            warp_image_with_inverse(image, &inv, size)
        } else {
            warp_dy_to_upright_image_with_top_right(image, refined, Some(top_right), size)
        }
    } else if let Some(inv) = dy_upright_snap_inverse(refined, size) {
        warp_image_with_inverse(image, &inv, size)
    } else {
        warp_dy_to_upright_image_with_top_right(image, refined, None, size)
    };
    let binary = preprocess(&source);
    let finders = dy_upright_target_finders(refined, size);
    DyUprightCorrection {
        source,
        binary,
        finders,
    }
}

fn correct_dy_to_upright_with_weighted_badge(
    image: &DynamicImage,
    refined: &[DyFinder; 3],
    size: u32,
    badge_x_shift_scale: f64,
    badge_y_shift_scale: f64,
    badge_weight: f64,
) -> DyUprightCorrection {
    let source = detect_dy_badge_anchor(image, refined)
        .and_then(|badge| {
            dy_upright_to_source_weighted_badge_homography(
                refined,
                (
                    badge.cx + badge.radius * badge_x_shift_scale,
                    badge.cy + badge.radius * badge_y_shift_scale.max(0.0),
                ),
                size,
                badge_weight,
            )
        })
        .map(|inv| warp_image_with_inverse(image, &inv, size))
        .unwrap_or_else(|| correct_dy_to_upright_with_refined(image, refined, size).source);
    let binary = preprocess(&source);
    let finders = dy_upright_target_finders(refined, size);
    DyUprightCorrection {
        source,
        binary,
        finders,
    }
}

#[derive(Debug, Clone, Copy)]
struct DyScoreGeometry {
    center: (f64, f64),
    locator_distance: f64,
    r_max: f64,
}

#[derive(Debug, Clone, Copy)]
struct DyScoreRing {
    r_inner: f64,
    r_outer: f64,
    is_decoration: bool,
}

const DY_NO_BORDER_STANDARD_LOCATOR_DISTANCE: f64 = 240.529442688416;
const DY_NO_BORDER_STANDARD_SAMPLE_THETA_OFFSET: f64 = -0.5_f64.to_radians();
const DY_NO_BORDER_BLACK_THRESHOLD: f64 = 0.55;
const DY_NO_BORDER_DECORATIVE_BLACK_THRESHOLD: f64 = 0.50;
const DY_NO_BORDER_DECORATIVE_RADIUS_SCORE_WEIGHT: f64 = 0.80;
const DY_NO_BORDER_RADIUS_SCORE_THRESHOLD: f64 = 0.26;
const DY_NO_BORDER_RADIUS_SCORE_THETA_OFFSETS: [f64; 3] = [-0.15, 0.0, 0.15];
const DY_NO_BORDER_RADIUS_SCORE_RADIAL_OFFSETS: [f64; 3] = [-0.20, 0.0, 0.20];
const DY_NO_BORDER_SCORE_CENTER_REFINE_MAX_RADIUS: f64 = 8.0;
const DY_NO_BORDER_SCORE_CENTER_REFINE_STEP: f64 = 1.0;
const DY_NO_BORDER_SCORE_CENTER_OFFSET_WEIGHT: f64 = 0.001;
const DY_NO_BORDER_BADGE_SOURCE_Y_SHIFT_SCALE: f64 = 0.15;
const DY_NO_BORDER_BADGE_SOURCE_X_SHIFT_SCALE: f64 = -0.15;
const DY_NO_BORDER_BADGE_SOURCE_SMALL_SHIFT_SCALE: f64 = 0.12;
const DY_NO_BORDER_SHIFTED_DOT_MAX_TILT_DEG: f64 = 6.0;
const DY_NO_BORDER_BADGE_HOMOGRAPHY_WEIGHT: f64 = 0.45;
const DY_DIRECT_NO_BORDER_CORRECTION_SCORE_MARGIN: f64 = 0.0;
const DY_NO_BORDER_RINGS: [(f64, f64, bool); 6] = [
    (228.66, 5.0, true),
    (207.98, 5.0, false),
    (188.59, 5.0, true),
    (171.71, 5.0, false),
    (153.74, 5.0, false),
    (133.24, 5.0, false),
];

fn dy_corrected_has_black_border(bin: &BinaryImage, finders: &[DyFinder; 3]) -> bool {
    let geometry = dy_score_geometry(finders);
    let mut score = 0.0_f64;
    for ratio in [0.88, 0.92, 0.96, 1.0] {
        score = score.max(dy_radial_black_score(
            bin,
            geometry.center,
            geometry.r_max * ratio,
        ));
    }
    let outside_score = dy_radial_black_score(bin, geometry.center, geometry.r_max * 1.06);
    score > 0.30 && outside_score < 0.45
}

fn dy_corrected_no_border_score(bin: &BinaryImage, finders: &[DyFinder; 3]) -> f64 {
    let theta_offset = dy_no_border_score_theta_offset(finders);
    let base_geometry = dy_score_geometry(finders);
    let mut best = f64::INFINITY;
    let steps = (DY_NO_BORDER_SCORE_CENTER_REFINE_MAX_RADIUS
        / DY_NO_BORDER_SCORE_CENTER_REFINE_STEP)
        .ceil() as i32;

    for dy_step in -steps..=steps {
        for dx_step in -steps..=steps {
            let dx = f64::from(dx_step) * DY_NO_BORDER_SCORE_CENTER_REFINE_STEP;
            let dy = f64::from(dy_step) * DY_NO_BORDER_SCORE_CENTER_REFINE_STEP;
            let offset2 = dx * dx + dy * dy;
            if offset2
                > DY_NO_BORDER_SCORE_CENTER_REFINE_MAX_RADIUS
                    * DY_NO_BORDER_SCORE_CENTER_REFINE_MAX_RADIUS
            {
                continue;
            }
            let geometry = dy_score_geometry_with_center_offset(&base_geometry, finders, dx, dy);
            let rings = dy_no_border_score_rings(&geometry);
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
            let score =
                dy_candidate_no_border_grid_score(bin, &geometry, &code_rings, 120, theta_offset)
                    + dy_candidate_no_border_grid_score(
                        bin,
                        &geometry,
                        &decorative_rings,
                        120,
                        theta_offset,
                    ) * DY_NO_BORDER_DECORATIVE_RADIUS_SCORE_WEIGHT
                    + offset2 * DY_NO_BORDER_SCORE_CENTER_OFFSET_WEIGHT;
            best = best.min(score);
        }
    }

    best
}

fn dy_score_geometry(finders: &[DyFinder; 3]) -> DyScoreGeometry {
    let ordered = order_dy_finders(finders);
    let tl = &ordered[0];
    let br = &ordered[2];
    let center = ((tl.cx + br.cx) * 0.5, (tl.cy + br.cy) * 0.5);
    let locator_radius = finders.iter().map(DyFinder::outer_radius).sum::<f64>() / 3.0;
    let locator_distance = finders
        .iter()
        .map(|finder| dy_point_distance(center, (finder.cx, finder.cy)))
        .sum::<f64>()
        / finders.len() as f64;
    let r_max = finders
        .iter()
        .map(|finder| {
            dy_point_distance(center, (finder.cx, finder.cy)) + finder.outer_radius() * 1.10
        })
        .fold(0.0, f64::max)
        .max(locator_radius * 5.0);

    DyScoreGeometry {
        center,
        locator_distance,
        r_max,
    }
}

fn dy_score_geometry_with_center_offset(
    base_geometry: &DyScoreGeometry,
    finders: &[DyFinder; 3],
    dx: f64,
    dy: f64,
) -> DyScoreGeometry {
    let center = (base_geometry.center.0 + dx, base_geometry.center.1 + dy);
    let locator_radius = finders.iter().map(DyFinder::outer_radius).sum::<f64>() / 3.0;
    let locator_distance = finders
        .iter()
        .map(|finder| dy_point_distance(center, (finder.cx, finder.cy)))
        .sum::<f64>()
        / finders.len() as f64;
    let r_max = finders
        .iter()
        .map(|finder| {
            dy_point_distance(center, (finder.cx, finder.cy)) + finder.outer_radius() * 1.10
        })
        .fold(0.0, f64::max)
        .max(locator_radius * 5.0);

    DyScoreGeometry {
        center,
        locator_distance,
        r_max,
    }
}

fn dy_no_border_score_rings(geometry: &DyScoreGeometry) -> Vec<DyScoreRing> {
    let scale = (geometry.locator_distance / DY_NO_BORDER_STANDARD_LOCATOR_DISTANCE).max(0.01);
    DY_NO_BORDER_RINGS
        .iter()
        .map(|&(radius, half_width, is_decoration)| DyScoreRing {
            r_inner: (radius - half_width) * scale,
            r_outer: (radius + half_width) * scale,
            is_decoration,
        })
        .collect()
}

fn dy_no_border_score_theta_offset(finders: &[DyFinder; 3]) -> f64 {
    let ordered = order_dy_finders(finders);
    let tl = &ordered[0];
    let br = &ordered[2];
    let diagonal_angle = (br.cy - tl.cy).atan2(br.cx - tl.cx);
    let rotation = diagonal_angle - std::f64::consts::FRAC_PI_4;

    (DY_NO_BORDER_STANDARD_SAMPLE_THETA_OFFSET + rotation).rem_euclid(std::f64::consts::TAU)
}

fn dy_candidate_no_border_grid_score(
    bin: &BinaryImage,
    geometry: &DyScoreGeometry,
    rings: &[DyScoreRing],
    points_per_ring: u32,
    theta_offset: f64,
) -> f64 {
    let mut uncertainty = 0.0;
    let mut score_black = 0_u32;
    let mut score_total = 0_u32;
    let mut ring_density_penalty = 0.0;
    let mut scored_rings = 0_u32;

    for ring in rings {
        let final_threshold = if ring.is_decoration {
            DY_NO_BORDER_DECORATIVE_BLACK_THRESHOLD
        } else {
            DY_NO_BORDER_BLACK_THRESHOLD
        };
        let min_density = if ring.is_decoration { 0.18 } else { 0.40 };
        let mut ring_black = 0_u32;
        let mut ring_total = 0_u32;

        for point in 0..points_per_ring {
            let ratio = dy_sample_cell_black_ratio_with_offsets(
                bin,
                geometry,
                ring,
                points_per_ring,
                theta_offset,
                point,
                (
                    &DY_NO_BORDER_RADIUS_SCORE_THETA_OFFSETS,
                    &DY_NO_BORDER_RADIUS_SCORE_RADIAL_OFFSETS,
                ),
            );
            uncertainty += ratio.min(1.0 - ratio);
            if ratio >= DY_NO_BORDER_RADIUS_SCORE_THRESHOLD {
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

fn dy_sample_cell_black_ratio_with_offsets(
    bin: &BinaryImage,
    geometry: &DyScoreGeometry,
    ring: &DyScoreRing,
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
            if dy_sample_polar(bin, geometry.center, sample_radius, sample_theta) {
                black += 1;
            }
            total += 1;
        }
    }

    black as f64 / total as f64
}

fn dy_sample_polar(bin: &BinaryImage, center: (f64, f64), radius: f64, theta: f64) -> bool {
    let x = (center.0 + radius * theta.cos()).round() as i32;
    let y = (center.1 + radius * theta.sin()).round() as i32;
    bin.is_black(x, y)
}

fn dy_radial_black_score(bin: &BinaryImage, center: (f64, f64), radius: f64) -> f64 {
    const SAMPLES: u32 = 144;
    let mut black = 0;
    let mut total = 0;
    for idx in 0..SAMPLES {
        let theta = f64::from(idx) * std::f64::consts::TAU / f64::from(SAMPLES);
        let x = (center.0 + radius * theta.cos()).round() as i32;
        let y = (center.1 + radius * theta.sin()).round() as i32;
        if x < 0 || y < 0 || x >= bin.w as i32 || y >= bin.h as i32 {
            continue;
        }
        total += 1;
        if bin.is_black(x, y) {
            black += 1;
        }
    }

    f64::from(black) / f64::from(total.max(1))
}

/// 判定为"本来就是正立图"的最大腿倾角。定位点检测误差通常表现为
/// 1 度以内的伪倾斜；真实拍摄倾斜一般明显大于该值。
const DY_UPRIGHT_SNAP_MAX_TILT_DEG: f64 = 1.5;
const DY_STANDARD_DIRECT_MAX_LEG_DELTA_RATIO: f64 = 0.035;
const DY_DIRECT_NO_BORDER_CORRECTION_MAX_DIM: u32 = 500;

fn dy_direct_no_border_correction_candidate_allowed(image: &DynamicImage) -> bool {
    image.width() != image.height()
        && image.width().max(image.height()) <= DY_DIRECT_NO_BORDER_CORRECTION_MAX_DIM
}

fn is_standard_dy_upright_input(finders: &[DyFinder; 3]) -> bool {
    let ordered = order_dy_finders(finders);
    let tl = &ordered[0];
    let bl = &ordered[1];
    let br = &ordered[2];
    let bottom_tilt = (br.cy - bl.cy).atan2(br.cx - bl.cx);
    let left_tilt = (bl.cx - tl.cx).atan2(bl.cy - tl.cy);
    let max_tilt = DY_UPRIGHT_SNAP_MAX_TILT_DEG.to_radians();
    if bottom_tilt.abs() > max_tilt || left_tilt.abs() > max_tilt {
        return false;
    }

    let left_leg = dy_distance(tl, bl);
    let bottom_leg = dy_distance(bl, br);
    let average_leg = ((left_leg + bottom_leg) * 0.5).max(1.0);
    (left_leg - bottom_leg).abs() / average_leg <= DY_STANDARD_DIRECT_MAX_LEG_DELTA_RATIO
}

/// 正立吸附：三点最小二乘的无旋转相似变换（目标像素 → 源像素）。
fn dy_finder_max_axis_tilt_deg(finders: &[DyFinder; 3]) -> f64 {
    let ordered = order_dy_finders(finders);
    let tl = &ordered[0];
    let bl = &ordered[1];
    let br = &ordered[2];
    let bottom_tilt = (br.cy - bl.cy).atan2(br.cx - bl.cx).abs();
    let left_tilt = (bl.cx - tl.cx).atan2(bl.cy - tl.cy).abs();

    bottom_tilt.max(left_tilt).to_degrees()
}

fn dy_upright_snap_inverse(finders: &[DyFinder; 3], target_size: u32) -> Option<Matrix3<f64>> {
    let ordered = order_dy_finders(finders);
    let tl = &ordered[0];
    let bl = &ordered[1];
    let br = &ordered[2];
    let bottom_tilt = (br.cy - bl.cy).atan2(br.cx - bl.cx);
    let left_tilt = (bl.cx - tl.cx).atan2(bl.cy - tl.cy);
    let max_tilt = DY_UPRIGHT_SNAP_MAX_TILT_DEG.to_radians();
    if bottom_tilt.abs() > max_tilt || left_tilt.abs() > max_tilt {
        return None;
    }

    let max = target_size.saturating_sub(1) as f64;
    let margin = max * 0.23;
    let far = max - margin;
    let target_leg = far - margin;
    let source_leg = (dy_distance(tl, bl) + dy_distance(bl, br)) * 0.5;
    if source_leg <= f64::EPSILON {
        return None;
    }

    let scale = target_leg / source_leg;
    let src_mean = ((tl.cx + bl.cx + br.cx) / 3.0, (tl.cy + bl.cy + br.cy) / 3.0);
    let dst_mean = ((margin * 2.0 + far) / 3.0, (margin + far * 2.0) / 3.0);
    let tx = dst_mean.0 - scale * src_mean.0;
    let ty = dst_mean.1 - scale * src_mean.1;

    Some(Matrix3::new(
        1.0 / scale,
        0.0,
        -tx / scale,
        0.0,
        1.0 / scale,
        -ty / scale,
        0.0,
        0.0,
        1.0,
    ))
}

fn dy_upright_badge_snap_inverse(
    finders: &[DyFinder; 3],
    top_right: (f64, f64),
    target_size: u32,
) -> Option<Matrix3<f64>> {
    dy_upright_badge_snap_inverse_with_offset(
        finders,
        top_right,
        target_size,
        DY_BADGE_CENTER_DX_PER_LOCATOR_LEG,
        DY_BADGE_CENTER_DY_PER_LOCATOR_LEG,
    )
}

fn dy_upright_badge_snap_inverse_with_offset(
    finders: &[DyFinder; 3],
    top_right: (f64, f64),
    target_size: u32,
    dx_per_locator_leg: f64,
    dy_per_locator_leg: f64,
) -> Option<Matrix3<f64>> {
    let ordered = order_dy_finders(finders);
    let bottom_tilt = (ordered[2].cy - ordered[1].cy).atan2(ordered[2].cx - ordered[1].cx);
    let left_tilt = (ordered[1].cx - ordered[0].cx).atan2(ordered[1].cy - ordered[0].cy);
    let max_tilt = DY_UPRIGHT_SNAP_MAX_TILT_DEG.to_radians();
    if bottom_tilt.abs() > max_tilt || left_tilt.abs() > max_tilt {
        return None;
    }

    let max = target_size.saturating_sub(1) as f64;
    let margin = max * 0.23;
    let far = max - margin;
    let target_leg = far - margin;
    let target_badge = (
        far + dx_per_locator_leg * target_leg,
        margin + dy_per_locator_leg * target_leg,
    );
    let target = [(margin, margin), target_badge, (margin, far), (far, far)];
    let source = [
        (ordered[0].cx, ordered[0].cy),
        top_right,
        (ordered[1].cx, ordered[1].cy),
        (ordered[2].cx, ordered[2].cy),
    ];

    no_rotation_similarity_inverse(&source, &target)
}

fn no_rotation_similarity_inverse(
    source: &[(f64, f64)],
    target: &[(f64, f64)],
) -> Option<Matrix3<f64>> {
    if source.len() < 2 || source.len() != target.len() {
        return None;
    }

    let count = source.len() as f64;
    let source_center = (
        source.iter().map(|point| point.0).sum::<f64>() / count,
        source.iter().map(|point| point.1).sum::<f64>() / count,
    );
    let target_center = (
        target.iter().map(|point| point.0).sum::<f64>() / count,
        target.iter().map(|point| point.1).sum::<f64>() / count,
    );
    let mut dot = 0.0;
    let mut source_norm2 = 0.0;

    for (source, target) in source.iter().zip(target) {
        let sx = source.0 - source_center.0;
        let sy = source.1 - source_center.1;
        let tx = target.0 - target_center.0;
        let ty = target.1 - target_center.1;
        dot += sx * tx + sy * ty;
        source_norm2 += sx * sx + sy * sy;
    }
    if source_norm2 <= f64::EPSILON {
        return None;
    }

    let scale = dot / source_norm2;
    if scale.abs() <= f64::EPSILON {}

    let tx = target_center.0 - scale * source_center.0;
    let ty = target_center.1 - scale * source_center.1;

    Some(Matrix3::new(
        1.0 / scale,
        0.0,
        -tx / scale,
        0.0,
        1.0 / scale,
        -ty / scale,
        0.0,
        0.0,
        1.0,
    ))
}

pub fn detect_dy_badge_anchor(
    image: &DynamicImage,
    finders: &[DyFinder; 3],
) -> Option<DyBadgeAnchor> {
    let ordered = order_dy_finders(finders);
    let tl = &ordered[0];
    let bl = &ordered[1];
    let br = &ordered[2];
    let expected = (tl.cx + br.cx - bl.cx, tl.cy + br.cy - bl.cy);
    let center = ((tl.cx + br.cx) * 0.5, (tl.cy + br.cy) * 0.5);
    let locator_radius = finders.iter().map(DyFinder::outer_radius).sum::<f64>() / 3.0;
    let locator_distance = finders
        .iter()
        .map(|finder| dy_point_distance(center, (finder.cx, finder.cy)))
        .sum::<f64>()
        / finders.len() as f64;
    let r_max = finders
        .iter()
        .map(|finder| dy_point_distance(center, (finder.cx, finder.cy)) + finder.outer_radius())
        .fold(0.0, f64::max)
        .max(locator_radius * 5.0);
    let r_min = (r_max * 0.36).max(locator_radius * 2.0);
    let rgba = image.to_rgba8();
    let min_dim = rgba.width().min(rgba.height()) as f64;
    let min_area = (min_dim * 0.045).powi(2) as u32;
    let mut visited = vec![false; (rgba.width() * rgba.height()) as usize];
    let mut best: Option<(f64, DyBadgeAnchor)> = None;

    for y in 0..rgba.height() as i32 {
        for x in 0..rgba.width() as i32 {
            let idx = (y as u32 * rgba.width() + x as u32) as usize;
            if visited[idx] || !dy_badge_dark_pixel(rgba.get_pixel(x as u32, y as u32).0) {
                continue;
            }
            let Some(component) = flood_dy_badge_component(&rgba, &mut visited, x, y) else {
                continue;
            };
            if component.area < min_area || !component.is_roundish(min_dim) {
                continue;
            }
            let anchor = component.to_anchor();
            let distance_to_center = dy_point_distance((anchor.cx, anchor.cy), center);
            if distance_to_center < r_min || distance_to_center > r_max * 1.25 {
                continue;
            }
            if dy_point_distance((anchor.cx, anchor.cy), expected) > locator_distance * 0.42 {
                continue;
            }
            if anchor.radius < r_max * 0.10 || anchor.radius > r_max * 0.34 {
                continue;
            }

            let expected_penalty =
                dy_point_distance((anchor.cx, anchor.cy), expected) / locator_distance.max(1.0);
            let score = component.area as f64 * component.shape_score() / (1.0 + expected_penalty);
            if best
                .as_ref()
                .is_none_or(|(best_score, _)| score > *best_score)
            {
                best = Some((score, anchor));
            }
        }
    }

    best.map(|(_, anchor)| anchor)
        .or_else(|| scan_dy_badge_anchor(&rgba, expected, locator_radius, locator_distance))
}

fn scan_dy_badge_anchor(
    image: &RgbaImage,
    expected: (f64, f64),
    locator_radius: f64,
    locator_distance: f64,
) -> Option<DyBadgeAnchor> {
    let search_radius = (locator_distance * 0.20).max(locator_radius * 1.5);
    let radius_min = (locator_radius * 1.5).max(8.0);
    let radius_max = (locator_radius * 3.2).max(radius_min + 1.0);
    let step = (locator_radius * 0.18).round().max(2.0) as i32;
    let radius_step = (locator_radius * 0.12).round().max(1.0);
    let mut best: Option<(f64, DyBadgeAnchor)> = None;

    let x0 = (expected.0 - search_radius).floor() as i32;
    let x1 = (expected.0 + search_radius).ceil() as i32;
    let y0 = (expected.1 - search_radius).floor() as i32;
    let y1 = (expected.1 + search_radius).ceil() as i32;
    let mut y = y0;
    while y <= y1 {
        let mut x = x0;
        while x <= x1 {
            let distance_to_expected = dy_point_distance((x as f64, y as f64), expected);
            if distance_to_expected <= search_radius {
                let mut radius = radius_min;
                while radius <= radius_max {
                    let score = dy_badge_template_score(image, x as f64, y as f64, radius)
                        - distance_to_expected / locator_distance.max(1.0) * 0.35;
                    if score > 1.55
                        && best
                            .as_ref()
                            .is_none_or(|(best_score, _)| score > *best_score)
                    {
                        best = Some((
                            score,
                            DyBadgeAnchor {
                                cx: x as f64,
                                cy: y as f64,
                                radius,
                            },
                        ));
                    }
                    radius += radius_step;
                }
            }
            x += step;
        }
        y += step;
    }

    best.map(|(_, anchor)| anchor)
}

fn dy_badge_template_score(image: &RgbaImage, cx: f64, cy: f64, radius: f64) -> f64 {
    let ring = dy_badge_ring_dark_ratio(image, cx, cy, radius * 0.92);
    let inner_light = 1.0 - dy_badge_disk_dark_ratio(image, cx, cy, radius * 0.62);
    let outside_light = 1.0 - dy_badge_ring_dark_ratio(image, cx, cy, radius * 1.18);

    if ring < 0.42 || inner_light < 0.45 || outside_light < 0.45 {
        return 0.0;
    }

    ring * 1.6 + inner_light * 0.8 + outside_light * 0.5
}

fn dy_badge_ring_dark_ratio(image: &RgbaImage, cx: f64, cy: f64, radius: f64) -> f64 {
    const SAMPLES: u32 = 96;
    let mut dark = 0_u32;
    let mut total = 0_u32;

    for idx in 0..SAMPLES {
        let theta = idx as f64 * std::f64::consts::TAU / SAMPLES as f64;
        let x = (cx + radius * theta.cos()).round() as i32;
        let y = (cy + radius * theta.sin()).round() as i32;
        if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
            continue;
        }
        total += 1;
        if dy_badge_dark_pixel(image.get_pixel(x as u32, y as u32).0) {
            dark += 1;
        }
    }

    f64::from(dark) / f64::from(total.max(1))
}

fn dy_badge_disk_dark_ratio(image: &RgbaImage, cx: f64, cy: f64, radius: f64) -> f64 {
    let min_x = (cx - radius).floor().max(0.0) as i32;
    let max_x = (cx + radius).ceil().min(image.width() as f64 - 1.0) as i32;
    let min_y = (cy - radius).floor().max(0.0) as i32;
    let max_y = (cy + radius).ceil().min(image.height() as f64 - 1.0) as i32;
    let radius2 = radius * radius;
    let mut dark = 0_u32;
    let mut total = 0_u32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f64 + 0.5 - cx;
            let dy = y as f64 + 0.5 - cy;
            if dx * dx + dy * dy > radius2 {
                continue;
            }
            total += 1;
            if dy_badge_dark_pixel(image.get_pixel(x as u32, y as u32).0) {
                dark += 1;
            }
        }
    }

    f64::from(dark) / f64::from(total.max(1))
}

pub fn dy_upright_target_finders(finders: &[DyFinder; 3], target_size: u32) -> [DyFinder; 3] {
    let ordered = order_dy_finders(finders);
    let max = target_size.saturating_sub(1) as f64;
    let margin = max * 0.23;
    let far = max - margin;
    let target_leg = far - margin;
    let source_leg =
        (dy_distance(&ordered[0], &ordered[1]) + dy_distance(&ordered[1], &ordered[2])) * 0.5;
    let scale = if source_leg <= f64::EPSILON {
        1.0
    } else {
        target_leg / source_leg
    };

    [
        DyFinder {
            cx: margin,
            cy: margin,
            rings: scaled_dy_rings(&ordered[0], scale),
        },
        DyFinder {
            cx: margin,
            cy: far,
            rings: scaled_dy_rings(&ordered[1], scale),
        },
        DyFinder {
            cx: far,
            cy: far,
            rings: scaled_dy_rings(&ordered[2], scale),
        },
    ]
}

/// Computes a projective transform mapping `src` points to `dst` points.
pub fn homography_from_4pts(src: &[(f64, f64); 4], dst: &[(f64, f64); 4]) -> Matrix3<f64> {
    let mut a = SMatrix::<f64, 8, 8>::zeros();
    let mut b = SVector::<f64, 8>::zeros();

    for i in 0..4 {
        let (x, y) = src[i];
        let (u, v) = dst[i];
        let row = i * 2;

        a[(row, 0)] = x;
        a[(row, 1)] = y;
        a[(row, 2)] = 1.0;
        a[(row, 6)] = -u * x;
        a[(row, 7)] = -u * y;
        b[row] = u;

        a[(row + 1, 3)] = x;
        a[(row + 1, 4)] = y;
        a[(row + 1, 5)] = 1.0;
        a[(row + 1, 6)] = -v * x;
        a[(row + 1, 7)] = -v * y;
        b[row + 1] = v;
    }

    let Some(h) = a.lu().solve(&b) else {
        return Matrix3::identity();
    };

    Matrix3::new(h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0)
}

fn order_qr_finders(finders: &[QrFinder; 3]) -> (QrFinder, QrFinder, QrFinder) {
    let distances = [
        (0, 1, squared_distance(finders[0], finders[1])),
        (0, 2, squared_distance(finders[0], finders[2])),
        (1, 2, squared_distance(finders[1], finders[2])),
    ];
    let &(a_idx, b_idx, _) = distances
        .iter()
        .max_by(|lhs, rhs| lhs.2.total_cmp(&rhs.2))
        .expect("three finder distances exist");

    let tl_idx = 3 - a_idx - b_idx;
    let tl = finders[tl_idx];
    let a = finders[a_idx];
    let b = finders[b_idx];

    if cross(tl, a, b) > 0.0 {
        (tl, a, b)
    } else {
        (tl, b, a)
    }
}

fn qr_outer_corners(
    bin: &BinaryImage,
    tl: QrFinder,
    tr: QrFinder,
    bl: QrFinder,
) -> [(f64, f64); 4] {
    let corners = affine_qr_outer_corners(tl, tr, bl);
    let module_counts = estimate_qr_module_counts(tl, tr, bl);

    for modules in module_counts {
        if let Some(alignment) = find_bottom_right_alignment(bin, corners, modules, (tl, tr, bl)) {
            return qr_outer_corners_from_alignment_points(modules, tl, tr, bl, alignment);
        }
    }

    corners
}

fn affine_qr_outer_corners(tl: QrFinder, tr: QrFinder, bl: QrFinder) -> [(f64, f64); 4] {
    let module = ((tl.module + tr.module + bl.module) / 3.0).max(1.0);
    let ux = unit_vector(tl, tr, module);
    let uy = unit_vector(tl, bl, module);
    let br = (tr.cx + bl.cx - tl.cx, tr.cy + bl.cy - tl.cy);

    [
        (
            tl.cx - 3.5 * ux.0 - 3.5 * uy.0,
            tl.cy - 3.5 * ux.1 - 3.5 * uy.1,
        ),
        (
            tr.cx + 3.5 * ux.0 - 3.5 * uy.0,
            tr.cy + 3.5 * ux.1 - 3.5 * uy.1,
        ),
        (
            bl.cx - 3.5 * ux.0 + 3.5 * uy.0,
            bl.cy - 3.5 * ux.1 + 3.5 * uy.1,
        ),
        (
            br.0 + 3.5 * ux.0 + 3.5 * uy.0,
            br.1 + 3.5 * ux.1 + 3.5 * uy.1,
        ),
    ]
}

fn estimate_qr_module_counts(tl: QrFinder, tr: QrFinder, bl: QrFinder) -> Vec<usize> {
    let module = ((tl.module + tr.module + bl.module) / 3.0).max(1.0);
    let center_distance = (squared_distance(tl, tr).sqrt() + squared_distance(tl, bl).sqrt()) * 0.5;
    let estimated = center_distance / module + 7.0;
    let nearest_version = ((estimated - 21.0) / 4.0).round().clamp(0.0, 39.0) as i32;

    let mut counts = Vec::new();
    for offset in [0, -1, 1, -2, 2] {
        let version = nearest_version + offset;
        if (0..=39).contains(&version) {
            let count = (21 + version * 4) as usize;
            if !counts.contains(&count) {
                counts.push(count);
            }
        }
    }
    counts
}

fn find_bottom_right_alignment(
    bin: &BinaryImage,
    affine_corners: [(f64, f64); 4],
    modules: usize,
    finders: (QrFinder, QrFinder, QrFinder),
) -> Option<(f64, f64)> {
    if modules < 25 {
        return None;
    }

    let module_to_source = homography_from_4pts(
        &[
            (0.0, 0.0),
            (modules as f64, 0.0),
            (0.0, modules as f64),
            (modules as f64, modules as f64),
        ],
        &affine_corners,
    );
    let expected = apply_homography_point(
        &module_to_source,
        (modules as f64 - 6.5, modules as f64 - 6.5),
    );
    let expected_module = ((finders.0.module + finders.1.module + finders.2.module) / 3.0).max(1.0);
    let u_axis = unit_tuple(
        sub_tuple(affine_corners[1], affine_corners[0]),
        expected_module,
    );
    let v_axis = unit_tuple(
        sub_tuple(affine_corners[2], affine_corners[0]),
        expected_module,
    );
    let radius = (expected_module * 9.0).ceil() as i32;
    let min_x = ((expected.0 - radius as f64).floor() as i32).max(0);
    let max_x = ((expected.0 + radius as f64).ceil() as i32).min(bin.w as i32 - 1);
    let min_y = ((expected.1 - radius as f64).floor() as i32).max(0);
    let max_y = ((expected.1 + radius as f64).ceil() as i32).min(bin.h as i32 - 1);
    let mut best: Option<(f64, (f64, f64))> = None;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if !bin.is_black(x, y) {
                continue;
            }

            for scale_factor in [0.7, 0.85, 1.0, 1.15, 1.3] {
                let u_step = scale_tuple(u_axis, scale_factor);
                let v_step = scale_tuple(v_axis, scale_factor);
                let matches = alignment_template_matches(bin, (x as f64, y as f64), u_step, v_step);
                if matches < 21 {
                    continue;
                }

                let distance =
                    (x as f64 - expected.0).hypot(y as f64 - expected.1) / expected_module;
                let scale_error = (scale_factor - 1.0_f64).abs();
                let score = matches as f64 - distance * 0.35 - scale_error * 2.0;

                if best.is_none_or(|(best_score, _)| score > best_score) {
                    best = Some((score, (x as f64, y as f64)));
                }
            }
        }
    }

    best.map(|(_, center)| center)
}

fn alignment_template_matches(
    bin: &BinaryImage,
    center: (f64, f64),
    u_step: (f64, f64),
    v_step: (f64, f64),
) -> u32 {
    let mut matches = 0;

    for y in 0..5 {
        for x in 0..5 {
            let dx = x as f64 - 2.0;
            let dy = y as f64 - 2.0;
            let sample = (
                center.0 + u_step.0 * dx + v_step.0 * dy,
                center.1 + u_step.1 * dx + v_step.1 * dy,
            );
            let actual = bilinear_sample(bin, sample.0, sample.1) < 128.0;
            let expected = x == 0 || x == 4 || y == 0 || y == 4 || (x == 2 && y == 2);
            if actual == expected {
                matches += 1;
            }
        }
    }

    matches
}

fn qr_outer_corners_from_alignment_points(
    modules: usize,
    tl: QrFinder,
    tr: QrFinder,
    bl: QrFinder,
    alignment: (f64, f64),
) -> [(f64, f64); 4] {
    let modules = modules as f64;
    let module_to_source = homography_from_4pts(
        &[
            (3.5, 3.5),
            (modules - 3.5, 3.5),
            (3.5, modules - 3.5),
            (modules - 6.5, modules - 6.5),
        ],
        &[(tl.cx, tl.cy), (tr.cx, tr.cy), (bl.cx, bl.cy), alignment],
    );

    [
        apply_homography_point(&module_to_source, (0.0, 0.0)),
        apply_homography_point(&module_to_source, (modules, 0.0)),
        apply_homography_point(&module_to_source, (0.0, modules)),
        apply_homography_point(&module_to_source, (modules, modules)),
    ]
}

fn apply_homography_point(h: &Matrix3<f64>, point: (f64, f64)) -> (f64, f64) {
    let p = h * Vector3::new(point.0, point.1, 1.0);
    if p.z.abs() < f64::EPSILON {
        return point;
    }
    (p.x / p.z, p.y / p.z)
}

fn sub_tuple(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
}

fn unit_tuple(vector: (f64, f64), length: f64) -> (f64, f64) {
    let norm = vector.0.hypot(vector.1);
    if norm <= f64::EPSILON {
        return (length, 0.0);
    }
    (vector.0 / norm * length, vector.1 / norm * length)
}

fn scale_tuple(vector: (f64, f64), factor: f64) -> (f64, f64) {
    (vector.0 * factor, vector.1 * factor)
}

fn unit_vector(from: QrFinder, to: QrFinder, length: f64) -> (f64, f64) {
    let dx = to.cx - from.cx;
    let dy = to.cy - from.cy;
    let distance = dx.hypot(dy);
    if distance <= f64::EPSILON {
        return (length, 0.0);
    }
    (dx / distance * length, dy / distance * length)
}

fn squared_distance(a: QrFinder, b: QrFinder) -> f64 {
    let dx = a.cx - b.cx;
    let dy = a.cy - b.cy;
    dx * dx + dy * dy
}

fn cross(origin: QrFinder, a: QrFinder, b: QrFinder) -> f64 {
    let ax = a.cx - origin.cx;
    let ay = a.cy - origin.cy;
    let bx = b.cx - origin.cx;
    let by = b.cy - origin.cy;
    ax * by - ay * bx
}

fn wx_upright_to_source_homography(
    finders: &[WxFinder; 3],
    anchor: Option<WxUprightAnchor>,
    target_size: u32,
) -> Option<Matrix3<f64>> {
    let (tl, tr, bl) = order_wx_finders(finders);
    let br = (tr.cx + bl.cx - tl.cx, tr.cy + bl.cy - tl.cy);
    let src = vec![
        (tl.cx, tl.cy),
        (tr.cx, tr.cy),
        (bl.cx, bl.cy),
        match anchor {
            Some(WxUprightAnchor::Badge(point)) => point,
            None => br,
        },
    ];
    let max = target_size.saturating_sub(1) as f64;
    let margin = max * 0.23;
    let leg = max - margin * 2.0;
    let dst = vec![
        (margin, margin),
        (max - margin, margin),
        (margin, max - margin),
        match anchor {
            Some(WxUprightAnchor::Badge(_)) => {
                let far = max - margin;
                let badge_offset = leg * 0.011;
                (far + badge_offset, far + badge_offset)
            }
            None => (max - margin, max - margin),
        },
    ];
    let weights = vec![1.0; src.len()];
    homography_from_points(&src, &dst, &weights)?.try_inverse()
}

fn dy_upright_to_source_homography(
    finders: &[DyFinder; 3],
    top_right: Option<(f64, f64)>,
    target_size: u32,
) -> Option<Matrix3<f64>> {
    dy_upright_to_source_homography_with_badge_offset(
        finders,
        top_right,
        target_size,
        DY_BADGE_CENTER_DX_PER_LOCATOR_LEG,
        DY_BADGE_CENTER_DY_PER_LOCATOR_LEG,
    )
}

fn dy_upright_to_source_homography_with_badge_offset(
    finders: &[DyFinder; 3],
    top_right: Option<(f64, f64)>,
    target_size: u32,
    dx_per_locator_leg: f64,
    dy_per_locator_leg: f64,
) -> Option<Matrix3<f64>> {
    let ordered = order_dy_finders(finders);
    let tl = &ordered[0];
    let bl = &ordered[1];
    let br = &ordered[2];
    let tr = top_right.unwrap_or((tl.cx + br.cx - bl.cx, tl.cy + br.cy - bl.cy));
    let max = target_size.saturating_sub(1) as f64;
    let margin = max * 0.23;
    let far = max - margin;
    let target_leg = far - margin;
    let tr_dst = if top_right.is_some() {
        (
            far + dx_per_locator_leg * target_leg,
            margin + dy_per_locator_leg * target_leg,
        )
    } else {
        (far, margin)
    };
    let src = [(tl.cx, tl.cy), tr, (bl.cx, bl.cy), (br.cx, br.cy)];
    let dst = [(margin, margin), tr_dst, (margin, far), (far, far)];

    homography_from_4pts(&src, &dst).try_inverse()
}

fn dy_upright_to_source_weighted_badge_homography(
    finders: &[DyFinder; 3],
    top_right: (f64, f64),
    target_size: u32,
    badge_weight: f64,
) -> Option<Matrix3<f64>> {
    let ordered = order_dy_finders(finders);
    let tl = &ordered[0];
    let bl = &ordered[1];
    let br = &ordered[2];
    let max = target_size.saturating_sub(1) as f64;
    let margin = max * 0.23;
    let far = max - margin;
    let target_leg = far - margin;
    let target_badge = (
        far + DY_BADGE_CENTER_DX_PER_LOCATOR_LEG * target_leg,
        margin + DY_BADGE_CENTER_DY_PER_LOCATOR_LEG * target_leg,
    );
    let src = vec![(tl.cx, tl.cy), top_right, (bl.cx, bl.cy), (br.cx, br.cy)];
    let dst = vec![(margin, margin), target_badge, (margin, far), (far, far)];
    let weights = vec![1.0, badge_weight.clamp(0.05, 1.0), 1.0, 1.0];

    homography_from_points(&src, &dst, &weights)?.try_inverse()
}

fn order_dy_finders(finders: &[DyFinder; 3]) -> [DyFinder; 3] {
    let distances = [
        (dy_distance2(&finders[0], &finders[1]), 0_usize, 1_usize),
        (dy_distance2(&finders[0], &finders[2]), 0, 2),
        (dy_distance2(&finders[1], &finders[2]), 1, 2),
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

fn scaled_dy_rings(finder: &DyFinder, scale: f64) -> Vec<f64> {
    let rings: Vec<f64> = finder
        .rings
        .iter()
        .map(|radius| (radius * scale).max(1.0))
        .collect();
    if rings.is_empty() { vec![1.0] } else { rings }
}

fn dy_distance(a: &DyFinder, b: &DyFinder) -> f64 {
    dy_distance2(a, b).sqrt()
}

fn dy_point_distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

fn dy_distance2(a: &DyFinder, b: &DyFinder) -> f64 {
    let dx = a.cx - b.cx;
    let dy = a.cy - b.cy;
    dx * dx + dy * dy
}

#[derive(Debug, Clone, Copy)]
struct DyBadgeComponent {
    area: u32,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

impl DyBadgeComponent {
    fn width(self) -> f64 {
        (self.max_x - self.min_x + 1) as f64
    }

    fn height(self) -> f64 {
        (self.max_y - self.min_y + 1) as f64
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

    fn to_anchor(self) -> DyBadgeAnchor {
        DyBadgeAnchor {
            cx: (self.min_x + self.max_x) as f64 * 0.5,
            cy: (self.min_y + self.max_y) as f64 * 0.5,
            radius: (self.width() + self.height()) * 0.25,
        }
    }
}

fn flood_dy_badge_component(
    image: &RgbaImage,
    visited: &mut [bool],
    start_x: i32,
    start_y: i32,
) -> Option<DyBadgeComponent> {
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
        if visited[idx] || !dy_badge_dark_pixel(image.get_pixel(x as u32, y as u32).0) {
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

    (area > 0).then_some(DyBadgeComponent {
        area,
        min_x,
        max_x,
        min_y,
        max_y,
    })
}

fn dy_badge_dark_pixel(pixel: [u8; 4]) -> bool {
    let [r, g, b, a] = pixel;
    let luma = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
    a > 128 && luma < 96.0
}

fn homography_from_points(
    src: &[(f64, f64)],
    dst: &[(f64, f64)],
    weights: &[f64],
) -> Option<Matrix3<f64>> {
    if src.len() != dst.len() || src.len() < 4 || src.len() != weights.len() {
        return None;
    }

    let mut a = DMatrix::<f64>::zeros(src.len() * 2, 8);
    let mut b = DVector::<f64>::zeros(src.len() * 2);

    for (i, ((x, y), (u, v))) in src.iter().zip(dst).enumerate() {
        let row = i * 2;
        let weight = weights[i].max(f64::EPSILON);
        a[(row, 0)] = *x * weight;
        a[(row, 1)] = *y * weight;
        a[(row, 2)] = weight;
        a[(row, 6)] = -*u * *x * weight;
        a[(row, 7)] = -*u * *y * weight;
        b[row] = *u * weight;

        a[(row + 1, 3)] = *x * weight;
        a[(row + 1, 4)] = *y * weight;
        a[(row + 1, 5)] = weight;
        a[(row + 1, 6)] = -*v * *x * weight;
        a[(row + 1, 7)] = -*v * *y * weight;
        b[row + 1] = *v * weight;
    }

    let h = a.svd(true, true).solve(&b, 1e-9).ok()?;
    Some(Matrix3::new(
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0,
    ))
}

fn order_wx_finders(finders: &[WxFinder; 3]) -> (WxFinder, WxFinder, WxFinder) {
    let distances = [
        (0, 1, squared_distance_wx(finders[0], finders[1])),
        (0, 2, squared_distance_wx(finders[0], finders[2])),
        (1, 2, squared_distance_wx(finders[1], finders[2])),
    ];
    let &(a_idx, b_idx, _) = distances
        .iter()
        .max_by(|lhs, rhs| lhs.2.total_cmp(&rhs.2))
        .expect("three finder distances exist");

    let tl_idx = 3 - a_idx - b_idx;
    let tl = finders[tl_idx];
    let a = finders[a_idx];
    let b = finders[b_idx];

    if cross_wx(tl, a, b) > 0.0 {
        (tl, a, b)
    } else {
        (tl, b, a)
    }
}

fn squared_distance_wx(a: WxFinder, b: WxFinder) -> f64 {
    let dx = a.cx - b.cx;
    let dy = a.cy - b.cy;
    dx * dx + dy * dy
}

fn cross_wx(origin: WxFinder, a: WxFinder, b: WxFinder) -> f64 {
    let ax = a.cx - origin.cx;
    let ay = a.cy - origin.cy;
    let bx = b.cx - origin.cx;
    let by = b.cy - origin.cy;
    ax * by - ay * bx
}

fn bilinear_sample(bin: &BinaryImage, x: f64, y: f64) -> f64 {
    if x < 0.0
        || y < 0.0
        || x > (bin.w.saturating_sub(1)) as f64
        || y > (bin.h.saturating_sub(1)) as f64
    {
        return 255.0;
    }

    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;

    let p00 = bin.get(x0, y0) as f64;
    let p10 = bin.get(x1, y0) as f64;
    let p01 = bin.get(x0, y1) as f64;
    let p11 = bin.get(x1, y1) as f64;
    let top = p00 * (1.0 - tx) + p10 * tx;
    let bottom = p01 * (1.0 - tx) + p11 * tx;

    top * (1.0 - ty) + bottom * ty
}

fn bilinear_sample_rgba(image: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    if x < 0.0
        || y < 0.0
        || x > (image.width().saturating_sub(1)) as f64
        || y > (image.height().saturating_sub(1)) as f64
    {
        return Rgba([255, 255, 255, 255]);
    }

    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;
    let p00 = rgba_pixel(image, x0, y0);
    let p10 = rgba_pixel(image, x1, y0);
    let p01 = rgba_pixel(image, x0, y1);
    let p11 = rgba_pixel(image, x1, y1);
    let mut out = [255_u8; 4];

    for channel in 0..4 {
        let top = p00[channel] * (1.0 - tx) + p10[channel] * tx;
        let bottom = p01[channel] * (1.0 - tx) + p11[channel] * tx;
        out[channel] = (top * (1.0 - ty) + bottom * ty).round().clamp(0.0, 255.0) as u8;
    }

    Rgba(out)
}

fn rgba_pixel(image: &RgbaImage, x: i32, y: i32) -> [f64; 4] {
    if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
        return [255.0, 255.0, 255.0, 255.0];
    }
    let pixel = image.get_pixel(x as u32, y as u32).0;
    [
        pixel[0] as f64,
        pixel[1] as f64,
        pixel[2] as f64,
        pixel[3] as f64,
    ]
}

#[derive(Debug, Clone, Copy)]
struct BadgeShapeComponent {
    area: u32,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

impl BadgeShapeComponent {
    fn center(self) -> (f64, f64) {
        (
            (self.min_x + self.max_x) as f64 * 0.5,
            (self.min_y + self.max_y) as f64 * 0.5,
        )
    }

    fn width(self) -> f64 {
        (self.max_x - self.min_x + 1) as f64
    }

    fn height(self) -> f64 {
        (self.max_y - self.min_y + 1) as f64
    }

    fn is_badge_like(self, min_dim: f64) -> bool {
        let width = self.width();
        let height = self.height();
        if width < min_dim * 0.08 || height < min_dim * 0.08 {
            return false;
        }

        let aspect = width / height.max(1.0);
        if !(0.55..=1.80).contains(&aspect) {
            return false;
        }

        let fill = self.fill();
        (0.22..=1.18).contains(&fill)
    }

    fn shape_score(self) -> f64 {
        let aspect = self.width() / self.height().max(1.0);
        let aspect_score = 1.0 - (aspect.ln().abs() / 0.8).min(0.8);
        let ellipse_area = std::f64::consts::PI * self.width() * self.height() * 0.25;
        let fill = self.area as f64 / ellipse_area.max(1.0);
        let fill_score = 1.0 - (fill - 0.72).abs().min(0.5);
        aspect_score.max(0.1) * fill_score.max(0.1)
    }

    fn fill(self) -> f64 {
        let ellipse_area = std::f64::consts::PI * self.width() * self.height() * 0.25;
        self.area as f64 / ellipse_area.max(1.0)
    }
}

fn flood_badge_shape_component(
    image: &RgbaImage,
    visited: &mut [bool],
    start_x: i32,
    start_y: i32,
) -> Option<BadgeShapeComponent> {
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
        if visited[idx] || !is_wx_badge_shape_pixel(image.get_pixel(x as u32, y as u32).0) {
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

    (area > 0).then_some(BadgeShapeComponent {
        area,
        min_x,
        max_x,
        min_y,
        max_y,
    })
}

fn is_wx_badge_shape_pixel(pixel: [u8; 4]) -> bool {
    let [r, g, b, a] = pixel;
    let luma = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
    a > 128 && (45.0..=180.0).contains(&luma)
}

fn scan_badge_shape_anchor(image: &RgbaImage) -> Option<(f64, f64)> {
    let min_dim = image.width().min(image.height()) as f64;
    let radius_min = (min_dim * 0.055).max(12.0);
    let radius_max = (min_dim * 0.145).max(radius_min + 1.0);
    let step = ((min_dim / 70.0).round() as i32).max(4);
    let radius_step = ((min_dim / 120.0).round() as i32).max(2);
    let x0 = (image.width() as f64 * 0.56) as i32;
    let x1 = (image.width() as f64 * 0.90) as i32;
    let y0 = (image.height() as f64 * 0.56) as i32;
    let y1 = (image.height() as f64 * 0.90) as i32;
    let mut best: Option<(f64, (f64, f64))> = None;

    let mut y = y0;
    while y <= y1 {
        let mut x = x0;
        while x <= x1 {
            let mut radius = radius_min;
            while radius <= radius_max {
                let score = badge_template_score(image, x as f64, y as f64, radius);
                if score > 0.34
                    && best
                        .as_ref()
                        .is_none_or(|(best_score, _)| score > *best_score)
                {
                    best = Some((score, (x as f64, y as f64)));
                }
                radius += radius_step as f64;
            }
            x += step;
        }
        y += step;
    }

    best.map(|(_, center)| center)
}

fn badge_template_score(image: &RgbaImage, cx: f64, cy: f64, radius: f64) -> f64 {
    let samples = 17;
    let mut inner_hits = 0_u32;
    let mut inner_total = 0_u32;
    let mut outer_hits = 0_u32;
    let mut outer_total = 0_u32;

    for iy in 0..samples {
        for ix in 0..samples {
            let dx = (ix as f64 / (samples - 1) as f64 - 0.5) * radius * 3.0;
            let dy = (iy as f64 / (samples - 1) as f64 - 0.5) * radius * 3.0;
            let d = dx.hypot(dy) / radius;
            let x = (cx + dx).round() as i32;
            let y = (cy + dy).round() as i32;
            let hit = x >= 0
                && y >= 0
                && x < image.width() as i32
                && y < image.height() as i32
                && is_wx_badge_shape_pixel(image.get_pixel(x as u32, y as u32).0);

            if d <= 0.88 {
                inner_total += 1;
                if hit {
                    inner_hits += 1;
                }
            } else if (1.08..=1.42).contains(&d) {
                outer_total += 1;
                if hit {
                    outer_hits += 1;
                }
            }
        }
    }

    if inner_total == 0 || outer_total == 0 {
        return 0.0;
    }

    let inner = inner_hits as f64 / inner_total as f64;
    let outer = outer_hits as f64 / outer_total as f64;
    inner - outer * 0.75
}
