use crate::codec::qr::{estimate_module_count, QrMatrix};
use crate::error::{QRacerError, Result};
use crate::pipeline::preprocess::BinaryImage;

pub fn infer_qr_version(warped: &BinaryImage) -> Result<u8> {
    let modules = estimate_module_count(warped)
        .ok_or_else(|| QRacerError::QrDecode("unable to infer QR grid size".to_owned()))?;
    version_for_modules(modules)
        .ok_or_else(|| QRacerError::QrDecode(format!("invalid QR grid size: {modules}")))
}

pub fn sample_qr_grid(warped: &BinaryImage, version: u8) -> Result<QrMatrix> {
    if !(1..=40).contains(&version) {
        return Err(QRacerError::QrDecode(format!(
            "invalid QR version: {version}"
        )));
    }

    let modules = modules_for_version(version);
    let grid = best_sampling_grid(warped, version, modules);
    let mut matrix = vec![vec![false; modules]; modules];
    for (y, row) in matrix.iter_mut().enumerate() {
        for (x, module) in row.iter_mut().enumerate() {
            *module = sample_qr_module_with_grid(warped, modules, x, y, grid);
        }
    }

    Ok(matrix)
}

fn modules_for_version(version: u8) -> usize {
    (version as usize - 1) * 4 + 21
}

fn version_for_modules(modules: usize) -> Option<u8> {
    if !(21..=177).contains(&modules) || !(modules - 21).is_multiple_of(4) {
        return None;
    }
    Some(((modules - 21) / 4 + 1) as u8)
}

#[derive(Clone, Copy)]
struct QrSamplingGrid {
    shift_x: f64,
    shift_y: f64,
    scale_x: f64,
    scale_y: f64,
}

impl QrSamplingGrid {
    const IDENTITY: Self = Self {
        shift_x: 0.0,
        shift_y: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
    };
}

fn best_sampling_grid(warped: &BinaryImage, version: u8, modules: usize) -> QrSamplingGrid {
    let shifts = [-0.24, -0.16, -0.08, 0.0, 0.08, 0.16, 0.24];
    let scales = [0.965, 0.98, 0.995, 1.0, 1.005, 1.02, 1.035];
    let mut best = (f64::NEG_INFINITY, QrSamplingGrid::IDENTITY);

    for shift_y in shifts {
        for shift_x in shifts {
            for scale_y in scales {
                for scale_x in scales {
                    let grid = QrSamplingGrid {
                        shift_x,
                        shift_y,
                        scale_x,
                        scale_y,
                    };
                    let score =
                        score_sampling_grid(warped, version, modules, grid) - grid_penalty(grid);
                    if score > best.0 {
                        best = (score, grid);
                    }
                }
            }
        }
    }

    best.1
}

fn grid_penalty(grid: QrSamplingGrid) -> f64 {
    (grid.shift_x.abs() + grid.shift_y.abs()) * 0.015
        + ((grid.scale_x - 1.0).abs() + (grid.scale_y - 1.0).abs()) * 0.45
}

fn score_sampling_grid(
    warped: &BinaryImage,
    version: u8,
    modules: usize,
    grid: QrSamplingGrid,
) -> f64 {
    let ctx = GridSampleContext {
        warped,
        modules,
        grid,
    };
    let mut scorer = GridScore::default();
    score_finder(&mut scorer, ctx, 0, 0);
    score_finder(&mut scorer, ctx, modules - 7, 0);
    score_finder(&mut scorer, ctx, 0, modules - 7);
    score_separators(&mut scorer, ctx);
    score_timing_patterns(&mut scorer, ctx);
    score_alignment_patterns(&mut scorer, ctx, version);

    scorer.finish() * 0.86 + score_sampling_contrast(warped, modules, grid) * 0.14
}

fn score_sampling_contrast(warped: &BinaryImage, modules: usize, grid: QrSamplingGrid) -> f64 {
    if modules == 0 {
        return 0.0;
    }

    let step = (modules / 41).max(1);
    let mut total = 0.0;
    let mut count = 0_usize;
    for y in (0..modules).step_by(step) {
        for x in (0..modules).step_by(step) {
            let (black, samples) = qr_module_vote(warped, modules, x, y, grid);
            let white = samples - black;
            total += black.max(white) as f64 / samples.max(1) as f64;
            count += 1;
        }
    }
    total / count.max(1) as f64
}

#[derive(Default)]
struct GridScore {
    correct: f64,
    total: f64,
}

#[derive(Clone, Copy)]
struct GridSampleContext<'a> {
    warped: &'a BinaryImage,
    modules: usize,
    grid: QrSamplingGrid,
}

