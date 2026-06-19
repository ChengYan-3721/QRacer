use crate::pipeline::preprocess::BinaryImage;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WxFinder {
    pub cx: f64,
    pub cy: f64,
    pub r_outer: f64,
}

#[derive(Debug, Clone)]
struct Component {
    cx: f64,
    cy: f64,
    radius: f64,
    area: u32,
}

pub fn find_wx_finders(bin: &BinaryImage) -> Vec<WxFinder> {
    let components = circular_components(bin);
    let mut candidates = Vec::new();

    for outer in &components {
        if outer.radius > bin.w.min(bin.h) as f64 * 0.08 {
            continue;
        }
        let nested = components
            .iter()
            .filter(|inner| {
                inner.radius < outer.radius * 0.78
                    && inner.radius > outer.radius * 0.10
                    && distance(outer.cx, outer.cy, inner.cx, inner.cy) < outer.radius * 0.35
            })
            .count();

        if nested == 0 {
            continue;
        }

        let finder = WxFinder {
            cx: outer.cx,
            cy: outer.cy,
            r_outer: outer.radius,
        };
        add_unique_finder(&mut candidates, finder);
    }

    for dot in &components {
        let Some(r_outer) = estimate_bullseye_from_center_dot(bin, dot) else {
            continue;
        };
        let finder = WxFinder {
            cx: dot.cx,
            cy: dot.cy,
            r_outer,
        };
        add_unique_finder(&mut candidates, finder);
    }

    scan_standard_corner_templates(bin, &mut candidates);

    candidates.sort_by(|a, b| b.r_outer.total_cmp(&a.r_outer));
    candidates
}

pub fn select_wx_finders(finders: &[WxFinder]) -> Option<[WxFinder; 3]> {
    select_wx_finders_raw(finders).map(normalize_wx_finder_triplet)
}

pub fn select_wx_finders_raw(finders: &[WxFinder]) -> Option<[WxFinder; 3]> {
    select_wx_finders_raw_impl(finders, None)
}

pub fn select_wx_finders_raw_with_badge(
    finders: &[WxFinder],
    badge: (f64, f64),
) -> Option<[WxFinder; 3]> {
    select_wx_finders_raw_impl(finders, Some(badge))
}

fn select_wx_finders_raw_impl(
    finders: &[WxFinder],
    badge: Option<(f64, f64)>,
) -> Option<[WxFinder; 3]> {
    if finders.len() < 3 {
        return None;
    }

    let mut best: Option<(f64, [WxFinder; 3])> = None;
    for a in 0..finders.len() - 2 {
        for b in a + 1..finders.len() - 1 {
            for c in b + 1..finders.len() {
                let triplet = [finders[a], finders[b], finders[c]];
                let area = triangle_area(triplet[0], triplet[1], triplet[2]);
                let radius = (triplet[0].r_outer + triplet[1].r_outer + triplet[2].r_outer) / 3.0;
                if area < radius * radius * 6.0 {
                    continue;
                }
                let Some(shape_penalty) = right_isosceles_penalty(triplet) else {
                    continue;
                };
                let radius_spread = radius_spread(triplet);
                let Some(size_penalty) = finder_size_penalty(triplet, radius) else {
                    continue;
                };
                let Some(badge_penalty) = badge_geometry_penalty(triplet, badge) else {
                    continue;
                };
                let score = area
                    - radius_spread * radius * 20.0
                    - shape_penalty * area
                    - size_penalty * area
                    - badge_penalty * area;
                if best.is_none_or(|(best_score, _)| score > best_score) {
                    best = Some((score, triplet));
                }
            }
        }
    }

    best.map(|(_, triplet)| triplet)
}

fn circular_components(bin: &BinaryImage) -> Vec<Component> {
    let mut visited = vec![false; (bin.w * bin.h) as usize];
    let mut components = Vec::new();
    let min_area = ((bin.w.min(bin.h) as f64 * 0.003).powi(2)).max(4.0) as u32;

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
    let fill = component.area as f64 / disk_area;
    (0.10..=1.10).contains(&fill)
}

