use crate::detect::finder_wx::WxFinder;
use crate::error::{QRacerError, Result};
use crate::pipeline::preprocess::BinaryImage;
use image::DynamicImage;

#[derive(Debug, Clone, PartialEq)]
pub struct WxGrid {
    pub center: (f64, f64),
    pub r_min: f64,
    pub r_max: f64,
    pub theta_offset: f64,
    pub finders: [WxFinder; 3],
    pub badge: Option<WxBadge>,
    pub lines: u32,
    pub points_per_line: u32,
    pub samples: Vec<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WxBadge {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
    pub color: [u8; 3],
}

impl WxGrid {
    pub fn sample(&self, line: u32, point: u32) -> bool {
        self.samples[(line * self.points_per_line + point) as usize]
    }
}

pub fn detect_wx_version(bin: &BinaryImage, finders: &[WxFinder; 3]) -> Result<u32> {
    let geometry = wx_geometry(finders)?;
    let points_per_line = 13;
    let mut best: Option<(u32, f64)> = None;
    for lines in [36, 54, 72] {
        let (theta_offset, black_score) = best_theta_offset(bin, &geometry, lines, points_per_line);
        let error = reconstruction_error(
            bin,
            &geometry,
            finders,
            lines,
            points_per_line,
            theta_offset,
        );
        let average = black_score / (lines * points_per_line) as f64;
        let score = error - average * 0.02;
        if best.is_none_or(|(_, best_score)| score < best_score) {
            best = Some((lines, score));
        }
    }

    let estimated = best.map(|(lines, _)| lines).unwrap_or_else(|| {
        let sample_radius = (geometry.r_min + geometry.r_max) * 0.5;
        let transitions = angular_transitions(bin, geometry.center, sample_radius);
        nearest_version(transitions).unwrap_or_else(|| fallback_version(&geometry))
    });

    Ok(estimated)
}

pub fn sample_wx(bin: &BinaryImage, finders: &[WxFinder; 3], version: u32) -> Result<WxGrid> {
    sample_wx_impl(bin, None, finders, version)
}

pub fn sample_wx_with_badge(
    bin: &BinaryImage,
    source: &DynamicImage,
    finders: &[WxFinder; 3],
    version: u32,
) -> Result<WxGrid> {
    sample_wx_impl(bin, Some(source), finders, version)
}

fn sample_wx_impl(
    bin: &BinaryImage,
    source: Option<&DynamicImage>,
    finders: &[WxFinder; 3],
    version: u32,
) -> Result<WxGrid> {
    if ![36, 54, 72].contains(&version) {
        return Err(QRacerError::QrDecode(format!(
            "invalid mini-program line count: {version}"
        )));
    }

    let geometry = wx_geometry(finders)?;
    let points_per_line = 13;
    let (theta_offset, _) = best_theta_offset(bin, &geometry, version, points_per_line);
    let badge = source.and_then(|source| detect_wx_badge(source, &geometry));
    let mut samples = Vec::with_capacity((version * points_per_line) as usize);

    for line in 0..version {
        for point in 0..points_per_line {
            let sample_point = cell_center(
                &geometry,
                version,
                points_per_line,
                theta_offset,
                line,
                point,
            );
            let reserved = is_reserved_sample(sample_point, finders, badge.as_ref());
            samples.push(
                !reserved
                    && sample_polar_cell(
                        bin,
                        &geometry,
                        version,
                        points_per_line,
                        theta_offset,
                        line,
                        point,
                    ),
            );
        }
    }

    Ok(WxGrid {
        center: geometry.center,
        r_min: geometry.r_min,
        r_max: geometry.r_max,
        theta_offset,
        finders: *finders,
        badge,
        lines: version,
        points_per_line,
        samples,
    })
}

#[derive(Debug, Clone, Copy)]
struct WxGeometry {
    center: (f64, f64),
    r_min: f64,
    r_max: f64,
}

fn wx_geometry(finders: &[WxFinder; 3]) -> Result<WxGeometry> {
    let center = circumcenter(finders).unwrap_or_else(|| {
        (
            (finders[0].cx + finders[1].cx + finders[2].cx) / 3.0,
            (finders[0].cy + finders[1].cy + finders[2].cy) / 3.0,
        )
    });
    let r_max = finders
        .iter()
        .map(|finder| distance(center, (finder.cx, finder.cy)) + finder.r_outer * 1.41)
        .fold(0.0, f64::max);
    let r_min = r_max * 0.50;

    if r_max <= r_min {
        return Err(QRacerError::QrDecode(
            "invalid mini-program radial geometry".to_owned(),
        ));
    }

    Ok(WxGeometry {
        center,
        r_min,
        r_max,
    })
}

fn circumcenter(finders: &[WxFinder; 3]) -> Option<(f64, f64)> {
    let ax = finders[0].cx;
    let ay = finders[0].cy;
    let bx = finders[1].cx;
    let by = finders[1].cy;
    let cx = finders[2].cx;
    let cy = finders[2].cy;
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-6 {
        return None;
    }

    let a2 = ax * ax + ay * ay;
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    Some((
        (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d,
        (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d,
    ))
}

fn angular_transitions(bin: &BinaryImage, center: (f64, f64), radius: f64) -> u32 {
    let samples = 720;
    let mut values = Vec::with_capacity(samples);
    for i in 0..samples {
        let theta = i as f64 * std::f64::consts::TAU / samples as f64;
        values.push(sample_polar(bin, center, radius, theta));
    }

    let mut transitions = 0;
    for i in 0..samples {
        if values[i] != values[(i + 1) % samples] {
            transitions += 1;
        }
    }

    transitions
}

fn nearest_version(transitions: u32) -> Option<u32> {
    [36, 54, 72]
        .into_iter()
        .min_by_key(|version| transitions.abs_diff(*version))
        .filter(|version| transitions.abs_diff(*version) <= 12)
}

fn best_theta_offset(
    bin: &BinaryImage,
    geometry: &WxGeometry,
    lines: u32,
    points_per_line: u32,
) -> (f64, f64) {
    let theta_step = std::f64::consts::TAU / lines as f64;
    let offset_steps = 48;
    let mut best = (0.0, f64::NEG_INFINITY);

    for idx in 0..offset_steps {
        let theta_offset = idx as f64 * theta_step / offset_steps as f64;
        let mut score = 0.0;
        for line in 0..lines {
            for point in 0..points_per_line {
                score += sample_cell_black_ratio(
                    bin,
                    geometry,
                    lines,
                    points_per_line,
                    theta_offset,
                    line,
                    point,
                );
            }
        }

        if score > best.1 {
            best = (theta_offset, score);
        }
    }

    best
}

fn sample_polar_cell(
    bin: &BinaryImage,
    geometry: &WxGeometry,
    lines: u32,
    points_per_line: u32,
    theta_offset: f64,
    line: u32,
    point: u32,
) -> bool {
    sample_cell_black_ratio(
        bin,
        geometry,
        lines,
        points_per_line,
        theta_offset,
        line,
        point,
    ) >= 0.24
}

fn reconstruction_error(
    bin: &BinaryImage,
    geometry: &WxGeometry,
    finders: &[WxFinder; 3],
    lines: u32,
    points_per_line: u32,
    theta_offset: f64,
) -> f64 {
    let mut error = 0.0;
    let mut total = 0_u32;

    for line in 0..lines {
        for point in 0..points_per_line {
            let sample_point =
                cell_center(geometry, lines, points_per_line, theta_offset, line, point);
            if is_reserved_sample(sample_point, finders, None) {
                continue;
            }

            let ratio = sample_cell_black_ratio(
                bin,
                geometry,
                lines,
                points_per_line,
                theta_offset,
                line,
                point,
            );
            error += if ratio >= 0.24 { 1.0 - ratio } else { ratio };
            total += 1;
        }
    }

    if total == 0 {
        return f64::INFINITY;
    }
    error / total as f64
}

fn sample_cell_black_ratio(
    bin: &BinaryImage,
    geometry: &WxGeometry,
    lines: u32,
    points_per_line: u32,
    theta_offset: f64,
    line: u32,
    point: u32,
) -> f64 {
    let (radius, theta) = cell_polar(geometry, lines, points_per_line, theta_offset, line, point);
    let theta_step = std::f64::consts::TAU / lines as f64;
    let radial_step = (geometry.r_max - geometry.r_min) / points_per_line as f64;
    let theta_offsets = [-0.18, 0.0, 0.18];
    let radial_offsets = [-0.28, 0.0, 0.28];
    let mut black = 0;
    let mut total = 0;

    for theta_delta in theta_offsets {
        for radial_delta in radial_offsets {
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

fn cell_center(
    geometry: &WxGeometry,
    lines: u32,
    points_per_line: u32,
    theta_offset: f64,
    line: u32,
    point: u32,
) -> (f64, f64) {
    let (radius, theta) = cell_polar(geometry, lines, points_per_line, theta_offset, line, point);
    (
        geometry.center.0 + radius * theta.cos(),
        geometry.center.1 + radius * theta.sin(),
    )
}

fn cell_polar(
    geometry: &WxGeometry,
    lines: u32,
    points_per_line: u32,
    theta_offset: f64,
    line: u32,
    point: u32,
) -> (f64, f64) {
    let theta_step = std::f64::consts::TAU / lines as f64;
    let radial_step = (geometry.r_max - geometry.r_min) / points_per_line as f64;
    let theta = theta_offset + (line as f64 + 0.5) * theta_step;
    let radius = geometry.r_min + (point as f64 + 0.5) * radial_step;
    (radius, theta)
}

fn is_reserved_sample(point: (f64, f64), finders: &[WxFinder; 3], badge: Option<&WxBadge>) -> bool {
    finders
        .iter()
        .any(|finder| distance(point, (finder.cx, finder.cy)) <= finder.r_outer * 1.45)
        || badge.is_some_and(|badge| distance(point, (badge.cx, badge.cy)) <= badge.radius * 1.04)
}

fn detect_wx_badge(source: &DynamicImage, geometry: &WxGeometry) -> Option<WxBadge> {
    let rgba = source.to_rgba8();
    let mut visited = vec![false; (rgba.width() * rgba.height()) as usize];
    let min_area = ((rgba.width().min(rgba.height()) as f64 * 0.04).powi(2)) as u32;
    let mut best: Option<(u32, WxBadge)> = None;

    for y in 0..rgba.height() as i32 {
        for x in 0..rgba.width() as i32 {
            let idx = (y as u32 * rgba.width() + x as u32) as usize;
            if visited[idx] || !is_badge_shape_pixel(rgba.get_pixel(x as u32, y as u32).0) {
                continue;
            }

            let Some(component) = flood_badge_component(&rgba, &mut visited, x, y) else {
                continue;
            };
            if component.area < min_area
                || !component.is_badge_like(rgba.width().min(rgba.height()) as f64)
            {
                continue;
            }
            let badge = component.badge;
            if badge.cx <= geometry.center.0 || badge.cy <= geometry.center.1 {
                continue;
            }
            if distance((badge.cx, badge.cy), geometry.center) < geometry.r_min * 0.75 {
                continue;
            }
            if best
                .as_ref()
                .is_none_or(|(best_area, _)| component.area > *best_area)
            {
                best = Some((component.area, badge));
            }
        }
    }

    best.map(|(_, badge)| badge)
}

#[derive(Debug, Clone, Copy)]
struct BadgeComponent {
    area: u32,
    width: f64,
    height: f64,
    badge: WxBadge,
}

impl BadgeComponent {
    fn is_badge_like(self, min_dim: f64) -> bool {
        if self.width < min_dim * 0.08 || self.height < min_dim * 0.08 {
            return false;
        }
        let aspect = self.width / self.height.max(1.0);
        if !(0.55..=1.80).contains(&aspect) {
            return false;
        }

        let ellipse_area = std::f64::consts::PI * self.width * self.height * 0.25;
        let fill = self.area as f64 / ellipse_area.max(1.0);
        (0.22..=1.18).contains(&fill)
    }
}

fn flood_badge_component(
    image: &image::RgbaImage,
    visited: &mut [bool],
    start_x: i32,
    start_y: i32,
) -> Option<BadgeComponent> {
    let mut stack = vec![(start_x, start_y)];
    let mut area = 0_u32;
    let mut sum_r = 0_u64;
    let mut sum_g = 0_u64;
    let mut sum_b = 0_u64;
    let mut min_x = start_x;
    let mut max_x = start_x;
    let mut min_y = start_y;
    let mut max_y = start_y;

    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
            continue;
        }

        let idx = (y as u32 * image.width() + x as u32) as usize;
        if visited[idx] || !is_badge_shape_pixel(image.get_pixel(x as u32, y as u32).0) {
            continue;
        }

        visited[idx] = true;
        area += 1;
        let pixel = image.get_pixel(x as u32, y as u32).0;
        sum_r += pixel[0] as u64;
        sum_g += pixel[1] as u64;
        sum_b += pixel[2] as u64;
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

    let width = (max_x - min_x + 1) as f64;
    let height = (max_y - min_y + 1) as f64;
    Some(BadgeComponent {
        area,
        width,
        height,
        badge: WxBadge {
            cx: (min_x + max_x) as f64 * 0.5,
            cy: (min_y + max_y) as f64 * 0.5,
            radius: (width + height) * 0.25,
            color: [
                (sum_r / area as u64) as u8,
                (sum_g / area as u64) as u8,
                (sum_b / area as u64) as u8,
            ],
        },
    })
}

fn is_badge_shape_pixel(pixel: [u8; 4]) -> bool {
    let [r, g, b, a] = pixel;
    let luma = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
    a > 128 && (45.0..=180.0).contains(&luma)
}

fn fallback_version(geometry: &WxGeometry) -> u32 {
    let diameter = geometry.r_max * 2.0;
    if diameter >= 420.0 {
        72
    } else if diameter >= 280.0 {
        54
    } else {
        36
    }
}

fn sample_polar(bin: &BinaryImage, center: (f64, f64), radius: f64, theta: f64) -> bool {
    let x = (center.0 + radius * theta.cos()).round() as i32;
    let y = (center.1 + radius * theta.sin()).round() as i32;
    bin.is_black(x, y)
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_kind::CodeKind;
    use crate::detect::detect_kind;
    use crate::detect::finder_wx::{
        find_wx_finders, select_wx_finders, select_wx_finders_raw, select_wx_finders_raw_with_badge,
    };
    use crate::pipeline::perspective::{
        WxUprightAnchor, detect_wx_badge_anchor, warp_wx_to_upright_image,
        wx_upright_target_finders,
    };
    use crate::pipeline::preprocess::preprocess;
    use crate::vector::svg::wx_grid_to_diff_preview_image;
    use std::path::Path;

    #[test]
    fn samples_synthetic_radial_grid() {
        let finders = synthetic_finders();
        let geometry = wx_geometry(&finders).unwrap();
        let mut bin = BinaryImage::new(220, 220, vec![255; 220 * 220]);
        draw_radial_pattern(
            &mut bin,
            geometry.center,
            geometry.r_min,
            geometry.r_max,
            36,
            13,
        );

        let grid = sample_wx(&bin, &finders, 36).unwrap();

        assert_eq!(grid.lines, 36);
        assert_eq!(grid.points_per_line, 13);
        assert_eq!(grid.samples.len(), 36 * 13);
        let black = grid.samples.iter().filter(|&&sample| sample).count();
        assert!(black > 150);
    }

    #[test]
    fn standard_wx_samples_process() {
        let samples_dir = Path::new("samples");
        if !samples_dir.exists() {
            return;
        }

        let mut processed = 0;
        for entry in std::fs::read_dir(samples_dir).unwrap() {
            let path = entry.unwrap().path();
            if !path.is_file() {
                continue;
            }
            if !is_supported_bitmap(&path) {
                continue;
            }
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            if !file_name.contains("小程序码") {
                continue;
            }

            let img = image::open(&path).unwrap();
            let bin = preprocess(&img);
            let finders = find_wx_finders(&bin);
            let selected = select_wx_finders(&finders)
                .unwrap_or_else(|| panic!("failed to select wx finders for {}", path.display()));
            let version = detect_wx_version(&bin, &selected).unwrap();
            let grid = sample_wx_with_badge(&bin, &img, &selected, version).unwrap();
            let black = grid.samples.iter().filter(|&&sample| sample).count();
            let (_, diff) = wx_grid_to_diff_preview_image(&grid, &bin, false, 512);

            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().contains("小程序码9"))
            {
                assert_eq!(grid.lines, 72);
            }
            assert!(black > 20, "too few black samples for {}", path.display());
            assert!(diff < 12_000, "too much pixel diff for {}", path.display());
            assert!(grid.badge.is_some(), "missing badge for {}", path.display());
            processed += 1;
        }

        assert!(processed > 0, "no mini-program samples found");
    }

    fn is_supported_bitmap(path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "jpg" | "jpeg" | "png" | "bmp" | "webp"
                )
            })
    }

    #[test]
    fn transformed_wx_samples_process_after_upright_warp() {
        let paths = ["标准.jpg", "拍照1.jpg", "拍照2.jpg", "拍照3.jpg"];
        if !Path::new(paths[0]).exists() {
            return;
        }
        let standard_img = image::open(paths[0]).unwrap();
        let standard_bin = preprocess(&standard_img);
        if detect_kind(&standard_bin) != CodeKind::WxMiniprogram {
            return;
        }

        let mut processed = 0;
        let mut baseline: Option<Vec<bool>> = None;
        for path in paths {
            let img = image::open(path).unwrap();
            let bin = preprocess(&img);
            let raw_finders = find_wx_finders(&bin);
            let badge_anchor = detect_wx_badge_anchor(&img);
            let raw_selected = badge_anchor
                .and_then(|badge| select_wx_finders_raw_with_badge(&raw_finders, badge))
                .or_else(|| select_wx_finders_raw(&raw_finders))
                .unwrap_or_else(|| panic!("failed to select raw wx finders for {path}"));
            let target_size = img.width().max(img.height()).clamp(1024, 1600);
            let anchor = badge_anchor.map(WxUprightAnchor::Badge);
            let upright_img = warp_wx_to_upright_image(&img, &raw_selected, anchor, target_size);
            let upright = preprocess(&upright_img);
            let selected = wx_upright_target_finders(&raw_selected, target_size);
            let version = detect_wx_version(&upright, &selected).unwrap();
            let grid = sample_wx_with_badge(&upright, &upright_img, &selected, version).unwrap();
            let black = grid.samples.iter().filter(|&&sample| sample).count();
            let (_, diff) = wx_grid_to_diff_preview_image(&grid, &upright, false, 512);
            let baseline_diff = baseline
                .as_ref()
                .map(|baseline| {
                    baseline
                        .iter()
                        .zip(&grid.samples)
                        .filter(|(lhs, rhs)| lhs != rhs)
                        .count()
                })
                .unwrap_or(0);

            assert_eq!(grid.lines, 72, "{path}");
            assert!(black > 20, "too few black samples for {path}");
            assert!(grid.badge.is_some(), "missing badge for {path}");
            assert!(diff < 120_000, "too much pixel diff for {path}");
            if path == "标准.jpg" {
                baseline = Some(grid.samples.clone());
            } else {
                assert_eq!(
                    baseline_diff, 0,
                    "sample differences from standard image for {path}: {baseline_diff}"
                );
            }
            processed += 1;
        }

        assert_eq!(processed, paths.len());
    }

    fn synthetic_finders() -> [WxFinder; 3] {
        [
            WxFinder {
                cx: 110.0,
                cy: 20.0,
                r_outer: 12.0,
            },
            WxFinder {
                cx: 187.9,
                cy: 155.0,
                r_outer: 12.0,
            },
            WxFinder {
                cx: 32.1,
                cy: 155.0,
                r_outer: 12.0,
            },
        ]
    }

    fn draw_radial_pattern(
        bin: &mut BinaryImage,
        center: (f64, f64),
        r_min: f64,
        r_max: f64,
        lines: u32,
        points: u32,
    ) {
        for y in 0..bin.h {
            for x in 0..bin.w {
                let dx = x as f64 - center.0;
                let dy = y as f64 - center.1;
                let radius = dx.hypot(dy);
                if radius < r_min || radius >= r_max {
                    continue;
                }

                let mut theta = dy.atan2(dx);
                if theta < 0.0 {
                    theta += std::f64::consts::TAU;
                }
                let line = (theta / std::f64::consts::TAU * lines as f64).floor() as u32;
                let point = ((radius - r_min) / (r_max - r_min) * points as f64).floor() as u32;
                if (line + point).is_multiple_of(2) {
                    bin.data[(y * bin.w + x) as usize] = 0;
                }
            }
        }
    }
}
