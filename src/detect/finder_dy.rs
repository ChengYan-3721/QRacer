use crate::pipeline::preprocess::BinaryImage;

#[derive(Debug, Clone, PartialEq)]
pub struct DyFinder {
    pub cx: f64,
    pub cy: f64,
    pub rings: Vec<f64>,
}

#[derive(Debug, Clone)]
struct Component {
    cx: f64,
    cy: f64,
    radius: f64,
    area: u32,
}

/// Detects Douyin code locator bullseyes.
///
/// Douyin codes use three small concentric locators at top-left, bottom-left,
/// and bottom-right. The top-right large logo circle is intentionally ignored.
pub fn find_dy_finders(bin: &BinaryImage) -> Vec<DyFinder> {
    let components = circular_components(bin);
    let mut candidates = Vec::new();

    for outer in &components {
        if outer.radius > bin.w.min(bin.h) as f64 * 0.10 {
            continue;
        }
        let mut rings = vec![outer.radius];
        for inner in &components {
            if inner.radius >= outer.radius * 0.86 || inner.radius <= outer.radius * 0.10 {
                continue;
            }
            if distance((outer.cx, outer.cy), (inner.cx, inner.cy)) < outer.radius * 0.28 {
                rings.push(inner.radius);
            }
        }
        rings.sort_by(f64::total_cmp);

        let has_gap = rings
            .windows(2)
            .any(|pair| pair[1] / pair[0].max(1.0) > 1.65);
        if rings.len() < 2 || !has_gap {
            continue;
        }

        add_unique_finder(
            &mut candidates,
            DyFinder {
                cx: outer.cx,
                cy: outer.cy,
                rings,
            },
        );
    }

    for dot in &components {
        let Some(outer_radius) = estimate_bullseye_from_center_dot(bin, dot) else {
            continue;
        };
        add_unique_finder(
            &mut candidates,
            DyFinder {
                cx: dot.cx,
                cy: dot.cy,
                rings: vec![dot.radius, outer_radius],
            },
        );
    }

    scan_douyin_corner_templates(bin, &mut candidates);
    candidates.sort_by(|a, b| b.outer_radius().total_cmp(&a.outer_radius()));
    candidates
}

/// 把牛眼中心精修到亚像素：对外环带（[0.55, 1.05] × 外环半径）内的黑像素
/// 质心做 mean-shift 迭代。连通域质心会被相邻码点或形态学桥接像素带偏
/// 2~6px，而完整圆环的环带质心收敛到真实圆心；限幅 4px 防止异常发散。
pub fn refine_dy_finder_center(bin: &BinaryImage, finder: &DyFinder) -> DyFinder {
    let r_outer = finder.outer_radius();
    let band_min = r_outer * 0.55;
    let band_max = r_outer * 1.05;
    let mut cx = finder.cx;
    let mut cy = finder.cy;

    for _ in 0..6 {
        let min_x = (cx - band_max).floor() as i32;
        let max_x = (cx + band_max).ceil() as i32;
        let min_y = (cy - band_max).floor() as i32;
        let max_y = (cy + band_max).ceil() as i32;
        let mut sum_x = 0.0_f64;
        let mut sum_y = 0.0_f64;
        let mut count = 0_u32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if !bin.is_black(x, y) {
                    continue;
                }
                let px = x as f64 + 0.5;
                let py = y as f64 + 0.5;
                let dist = (px - cx).hypot(py - cy);
                if dist < band_min || dist > band_max {
                    continue;
                }
                sum_x += px;
                sum_y += py;
                count += 1;
            }
        }

        if count == 0 {
            break;
        }
        let next_x = sum_x / f64::from(count);
        let next_y = sum_y / f64::from(count);
        if (next_x - finder.cx).abs() > 4.0 || (next_y - finder.cy).abs() > 4.0 {
            break;
        }
        let moved = (next_x - cx).hypot(next_y - cy);
        cx = next_x;
        cy = next_y;
        if moved < 0.05 {
            break;
        }
    }

    DyFinder {
        cx,
        cy,
        rings: finder.rings.clone(),
    }
}

pub fn select_dy_finders(finders: &[DyFinder]) -> Option<[DyFinder; 3]> {
    select_dy_finders_raw(finders).map(normalize_dy_finder_triplet)
}

