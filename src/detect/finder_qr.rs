use crate::pipeline::preprocess::BinaryImage;

/// A detected QR finder pattern center and estimated module size in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QrFinder {
    pub cx: f64,
    pub cy: f64,
    pub module: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QrLatticeSignature {
    pub finder: f64,
    pub timing: f64,
    pub separator: f64,
}

impl QrLatticeSignature {
    pub fn is_confident(self) -> bool {
        self.finder >= 0.58 && self.timing >= 0.72 && self.separator >= 0.70
    }
}

#[derive(Debug, Clone, Copy)]
struct Run {
    black: bool,
    start: u32,
    len: u32,
}

#[derive(Debug, Clone)]
struct FinderCluster {
    cx: f64,
    cy: f64,
    module: f64,
    hits: u32,
}

/// Finds QR finder patterns using horizontal RLE candidates plus vertical
/// 1:1:3:1:1 cross-checking.
pub fn find_qr_finders(bin: &BinaryImage) -> Vec<QrFinder> {
    let mut clusters: Vec<FinderCluster> = Vec::new();

    for y in 0..bin.h {
        let runs = row_runs(bin, y);
        if runs.len() < 5 {
            continue;
        }

        for window in runs.windows(5) {
            if !is_finder_color_sequence(window) {
                continue;
            }

            let counts = [
                window[0].len,
                window[1].len,
                window[2].len,
                window[3].len,
                window[4].len,
            ];
            let Some(horizontal_module) = pattern_module(counts) else {
                continue;
            };

            let center_x = window[0].start as f64
                + counts[0] as f64
                + counts[1] as f64
                + counts[2] as f64 / 2.0;
            let center_x_i = center_x.round() as i32;
            let Some((center_y, vertical_module)) = cross_check_vertical(bin, center_x_i, y as i32)
            else {
                continue;
            };

            let module = (horizontal_module + vertical_module) * 0.5;
            add_to_clusters(&mut clusters, center_x, center_y, module);
        }
    }

    clusters
        .into_iter()
        .filter(|cluster| cluster.hits >= 2)
        .map(|cluster| QrFinder {
            cx: cluster.cx,
            cy: cluster.cy,
            module: cluster.module,
        })
        .collect()
}

/// Selects the three finder candidates that best form the QR corner triangle.
pub fn select_qr_finder_triplet(bin: &BinaryImage, finders: &[QrFinder]) -> Option<[QrFinder; 3]> {
    if finders.len() < 3 {
        return None;
    }

    let mut best_confident: Option<(f64, [QrFinder; 3])> = None;
    let mut best_fallback: Option<(f64, [QrFinder; 3])> = None;
    for a in 0..finders.len() - 2 {
        for b in a + 1..finders.len() - 1 {
            for c in b + 1..finders.len() {
                let triplet = [finders[a], finders[b], finders[c]];
                let Some(score) = score_finder_triplet(bin, triplet) else {
                    continue;
                };

                if qr_lattice_signature(bin, &triplet)
                    .is_some_and(|signature| signature.is_confident())
                {
                    if best_confident.is_none_or(|(best_score, _)| score > best_score) {
                        best_confident = Some((score, triplet));
                    }
                } else if best_fallback.is_none_or(|(best_score, _)| score > best_score) {
                    best_fallback = Some((score, triplet));
                }
            }
        }
    }

    best_confident.or(best_fallback).map(|(_, triplet)| triplet)
}

pub fn qr_lattice_signature(
    bin: &BinaryImage,
    finders: &[QrFinder; 3],
) -> Option<QrLatticeSignature> {
    let (tl, tr, bl) = order_finder_triplet(*finders);
    let module = ((tl.module + tr.module + bl.module) / 3.0).max(1.0);
    let horizontal_modules = squared_distance(tl, tr).sqrt() / module + 7.0;
    let vertical_modules = squared_distance(tl, bl).sqrt() / module + 7.0;
    let modules = ((horizontal_modules + vertical_modules) * 0.5)
        .round()
        .clamp(21.0, 177.0) as i32;
    if modules < 21 {
        return None;
    }

    let u_axis = unit_vector(tl, tr, module);
    let v_axis = unit_vector(tl, bl, module);
    let finder = [
        finder_template_score(bin, tl, u_axis, v_axis),
        finder_template_score(bin, tr, u_axis, v_axis),
        finder_template_score(bin, bl, u_axis, v_axis),
    ]
    .iter()
    .sum::<f64>()
        / 3.0;
    let timing = (timing_axis_score(bin, tl, u_axis, v_axis, modules)
        + timing_axis_score(bin, tl, v_axis, u_axis, modules))
        * 0.5;
    let separator = separator_score(bin, tl, u_axis, v_axis, modules);

    Some(QrLatticeSignature {
        finder,
        timing,
        separator,
    })
}

