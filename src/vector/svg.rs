use crate::codec::qr::QrMatrix;
use crate::codec::wx_grid::WxGrid;
use crate::pipeline::preprocess::BinaryImage;
use image::{DynamicImage, Rgba, RgbaImage};

pub fn qr_matrix_to_svg(matrix: &QrMatrix, module_mm: f64) -> String {
    let size = matrix.len();
    let module_mm = module_mm.max(0.01);
    let canvas = size as f64 * module_mm;

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{canvas:.3}mm" height="{canvas:.3}mm" viewBox="0 0 {canvas:.3} {canvas:.3}" shape-rendering="crispEdges">"#
    ));
    svg.push_str(&format!(
        r##"<rect x="0" y="0" width="{canvas:.3}" height="{canvas:.3}" fill="#fff"/>"##
    ));

    for (y, row) in matrix.iter().enumerate() {
        for (x, &is_black) in row.iter().enumerate() {
            if is_black {
                let px = x as f64 * module_mm;
                let py = y as f64 * module_mm;
                svg.push_str(&format!(
                    r##"<rect x="{px:.3}" y="{py:.3}" width="{module_mm:.3}" height="{module_mm:.3}" fill="#000"/>"##
                ));
            }
        }
    }

    svg.push_str("</svg>");
    svg
}

pub fn wx_grid_to_svg(grid: &WxGrid) -> String {
    let canvas = (grid.r_max * 2.0).max(1.0);
    let center = grid.r_max;
    let radial_step = (grid.r_max - grid.r_min) / grid.points_per_line.max(1) as f64;
    let stroke_width = radial_step;

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{canvas:.3}mm" height="{canvas:.3}mm" viewBox="0 0 {canvas:.3} {canvas:.3}" shape-rendering="geometricPrecision">"#
    ));
    svg.push_str(&format!(
        r##"<rect x="0" y="0" width="{canvas:.3}" height="{canvas:.3}" fill="#fff"/>"##
    ));
    if grid.lines == 0 || grid.points_per_line == 0 || grid.r_max <= grid.r_min {
        svg.push_str("</svg>");
        return svg;
    }

    let theta_step = std::f64::consts::TAU / grid.lines as f64;
    for line in 0..grid.lines {
        let theta = grid.theta_offset + (line as f64 + 0.5) * theta_step;
        let angle = theta.to_degrees();
        let mut point = 0;
        while point < grid.points_per_line {
            if !grid.sample(line, point) {
                point += 1;
                continue;
            }

            let start = point;
            while point + 1 < grid.points_per_line && grid.sample(line, point + 1) {
                point += 1;
            }
            let end = point;

            let r_mid = grid.r_min + ((start + end) as f64 * 0.5 + 0.5) * radial_step;
            let p_mid = polar_point(center, center, r_mid, theta);
            let length = (end - start + 1) as f64 * radial_step;
            svg.push_str(&format!(
                r##"<rect x="{:.3}" y="{:.3}" width="{length:.3}" height="{stroke_width:.3}" rx="{:.3}" fill="#000" transform="rotate({angle:.3} {:.3} {:.3})"/>"##,
                p_mid.0 - length * 0.5,
                p_mid.1 - stroke_width * 0.5,
                stroke_width * 0.5,
                p_mid.0,
                p_mid.1
            ));
            point += 1;
        }
    }

    for finder in grid.finders {
        let cx = center + finder.cx - grid.center.0;
        let cy = center + finder.cy - grid.center.1;
        svg.push_str(&format!(
            r##"<circle cx="{cx:.3}" cy="{cy:.3}" r="{:.3}" fill="#000"/>"##,
            finder.r_outer
        ));
        svg.push_str(&format!(
            r##"<circle cx="{cx:.3}" cy="{cy:.3}" r="{:.3}" fill="#fff"/>"##,
            finder.r_outer * 0.62
        ));
        svg.push_str(&format!(
            r##"<circle cx="{cx:.3}" cy="{cy:.3}" r="{:.3}" fill="#000"/>"##,
            finder.r_outer * 0.18
        ));
    }

    if let Some(badge) = grid.badge {
        let cx = center + badge.cx - grid.center.0;
        let cy = center + badge.cy - grid.center.1;
        let fill = rgb_hex(badge.color);
        svg.push_str(&format!(
            r##"<circle cx="{cx:.3}" cy="{cy:.3}" r="{:.3}" fill="{fill}"/>"##,
            badge.radius
        ));
        svg.push_str(&mini_program_logo_path(cx, cy, badge.radius));
    }

    svg.push_str("</svg>");
    svg
}

