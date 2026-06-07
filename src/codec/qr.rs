use std::collections::HashSet;

use image::{DynamicImage, GrayImage, Luma};
use qrcodegen::{Mask, QrCode, QrCodeEcc, QrSegment, Version};
use rxing::{
    BarcodeFormat, BinaryBitmap, DecodeHints, Luma8LuminanceSource, MultiFormatReader, RXingResult,
    RXingResultMetadataType, RXingResultMetadataValue, Reader, common::HybridBinarizer,
};

use crate::error::{QRacerError, Result};
use crate::pipeline::preprocess::BinaryImage;

pub type QrMatrix = Vec<Vec<bool>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QrEcc {
    L,
    M,
    Q,
    H,
}

impl QrEcc {
    pub fn label(self) -> &'static str {
        match self {
            Self::L => "L",
            Self::M => "M",
            Self::Q => "Q",
            Self::H => "H",
        }
    }

    fn from_label(value: &str) -> Option<Self> {
        match value.trim() {
            "L" => Some(Self::L),
            "M" => Some(Self::M),
            "Q" => Some(Self::Q),
            "H" => Some(Self::H),
            _ => None,
        }
    }

    fn from_format_bits(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Self::M),
            1 => Some(Self::L),
            2 => Some(Self::H),
            3 => Some(Self::Q),
            _ => None,
        }
    }

    fn to_qrcodegen(self) -> QrCodeEcc {
        match self {
            Self::L => QrCodeEcc::Low,
            Self::M => QrCodeEcc::Medium,
            Self::Q => QrCodeEcc::Quartile,
            Self::H => QrCodeEcc::High,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrDecoded {
    pub text: String,
    pub version: u8,
    pub ecc: QrEcc,
    pub original_mask: Option<u8>,
}

#[derive(Clone, Copy, Debug)]
struct QrImageMetadata {
    version: u8,
    ecc: Option<QrEcc>,
    mask: Option<u8>,
}

#[derive(Clone, Copy, Debug)]
struct FormatCandidate {
    ecc: QrEcc,
    mask: u8,
    distance: u32,
}

pub fn decode_qr(img: &DynamicImage, warped: Option<&BinaryImage>) -> Result<QrDecoded> {
    let result = decode_dynamic_image(img).or_else(|first_error| {
        let Some(warped) = warped else {
            return Err(first_error);
        };
        decode_dynamic_image(&padded_warped_image(warped)).map_err(|_| first_error)
    })?;

    if result.getBarcodeFormat() != &BarcodeFormat::QR_CODE {
        return Err(QRacerError::QrDecode("decoded format is not QR".to_owned()));
    }

    let image_metadata = warped.and_then(infer_qr_image_metadata);
    let ecc = ecc_from_result_metadata(&result)
        .or_else(|| image_metadata.and_then(|metadata| metadata.ecc))
        .ok_or_else(|| QRacerError::QrDecode("missing QR error correction metadata".to_owned()))?;
    let version = image_metadata
        .map(|metadata| metadata.version)
        .unwrap_or_else(|| minimal_version_for_text(result.getText(), ecc));

    Ok(QrDecoded {
        text: result.getText().to_owned(),
        version,
        ecc,
        original_mask: image_metadata.and_then(|metadata| metadata.mask),
    })
}

pub fn regenerate_qr(decoded: &QrDecoded, mask: u8) -> Result<QrMatrix> {
    if !(1..=40).contains(&decoded.version) {
        return Err(QRacerError::QrDecode(format!(
            "invalid QR version: {}",
            decoded.version
        )));
    }
    if mask > 7 {
        return Err(QRacerError::QrDecode(format!("invalid QR mask: {mask}")));
    }

    let segments = QrSegment::make_segments(&decoded.text);
    let qr = QrCode::encode_segments_advanced(
        &segments,
        decoded.ecc.to_qrcodegen(),
        Version::new(decoded.version),
        Version::new(decoded.version),
        Some(Mask::new(mask)),
        false,
    )
    .map_err(|error| QRacerError::QrDecode(error.to_string()))?;

    Ok(matrix_from_qrcode(&qr))
}

pub fn matrix_from_qrcode(qr: &QrCode) -> QrMatrix {
    let size = qr.size() as usize;
    let mut matrix = vec![vec![false; size]; size];
    for (y, row) in matrix.iter_mut().enumerate() {
        for (x, module) in row.iter_mut().enumerate() {
            *module = qr.get_module(x as i32, y as i32);
        }
    }
    matrix
}

pub fn estimate_module_count(warped: &BinaryImage) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;

    for version in 1..=40 {
        let modules = qr_modules_for_version(version);
        let mut score = score_module_count(warped, modules);
        if read_format_info_with_modules(warped, modules).is_some() {
            score += 0.12;
        }

        if best.is_none_or(|(best_score, _)| score > best_score) {
            best = Some((score, modules));
        }
    }

    let (score, modules) = best?;
    (score >= 0.72).then_some(modules)
}

pub fn sample_qr_module(warped: &BinaryImage, modules: usize, x: usize, y: usize) -> bool {
    if modules == 0 {
        return false;
    }

    let cell_w = warped.w as f64 / modules as f64;
    let cell_h = warped.h as f64 / modules as f64;
    let center_x = (x as f64 + 0.5) * cell_w;
    let center_y = (y as f64 + 0.5) * cell_h;
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

fn decode_dynamic_image(img: &DynamicImage) -> Result<RXingResult> {
    let luma = img.to_luma8();
    let (width, height) = luma.dimensions();
    let source = Luma8LuminanceSource::new(luma.into_raw(), width, height);
    let binarizer = HybridBinarizer::new(source);
    let mut bitmap = BinaryBitmap::new(binarizer);
    let mut reader = MultiFormatReader::default();
    let hints = DecodeHints {
        PossibleFormats: Some(HashSet::from([BarcodeFormat::QR_CODE])),
        TryHarder: Some(true),
        AlsoInverted: Some(true),
        ..Default::default()
    };

    reader
        .decode_with_hints(&mut bitmap, &hints)
        .map_err(|error| QRacerError::QrDecode(error.to_string()))
}

fn padded_warped_image(warped: &BinaryImage) -> DynamicImage {
    let border = (warped.w.min(warped.h) / 8).max(16);
    let width = warped.w + border * 2;
    let height = warped.h + border * 2;
    let mut image = GrayImage::from_pixel(width, height, Luma([255]));

    for y in 0..warped.h {
        for x in 0..warped.w {
            image.put_pixel(
                x + border,
                y + border,
                Luma([warped.get(x as i32, y as i32)]),
            );
        }
    }

    DynamicImage::ImageLuma8(image)
}

fn ecc_from_result_metadata(result: &RXingResult) -> Option<QrEcc> {
    let value = result
        .getRXingResultMetadata()
        .get(&RXingResultMetadataType::ERROR_CORRECTION_LEVEL)?;

    match value {
        RXingResultMetadataValue::ErrorCorrectionLevel(level) => QrEcc::from_label(level),
        _ => None,
    }
}

fn infer_qr_image_metadata(warped: &BinaryImage) -> Option<QrImageMetadata> {
    let modules = estimate_module_count(warped)?;
    let version = version_for_modules(modules)?;
    let format = read_format_info_with_modules(warped, modules);

    Some(QrImageMetadata {
        version,
        ecc: format.map(|candidate| candidate.ecc),
        mask: format.map(|candidate| candidate.mask),
    })
}

fn minimal_version_for_text(text: &str, ecc: QrEcc) -> u8 {
    let segments = QrSegment::make_segments(text);
    QrCode::encode_segments_advanced(
        &segments,
        ecc.to_qrcodegen(),
        Version::MIN,
        Version::MAX,
        None,
        false,
    )
    .map(|qr| qr.version().value())
    .unwrap_or(40)
}

fn qr_modules_for_version(version: u8) -> usize {
    (version as usize - 1) * 4 + 21
}

fn version_for_modules(modules: usize) -> Option<u8> {
    if !(21..=177).contains(&modules) || !(modules - 21).is_multiple_of(4) {
        return None;
    }
    Some(((modules - 21) / 4 + 1) as u8)
}

fn score_module_count(warped: &BinaryImage, modules: usize) -> f64 {
    let mut scorer = ModuleScore::new(warped, modules);

    score_finder(&mut scorer, 0, 0);
    score_finder(&mut scorer, modules - 7, 0);
    score_finder(&mut scorer, 0, modules - 7);
    score_separators(&mut scorer);
    score_timing_patterns(&mut scorer);

    if modules > 8 {
        scorer.add(8, modules - 8, true, 1.0);
    }

    scorer.finish()
}

fn score_finder(scorer: &mut ModuleScore<'_>, start_x: usize, start_y: usize) {
    for y in 0..7 {
        for x in 0..7 {
            let expected = finder_expected(x, y);
            scorer.add(start_x + x, start_y + y, expected, 4.0);
        }
    }
}

fn score_separators(scorer: &mut ModuleScore<'_>) {
    let modules = scorer.modules;
    for i in 0..8 {
        scorer.add(i, 7, false, 2.0);
        scorer.add(7, i, false, 2.0);
        scorer.add(modules - 8, i, false, 2.0);
        scorer.add(modules - 1 - i, 7, false, 2.0);
        scorer.add(i, modules - 8, false, 2.0);
        scorer.add(7, modules - 1 - i, false, 2.0);
    }
}

fn score_timing_patterns(scorer: &mut ModuleScore<'_>) {
    let modules = scorer.modules;
    if modules <= 16 {
        return;
    }

    for i in 8..modules - 8 {
        let expected = i % 2 == 0;
        scorer.add(i, 6, expected, 3.0);
        scorer.add(6, i, expected, 3.0);
    }
}

struct ModuleScore<'a> {
    warped: &'a BinaryImage,
    modules: usize,
    correct: f64,
    total: f64,
}

impl<'a> ModuleScore<'a> {
    fn new(warped: &'a BinaryImage, modules: usize) -> Self {
        Self {
            warped,
            modules,
            correct: 0.0,
            total: 0.0,
        }
    }

    fn add(&mut self, x: usize, y: usize, expected: bool, weight: f64) {
        if sample_qr_module(self.warped, self.modules, x, y) == expected {
            self.correct += weight;
        }
        self.total += weight;
    }

    fn finish(self) -> f64 {
        if self.total == 0.0 {
            0.0
        } else {
            self.correct / self.total
        }
    }
}

fn finder_expected(x: usize, y: usize) -> bool {
    let dx = (x as i32 - 3).abs();
    let dy = (y as i32 - 3).abs();
    dx.max(dy) != 2
}

fn read_format_info_with_modules(warped: &BinaryImage, modules: usize) -> Option<FormatCandidate> {
    let first = decode_format_code(read_format_copy_a(warped, modules));
    let second = decode_format_code(read_format_copy_b(warped, modules));

    match (first, second) {
        (Some(a), Some(b)) => Some(if a.distance <= b.distance { a } else { b }),
        (Some(candidate), None) | (None, Some(candidate)) => Some(candidate),
        (None, None) => None,
    }
}

fn read_format_copy_a(warped: &BinaryImage, modules: usize) -> u32 {
    let mut bits = 0_u32;

    for i in 0..6 {
        set_format_bit(&mut bits, i, sample_qr_module(warped, modules, 8, i));
    }
    set_format_bit(&mut bits, 6, sample_qr_module(warped, modules, 8, 7));
    set_format_bit(&mut bits, 7, sample_qr_module(warped, modules, 8, 8));
    set_format_bit(&mut bits, 8, sample_qr_module(warped, modules, 7, 8));
    for i in 9..15 {
        set_format_bit(&mut bits, i, sample_qr_module(warped, modules, 14 - i, 8));
    }

    bits
}

fn read_format_copy_b(warped: &BinaryImage, modules: usize) -> u32 {
    let mut bits = 0_u32;

    for i in 0..8 {
        set_format_bit(
            &mut bits,
            i,
            sample_qr_module(warped, modules, modules - 1 - i, 8),
        );
    }
    for i in 8..15 {
        set_format_bit(
            &mut bits,
            i,
            sample_qr_module(warped, modules, 8, modules - 15 + i),
        );
    }

    bits
}

fn set_format_bit(bits: &mut u32, index: usize, value: bool) {
    if value {
        *bits |= 1 << index;
    }
}

fn decode_format_code(code: u32) -> Option<FormatCandidate> {
    let mut best: Option<FormatCandidate> = None;

    for ecc_bits in 0..4 {
        for mask in 0..8 {
            let expected = format_code(ecc_bits, mask);
            let distance = (code ^ expected).count_ones();
            if best.is_none_or(|candidate| distance < candidate.distance) {
                best = Some(FormatCandidate {
                    ecc: QrEcc::from_format_bits(ecc_bits as u8)?,
                    mask: mask as u8,
                    distance,
                });
            }
        }
    }

    best.filter(|candidate| candidate.distance <= 3)
}

fn format_code(ecc_bits: u32, mask: u32) -> u32 {
    let data = (ecc_bits << 3) | mask;
    let mut rem = data;
    for _ in 0..10 {
        rem = (rem << 1) ^ ((rem >> 9) * 0x537);
    }
    ((data << 10) | rem) ^ 0x5412
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageBuffer;

    #[test]
    fn decode_synthetic_qr_extracts_version_ecc_and_mask() {
        let text = "https://example.com/qracer/phase3";
        let qr = fixed_qr(text, QrEcc::M, 5, 3);
        let decode_img = DynamicImage::ImageLuma8(render_qr_gray(&qr, 8, 4));
        let warped = render_qr_binary(&qr, 9);

        let decoded = decode_qr(&decode_img, Some(&warped)).unwrap();

        assert_eq!(decoded.text, text);
        assert_eq!(decoded.version, 5);
        assert_eq!(decoded.ecc, QrEcc::M);
        assert_eq!(decoded.original_mask, Some(3));
    }

    #[test]
    fn regenerate_with_mask_matches_canonical_matrix() {
        let text = "QRACER PHASE 3 MASK TEST";
        let qr = fixed_qr(text, QrEcc::Q, 6, 5);
        let decode_img = DynamicImage::ImageLuma8(render_qr_gray(&qr, 8, 4));
        let warped = render_qr_binary(&qr, 8);
        let decoded = decode_qr(&decode_img, Some(&warped)).unwrap();

        let matrix = regenerate_qr(&decoded, 5).unwrap();

        assert_eq!(matrix, matrix_from_qrcode(&qr));
    }

    #[test]
    fn estimate_module_count_uses_timing_and_finders() {
        let qr = fixed_qr("QRACER GRID SIZE", QrEcc::H, 8, 2);
        let warped = render_qr_binary(&qr, 5);

        assert_eq!(estimate_module_count(&warped), Some(qr.size() as usize));
    }

    fn fixed_qr(text: &str, ecc: QrEcc, version: u8, mask: u8) -> QrCode {
        QrCode::encode_segments_advanced(
            &QrSegment::make_segments(text),
            ecc.to_qrcodegen(),
            Version::new(version),
            Version::new(version),
            Some(Mask::new(mask)),
            false,
        )
        .unwrap()
    }

    fn render_qr_gray(qr: &QrCode, scale: u32, border: u32) -> GrayImage {
        let size = qr.size() as u32;
        let image_size = (size + border * 2) * scale;
        let mut image: GrayImage = ImageBuffer::from_pixel(image_size, image_size, Luma([255]));

        for y in 0..size {
            for x in 0..size {
                if qr.get_module(x as i32, y as i32) {
                    let start_x = (x + border) * scale;
                    let start_y = (y + border) * scale;
                    for yy in start_y..start_y + scale {
                        for xx in start_x..start_x + scale {
                            image.put_pixel(xx, yy, Luma([0]));
                        }
                    }
                }
            }
        }

        image
    }

    fn render_qr_binary(qr: &QrCode, scale: u32) -> BinaryImage {
        let gray = render_qr_gray(qr, scale, 0);
        BinaryImage::new(gray.width(), gray.height(), gray.into_raw())
    }
}