impl GridScore {
    fn add(&mut self, ctx: GridSampleContext<'_>, x: usize, y: usize, expected: bool, weight: f64) {
        if x >= ctx.modules || y >= ctx.modules {
            return;
        }
        if sample_qr_module_with_grid(ctx.warped, ctx.modules, x, y, ctx.grid) == expected {
            self.correct += weight;
        }
        self.total += weight;
    }

    fn finish(self) -> f64 {
        if self.total <= f64::EPSILON {
            0.0
        } else {
            self.correct / self.total
        }
    }
}

fn score_finder(
    scorer: &mut GridScore,
    ctx: GridSampleContext<'_>,
    start_x: usize,
    start_y: usize,
) {
    for y in 0..7 {
        for x in 0..7 {
            scorer.add(ctx, start_x + x, start_y + y, finder_expected(x, y), 5.0);
        }
    }
}

fn score_separators(scorer: &mut GridScore, ctx: GridSampleContext<'_>) {
    for i in 0..8 {
        scorer.add(ctx, i, 7, false, 2.0);
        scorer.add(ctx, 7, i, false, 2.0);
        scorer.add(ctx, ctx.modules - 8, i, false, 2.0);
        scorer.add(ctx, ctx.modules - 1 - i, 7, false, 2.0);
        scorer.add(ctx, i, ctx.modules - 8, false, 2.0);
        scorer.add(ctx, 7, ctx.modules - 1 - i, false, 2.0);
    }
}

fn score_timing_patterns(scorer: &mut GridScore, ctx: GridSampleContext<'_>) {
    if ctx.modules <= 16 {
        return;
    }
    for i in 8..ctx.modules - 8 {
        let expected = i % 2 == 0;
        scorer.add(ctx, i, 6, expected, 3.0);
        scorer.add(ctx, 6, i, expected, 3.0);
    }
}

fn score_alignment_patterns(scorer: &mut GridScore, ctx: GridSampleContext<'_>, version: u8) {
    for cy in alignment_pattern_positions(version, ctx.modules) {
        for cx in alignment_pattern_positions(version, ctx.modules) {
            if alignment_overlaps_finder(cx, cy, ctx.modules) {
                continue;
            }
            for dy in 0..5 {
                for dx in 0..5 {
                    let x = cx + dx - 2;
                    let y = cy + dy - 2;
                    let expected = dx == 0 || dx == 4 || dy == 0 || dy == 4 || (dx == 2 && dy == 2);
                    scorer.add(ctx, x, y, expected, 4.0);
                }
            }
        }
    }
}

fn alignment_pattern_positions(version: u8, modules: usize) -> Vec<usize> {
    if version <= 1 {
        return Vec::new();
    }
    let count = version as usize / 7 + 2;
    let step = if version == 32 {
        26
    } else {
        ((version as usize * 4 + count * 2 + 1) / (count * 2 - 2)) * 2
    };
    let mut positions = vec![6; count];
    let mut pos = modules - 7;
    for index in (1..count).rev() {
        positions[index] = pos;
        pos = pos.saturating_sub(step);
    }
    positions
}

fn alignment_overlaps_finder(cx: usize, cy: usize, modules: usize) -> bool {
    (cx == 6 && (cy == 6 || cy == modules - 7)) || (cy == 6 && cx == modules - 7)
}

fn finder_expected(x: usize, y: usize) -> bool {
    let dx = (x as i32 - 3).abs();
    let dy = (y as i32 - 3).abs();
    dx.max(dy) != 2
}

fn sample_qr_module_with_grid(
    warped: &BinaryImage,
    modules: usize,
    x: usize,
    y: usize,
    grid: QrSamplingGrid,
) -> bool {
    if modules == 0 {
        return false;
    }

    let (black, total) = qr_module_vote(warped, modules, x, y, grid);
    black * 2 + 1 >= total
}

fn qr_module_vote(
    warped: &BinaryImage,
    modules: usize,
    x: usize,
    y: usize,
    grid: QrSamplingGrid,
) -> (usize, usize) {
    let cell_w = warped.w as f64 / modules as f64 * grid.scale_x;
    let cell_h = warped.h as f64 / modules as f64 * grid.scale_y;
    let center_x =
        warped.w as f64 * 0.5 + (x as f64 + 0.5 - modules as f64 * 0.5 + grid.shift_x) * cell_w;
    let center_y =
        warped.h as f64 * 0.5 + (y as f64 + 0.5 - modules as f64 * 0.5 + grid.shift_y) * cell_h;
    let offsets = [-0.18, 0.0, 0.18];

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

    (black, total)
}