pub fn select_dy_finders_raw(finders: &[DyFinder]) -> Option<[DyFinder; 3]> {
    if finders.len() < 3 {
        return None;
    }

    let mut best: Option<(f64, [DyFinder; 3])> = None;
    for a in 0..finders.len() - 2 {
        for b in a + 1..finders.len() - 1 {
            for c in b + 1..finders.len() {
                let triplet = [finders[a].clone(), finders[b].clone(), finders[c].clone()];
                let area = triangle_area(&triplet[0], &triplet[1], &triplet[2]);
                let radius = triplet.iter().map(DyFinder::outer_radius).sum::<f64>() / 3.0;
                if area < radius * radius * 8.0 {
                    continue;
                }
                let Some(shape_penalty) = right_isosceles_penalty(&triplet) else {
                    continue;
                };
                let Some(size_penalty) = finder_size_penalty(&triplet, radius) else {
                    continue;
                };
                let Some(orientation_penalty) = douyin_orientation_penalty(&triplet) else {
                    continue;
                };
                let spread = radius_spread(&triplet);
                let score = area
                    - shape_penalty * area
                    - size_penalty * area
                    - orientation_penalty * area
                    - spread * radius * 18.0;
                if best
                    .as_ref()
                    .is_none_or(|(best_score, _)| score > *best_score)
                {
                    best = Some((score, triplet));
                }
            }
        }
    }

    best.map(|(_, triplet)| triplet)
}

impl DyFinder {
    pub fn outer_radius(&self) -> f64 {
        self.rings.iter().copied().fold(0.0, f64::max)
    }
}

fn circular_components(bin: &BinaryImage) -> Vec<Component> {
    let mut visited = vec![false; (bin.w * bin.h) as usize];
    let mut components = Vec::new();
    let min_area = ((bin.w.min(bin.h) as f64 * 0.0025).powi(2)).max(4.0) as u32;

    for y in 0..bin.h as i32 {
        for x in 0..bin.w as i32 {
            let idx = (y as u32 * bin.w + x as u32) as usize;
            if visited[idx] || !bin.is_black(x, y) {
                continue;
            }

            let Some(component) = flood_component(bin, &mut visited, x, y) else {
                continue;
            };
            if component.area >= min_area && is_circular_component(&component) {
                components.push(component);
            }
        }
    }

    components.sort_by(|a, b| b.radius.total_cmp(&a.radius));
    components
}

fn flood_component(
    bin: &BinaryImage,
    visited: &mut [bool],
    start_x: i32,
    start_y: i32,
) -> Option<Component> {
    let mut stack = vec![(start_x, start_y)];
    let mut area = 0_u32;
    let mut sum_x = 0_f64;
    let mut sum_y = 0_f64;
    let mut min_x = start_x;
    let mut max_x = start_x;
    let mut min_y = start_y;
    let mut max_y = start_y;

    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= bin.w as i32 || y >= bin.h as i32 {
            continue;
        }
        let idx = (y as u32 * bin.w + x as u32) as usize;
        if visited[idx] || !bin.is_black(x, y) {
            continue;
        }

        visited[idx] = true;
        area += 1;
        sum_x += x as f64;
        sum_y += y as f64;
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
    Some(Component {
        cx: sum_x / area as f64,
        cy: sum_y / area as f64,
        radius: (width + height) * 0.25,
        area,
    })
}

fn is_circular_component(component: &Component) -> bool {
    if component.radius < 1.2 {
        return false;
    }
    let disk_area = std::f64::consts::PI * component.radius * component.radius;
    let fill = component.area as f64 / disk_area.max(1.0);
    (0.08..=1.18).contains(&fill)
}

fn estimate_bullseye_from_center_dot(bin: &BinaryImage, dot: &Component) -> Option<f64> {
    let min_dim = bin.w.min(bin.h) as f64;
    if dot.radius < min_dim * 0.005 || dot.radius > min_dim * 0.055 {
        return None;
    }

    let gap = circle_black_ratio(bin, dot.cx, dot.cy, dot.radius * 2.0);
    if gap > 0.50 {
        return None;
    }

    let mut best_ring: Option<(f64, f64)> = None;
    let start = (dot.radius * 2.7).ceil() as u32;
    let end = (dot.radius * 7.5).ceil() as u32;
    for radius in start..=end {
        let radius = radius as f64;
        let ratio = circle_black_ratio(bin, dot.cx, dot.cy, radius);
        if best_ring.is_none_or(|(_, best_ratio)| ratio > best_ratio) {
            best_ring = Some((radius, ratio));
        }
    }

    let (ring_radius, ring_ratio) = best_ring?;
    if ring_ratio < 0.30 {
        return None;
    }

    let mut outer = ring_radius + dot.radius;
    for radius in ring_radius.ceil() as u32..=(dot.radius * 8.5).ceil() as u32 {
        let radius = radius as f64;
        if circle_black_ratio(bin, dot.cx, dot.cy, radius) < 0.30 {
            outer = radius;
            break;
        }
    }

    if outer > min_dim * 0.10 {
        return None;
    }

    Some(outer)
}

