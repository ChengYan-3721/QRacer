use crate::codec::data_matrix_grid::{DataMatrixSymbol, score_data_matrix_corners};
use crate::pipeline::preprocess::BinaryImage;

const MAX_COMPONENT_POINTS: usize = 6_000;
const MAX_COMPONENTS_TO_SCORE: usize = 3;
const MIN_DETECTION_SCORE: f64 = 0.70;
const EARLY_ACCEPT_SCORE: f64 = 0.88;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataMatrixCandidate {
    pub corners: [(f64, f64); 4],
    pub symbol: DataMatrixSymbol,
    pub score: f64,
}

impl DataMatrixCandidate {
    pub fn rows(self) -> usize {
        self.symbol.rows
    }

    pub fn cols(self) -> usize {
        self.symbol.cols
    }
}

pub fn select_data_matrix_candidate(bin: &BinaryImage) -> Option<DataMatrixCandidate> {
    find_data_matrix_candidates(bin).into_iter().next()
}

pub fn find_data_matrix_candidates(bin: &BinaryImage) -> Vec<DataMatrixCandidate> {
    if bin.w < 16 || bin.h < 16 {
        return Vec::new();
    }

    let mut components = black_components(bin);
    components.sort_by(|a, b| b.area.cmp(&a.area));

    let mut candidates = Vec::new();
    for component in components
        .into_iter()
        .filter(component_size_is_plausible)
        .take(MAX_COMPONENTS_TO_SCORE)
    {
        for rect in component_rectangles(&component) {
            for corners in corner_orientations(rect) {
                let Some(score) = score_data_matrix_corners(bin, &corners) else {
                    continue;
                };
                if score.score < MIN_DETECTION_SCORE {
                    continue;
                }
                let candidate = DataMatrixCandidate {
                    corners,
                    symbol: score.symbol,
                    score: score.score,
                };
                if candidate.score >= EARLY_ACCEPT_SCORE {
                    return vec![candidate];
                }
                candidates.push(candidate);
            }
        }
    }

    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    dedupe_candidates(candidates)
}

#[derive(Debug, Clone)]
struct BlackComponent {
    area: u32,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    points: Vec<(f64, f64)>,
}

impl BlackComponent {
    fn new(x: i32, y: i32) -> Self {
        Self {
            area: 0,
            min_x: x,
            max_x: x,
            min_y: y,
            max_y: y,
            points: Vec::new(),
        }
    }

    fn add(&mut self, x: i32, y: i32) {
        self.area += 1;
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);

        let point = (x as f64 + 0.5, y as f64 + 0.5);
        if self.points.len() < MAX_COMPONENT_POINTS {
            self.points.push(point);
        } else if self.area % 13 == 0 {
            let idx = (self.area as usize / 13) % MAX_COMPONENT_POINTS;
            self.points[idx] = point;
        }
    }

    fn width(&self) -> i32 {
        self.max_x - self.min_x + 1
    }

    fn height(&self) -> i32 {
        self.max_y - self.min_y + 1
    }
}

fn black_components(bin: &BinaryImage) -> Vec<BlackComponent> {
    let mut visited = vec![false; (bin.w * bin.h) as usize];
    let mut components = Vec::new();

    for y in 0..bin.h as i32 {
        for x in 0..bin.w as i32 {
            let idx = (y as u32 * bin.w + x as u32) as usize;
            if visited[idx] || !bin.is_black(x, y) {
                continue;
            }
            components.push(flood_black_component(bin, &mut visited, x, y));
        }
    }

    components
}

fn flood_black_component(
    bin: &BinaryImage,
    visited: &mut [bool],
    start_x: i32,
    start_y: i32,
) -> BlackComponent {
    let mut component = BlackComponent::new(start_x, start_y);
    let mut stack = vec![(start_x, start_y)];

    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= bin.w as i32 || y >= bin.h as i32 {
            continue;
        }
        let idx = (y as u32 * bin.w + x as u32) as usize;
        if visited[idx] || !bin.is_black(x, y) {
            continue;
        }
        visited[idx] = true;
        component.add(x, y);

        stack.push((x - 1, y));
        stack.push((x + 1, y));
        stack.push((x, y - 1));
        stack.push((x, y + 1));
    }

    component
}

