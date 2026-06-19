pub mod finder_dy;
pub mod finder_qr;
pub mod finder_wx;

use crate::code_kind::CodeKind;
use crate::pipeline::preprocess::BinaryImage;
use image::{DynamicImage, RgbaImage};

/// Detects the code kind with optional color-logo hints from the source image.
pub fn detect_kind_with_image(image: &DynamicImage, bin: &BinaryImage) -> CodeKind {
    detect_kind_impl(bin, DetectionHints::from_image(image))
}

fn detect_kind_impl(bin: &BinaryImage, hints: DetectionHints) -> CodeKind {
    let signature = polar_signature(bin);
    let qr_finders = finder_qr::find_qr_finders(bin);
    let qr_triplet = finder_qr::select_qr_finder_triplet(bin, &qr_finders);
    let has_qr = qr_triplet.is_some();
    let has_qr_lattice = qr_triplet
        .as_ref()
        .and_then(|triplet| finder_qr::qr_lattice_signature(bin, triplet))
        .is_some_and(finder_qr::QrLatticeSignature::is_confident);
    let wx_finders = finder_wx::find_wx_finders(bin);
    let has_wx = finder_wx::select_wx_finders(&wx_finders).is_some();
    let dy_finders = finder_dy::find_dy_finders(bin);
    let has_dy = finder_dy::select_dy_finders(&dy_finders).is_some();

    if has_qr_lattice {
        return CodeKind::Qr;
    }

    let circular_kind = choose_circular_kind(has_wx, has_dy, signature, hints);
    if let Some(kind) = circular_kind
        && (!has_qr || circular_kind_is_confident(kind, signature, hints))
    {
        return kind;
    }

    if has_qr {
        CodeKind::Qr
    } else {
        circular_kind.unwrap_or(CodeKind::Unknown)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct DetectionHints {
    wx_badge: bool,
    douyin_logo: bool,
}

impl DetectionHints {
    fn from_image(image: &DynamicImage) -> Self {
        Self {
            wx_badge: has_wx_green_badge(image),
            douyin_logo: has_douyin_color_logo(image),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PolarSignature {
    tangential: f64,
    radial: f64,
    black_ratio: f64,
}

fn choose_circular_kind(
    has_wx: bool,
    has_dy: bool,
    signature: Option<PolarSignature>,
    hints: DetectionHints,
) -> Option<CodeKind> {
    if !has_wx && !has_dy {
        return None;
    }
    if has_wx
        && has_dy
        && signature.is_some_and(|signature| signature.tangential > signature.radial + 0.10)
    {
        return Some(CodeKind::Douyin);
    }
    if has_wx && hints.wx_badge {
        return Some(CodeKind::WxMiniprogram);
    }
    if has_dy && hints.douyin_logo {
        return Some(CodeKind::Douyin);
    }

    if has_wx && has_dy {
        if let Some(signature) = signature {
            if signature.tangential > signature.radial + 0.035 {
                return Some(CodeKind::Douyin);
            }
            if signature.radial > signature.tangential + 0.020 {
                return Some(CodeKind::WxMiniprogram);
            }
        }
        return Some(CodeKind::WxMiniprogram);
    }

    if has_wx {
        Some(CodeKind::WxMiniprogram)
    } else {
        Some(CodeKind::Douyin)
    }
}

fn circular_kind_is_confident(
    kind: CodeKind,
    signature: Option<PolarSignature>,
    hints: DetectionHints,
) -> bool {
    match kind {
        CodeKind::WxMiniprogram if hints.wx_badge => true,
        CodeKind::Douyin if hints.douyin_logo => true,
        CodeKind::WxMiniprogram => signature.is_some_and(|signature| {
            signature.black_ratio > 0.035 && signature.radial > signature.tangential + 0.020
        }),
        CodeKind::Douyin => signature.is_some_and(|signature| {
            signature.black_ratio > 0.035 && signature.tangential > signature.radial + 0.035
        }),
        _ => false,
    }
}

fn polar_signature(bin: &BinaryImage) -> Option<PolarSignature> {
    let bbox = foreground_bbox(bin)?;
    let center = (
        (bbox.min_x + bbox.max_x) as f64 * 0.5,
        (bbox.min_y + bbox.max_y) as f64 * 0.5,
    );
    let radius = ((bbox.max_x - bbox.min_x + 1).max(bbox.max_y - bbox.min_y + 1)) as f64 * 0.5;
    if radius < 8.0 {
        return None;
    }

    const RADIUS_BINS: usize = 32;
    const THETA_BINS: usize = 144;
    let mut polar = vec![false; RADIUS_BINS * THETA_BINS];
    let mut black = 0_usize;

    for r_idx in 0..RADIUS_BINS {
        let r_ratio = 0.18 + (r_idx as f64 + 0.5) / RADIUS_BINS as f64 * 0.62;
        let sample_radius = radius * r_ratio;
        for theta_idx in 0..THETA_BINS {
            let theta = theta_idx as f64 * std::f64::consts::TAU / THETA_BINS as f64;
            let x = (center.0 + sample_radius * theta.cos()).round() as i32;
            let y = (center.1 + sample_radius * theta.sin()).round() as i32;
            let is_black = bin.is_black(x, y);
            polar[r_idx * THETA_BINS + theta_idx] = is_black;
            if is_black {
                black += 1;
            }
        }
    }

    if black == 0 {
        return None;
    }

    let mut tangential = 0_usize;
    let mut radial = 0_usize;
    for r_idx in 0..RADIUS_BINS {
        for theta_idx in 0..THETA_BINS {
            if !polar[r_idx * THETA_BINS + theta_idx] {
                continue;
            }
            let next_theta = (theta_idx + 1) % THETA_BINS;
            if polar[r_idx * THETA_BINS + next_theta] {
                tangential += 1;
            }
            if r_idx + 1 < RADIUS_BINS && polar[(r_idx + 1) * THETA_BINS + theta_idx] {
                radial += 1;
            }
        }
    }

    Some(PolarSignature {
        tangential: tangential as f64 / black as f64,
        radial: radial as f64 / black as f64,
        black_ratio: black as f64 / (RADIUS_BINS * THETA_BINS) as f64,
    })
}

#[derive(Debug, Clone, Copy)]
struct ForegroundBbox {
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

fn foreground_bbox(bin: &BinaryImage) -> Option<ForegroundBbox> {
    let mut bbox = ForegroundBbox {
        min_x: bin.w as i32,
        max_x: -1,
        min_y: bin.h as i32,
        max_y: -1,
    };

    for y in 0..bin.h as i32 {
        for x in 0..bin.w as i32 {
            if !bin.is_black(x, y) {
                continue;
            }
            bbox.min_x = bbox.min_x.min(x);
            bbox.max_x = bbox.max_x.max(x);
            bbox.min_y = bbox.min_y.min(y);
            bbox.max_y = bbox.max_y.max(y);
        }
    }

    (bbox.max_x >= bbox.min_x && bbox.max_y >= bbox.min_y).then_some(bbox)
}

fn has_douyin_color_logo(image: &DynamicImage) -> bool {
    let rgba = image.to_rgba8();
    let min_dim = rgba.width().min(rgba.height()) as f64;
    let min_pixels = ((min_dim * 0.010).powi(2)).max(12.0) as usize;
    let mut red = 0_usize;
    let mut cyan = 0_usize;

    for pixel in rgba.pixels() {
        let [r, g, b, a] = pixel.0;
        if a < 128 {
            continue;
        }
        let red_logo = r > 150 && g < 120 && r as i16 - g as i16 > 45 && r as i16 - b as i16 > 15;
        let cyan_logo = b > 130
            && g > 115
            && r < 105
            && b as i16 - r as i16 > 45
            && g as i16 - r as i16 > 30
            && (b as i16 - g as i16).abs() < 85;
        if red_logo {
            red += 1;
        }
        if cyan_logo {
            cyan += 1;
        }
    }

    red >= min_pixels && cyan >= min_pixels
}

fn has_wx_green_badge(image: &DynamicImage) -> bool {
    let rgba = image.to_rgba8();
    let min_dim = rgba.width().min(rgba.height()) as f64;
    let mut visited = vec![false; (rgba.width() * rgba.height()) as usize];
    let min_area = ((min_dim * 0.035).powi(2)).max(48.0) as u32;
    let max_area = (rgba.width() * rgba.height()) / 10;
    let image_center = (rgba.width() as f64 * 0.5, rgba.height() as f64 * 0.5);

    for y in 0..rgba.height() as i32 {
        for x in 0..rgba.width() as i32 {
            let idx = (y as u32 * rgba.width() + x as u32) as usize;
            if visited[idx] || !is_wx_green_pixel(rgba.get_pixel(x as u32, y as u32).0) {
                continue;
            }

            let component = flood_color_component(&rgba, &mut visited, x, y, is_wx_green_pixel);
            if component.area < min_area || component.area > max_area {
                continue;
            }
            if !component.is_roundish() {
                continue;
            }

            let center = component.center();
            if distance(center, image_center) < min_dim * 0.18 {
                continue;
            }
            return true;
        }
    }

    false
}

#[derive(Debug, Clone, Copy)]
struct ColorComponent {
    area: u32,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

impl ColorComponent {
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

    fn is_roundish(self) -> bool {
        let aspect = self.width() / self.height().max(1.0);
        if !(0.72..=1.32).contains(&aspect) {
            return false;
        }
        let ellipse_area = std::f64::consts::PI * self.width() * self.height() * 0.25;
        let fill = self.area as f64 / ellipse_area.max(1.0);
        (0.42..=1.20).contains(&fill)
    }
}

fn flood_color_component(
    image: &RgbaImage,
    visited: &mut [bool],
    start_x: i32,
    start_y: i32,
    accepts: fn([u8; 4]) -> bool,
) -> ColorComponent {
    let mut stack = vec![(start_x, start_y)];
    let mut component = ColorComponent {
        area: 0,
        min_x: start_x,
        max_x: start_x,
        min_y: start_y,
        max_y: start_y,
    };

    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
            continue;
        }
        let idx = (y as u32 * image.width() + x as u32) as usize;
        if visited[idx] || !accepts(image.get_pixel(x as u32, y as u32).0) {
            continue;
        }

        visited[idx] = true;
        component.area += 1;
        component.min_x = component.min_x.min(x);
        component.max_x = component.max_x.max(x);
        component.min_y = component.min_y.min(y);
        component.max_y = component.max_y.max(y);

        stack.push((x - 1, y));
        stack.push((x + 1, y));
        stack.push((x, y - 1));
        stack.push((x, y + 1));
    }

    component
}

fn is_wx_green_pixel(pixel: [u8; 4]) -> bool {
    let [r, g, b, a] = pixel;
    a > 128 && g > 105 && g as i16 - r as i16 > 35 && g as i16 - b as i16 > 20 && r < 150 && b < 170
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}