fn score_finder_triplet(bin: &BinaryImage, finders: [QrFinder; 3]) -> Option<f64> {
    let mut modules = [finders[0].module, finders[1].module, finders[2].module];
    modules.sort_by(f64::total_cmp);
    let min_module = modules[0];
    let max_module = modules[2];
    if min_module < 1.0 {
        return None;
    }

    let module_ratio = max_module / min_module;
    if module_ratio > 1.8 {
        return None;
    }

    let avg_module = (modules[0] + modules[1] + modules[2]) / 3.0;
    let mut distances = [
        squared_distance(finders[0], finders[1]),
        squared_distance(finders[0], finders[2]),
        squared_distance(finders[1], finders[2]),
    ];
    distances.sort_by(f64::total_cmp);

    let short = distances[0].sqrt();
    let long_leg = distances[1].sqrt();
    let diagonal_sq = distances[2];
    let min_side_modules = short / avg_module;
    if min_side_modules < 10.0 {
        return None;
    }

    let leg_ratio = long_leg / short;
    if leg_ratio > 2.2 {
        return None;
    }

    let right_error = (diagonal_sq - distances[0] - distances[1]).abs() / diagonal_sq;
    if right_error > 0.25 {
        return None;
    }

    let area = triangle_area(finders[0], finders[1], finders[2]);
    if area < avg_module * avg_module * 20.0 {
        return None;
    }

    let (tl, tr, bl) = order_finder_triplet(finders);
    let module = avg_module.max(1.0);
    let u_axis = unit_vector(tl, tr, module);
    let v_axis = unit_vector(tl, bl, module);
    let finder_scores = [
        finder_template_score(bin, tl, u_axis, v_axis),
        finder_template_score(bin, tr, u_axis, v_axis),
        finder_template_score(bin, bl, u_axis, v_axis),
    ];
    let min_finder_score = finder_scores.iter().copied().fold(1.0, f64::min);
    if min_finder_score < 0.20 {
        return None;
    }
    let finder_score = finder_scores.iter().sum::<f64>() / finder_scores.len() as f64;
    let timing_score = timing_pattern_score(bin, tl, tr, bl);
    let normalized_area = area / (avg_module * avg_module);
    Some(
        normalized_area
            - right_error * 40.0
            - (leg_ratio - 1.0).abs() * 8.0
            - (module_ratio - 1.0) * 12.0
            + finder_score * 520.0
            + min_finder_score * 260.0
            + timing_score * 35.0,
    )
}

fn order_finder_triplet(finders: [QrFinder; 3]) -> (QrFinder, QrFinder, QrFinder) {
    let distances = [
        (0, 1, squared_distance(finders[0], finders[1])),
        (0, 2, squared_distance(finders[0], finders[2])),
        (1, 2, squared_distance(finders[1], finders[2])),
    ];
    let &(a_idx, b_idx, _) = distances
        .iter()
        .max_by(|lhs, rhs| lhs.2.total_cmp(&rhs.2))
        .expect("three finder distances exist");

    let tl_idx = 3 - a_idx - b_idx;
    let tl = finders[tl_idx];
    let a = finders[a_idx];
    let b = finders[b_idx];

    if cross(tl, a, b) > 0.0 {
        (tl, a, b)
    } else {
        (tl, b, a)
    }
}