pub fn wx_grid_to_preview_image(grid: &WxGrid, size: u32) -> DynamicImage {
    let size = size.max(1);
    let mut image = RgbaImage::from_pixel(size, size, Rgba([255, 255, 255, 255]));
    if grid.lines == 0 || grid.points_per_line == 0 || grid.r_max <= grid.r_min {
        return DynamicImage::ImageRgba8(image);
    }

    let center = (size as f64 - 1.0) * 0.5;
    let scale = (size as f64 - 1.0) / (grid.r_max * 2.0).max(1.0);
    let radial_step = (grid.r_max - grid.r_min) / grid.points_per_line as f64;
    let stroke_radius = radial_step * scale * 0.5;
    let theta_step = std::f64::consts::TAU / grid.lines as f64;
    for line in 0..grid.lines {
        let theta = grid.theta_offset + (line as f64 + 0.5) * theta_step;
        let mut point = 0;
        while point < grid.points_per_line {
            if !grid.sample(line, point) {
                point += 1;
                continue;
            }

            let start = point;
            while point + 1 < grid.points_per_line && grid.sample(line, point + 1) {
                point += 1;
            }
            let end = point;

            let r_start = grid.r_min + (start as f64 + 0.5) * radial_step;
            let r_end = grid.r_min + (end as f64 + 0.5) * radial_step;
            let p_start = scaled_polar_point(center, scale, r_start, theta);
            let p_end = scaled_polar_point(center, scale, r_end, theta);
            paint_capsule(
                &mut image,
                p_start,
                p_end,
                stroke_radius,
                Rgba([0, 0, 0, 255]),
            );
            point += 1;
        }
    }

    for finder in grid.finders {
        let cx = center + (finder.cx - grid.center.0) * scale;
        let cy = center + (finder.cy - grid.center.1) * scale;
        paint_filled_circle(
            &mut image,
            (cx, cy),
            finder.r_outer * scale,
            Rgba([0, 0, 0, 255]),
        );
        paint_filled_circle(
            &mut image,
            (cx, cy),
            finder.r_outer * scale * 0.62,
            Rgba([255, 255, 255, 255]),
        );
        paint_filled_circle(
            &mut image,
            (cx, cy),
            finder.r_outer * scale * 0.18,
            Rgba([0, 0, 0, 255]),
        );
    }

    if let Some(badge) = grid.badge {
        let cx = center + (badge.cx - grid.center.0) * scale;
        let cy = center + (badge.cy - grid.center.1) * scale;
        let radius = badge.radius * scale;
        paint_filled_circle(
            &mut image,
            (cx, cy),
            radius,
            Rgba([badge.color[0], badge.color[1], badge.color[2], 255]),
        );
        paint_mini_program_logo(&mut image, (cx, cy), radius);
    }

    DynamicImage::ImageRgba8(image)
}