fn estimate_bullseye_from_center_dot(bin: &BinaryImage, dot: &Component) -> Option<f64> {
    let min_dim = bin.w.min(bin.h) as f64;
    if dot.radius < min_dim * 0.004 || dot.radius > min_dim * 0.065 {
        return None;
    }

    let gap_ratio = circle_black_ratio(bin, dot.cx, dot.cy, dot.radius * 1.9);
    if gap_ratio > 0.55 {
        return None;
    }

    let mut best_ring: Option<(f64, f64)> = None;
    let start = (dot.radius * 2.4).ceil() as u32;
    let end = (dot.radius * 8.0).ceil() as u32;
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
    for radius in ring_radius.ceil() as u32..=(dot.radius * 9.0).ceil() as u32 {
        let radius = radius as f64;
        if circle_black_ratio(bin, dot.cx, dot.cy, radius) < 0.30 {
            outer = radius;
            break;
        }
    }

    if outer > min_dim * 0.08 {
        return None;
    }

    let ratio = outer / dot.radius.max(1.0);
    if !(2.5..=9.5).contains(&ratio) {
        return None;
    }

    Some(outer)
}

fn scan_standard_corner_templates(bin: &BinaryImage, candidates: &mut Vec<WxFinder>) {
    let min_dim = bin.w.min(bin.h) as f64;
    let radius_min = (min_dim * 0.025).max(5.0);
    let radius_max = (min_dim * 0.065).max(radius_min + 1.0);
    let step = ((min_dim / 160.0).round() as i32).max(2);
    let radius_step = ((min_dim / 180.0).round() as i32).max(1);
    let regions = [
        (
            (bin.w as f64 * 0.08) as i32,
            (bin.w as f64 * 0.36) as i32,
            (bin.h as f64 * 0.08) as i32,
            (bin.h as f64 * 0.36) as i32,
        ),
        (
            (bin.w as f64 * 0.64) as i32,
            (bin.w as f64 * 0.92) as i32,
            (bin.h as f64 * 0.08) as i32,
            (bin.h as f64 * 0.36) as i32,
        ),
        (
            (bin.w as f64 * 0.08) as i32,
            (bin.w as f64 * 0.36) as i32,
            (bin.h as f64 * 0.64) as i32,
            (bin.h as f64 * 0.92) as i32,
        ),
    ];

    for (x0, x1, y0, y1) in regions {
        let mut matches: Vec<(f64, WxFinder)> = Vec::new();
        let mut y = y0;
        while y <= y1 {
            let mut x = x0;
            while x <= x1 {
                let mut radius = radius_min;
                while radius <= radius_max {
                    let score = bullseye_template_score(bin, x as f64, y as f64, radius);
                    if score >= 2.9 {
                        matches.push((
                            score,
                            WxFinder {
                                cx: x as f64,
                                cy: y as f64,
                                r_outer: radius,
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
        for (_, finder) in matches.into_iter().take(4) {
            add_unique_finder(candidates, finder);
        }
    }
}

fn bullseye_template_score(bin: &BinaryImage, cx: f64, cy: f64, r_outer: f64) -> f64 {
    let center = circle_black_ratio(bin, cx, cy, r_outer * 0.16);
    let gap = circle_black_ratio(bin, cx, cy, r_outer * 0.46);
    let ring = (circle_black_ratio(bin, cx, cy, r_outer * 0.72)
        + circle_black_ratio(bin, cx, cy, r_outer * 0.88))
        * 0.5;
    let outside = circle_black_ratio(bin, cx, cy, r_outer * 1.18);

    if center < 0.42 || gap > 0.62 || ring < 0.46 {
        return 0.0;
    }

    center * 1.2 + (1.0 - gap) * 1.0 + ring * 1.8 + (1.0 - outside) * 0.4
}

fn circle_black_ratio(bin: &BinaryImage, cx: f64, cy: f64, radius: f64) -> f64 {
    let samples = 48;
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

fn add_unique_finder(finders: &mut Vec<WxFinder>, candidate: WxFinder) {
    let merge_radius = candidate.r_outer.max(4.0);
    if finders
        .iter()
        .any(|finder| distance(finder.cx, finder.cy, candidate.cx, candidate.cy) < merge_radius)
    {
        return;
    }
    finders.push(candidate);
}

fn radius_spread(finders: [WxFinder; 3]) -> f64 {
    let min = finders
        .iter()
        .map(|finder| finder.r_outer)
        .fold(f64::INFINITY, f64::min);
    let max = finders
        .iter()
        .map(|finder| finder.r_outer)
        .fold(0.0, f64::max);
    max - min
}

fn right_isosceles_penalty(finders: [WxFinder; 3]) -> Option<f64> {
    let mut distances = [
        distance(finders[0].cx, finders[0].cy, finders[1].cx, finders[1].cy),
        distance(finders[0].cx, finders[0].cy, finders[2].cx, finders[2].cy),
        distance(finders[1].cx, finders[1].cy, finders[2].cx, finders[2].cy),
    ];
    distances.sort_by(f64::total_cmp);

    let leg_a = distances[0];
    let leg_b = distances[1];
    let diagonal = distances[2];
    if leg_a <= 0.0 {
        return None;
    }

    let leg_ratio = leg_b / leg_a;
    let diagonal_ratio = diagonal / ((leg_a + leg_b) * 0.5);
    let diagonal_error = (diagonal_ratio - std::f64::consts::SQRT_2).abs();
    let leg_error = (leg_ratio - 1.0).abs();

    if leg_error > 0.32 || diagonal_error > 0.34 {
        return None;
    }

    Some(leg_error * 1.8 + diagonal_error * 2.4)
}

fn finder_size_penalty(finders: [WxFinder; 3], radius: f64) -> Option<f64> {
    let mut distances = [
        distance(finders[0].cx, finders[0].cy, finders[1].cx, finders[1].cy),
        distance(finders[0].cx, finders[0].cy, finders[2].cx, finders[2].cy),
        distance(finders[1].cx, finders[1].cy, finders[2].cx, finders[2].cy),
    ];
    distances.sort_by(f64::total_cmp);

    let leg = (distances[0] + distances[1]) * 0.5;
    if leg <= f64::EPSILON {
        return None;
    }

    let ratio = radius / leg;
    if !(0.045..=0.115).contains(&ratio) {
        return None;
    }

    Some(((ratio - 0.0786) / 0.0786).abs() * 0.12)
}

fn badge_geometry_penalty(finders: [WxFinder; 3], badge: Option<(f64, f64)>) -> Option<f64> {
    let Some(badge) = badge else {
        return Some(0.0);
    };
    let (corner_idx, a_idx, b_idx) = right_angle_indices(finders)?;

    let corner = finders[corner_idx];
    let a = finders[a_idx];
    let b = finders[b_idx];
    let leg = (distance(corner.cx, corner.cy, a.cx, a.cy)
        + distance(corner.cx, corner.cy, b.cx, b.cy))
        * 0.5;
    if leg <= f64::EPSILON {
        return None;
    }

    let expected_badge = (a.cx + b.cx - corner.cx, a.cy + b.cy - corner.cy);
    let error = distance(expected_badge.0, expected_badge.1, badge.0, badge.1) / leg;
    if error > 0.42 {
        return None;
    }

    Some(error * 1.6)
}

fn normalize_wx_finder_triplet(finders: [WxFinder; 3]) -> [WxFinder; 3] {
    let Some((corner_idx, a_idx, b_idx)) = right_angle_indices(finders) else {
        return finders;
    };

    let corner = finders[corner_idx];
    let a = finders[a_idx];
    let b = finders[b_idx];
    let va = (a.cx - corner.cx, a.cy - corner.cy);
    let vb = (b.cx - corner.cx, b.cy - corner.cy);
    let cross = va.0 * vb.1 - va.1 * vb.0;
    if cross.abs() < f64::EPSILON {
        return finders;
    }

    let sign = cross.signum();
    let angle_a = va.1.atan2(va.0);
    let angle_b = vb.1.atan2(vb.0) - sign * std::f64::consts::FRAC_PI_2;
    let angle = snap_near_cardinal(average_angle(angle_a, angle_b), 1.0_f64.to_radians());
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

fn right_angle_indices(finders: [WxFinder; 3]) -> Option<(usize, usize, usize)> {
    let mut pairs = [
        (finder_distance2(finders[0], finders[1]), 0_usize, 1_usize),
        (finder_distance2(finders[0], finders[2]), 0, 2),
        (finder_distance2(finders[1], finders[2]), 1, 2),
    ];
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (_, a_idx, b_idx) = pairs[2];
    let corner_idx = 3 - a_idx - b_idx;
    Some((corner_idx, a_idx, b_idx))
}

fn finder_distance2(a: WxFinder, b: WxFinder) -> f64 {
    let dx = a.cx - b.cx;
    let dy = a.cy - b.cy;
    dx * dx + dy * dy
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

fn triangle_area(a: WxFinder, b: WxFinder, c: WxFinder) -> f64 {
    ((b.cx - a.cx) * (c.cy - a.cy) - (b.cy - a.cy) * (c.cx - a.cx)).abs() * 0.5
}

fn distance(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    (ax - bx).hypot(ay - by)
}