fn timing_pattern_score(bin: &BinaryImage, tl: QrFinder, tr: QrFinder, bl: QrFinder) -> f64 {
    let module = ((tl.module + tr.module + bl.module) / 3.0).max(1.0);
    let horizontal_modules = squared_distance(tl, tr).sqrt() / module + 7.0;
    let vertical_modules = squared_distance(tl, bl).sqrt() / module + 7.0;
    let modules = ((horizontal_modules + vertical_modules) * 0.5)
        .round()
        .clamp(21.0, 177.0) as i32;
    if modules < 21 {
        return 0.0;
    }

    let u = unit_vector(tl, tr, module);
    let v = unit_vector(tl, bl, module);
    let horizontal = timing_axis_score(bin, tl, u, v, modules);
    let vertical = timing_axis_score(bin, tl, v, u, modules);

    (horizontal + vertical) * 0.5
}

fn finder_template_score(
    bin: &BinaryImage,
    finder: QrFinder,
    u_axis: (f64, f64),
    v_axis: (f64, f64),
) -> f64 {
    let mut matches = 0_u32;
    let mut total = 0_u32;

    for y in 0..7 {
        for x in 0..7 {
            let dx = x as f64 - 3.0;
            let dy = y as f64 - 3.0;
            let sample = (
                finder.cx + u_axis.0 * dx + v_axis.0 * dy,
                finder.cy + u_axis.1 * dx + v_axis.1 * dy,
            );
            let actual = bilinear_sample(bin, sample.0, sample.1) < 128.0;
            let expected = x == 0
                || x == 6
                || y == 0
                || y == 6
                || ((2..=4).contains(&x) && (2..=4).contains(&y));
            if actual == expected {
                matches += 1;
            }
            total += 1;
        }
    }

    matches as f64 / total as f64
}

fn timing_axis_score(
    bin: &BinaryImage,
    tl: QrFinder,
    along: (f64, f64),
    inward: (f64, f64),
    modules: i32,
) -> f64 {
    let mut matches = 0_u32;
    let mut total = 0_u32;

    for module_idx in 8..modules - 8 {
        let point = (
            tl.cx + along.0 * (module_idx as f64 - 3.0) + inward.0 * 3.0,
            tl.cy + along.1 * (module_idx as f64 - 3.0) + inward.1 * 3.0,
        );
        let actual = bilinear_sample(bin, point.0, point.1) < 128.0;
        let expected = module_idx % 2 == 0;
        if actual == expected {
            matches += 1;
        }
        total += 1;
    }

    if total == 0 {
        return 0.0;
    }

    matches as f64 / total as f64
}

fn separator_score(
    bin: &BinaryImage,
    tl: QrFinder,
    u_axis: (f64, f64),
    v_axis: (f64, f64),
    modules: i32,
) -> f64 {
    let mut white = 0_u32;
    let mut total = 0_u32;

    for i in 0..8 {
        for (x, y) in [
            (i, 7),
            (7, i),
            (modules - 8, i),
            (modules - 1 - i, 7),
            (i, modules - 8),
            (7, modules - 1 - i),
        ] {
            let point = (
                tl.cx + u_axis.0 * (x as f64 - 3.0) + v_axis.0 * (y as f64 - 3.0),
                tl.cy + u_axis.1 * (x as f64 - 3.0) + v_axis.1 * (y as f64 - 3.0),
            );
            if bilinear_sample(bin, point.0, point.1) >= 128.0 {
                white += 1;
            }
            total += 1;
        }
    }

    white as f64 / total.max(1) as f64
}

fn row_runs(bin: &BinaryImage, y: u32) -> Vec<Run> {
    if bin.w == 0 {
        return Vec::new();
    }

    let mut runs = Vec::new();
    let mut start = 0;
    let mut black = bin.is_black(0, y as i32);

    for x in 1..bin.w {
        let next_black = bin.is_black(x as i32, y as i32);
        if next_black != black {
            runs.push(Run {
                black,
                start,
                len: x - start,
            });
            start = x;
            black = next_black;
        }
    }

    runs.push(Run {
        black,
        start,
        len: bin.w - start,
    });
    runs
}

fn is_finder_color_sequence(runs: &[Run]) -> bool {
    runs.len() == 5
        && runs[0].black
        && !runs[1].black
        && runs[2].black
        && !runs[3].black
        && runs[4].black
}

