use nalgebra::{DMatrix, DVector, Matrix3, SMatrix, SVector, Vector3};

use crate::detect::finder_dy::DyFinder;
use crate::detect::finder_qr::QrFinder;
use crate::detect::finder_wx::WxFinder;
use crate::pipeline::preprocess::BinaryImage;
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

/// Warps a detected QR code into a square binary image of `target_size`.
#[cfg(test)]
pub fn warp_qr_to_square(
    bin: &BinaryImage,
    finders: &[QrFinder; 3],
    target_size: u32,
) -> BinaryImage {
    if target_size == 0 {
        return BinaryImage::new(0, 0, Vec::new());
    }

    let (tl, tr, bl) = order_qr_finders(finders);
    let src = qr_outer_corners(bin, tl, tr, bl);
    let max = target_size.saturating_sub(1) as f64;
    let dst = [(0.0, 0.0), (max, 0.0), (0.0, max), (max, max)];
    let h = homography_from_4pts(&src, &dst);
    let Some(inv) = h.try_inverse() else {
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

#[cfg(test)]
pub fn warp_dy_to_upright_image_with_top_right_offset(
    image: &DynamicImage,
    finders: &[DyFinder; 3],
    top_right: Option<(f64, f64)>,
    target_size: u32,
    dx_per_locator_leg: f64,
    dy_per_locator_leg: f64,
) -> DynamicImage {
    let size = target_size.max(1);
    let Some(inv) = dy_upright_to_source_homography_with_badge_offset(
        finders,
        top_right,
        size,
        dx_per_locator_leg,
        dy_per_locator_leg,
    ) else {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::finder_qr::{find_qr_finders, select_qr_finder_triplet};
    use qrcodegen::{QrCode, QrCodeEcc};

    #[test]
    fn homography_identity_on_unit_square() {
        let points = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
        let h = homography_from_4pts(&points, &points);

        let identity = Matrix3::identity();
        assert!((h - identity).iter().all(|value| value.abs() < 1e-9));
    }

    #[test]
    fn warp_axis_aligned_qr_keeps_finder_regions() {
        let qr = QrCode::encode_text("qracer warp test", QrCodeEcc::Medium).unwrap();
        let scale = 5;
        let bin = render_qr_binary(&qr, scale);
        let size = qr.size() as f64;
        let module = scale as f64;
        let finders = [
            QrFinder {
                cx: 3.5 * module,
                cy: 3.5 * module,
                module,
            },
            QrFinder {
                cx: (size - 3.5) * module,
                cy: 3.5 * module,
                module,
            },
            QrFinder {
                cx: 3.5 * module,
                cy: (size - 3.5) * module,
                module,
            },
        ];

        let warped = warp_qr_to_square(&bin, &finders, bin.w);

        assert!(warped.is_black(2, 2));
        assert!(warped.is_black(warped.w as i32 - 3, 2));
        assert!(warped.is_black(2, warped.h as i32 - 3));
    }

    #[test]
    fn warp_rotated_douyin_finders_to_upright_targets() {
        let mut bin = BinaryImage::new(420, 420, vec![255; 420 * 420]);
        let angle = 6.0_f64.to_radians();
        let center = (210.0, 210.0);
        let rotate = |x: f64, y: f64| {
            let dx = x - center.0;
            let dy = y - center.1;
            (
                center.0 + dx * angle.cos() - dy * angle.sin(),
                center.1 + dx * angle.sin() + dy * angle.cos(),
            )
        };
        let points = [
            rotate(110.0, 110.0),
            rotate(110.0, 310.0),
            rotate(310.0, 310.0),
        ];
        for &(cx, cy) in &points {
            draw_disk(&mut bin, cx, cy, 12.0);
        }
        let finders = [
            DyFinder {
                cx: points[0].0,
                cy: points[0].1,
                rings: vec![4.0, 12.0],
            },
            DyFinder {
                cx: points[1].0,
                cy: points[1].1,
                rings: vec![4.0, 12.0],
            },
            DyFinder {
                cx: points[2].0,
                cy: points[2].1,
                rings: vec![4.0, 12.0],
            },
        ];

        let warped = warp_dy_to_upright_binary(&bin, &finders, 360);
        let target = dy_upright_target_finders(&finders, 360);

        for finder in &target {
            assert!(warped.is_black(finder.cx.round() as i32, finder.cy.round() as i32));
        }
        assert!(
            (target[0].cx - target[1].cx).abs() < 1e-6
                && (target[1].cy - target[2].cy).abs() < 1e-6
        );
    }

    #[test]
    fn detected_finders_warp_synthetic_qr_to_module_grid() {
        let qr = QrCode::encode_text("https://example.com/qracer", QrCodeEcc::Medium).unwrap();
        let scale = 6;
        let bin = render_qr_binary_with_border(&qr, scale, 4);
        let finders = find_qr_finders(&bin);
        let selected = select_qr_finder_triplet(&bin, &finders).expect("QR triangle");
        let target_size = qr.size() as u32 * scale;

        let warped = warp_qr_to_square(&bin, &selected, target_size);

        let mut mismatches = 0;
        for y in 0..qr.size() {
            for x in 0..qr.size() {
                let px = (x as u32 * scale + scale / 2) as i32;
                let py = (y as u32 * scale + scale / 2) as i32;
                let actual = warped.is_black(px, py);
                let expected = qr.get_module(x, y);
                if actual != expected {
                    mismatches += 1;
                }
            }
        }

        assert!(
            mismatches <= 2,
            "mismatches={mismatches}, finders={finders:?}"
        );
    }

    #[test]
    fn detected_finders_correct_perspective_qr_to_module_grid() {
        let qr = QrCode::encode_text("https://example.com/perspective", QrCodeEcc::Medium).unwrap();
        let scale = 7;
        let src = render_qr_binary(&qr, scale);
        let dst_corners = [(58.0, 32.0), (286.0, 44.0), (40.0, 252.0), (292.0, 226.0)];
        let distorted = warp_source_square_to_canvas(&src, 340, 310, dst_corners);
        let finders = find_qr_finders(&distorted);
        let selected = select_qr_finder_triplet(&distorted, &finders).expect("QR triangle");
        let (tl, tr, bl) = order_qr_finders(&selected);
        let affine = affine_qr_outer_corners(tl, tr, bl);
        let corrected = qr_outer_corners(&distorted, tl, tr, bl);
        assert!(
            point_distance(corrected[3], dst_corners[3])
                < point_distance(affine[3], dst_corners[3]),
            "corrected={corrected:?}, affine={affine:?}"
        );
        let target_size = qr.size() as u32 * scale;

        let warped = warp_qr_to_square(&distorted, &selected, target_size);

        let mut mismatches = 0;
        for y in 0..qr.size() {
            for x in 0..qr.size() {
                let px = (x as u32 * scale + scale / 2) as i32;
                let py = (y as u32 * scale + scale / 2) as i32;
                let actual = warped.is_black(px, py);
                let expected = qr.get_module(x, y);
                if actual != expected {
                    mismatches += 1;
                }
            }
        }

        assert!(
            mismatches <= 130,
            "mismatches={mismatches}, finders={finders:?}, selected={selected:?}"
        );
    }

    #[test]
    fn detected_finders_correct_rotated_qr_to_module_grid() {
        let qr = QrCode::encode_text("https://example.com/rotated", QrCodeEcc::Medium).unwrap();
        let scale = 7;
        let src = render_qr_binary(&qr, scale);
        let rotated = warp_source_square_to_canvas(
            &src,
            430,
            360,
            [(74.0, 54.0), (335.0, 27.0), (103.0, 318.0), (364.0, 291.0)],
        );
        let finders = find_qr_finders(&rotated);
        let selected = select_qr_finder_triplet(&rotated, &finders).expect("QR triangle");
        let target_size = qr.size() as u32 * scale;

        let warped = warp_qr_to_square(&rotated, &selected, target_size);

        let mut mismatches = 0;
        for y in 0..qr.size() {
            for x in 0..qr.size() {
                let px = (x as u32 * scale + scale / 2) as i32;
                let py = (y as u32 * scale + scale / 2) as i32;
                let actual = warped.is_black(px, py);
                let expected = qr.get_module(x, y);
                if actual != expected {
                    mismatches += 1;
                }
            }
        }

        assert!(
            mismatches <= 24,
            "mismatches={mismatches}, finders={finders:?}, selected={selected:?}"
        );
    }

    fn render_qr_binary(qr: &QrCode, scale: u32) -> BinaryImage {
        let size = qr.size() as u32;
        let image_size = size * scale;
        let mut data = vec![255; (image_size * image_size) as usize];

        for y in 0..size {
            for x in 0..size {
                if qr.get_module(x as i32, y as i32) {
                    let start_x = x * scale;
                    let start_y = y * scale;
                    for yy in start_y..start_y + scale {
                        for xx in start_x..start_x + scale {
                            data[(yy * image_size + xx) as usize] = 0;
                        }
                    }
                }
            }
        }

        BinaryImage::new(image_size, image_size, data)
    }

    fn render_qr_binary_with_border(qr: &QrCode, scale: u32, border: u32) -> BinaryImage {
        let size = qr.size() as u32;
        let image_size = (size + border * 2) * scale;
        let mut data = vec![255; (image_size * image_size) as usize];

        for y in 0..size {
            for x in 0..size {
                if qr.get_module(x as i32, y as i32) {
                    let start_x = (x + border) * scale;
                    let start_y = (y + border) * scale;
                    for yy in start_y..start_y + scale {
                        for xx in start_x..start_x + scale {
                            data[(yy * image_size + xx) as usize] = 0;
                        }
                    }
                }
            }
        }

        BinaryImage::new(image_size, image_size, data)
    }

    fn draw_disk(bin: &mut BinaryImage, cx: f64, cy: f64, radius: f64) {
        let min_x = (cx - radius).floor().max(0.0) as i32;
        let max_x = (cx + radius).ceil().min(bin.w as f64 - 1.0) as i32;
        let min_y = (cy - radius).floor().max(0.0) as i32;
        let max_y = (cy + radius).ceil().min(bin.h as f64 - 1.0) as i32;
        let radius2 = radius * radius;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f64 + 0.5 - cx;
                let dy = y as f64 + 0.5 - cy;
                if dx * dx + dy * dy <= radius2 {
                    bin.data[(y as u32 * bin.w + x as u32) as usize] = 0;
                }
            }
        }
    }

    fn warp_source_square_to_canvas(
        src: &BinaryImage,
        w: u32,
        h: u32,
        dst_corners: [(f64, f64); 4],
    ) -> BinaryImage {
        let src_corners = [
            (0.0, 0.0),
            ((src.w - 1) as f64, 0.0),
            (0.0, (src.h - 1) as f64),
            ((src.w - 1) as f64, (src.h - 1) as f64),
        ];
        let dst_to_src = homography_from_4pts(&dst_corners, &src_corners);
        let mut data = vec![255; (w * h) as usize];

        for y in 0..h {
            for x in 0..w {
                let p = dst_to_src * Vector3::new(x as f64, y as f64, 1.0);
                if p.z.abs() < f64::EPSILON {
                    continue;
                }

                let sx = p.x / p.z;
                let sy = p.y / p.z;
                if sx >= 0.0 && sy >= 0.0 && sx < src.w as f64 && sy < src.h as f64 {
                    data[(y * w + x) as usize] = if bilinear_sample(src, sx, sy) < 128.0 {
                        0
                    } else {
                        255
                    };
                }
            }
        }

        BinaryImage::new(w, h, data)
    }

    fn point_distance(a: (f64, f64), b: (f64, f64)) -> f64 {
        (a.0 - b.0).hypot(a.1 - b.1)
    }
}
