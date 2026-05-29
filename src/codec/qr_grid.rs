#[cfg(test)]
use crate::codec::qr::matrix_from_qrcode;
use crate::codec::qr::{QrMatrix, estimate_module_count, sample_qr_module};
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
            *module = sample_qr_module(warped, modules, x, y);
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

#[cfg(test)]
mod tests {
    use super::*;
    use qrcodegen::{Mask, QrCode, QrCodeEcc, QrSegment, Version};

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
}