fn component_size_is_plausible(component: &BlackComponent) -> bool {
    if component.area < 24 {
        return false;
    }
    if component.width() < 14 || component.height() < 14 {
        return false;
    }
    let aspect = component.width() as f64 / component.height().max(1) as f64;
    (0.20..=6.0).contains(&aspect)
}

fn component_rectangles(component: &BlackComponent) -> Vec<[(f64, f64); 4]> {
    let mut rectangles = vec![axis_aligned_rectangle(component)];

    for angle in best_component_angles(component) {
        rectangles.push(oriented_rectangle(component, angle));
    }

    rectangles
}

fn axis_aligned_rectangle(component: &BlackComponent) -> [(f64, f64); 4] {
    let min_x = component.min_x as f64 - 0.5;
    let min_y = component.min_y as f64 - 0.5;
    let max_x = component.max_x as f64 + 1.5;
    let max_y = component.max_y as f64 + 1.5;
    [
        (min_x, min_y),
        (max_x, min_y),
        (min_x, max_y),
        (max_x, max_y),
    ]
}

fn best_component_angles(component: &BlackComponent) -> Vec<f64> {
    if component.points.len() < 16 {
        return Vec::new();
    }

    let mut coarse: Vec<(f64, f64)> = (0..36)
        .map(|idx| {
            let angle = (idx as f64 * 5.0).to_radians();
            (projected_area(&component.points, angle), angle)
        })
        .collect();
    coarse.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut refined = Vec::new();
    for &(_, angle) in coarse.iter().take(1) {
        let mut best = (f64::INFINITY, angle);
        for offset in -5..=5 {
            let candidate = angle + (offset as f64).to_radians();
            let area = projected_area(&component.points, candidate);
            if area < best.0 {
                best = (area, candidate);
            }
        }
        if !refined
            .iter()
            .any(|existing: &f64| angle_distance(*existing, best.1) < 1.5_f64.to_radians())
        {
            refined.push(best.1);
        }
    }

    refined
}

fn projected_area(points: &[(f64, f64)], angle: f64) -> f64 {
    let (min_u, max_u, min_v, max_v) = projected_bounds(points, angle);
    (max_u - min_u).max(1.0) * (max_v - min_v).max(1.0)
}

fn oriented_rectangle(component: &BlackComponent, angle: f64) -> [(f64, f64); 4] {
    let (mut min_u, mut max_u, mut min_v, mut max_v) = projected_bounds(&component.points, angle);
    min_u -= 0.75;
    max_u += 0.75;
    min_v -= 0.75;
    max_v += 0.75;

    let u = (angle.cos(), angle.sin());
    let v = (-angle.sin(), angle.cos());
    [
        unproject(u, v, min_u, min_v),
        unproject(u, v, max_u, min_v),
        unproject(u, v, min_u, max_v),
        unproject(u, v, max_u, max_v),
    ]
}

fn projected_bounds(points: &[(f64, f64)], angle: f64) -> (f64, f64, f64, f64) {
    let u = (angle.cos(), angle.sin());
    let v = (-angle.sin(), angle.cos());
    let mut min_u = f64::INFINITY;
    let mut max_u = f64::NEG_INFINITY;
    let mut min_v = f64::INFINITY;
    let mut max_v = f64::NEG_INFINITY;

    for &(x, y) in points {
        let pu = x * u.0 + y * u.1;
        let pv = x * v.0 + y * v.1;
        min_u = min_u.min(pu);
        max_u = max_u.max(pu);
        min_v = min_v.min(pv);
        max_v = max_v.max(pv);
    }

    (min_u, max_u, min_v, max_v)
}

fn unproject(u: (f64, f64), v: (f64, f64), pu: f64, pv: f64) -> (f64, f64) {
    (u.0 * pu + v.0 * pv, u.1 * pu + v.1 * pv)
}

fn corner_orientations(rect: [(f64, f64); 4]) -> Vec<[(f64, f64); 4]> {
    let [a, b, c, d] = rect;
    vec![[a, b, c, d], [b, d, a, c], [d, c, b, a], [c, a, d, b]]
}