pub fn wx_grid_to_diff_preview_image(
    grid: &WxGrid,
    source: &BinaryImage,
    show_diff: bool,
    size: u32,
) -> (DynamicImage, u32) {
    let mut image = wx_grid_to_preview_image(grid, size).to_rgba8();
    if grid.lines == 0 || grid.points_per_line == 0 || grid.r_max <= grid.r_min {
        return (DynamicImage::ImageRgba8(image), 0);
    }

    let preview_center = (image.width() as f64 - 1.0) * 0.5;
    let scale = (image.width() as f64 - 1.0) / (grid.r_max * 2.0).max(1.0);
    let mut diff_count = 0_u32;

    for y in 0..image.height() {
        for x in 0..image.width() {
            let source_point = (
                grid.center.0 + (x as f64 - preview_center) / scale,
                grid.center.1 + (y as f64 - preview_center) / scale,
            );
            if is_wx_diff_ignored(grid, source_point) {
                continue;
            }

            let generated = image.get_pixel(x, y).0;
            let generated_black = generated[0] < 96 && generated[1] < 96 && generated[2] < 96;
            let original_black =
                source.is_black(source_point.0.round() as i32, source_point.1.round() as i32);
            if original_black == generated_black {
                continue;
            }

            diff_count += 1;
            if show_diff {
                let color = if original_black {
                    Rgba([220, 32, 32, 255])
                } else {
                    Rgba([32, 96, 220, 255])
                };
                image.put_pixel(x, y, color);
            }
        }
    }

    (DynamicImage::ImageRgba8(image), diff_count)
}

fn is_wx_diff_ignored(grid: &WxGrid, point: (f64, f64)) -> bool {
    let radius = (point.0 - grid.center.0).hypot(point.1 - grid.center.1);
    if radius < grid.r_min * 0.96 {
        return true;
    }
    if radius > grid.r_max * 1.02 {
        return true;
    }

    grid.badge
        .is_some_and(|badge| (point.0 - badge.cx).hypot(point.1 - badge.cy) <= badge.radius * 1.08)
}

fn rgb_hex(color: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
}

fn mini_program_logo_path(cx: f64, cy: f64, radius: f64) -> String {
    let scale = radius / 40.0;
    const STANDARD_S_PATH: &str = "M333.06,347.8c-.02,1.02-.22,1.9-.54,2.64-.62,1.39-1.77,2.48-3.15,3.22-1.49,.8-3.22,1.19-4.87,1.16-1.09-.02-2.09-.21-2.91-.58-1.62-.72-2.82-1.84-3.56-3.13-.61-1.07-.9-2.26-.82-3.42,.07-1.15,.5-2.28,1.29-3.25,1-1.22,2.61-2.26,4.88-2.88,2.17-.6,3.44-2.84,2.84-5.02-.6-2.17-2.85-3.44-5.02-2.84-4.02,1.12-7,3.12-9.01,5.57-1.95,2.38-2.97,5.13-3.14,7.91-.17,2.76,.49,5.56,1.91,8.03,1.55,2.69,4.02,5.01,7.32,6.47,1.86,.83,3.96,1.26,6.07,1.3,3.01,.05,6.17-.66,8.91-2.14,2.85-1.53,5.28-3.9,6.7-7.08,.77-1.74,1.23-3.67,1.26-5.8l.38-21.11c.02-1.02,.22-1.9,.54-2.64,.62-1.39,1.77-2.48,3.15-3.22,1.49-.8,3.22-1.19,4.87-1.16,1.09,.02,2.09,.21,2.91,.58,1.62,.72,2.82,1.84,3.56,3.13,.61,1.07,.9,2.26,.82,3.42-.07,1.15-.5,2.28-1.29,3.25-1,1.22-2.61,2.26-4.88,2.88-2.17,.6-3.44,2.84-2.84,5.02,.6,2.17,2.85,3.44,5.02,2.84,4.02-1.12,7-3.12,9.01-5.57,1.95-2.38,2.97-5.13,3.14-7.91,.17-2.76-.49-5.56-1.91-8.03-1.55-2.69-4.02-5.01-7.32-6.47-1.86-.83-3.96-1.26-6.07-1.3-3.01-.05-6.17,.66-8.91,2.14-2.85,1.53-5.28,3.9-6.7,7.08-.77,1.74-1.23,3.67-1.26,5.8l-.38,21.11";
    format!(
        r##"<path d="{STANDARD_S_PATH}" fill="#fff" transform="translate({cx:.3} {cy:.3}) scale({scale:.6}) translate(-337.33 -337.33)"/>"##
    )
}

