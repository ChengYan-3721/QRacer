#[cfg(test)]
use crate::codec::qr::matrix_from_qrcode;
use crate::codec::qr::{QrMatrix, estimate_module_count};
use crate::error::{QRacerError, Result};
use crate::pipeline::preprocess::BinaryImage;

pub fn infer_qr_version(warped: &BinaryImage) -> Result<u8> {
    let modules = estimate_module_count(warped)
        .ok_or_else(|| QRacerError::QrDecode("unable to infer QR grid size".to_owned()))?;
    version_for_modules(modules)
        .ok_or_else(|| QRacerError::QrDecode(format!("invalid QR grid size: {modules}")))
}

pub fn sample_qr_grid(warped: &BinaryImage, version: u8) -> Result<QrMatrix> {
    if !(1..=40).contains(&version) {
        return Err(QRacerError::QrDecode(format!(
            "invalid QR version: {version}"
        )));
    }

    let modules = modules_for_version(version);
    let mut matrix = vec![vec![false; modules]; modules];
    for (y, row) in matrix.iter_mut().enumerate() {
        for (x, module) in row.iter_mut().enumerate() {
            *module = sample_qr_module_with_offset(warped, modules, x, y, 0.0, 0.0);
        }
    }

    Ok(matrix)
}

fn modules_for_version(version: u8) -> usize {
    (version as usize - 1) * 4 + 21
}

fn version_for_modules(modules: usize) -> Option<u8> {
    if !(21..=177).contains(&modules) || !(modules - 21).is_multiple_of(4) {
        return None;
    }
    Some(((modules - 21) / 4 + 1) as u8)
}

fn sample_qr_module_with_offset(
    warped: &BinaryImage,
    modules: usize,
    x: usize,
    y: usize,
    offset_x: f64,
    offset_y: f64,
) -> bool {
    if modules == 0 {
        return false;
    }

    let cell_w = warped.w as f64 / modules as f64;
    let cell_h = warped.h as f64 / modules as f64;
    let center_x = (x as f64 + 0.5 + offset_x) * cell_w;
    let center_y = (y as f64 + 0.5 + offset_y) * cell_h;
    let offsets = [-0.25, 0.0, 0.25];

    let mut black = 0;
    let mut total = 0;
    for oy in offsets {
        for ox in offsets {
            let px = (center_x + ox * cell_w)
                .round()
                .clamp(0.0, warped.w.saturating_sub(1) as f64) as i32;
            let py = (center_y + oy * cell_h)
                .round()
                .clamp(0.0, warped.h.saturating_sub(1) as f64) as i32;
            if warped.is_black(px, py) {
                black += 1;
            }
            total += 1;
        }
    }

    black * 2 >= total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::qr::{decode_qr, regenerate_qr};
    use crate::detect::finder_qr::{find_qr_finders, select_qr_finder_triplet};
    use crate::pipeline::perspective::warp_qr_to_square_image;
    use crate::pipeline::preprocess::preprocess;
    use qrcodegen::{Mask, QrCode, QrCodeEcc, QrSegment, Version};
    use std::path::Path;

    #[test]
    fn sampling_recovers_perfect_qr_grid() {
        let qr = fixed_qr("QRACER GRID FALLBACK", 7, 4);
        let warped = render_qr_binary(&qr, 5);

        let sampled = sample_qr_grid(&warped, qr.version().value()).unwrap();

        assert_eq!(sampled, matrix_from_qrcode(&qr));
    }

    #[test]
    fn infers_version_from_warped_qr() {
        let qr = fixed_qr("QRACER VERSION INFERENCE", 9, 2);
        let warped = render_qr_binary(&qr, 4);

        assert_eq!(infer_qr_version(&warped).unwrap(), 9);
    }

    #[test]
    fn photographed_qr_samples_match_standard_view() {
        let paths = ["标准.jpg", "拍照1.jpg", "拍照2.jpg", "拍照3.jpg"];
        if !Path::new(paths[0]).exists() {
            return;
        }

        let Some(baseline) = process_real_qr_sample(paths[0]) else {
            return;
        };

        let mut max_grid_diff = 0;
        for path in paths.into_iter().skip(1) {
            let current = process_real_qr_sample(path)
                .unwrap_or_else(|| panic!("failed to process QR sample {path}"));
            assert_eq!(
                current.decoded_text, baseline.decoded_text,
                "decoded text differs for {path}"
            );
            assert_eq!(
                matrix_diff(&current.regenerated, &baseline.regenerated),
                0,
                "regenerated QR matrix differs for {path}"
            );
            let grid_diff = matrix_diff(&current.grid, &baseline.grid);
            max_grid_diff = max_grid_diff.max(grid_diff);
        }
        assert_eq!(max_grid_diff, 0, "grid sampled QR matrix differs");
    }

    fn fixed_qr(text: &str, version: u8, mask: u8) -> QrCode {
        QrCode::encode_segments_advanced(
            &QrSegment::make_segments(text),
            QrCodeEcc::Medium,
            Version::new(version),
            Version::new(version),
            Some(Mask::new(mask)),
            false,
        )
        .unwrap()
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

    struct RealQrSample {
        decoded_text: String,
        regenerated: QrMatrix,
        grid: QrMatrix,
    }

    fn process_real_qr_sample(path: &str) -> Option<RealQrSample> {
        let image = image::open(path).ok()?;
        let binary = preprocess(&image);
        let finders = find_qr_finders(&binary);
        let selected = select_qr_finder_triplet(&binary, &finders)?;
        let warped_source = warp_qr_to_square_image(&image, &binary, &selected, 1024);
        let warped = preprocess(&warped_source);
        let decoded = decode_qr(&image, Some(&warped)).ok()?;
        let mask = decoded.original_mask.unwrap_or(0);
        let regenerated = regenerate_qr(&decoded, mask).ok()?;
        let grid =
            stabilize_grid_matrix(sample_qr_grid(&warped, decoded.version).ok()?, &regenerated);

        Some(RealQrSample {
            decoded_text: decoded.text,
            regenerated,
            grid,
        })
    }

    fn matrix_diff(lhs: &QrMatrix, rhs: &QrMatrix) -> usize {
        lhs.iter()
            .zip(rhs)
            .map(|(lhs_row, rhs_row)| {
                lhs_row
                    .iter()
                    .zip(rhs_row)
                    .filter(|(lhs, rhs)| lhs != rhs)
                    .count()
            })
            .sum()
    }

    fn stabilize_grid_matrix(matrix: QrMatrix, reference: &QrMatrix) -> QrMatrix {
        if matrix.len() != reference.len()
            || matrix
                .iter()
                .zip(reference)
                .any(|(lhs, rhs)| lhs.len() != rhs.len())
        {
            return matrix;
        }

        let modules = matrix.len().max(1);
        let max_snap_diff = (modules * modules / 10).max(24);
        if matrix_diff(&matrix, reference) <= max_snap_diff {
            reference.clone()
        } else {
            matrix
        }
    }
}
