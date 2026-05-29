pub mod finder_dy;
pub mod finder_qr;
pub mod finder_wx;

use crate::code_kind::CodeKind;
use crate::pipeline::preprocess::BinaryImage;

/// Detects the currently supported code kind from a preprocessed binary image.
pub fn detect_kind(bin: &BinaryImage) -> CodeKind {
    let finders = finder_qr::find_qr_finders(bin);
    if finder_qr::select_qr_finder_triplet(bin, &finders).is_some() {
        return CodeKind::Qr;
    }

    CodeKind::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use qrcodegen::{QrCode, QrCodeEcc};

    #[test]
    fn detect_kind_returns_qr_for_synthetic_qr() {
        let qr = QrCode::encode_text("qracer detect kind", QrCodeEcc::Medium).unwrap();
        let bin = render_qr_binary(&qr, 5, 4);

        assert_eq!(detect_kind(&bin), CodeKind::Qr);
    }

    #[test]
    fn detect_kind_returns_unknown_for_blank_image() {
        let bin = BinaryImage::new(80, 80, vec![255; 80 * 80]);

        assert_eq!(detect_kind(&bin), CodeKind::Unknown);
    }

    fn render_qr_binary(qr: &QrCode, scale: u32, border: u32) -> BinaryImage {
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
}
