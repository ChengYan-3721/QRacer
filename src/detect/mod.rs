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

    let wx_finders = finder_wx::find_wx_finders(bin);
    if finder_wx::select_wx_finders(&wx_finders).is_some() {
        return CodeKind::WxMiniprogram;
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

    #[test]
    fn detect_kind_returns_wx_for_standard_sample() {
        let path = std::path::Path::new("samples/小程序码1.jpg");
        if !path.exists() {
            return;
        }

        let img = image::open(path).unwrap();
        let bin = crate::pipeline::preprocess::preprocess(&img);

        assert_eq!(detect_kind(&bin), CodeKind::WxMiniprogram);
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