fn scan_douyin_corner_templates(bin: &BinaryImage, candidates: &mut Vec<DyFinder>) {
    let min_dim = bin.w.min(bin.h) as f64;
    let radius_min = (min_dim * 0.024).max(5.0);
    let radius_max = (min_dim * 0.065).max(radius_min + 1.0);
    let step = ((min_dim / 150.0).round() as i32).max(2);
    let radius_step = ((min_dim / 190.0).round() as i32).max(1);
    let regions = [
        (
            (bin.w as f64 * 0.04) as i32,
            (bin.w as f64 * 0.28) as i32,
            (bin.h as f64 * 0.04) as i32,
            (bin.h as f64 * 0.28) as i32,
        ),
        (
            (bin.w as f64 * 0.04) as i32,
            (bin.w as f64 * 0.28) as i32,
            (bin.h as f64 * 0.70) as i32,
            (bin.h as f64 * 0.96) as i32,
        ),
        (
            (bin.w as f64 * 0.70) as i32,
            (bin.w as f64 * 0.96) as i32,
            (bin.h as f64 * 0.70) as i32,
            (bin.h as f64 * 0.96) as i32,
        ),
    ];

    for (x0, x1, y0, y1) in regions {
        let mut matches: Vec<(f64, DyFinder)> = Vec::new();
        let mut y = y0;
        while y <= y1 {
            let mut x = x0;
            while x <= x1 {
                let mut radius = radius_min;
                while radius <= radius_max {
                    let score = bullseye_template_score(bin, x as f64, y as f64, radius);
                    if score >= 2.65 {
                        matches.push((
                            score,
                            DyFinder {
                                cx: x as f64,
                                cy: y as f64,
                                rings: vec![radius * 0.18, radius],
                            },
                        ));
                    }
                    radius += radius_step as f64;
                }
                x += step;
            }
            y += step;
        }

        matches.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, finder) in matches.into_iter().take(5) {
            add_unique_finder(candidates, finder);
        }
    }
}

fn bullseye_template_score(bin: &BinaryImage, cx: f64, cy: f64, r_outer: f64) -> f64 {
    let center = disk_black_ratio(bin, cx, cy, r_outer * 0.22);
    let gap = circle_black_ratio(bin, cx, cy, r_outer * 0.48);
    let ring = (circle_black_ratio(bin, cx, cy, r_outer * 0.76)
        + circle_black_ratio(bin, cx, cy, r_outer * 0.92))
        * 0.5;
    let outside = circle_black_ratio(bin, cx, cy, r_outer * 1.18);

    if center < 0.40 || gap > 0.58 || ring < 0.42 {
        return 0.0;
    }

    center * 1.25 + (1.0 - gap) * 1.0 + ring * 1.7 + (1.0 - outside) * 0.25
}

fn disk_black_ratio(bin: &BinaryImage, cx: f64, cy: f64, radius: f64) -> f64 {
    let min_x = (cx - radius).floor() as i32;
    let max_x = (cx + radius).ceil() as i32;
    let min_y = (cy - radius).floor() as i32;
    let max_y = (cy + radius).ceil() as i32;
    let radius2 = radius * radius;
    let mut black = 0;
    let mut total = 0;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f64 + 0.5 - cx;
            let dy = y as f64 + 0.5 - cy;
            if dx * dx + dy * dy > radius2 {
                continue;
            }
            total += 1;
            if bin.is_black(x, y) {
                black += 1;
            }
        }
    }

    black as f64 / total.max(1) as f64
}

fn circle_black_ratio(bin: &BinaryImage, cx: f64, cy: f64, radius: f64) -> f64 {
    let samples = 64;
    let black = (0..samples)
        .filter(|&idx| {
            let theta = idx as f64 * std::f64::consts::TAU / samples as f64;
            let x = (cx + radius * theta.cos()).round() as i32;
            let y = (cy + radius * theta.sin()).round() as i32;
            bin.is_black(x, y)
        })
        .count();

    black as f64 / samples as f64
}