fn dedupe_candidates(candidates: Vec<DataMatrixCandidate>) -> Vec<DataMatrixCandidate> {
    let mut deduped: Vec<DataMatrixCandidate> = Vec::new();
    for candidate in candidates {
        if deduped
            .iter()
            .any(|existing| candidates_overlap(*existing, candidate))
        {
            continue;
        }
        deduped.push(candidate);
    }
    deduped
}

fn candidates_overlap(a: DataMatrixCandidate, b: DataMatrixCandidate) -> bool {
    if a.symbol != b.symbol {
        return false;
    }
    let ac = candidate_center(a);
    let bc = candidate_center(b);
    let side = candidate_side(a).max(candidate_side(b)).max(1.0);
    (ac.0 - bc.0).hypot(ac.1 - bc.1) < side * 0.08
}

fn candidate_center(candidate: DataMatrixCandidate) -> (f64, f64) {
    let mut x = 0.0;
    let mut y = 0.0;
    for point in candidate.corners {
        x += point.0;
        y += point.1;
    }
    (x * 0.25, y * 0.25)
}

fn candidate_side(candidate: DataMatrixCandidate) -> f64 {
    let top = distance(candidate.corners[0], candidate.corners[1]);
    let bottom = distance(candidate.corners[2], candidate.corners[3]);
    let left = distance(candidate.corners[0], candidate.corners[2]);
    let right = distance(candidate.corners[1], candidate.corners[3]);
    ((top + bottom + left + right) * 0.25).max(1.0)
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

fn angle_distance(a: f64, b: f64) -> f64 {
    let pi = std::f64::consts::PI;
    let delta = (a - b).rem_euclid(pi);
    delta.min(pi - delta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::data_matrix_grid::{DATA_MATRIX_SYMBOLS, DataMatrixMatrix};

    #[test]
    fn detects_synthetic_data_matrix_component() {
        let symbol = DATA_MATRIX_SYMBOLS
            .iter()
            .copied()
            .find(|symbol| symbol.rows == 24 && symbol.cols == 24)
            .unwrap();
        let image = render_matrix(&synthetic_matrix(symbol), 8);
        let candidate = select_data_matrix_candidate(&image).unwrap();

        assert_eq!(candidate.rows(), symbol.rows);
        assert_eq!(candidate.cols(), symbol.cols);
        assert!(candidate.score >= 0.80, "score was {}", candidate.score);
    }

    fn synthetic_matrix(symbol: DataMatrixSymbol) -> DataMatrixMatrix {
        let mut matrix = vec![vec![false; symbol.cols]; symbol.rows];
        for y in 0..symbol.rows {
            for x in 0..symbol.cols {
                matrix[y][x] = data_matrix_function_module(symbol, x, y)
                    .unwrap_or_else(|| ((x * 17 + y * 31 + x * y) % 7) < 3);
            }
        }
        matrix
    }

    fn data_matrix_function_module(symbol: DataMatrixSymbol, x: usize, y: usize) -> Option<bool> {
        let region_h = symbol.region_rows + 2;
        let region_w = symbol.region_cols + 2;
        let local_x = x % region_w;
        let local_y = y % region_h;
        if local_x == 0 || local_y == region_h - 1 {
            Some(true)
        } else if local_y == 0 {
            Some(local_x % 2 == 0)
        } else if local_x == region_w - 1 {
            Some(local_y % 2 == 1)
        } else {
            None
        }
    }

    fn render_matrix(matrix: &DataMatrixMatrix, scale: u32) -> BinaryImage {
        let rows = matrix.len();
        let cols = matrix[0].len();
        let width = cols as u32 * scale;
        let height = rows as u32 * scale;
        let mut data = vec![255; (width * height) as usize];
        for (module_y, row) in matrix.iter().enumerate() {
            for (module_x, &black) in row.iter().enumerate() {
                if !black {
                    continue;
                }
                for y in 0..scale {
                    for x in 0..scale {
                        let px = module_x as u32 * scale + x;
                        let py = module_y as u32 * scale + y;
                        data[(py * width + px) as usize] = 0;
                    }
                }
            }
        }
        BinaryImage::new(width, height, data)
    }
}