fn polar_point(cx: f64, cy: f64, radius: f64, theta: f64) -> (f64, f64) {
    (cx + radius * theta.cos(), cy + radius * theta.sin())
}

fn scaled_polar_point(center: f64, scale: f64, radius: f64, theta: f64) -> (f64, f64) {
    (
        center + radius * scale * theta.cos(),
        center + radius * scale * theta.sin(),
    )
}

fn paint_capsule(
    image: &mut RgbaImage,
    start: (f64, f64),
    end: (f64, f64),
    radius: f64,
    color: Rgba<u8>,
) {
    let min_x = ((start.0.min(end.0) - radius).floor() as i32).max(0);
    let max_x = ((start.0.max(end.0) + radius).ceil() as i32).min(image.width() as i32 - 1);
    let min_y = ((start.1.min(end.1) - radius).floor() as i32).max(0);
    let max_y = ((start.1.max(end.1) + radius).ceil() as i32).min(image.height() as i32 - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            if distance_to_segment((px, py), start, end) <= radius {
                image.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn paint_mini_program_logo(image: &mut RgbaImage, center: (f64, f64), radius: f64) {
    let white = Rgba([255, 255, 255, 255]);
    let stroke = radius * 0.10;
    let mut points = Vec::new();

    push_cubic_points(
        &mut points,
        logo_point(center, radius, -0.36, 0.22),
        logo_point(center, radius, -0.52, 0.42),
        logo_point(center, radius, -0.10, 0.55),
        logo_point(center, radius, 0.01, 0.25),
        18,
    );
    push_cubic_points(
        &mut points,
        logo_point(center, radius, 0.01, 0.25),
        logo_point(center, radius, 0.05, 0.10),
        logo_point(center, radius, 0.04, -0.06),
        logo_point(center, radius, 0.04, -0.24),
        10,
    );
    push_cubic_points(
        &mut points,
        logo_point(center, radius, 0.04, -0.24),
        logo_point(center, radius, 0.08, -0.54),
        logo_point(center, radius, 0.52, -0.42),
        logo_point(center, radius, 0.38, -0.12),
        18,
    );

    for pair in points.windows(2) {
        paint_capsule(image, pair[0], pair[1], stroke, white);
    }
}

fn logo_point(center: (f64, f64), radius: f64, x: f64, y: f64) -> (f64, f64) {
    (center.0 + radius * x, center.1 + radius * y)
}

fn push_cubic_points(
    points: &mut Vec<(f64, f64)>,
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    steps: u32,
) {
    let start = if points.is_empty() { 0 } else { 1 };
    for step in start..=steps {
        let t = step as f64 / steps as f64;
        let mt = 1.0 - t;
        points.push((
            mt.powi(3) * p0.0
                + 3.0 * mt.powi(2) * t * p1.0
                + 3.0 * mt * t.powi(2) * p2.0
                + t.powi(3) * p3.0,
            mt.powi(3) * p0.1
                + 3.0 * mt.powi(2) * t * p1.1
                + 3.0 * mt * t.powi(2) * p2.1
                + t.powi(3) * p3.1,
        ));
    }
}

fn paint_filled_circle(image: &mut RgbaImage, center: (f64, f64), radius: f64, color: Rgba<u8>) {
    let min_x = ((center.0 - radius).floor() as i32).max(0);
    let max_x = ((center.0 + radius).ceil() as i32).min(image.width() as i32 - 1);
    let min_y = ((center.1 - radius).floor() as i32).max(0);
    let max_y = ((center.1 + radius).ceil() as i32).min(image.height() as i32 - 1);
    let radius2 = radius * radius;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f64 + 0.5 - center.0;
            let dy = y as f64 + 0.5 - center.1;
            if dx * dx + dy * dy <= radius2 {
                image.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn distance_to_segment(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    let vx = end.0 - start.0;
    let vy = end.1 - start.1;
    let wx = point.0 - start.0;
    let wy = point.1 - start.1;
    let len2 = vx * vx + vy * vy;
    if len2 <= f64::EPSILON {
        return wx.hypot(wy);
    }

    let t = ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0);
    let closest = (start.0 + t * vx, start.1 + t * vy);
    (point.0 - closest.0).hypot(point.1 - closest.1)
}

#[cfg(test)]
pub fn qr_matrix_to_binary(matrix: &QrMatrix, scale: u32, border: u32) -> BinaryImage {
    let modules = matrix.len() as u32;
    let scale = scale.max(1);
    let image_size = (modules + border * 2).max(1) * scale;
    let mut data = vec![255; (image_size * image_size) as usize];

    for (module_y, row) in matrix.iter().enumerate() {
        for (module_x, &is_black) in row.iter().enumerate() {
            if !is_black {
                continue;
            }

            let start_x = (module_x as u32 + border) * scale;
            let start_y = (module_y as u32 + border) * scale;
            for y in start_y..start_y + scale {
                for x in start_x..start_x + scale {
                    data[(y * image_size + x) as usize] = 0;
                }
            }
        }
    }

    BinaryImage::new(image_size, image_size, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_contains_one_rect_per_black_module_plus_background() {
        let matrix = vec![vec![true, false], vec![false, true]];
        let svg = qr_matrix_to_svg(&matrix, 1.0);

        assert_eq!(svg.matches("<rect").count(), 3);
    }

    #[test]
    fn binary_render_preserves_black_modules() {
        let matrix = vec![vec![true, false], vec![false, true]];
        let binary = qr_matrix_to_binary(&matrix, 2, 1);

        assert!(binary.is_black(2, 2));
        assert!(binary.is_black(4, 4));
        assert!(!binary.is_black(4, 2));
    }

    #[test]
    fn wx_svg_draws_black_samples_as_vector_marks() {
        let grid = WxGrid {
            center: (20.0, 20.0),
            r_min: 4.0,
            r_max: 20.0,
            theta_offset: 0.0,
            finders: test_finders(),
            badge: None,
            lines: 4,
            points_per_line: 2,
            samples: vec![true, false, false, true, false, false, false, false],
        };

        let svg = wx_grid_to_svg(&grid);

        assert_eq!(svg.matches("<rect").count(), 3);
        assert_eq!(svg.matches("<circle").count(), 9);
        assert!(svg.contains("viewBox"));
    }

    #[test]
    fn wx_preview_renders_black_sample() {
        let grid = WxGrid {
            center: (20.0, 20.0),
            r_min: 8.0,
            r_max: 20.0,
            theta_offset: 0.0,
            finders: test_finders(),
            badge: None,
            lines: 4,
            points_per_line: 2,
            samples: vec![true, false, false, false, false, false, false, false],
        };

        let image = wx_grid_to_preview_image(&grid, 64).to_rgba8();

        assert_eq!(image.get_pixel(44, 44), &Rgba([0, 0, 0, 255]));
        assert_eq!(image.get_pixel(32, 32), &Rgba([255, 255, 255, 255]));
    }

    fn test_finders() -> [crate::detect::finder_wx::WxFinder; 3] {
        [
            crate::detect::finder_wx::WxFinder {
                cx: 20.0,
                cy: 0.0,
                r_outer: 2.0,
            },
            crate::detect::finder_wx::WxFinder {
                cx: 34.0,
                cy: 34.0,
                r_outer: 2.0,
            },
            crate::detect::finder_wx::WxFinder {
                cx: 0.0,
                cy: 34.0,
                r_outer: 2.0,
            },
        ]
    }
}