fn add_unique_finder(finders: &mut Vec<DyFinder>, candidate: DyFinder) {
    let merge_radius = candidate.outer_radius().max(4.0);
    if let Some(existing) = finders.iter_mut().find(|finder| {
        distance((finder.cx, finder.cy), (candidate.cx, candidate.cy)) < merge_radius
    }) {
        if candidate.outer_radius() > existing.outer_radius() {
            *existing = candidate;
        }
        return;
    }
    finders.push(candidate);
}

fn normalize_dy_finder_triplet(finders: [DyFinder; 3]) -> [DyFinder; 3] {
    let Some((corner_idx, a_idx, b_idx)) = right_angle_indices(&finders) else {
        return finders;
    };

    let corner = finders[corner_idx].clone();
    let a = finders[a_idx].clone();
    let b = finders[b_idx].clone();
    let va = (a.cx - corner.cx, a.cy - corner.cy);
    let vb = (b.cx - corner.cx, b.cy - corner.cy);
    let cross = va.0 * vb.1 - va.1 * vb.0;
    if cross.abs() < f64::EPSILON {
        return finders;
    }

    let sign = cross.signum();
    let angle_a = va.1.atan2(va.0);
    let angle_b = vb.1.atan2(vb.0) - sign * std::f64::consts::FRAC_PI_2;
    let angle = snap_near_cardinal(average_angle(angle_a, angle_b), 1.2_f64.to_radians());
    let axis_a = (angle.cos(), angle.sin());
    let axis_b = (-sign * axis_a.1, sign * axis_a.0);
    let len_a = va.0 * axis_a.0 + va.1 * axis_a.1;
    let len_b = vb.0 * axis_b.0 + vb.1 * axis_b.1;
    if len_a <= 0.0 || len_b <= 0.0 {
        return finders;
    }

    let mut normalized = finders;
    normalized[a_idx].cx = corner.cx + axis_a.0 * len_a;
    normalized[a_idx].cy = corner.cy + axis_a.1 * len_a;
    normalized[b_idx].cx = corner.cx + axis_b.0 * len_b;
    normalized[b_idx].cy = corner.cy + axis_b.1 * len_b;
    normalized
}

fn right_isosceles_penalty(finders: &[DyFinder; 3]) -> Option<f64> {
    let mut distances = [
        distance(
            (finders[0].cx, finders[0].cy),
            (finders[1].cx, finders[1].cy),
        ),
        distance(
            (finders[0].cx, finders[0].cy),
            (finders[2].cx, finders[2].cy),
        ),
        distance(
            (finders[1].cx, finders[1].cy),
            (finders[2].cx, finders[2].cy),
        ),
    ];
    distances.sort_by(f64::total_cmp);

    let leg_a = distances[0];
    let leg_b = distances[1];
    let diagonal = distances[2];
    if leg_a <= 0.0 {
        return None;
    }

    let leg_error = (leg_b / leg_a - 1.0).abs();
    let diagonal_error = (diagonal / ((leg_a + leg_b) * 0.5) - std::f64::consts::SQRT_2).abs();
    if leg_error > 0.36 || diagonal_error > 0.36 {
        return None;
    }

    Some(leg_error * 1.7 + diagonal_error * 2.2)
}

fn finder_size_penalty(finders: &[DyFinder; 3], radius: f64) -> Option<f64> {
    let mut distances = [
        distance(
            (finders[0].cx, finders[0].cy),
            (finders[1].cx, finders[1].cy),
        ),
        distance(
            (finders[0].cx, finders[0].cy),
            (finders[2].cx, finders[2].cy),
        ),
        distance(
            (finders[1].cx, finders[1].cy),
            (finders[2].cx, finders[2].cy),
        ),
    ];
    distances.sort_by(f64::total_cmp);
    let leg = (distances[0] + distances[1]) * 0.5;
    if leg <= f64::EPSILON {
        return None;
    }

    let ratio = radius / leg;
    if !(0.040..=0.125).contains(&ratio) {
        return None;
    }

    Some(((ratio - 0.070) / 0.070).abs() * 0.10)
}