fn pattern_module(counts: [u32; 5]) -> Option<f64> {
    if counts.contains(&0) {
        return None;
    }

    let total: u32 = counts.iter().sum();
    if total < 7 {
        return None;
    }

    let module = total as f64 / 7.0;
    if module < 1.0 {
        return None;
    }

    let expected = [1.0, 1.0, 3.0, 1.0, 1.0];
    for (count, factor) in counts.into_iter().zip(expected) {
        let target = module * factor;
        let tolerance = if factor == 3.0 {
            module * 1.5
        } else {
            module * 0.8
        };
        if (count as f64 - target).abs() > tolerance {
            return None;
        }
    }

    Some(module)
}

fn cross_check_vertical(bin: &BinaryImage, center_x: i32, center_y: i32) -> Option<(f64, f64)> {
    if !bin.is_black(center_x, center_y) {
        return None;
    }

    let mut counts = [0_u32; 5];

    let mut y = center_y;
    while y >= 0 && bin.is_black(center_x, y) {
        counts[2] += 1;
        y -= 1;
    }
    while y >= 0 && !bin.is_black(center_x, y) {
        counts[1] += 1;
        y -= 1;
    }
    while y >= 0 && bin.is_black(center_x, y) {
        counts[0] += 1;
        y -= 1;
    }
    let top = y + 1;

    y = center_y + 1;
    while y < bin.h as i32 && bin.is_black(center_x, y) {
        counts[2] += 1;
        y += 1;
    }
    while y < bin.h as i32 && !bin.is_black(center_x, y) {
        counts[3] += 1;
        y += 1;
    }
    while y < bin.h as i32 && bin.is_black(center_x, y) {
        counts[4] += 1;
        y += 1;
    }

    let module = pattern_module(counts)?;
    let center = top as f64 + counts[0] as f64 + counts[1] as f64 + counts[2] as f64 / 2.0;
    Some((center, module))
}

fn add_to_clusters(clusters: &mut Vec<FinderCluster>, cx: f64, cy: f64, module: f64) {
    let merge_radius = (module * 2.0).max(3.0);

    for cluster in clusters.iter_mut() {
        let dx = cluster.cx - cx;
        let dy = cluster.cy - cy;
        if (dx * dx + dy * dy).sqrt() <= merge_radius {
            let hits = cluster.hits as f64;
            cluster.cx = (cluster.cx * hits + cx) / (hits + 1.0);
            cluster.cy = (cluster.cy * hits + cy) / (hits + 1.0);
            cluster.module = (cluster.module * hits + module) / (hits + 1.0);
            cluster.hits += 1;
            return;
        }
    }

    clusters.push(FinderCluster {
        cx,
        cy,
        module,
        hits: 1,
    });
}

fn squared_distance(a: QrFinder, b: QrFinder) -> f64 {
    let dx = a.cx - b.cx;
    let dy = a.cy - b.cy;
    dx * dx + dy * dy
}

fn cross(origin: QrFinder, a: QrFinder, b: QrFinder) -> f64 {
    let ax = a.cx - origin.cx;
    let ay = a.cy - origin.cy;
    let bx = b.cx - origin.cx;
    let by = b.cy - origin.cy;
    ax * by - ay * bx
}

fn unit_vector(from: QrFinder, to: QrFinder, length: f64) -> (f64, f64) {
    let dx = to.cx - from.cx;
    let dy = to.cy - from.cy;
    let distance = dx.hypot(dy);
    if distance <= f64::EPSILON {
        return (length, 0.0);
    }
    (dx / distance * length, dy / distance * length)
}

fn bilinear_sample(bin: &BinaryImage, x: f64, y: f64) -> f64 {
    if x < 0.0
        || y < 0.0
        || x > (bin.w.saturating_sub(1)) as f64
        || y > (bin.h.saturating_sub(1)) as f64
    {
        return 255.0;
    }

    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;

    let p00 = bin.get(x0, y0) as f64;
    let p10 = bin.get(x1, y0) as f64;
    let p01 = bin.get(x0, y1) as f64;
    let p11 = bin.get(x1, y1) as f64;
    let top = p00 * (1.0 - tx) + p10 * tx;
    let bottom = p01 * (1.0 - tx) + p11 * tx;

    top * (1.0 - ty) + bottom * ty
}

fn triangle_area(a: QrFinder, b: QrFinder, c: QrFinder) -> f64 {
    ((b.cx - a.cx) * (c.cy - a.cy) - (b.cy - a.cy) * (c.cx - a.cx)).abs() * 0.5
}
