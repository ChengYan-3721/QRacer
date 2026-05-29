use nalgebra::{Matrix3, SMatrix, SVector, Vector3};

use crate::detect::finder_qr::QrFinder;
use crate::pipeline::preprocess::BinaryImage;

/// Warps a detected QR code into a square binary image of `target_size`.
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
            return qr_outer_corners_from_alignment(modules, corners, alignment);
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

fn qr_outer_corners_from_alignment(
    modules: usize,
    corners: [(f64, f64); 4],
    alignment: (f64, f64),
) -> [(f64, f64); 4] {
    let modules = modules as f64;
    let row = sub_tuple(corners[1], corners[0]);
    let col = sub_tuple(corners[2], corners[0]);
    let expected = (modules - 6.5, modules - 6.5);
    let mut best = corners[3];
    let mut best_error =
        alignment_error_for_bottom_right(corners, best, alignment, expected, modules);

    for radius in [0.35, 0.16, 0.07, 0.03] {
        let origin = best;
        for iy in -5..=5 {
            for ix in -5..=5 {
                let candidate = (
                    origin.0 + row.0 * ix as f64 / 5.0 * radius + col.0 * iy as f64 / 5.0 * radius,
                    origin.1 + row.1 * ix as f64 / 5.0 * radius + col.1 * iy as f64 / 5.0 * radius,
                );
                let error = alignment_error_for_bottom_right(
                    corners, candidate, alignment, expected, modules,
                );
                if error < best_error {
                    best_error = error;
                    best = candidate;
                }
            }
        }
    }

    [corners[0], corners[1], corners[2], best]
}

fn alignment_error_for_bottom_right(
    corners: [(f64, f64); 4],
    bottom_right: (f64, f64),
    alignment: (f64, f64),
    expected: (f64, f64),
    modules: f64,
) -> f64 {
    let source_to_modules = homography_from_4pts(
        &[corners[0], corners[1], corners[2], bottom_right],
        &[
            (0.0, 0.0),
            (modules, 0.0),
            (0.0, modules),
            (modules, modules),
        ],
    );
    let actual = apply_homography_point(&source_to_modules, alignment);
    let dx = actual.0 - expected.0;
    let dy = actual.1 - expected.1;
    dx * dx + dy * dy
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