fn douyin_orientation_penalty(finders: &[DyFinder; 3]) -> Option<f64> {
    let (corner_idx, a_idx, b_idx) = right_angle_indices(finders)?;
    let corner = &finders[corner_idx];
    let a = &finders[a_idx];
    let b = &finders[b_idx];
    let center_x = (corner.cx + a.cx + b.cx) / 3.0;
    let center_y = (corner.cy + a.cy + b.cy) / 3.0;
    let missing = (a.cx + b.cx - corner.cx, a.cy + b.cy - corner.cy);

    // Upright Douyin codes place the three small finders at TL/BL/BR and the
    // large badge at TR. This rejects mini-program TL/TR/BL triplets.
    if corner.cx > center_x || corner.cy < center_y {
        return None;
    }
    if missing.0 < center_x || missing.1 > center_y {
        return None;
    }

    let dx = ((corner.cx - center_x).abs() - (missing.0 - center_x).abs()).abs();
    let dy = ((corner.cy - center_y).abs() - (center_y - missing.1).abs()).abs();
    let leg = (distance((corner.cx, corner.cy), (a.cx, a.cy))
        + distance((corner.cx, corner.cy), (b.cx, b.cy)))
        * 0.5;
    Some((dx + dy) / leg.max(1.0) * 0.08)
}

fn radius_spread(finders: &[DyFinder; 3]) -> f64 {
    let min = finders
        .iter()
        .map(DyFinder::outer_radius)
        .fold(f64::INFINITY, f64::min);
    let max = finders
        .iter()
        .map(DyFinder::outer_radius)
        .fold(0.0, f64::max);
    max - min
}

fn right_angle_indices(finders: &[DyFinder; 3]) -> Option<(usize, usize, usize)> {
    let mut pairs = [
        (finder_distance2(&finders[0], &finders[1]), 0_usize, 1_usize),
        (finder_distance2(&finders[0], &finders[2]), 0, 2),
        (finder_distance2(&finders[1], &finders[2]), 1, 2),
    ];
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (_, a_idx, b_idx) = pairs[2];
    let corner_idx = 3 - a_idx - b_idx;
    Some((corner_idx, a_idx, b_idx))
}

fn finder_distance2(a: &DyFinder, b: &DyFinder) -> f64 {
    let dx = a.cx - b.cx;
    let dy = a.cy - b.cy;
    dx * dx + dy * dy
}

fn triangle_area(a: &DyFinder, b: &DyFinder, c: &DyFinder) -> f64 {
    ((b.cx - a.cx) * (c.cy - a.cy) - (b.cy - a.cy) * (c.cx - a.cx)).abs() * 0.5
}

fn average_angle(a: f64, b: f64) -> f64 {
    let x = a.cos() + b.cos();
    let y = a.sin() + b.sin();
    if x.hypot(y) < 1e-9 { a } else { y.atan2(x) }
}

fn snap_near_cardinal(angle: f64, threshold: f64) -> f64 {
    let step = std::f64::consts::FRAC_PI_2;
    let snapped = (angle / step).round() * step;
    let delta = (angle - snapped).sin().atan2((angle - snapped).cos());
    if delta.abs() <= threshold {
        snapped
    } else {
        angle
    }
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_three_synthetic_douyin_bullseyes() {
        let mut bin = BinaryImage::new(420, 420, vec![255; 420 * 420]);
        draw_bullseye(&mut bin, 90, 90, 18);
        draw_bullseye(&mut bin, 90, 330, 18);
        draw_bullseye(&mut bin, 330, 330, 18);

        let finders = find_dy_finders(&bin);
        let selected = select_dy_finders(&finders).unwrap();

        assert_eq!(selected.len(), 3);
    }

    #[test]
    fn standard_douyin_samples_have_three_finders() {
        for path in ["samples/黑框版.jpg", "samples/无框版.jpg"] {
            if !std::path::Path::new(path).exists() {
                return;
            }

            let img = image::open(path).unwrap();
            let bin = crate::pipeline::preprocess::preprocess(&img);
            let finders = find_dy_finders(&bin);
            let selected = select_dy_finders(&finders)
                .unwrap_or_else(|| panic!("failed to select dy finders for {path}"));

            assert_eq!(selected.len(), 3);
        }
    }

    fn draw_bullseye(bin: &mut BinaryImage, cx: i32, cy: i32, r: i32) {
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                let dx = (x - cx) as f64;
                let dy = (y - cy) as f64;
                let d = dx.hypot(dy);
                let black = (d <= r as f64 && d >= r as f64 * 0.68) || d <= r as f64 * 0.30;
                if black && x >= 0 && y >= 0 && x < bin.w as i32 && y < bin.h as i32 {
                    bin.data[(y as u32 * bin.w + x as u32) as usize] = 0;
                }
            }
        }
    }
}
