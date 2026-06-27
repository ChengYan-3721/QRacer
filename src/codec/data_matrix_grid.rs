use nalgebra::Vector3;

use crate::error::{QRacerError, Result};
use crate::pipeline::perspective::homography_from_4pts;
use crate::pipeline::preprocess::BinaryImage;

pub type DataMatrixMatrix = Vec<Vec<bool>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataMatrixSymbol {
    pub rows: usize,
    pub cols: usize,
    pub region_rows: usize,
    pub region_cols: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataMatrixGrid {
    pub rows: usize,
    pub cols: usize,
    pub symbol: DataMatrixSymbol,
    pub matrix: DataMatrixMatrix,
    pub score: f64,
    pub sampling: DataMatrixSamplingGrid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DataMatrixCornerScore {
    pub symbol: DataMatrixSymbol,
    pub score: f64,
}

pub const DATA_MATRIX_SYMBOLS: &[DataMatrixSymbol] = &[
    symbol(10, 10, 8, 8),
    symbol(12, 12, 10, 10),
    symbol(8, 18, 6, 16),
    symbol(14, 14, 12, 12),
    symbol(8, 32, 6, 14),
    symbol(16, 16, 14, 14),
    symbol(12, 26, 10, 24),
    symbol(18, 18, 16, 16),
    symbol(20, 20, 18, 18),
    symbol(12, 36, 10, 16),
    symbol(22, 22, 20, 20),
    symbol(16, 36, 14, 16),
    symbol(24, 24, 22, 22),
    symbol(26, 26, 24, 24),
    symbol(16, 48, 14, 22),
    symbol(32, 32, 14, 14),
    symbol(36, 36, 16, 16),
    symbol(40, 40, 18, 18),
    symbol(44, 44, 20, 20),
    symbol(48, 48, 22, 22),
    symbol(52, 52, 24, 24),
    symbol(64, 64, 14, 14),
    symbol(72, 72, 16, 16),
    symbol(80, 80, 18, 18),
    symbol(88, 88, 20, 20),
    symbol(96, 96, 22, 22),
    symbol(104, 104, 24, 24),
    symbol(120, 120, 18, 18),
    symbol(132, 132, 20, 20),
    symbol(144, 144, 22, 22),
];

const fn symbol(
    rows: usize,
    cols: usize,
    region_rows: usize,
    region_cols: usize,
) -> DataMatrixSymbol {
    DataMatrixSymbol {
        rows,
        cols,
        region_rows,
        region_cols,
    }
}

pub fn sample_data_matrix_grid(warped: &BinaryImage) -> Result<DataMatrixGrid> {
    let Some((symbol, grid, score)) = best_data_matrix_grid(warped, None) else {
        return Err(QRacerError::DataMatrix(
            "unable to infer Data Matrix grid size".to_owned(),
        ));
    };
    if score < MIN_WARPED_GRID_SCORE {
        return Err(QRacerError::DataMatrix(format!(
            "Data Matrix grid score too low: {score:.3}"
        )));
    }

    Ok(sample_data_matrix_grid_with_sampling(
        warped, symbol, grid, score,
    ))
}

pub fn sample_data_matrix_grid_for_symbols<I>(
    warped: &BinaryImage,
    symbols: I,
) -> Result<DataMatrixGrid>
where
    I: IntoIterator<Item = DataMatrixSymbol>,
{
    let Some((symbol, grid, score)) = best_warped_grid_for_symbols(warped, symbols) else {
        return Err(QRacerError::DataMatrix(
            "unable to align Data Matrix sampling grid".to_owned(),
        ));
    };
    if score < MIN_WARPED_GRID_SCORE {
        return Err(QRacerError::DataMatrix(format!(
            "Data Matrix grid score too low: {score:.3}"
        )));
    }

    Ok(sample_data_matrix_grid_with_sampling(
        warped, symbol, grid, score,
    ))
}

pub fn score_data_matrix_corners(
    bin: &BinaryImage,
    corners: &[(f64, f64); 4],
) -> Option<DataMatrixCornerScore> {
    let mut best: Option<(DataMatrixSymbol, f64)> = None;
    let timing_hint = estimate_corner_timing_modules(bin, corners);
    for &symbol in DATA_MATRIX_SYMBOLS {
        let ratio_penalty = corner_aspect_penalty(corners, symbol);
        if ratio_penalty > 0.45 {
            continue;
        }
        let h = homography_from_4pts(
            &[
                (0.0, 0.0),
                (symbol.cols as f64, 0.0),
                (0.0, symbol.rows as f64),
                (symbol.cols as f64, symbol.rows as f64),
            ],
            corners,
        );
        for grid in corner_sampling_grids() {
            let score = score_data_matrix_outer_symbol(symbol, grid, |x, y, grid| {
                mapped_module_vote(bin, &h, symbol, x, y, grid)
            }) - ratio_penalty
                - corner_timing_penalty(timing_hint, symbol)
                - grid_penalty(grid);
            if best.is_none_or(|(_, best_score)| score > best_score) {
                best = Some((symbol, score));
            }
        }
    }

    best.map(|(symbol, score)| DataMatrixCornerScore { symbol, score })
}

fn sample_data_matrix_grid_with_sampling(
    warped: &BinaryImage,
    symbol: DataMatrixSymbol,
    grid: DataMatrixSamplingGrid,
    score: f64,
) -> DataMatrixGrid {
    let mut matrix = vec![vec![false; symbol.cols]; symbol.rows];
    for (y, row) in matrix.iter_mut().enumerate() {
        for (x, module) in row.iter_mut().enumerate() {
            let (black, total) = warped_module_vote(warped, symbol, x, y, grid);
            *module = data_matrix_function_module(symbol, x, y)
                .unwrap_or(module_vote_is_black(black, total));
        }
    }

    DataMatrixGrid {
        rows: symbol.rows,
        cols: symbol.cols,
        symbol,
        matrix,
        score,
        sampling: grid,
    }
}

fn best_data_matrix_grid(
    warped: &BinaryImage,
    forced_symbol: Option<DataMatrixSymbol>,
) -> Option<(DataMatrixSymbol, DataMatrixSamplingGrid, f64)> {
    if let Some(symbol) = forced_symbol {
        return best_warped_grid_for_symbols(warped, [symbol]);
    }

    let actual_ratio = if warped.h == 0 {
        1.0
    } else {
        warped.w as f64 / warped.h as f64
    };
    let timing = DataMatrixTimingEstimate::from_warped(warped);
    let mut coarse = Vec::new();
    for &symbol in DATA_MATRIX_SYMBOLS {
        let ratio_penalty = image_aspect_penalty(actual_ratio, symbol);
        if ratio_penalty > 0.45 {
            continue;
        }
        let mut best_symbol_score = f64::NEG_INFINITY;
        for grid in corner_sampling_grids() {
            let score = warped_symbol_score(warped, symbol, grid, ratio_penalty, timing);
            best_symbol_score = best_symbol_score.max(score);
        }
        coarse.push((best_symbol_score, symbol));
    }
    coarse.sort_by(|a, b| b.0.total_cmp(&a.0));
    let symbols = coarse
        .into_iter()
        .take(6)
        .map(|(_, symbol)| symbol)
        .collect::<Vec<_>>();

    best_warped_grid_for_symbols(warped, symbols)
}

fn best_warped_grid_for_symbols<I>(
    warped: &BinaryImage,
    symbols: I,
) -> Option<(DataMatrixSymbol, DataMatrixSamplingGrid, f64)>
where
    I: IntoIterator<Item = DataMatrixSymbol>,
{
    let mut best: Option<(DataMatrixSymbol, DataMatrixSamplingGrid, f64)> = None;
    let actual_ratio = if warped.h == 0 {
        1.0
    } else {
        warped.w as f64 / warped.h as f64
    };
    let timing = DataMatrixTimingEstimate::from_warped(warped);

    for symbol in symbols {
        let ratio_penalty = image_aspect_penalty(actual_ratio, symbol);
        if ratio_penalty > 0.45 {
            continue;
        }

        update_best_warped_grid_for_symbol(
            warped,
            symbol,
            ratio_penalty,
            timing,
            sampling_grids(),
            &mut best,
        );
    }

    best
}

fn update_best_warped_grid_for_symbol<I>(
    warped: &BinaryImage,
    symbol: DataMatrixSymbol,
    ratio_penalty: f64,
    timing: DataMatrixTimingEstimate,
    grids: I,
    best: &mut Option<(DataMatrixSymbol, DataMatrixSamplingGrid, f64)>,
) where
    I: IntoIterator<Item = DataMatrixSamplingGrid>,
{
    let mut ranked_grids = grids
        .into_iter()
        .enumerate()
        .map(|(order, grid)| {
            (
                warped_symbol_edge_score(warped, symbol, grid, ratio_penalty, timing),
                order,
                grid,
            )
        })
        .collect::<Vec<_>>();
    ranked_grids.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    for (edge_score, _, grid) in ranked_grids {
        if best
            .as_ref()
            .is_some_and(|(_, _, best_score)| edge_score + SAMPLING_CONTRAST_WEIGHT < *best_score)
        {
            break;
        }

        let score =
            edge_score + score_sampling_contrast(warped, symbol, grid) * SAMPLING_CONTRAST_WEIGHT;
        if best.is_none_or(|(_, _, best_score)| score > best_score) {
            *best = Some((symbol, grid, score));
        }
    }
}

fn warped_symbol_score(
    warped: &BinaryImage,
    symbol: DataMatrixSymbol,
    grid: DataMatrixSamplingGrid,
    ratio_penalty: f64,
    timing: DataMatrixTimingEstimate,
) -> f64 {
    warped_symbol_edge_score(warped, symbol, grid, ratio_penalty, timing)
        + score_sampling_contrast(warped, symbol, grid) * SAMPLING_CONTRAST_WEIGHT
}

fn warped_symbol_edge_score(
    warped: &BinaryImage,
    symbol: DataMatrixSymbol,
    grid: DataMatrixSamplingGrid,
    ratio_penalty: f64,
    timing: DataMatrixTimingEstimate,
) -> f64 {
    score_data_matrix_symbol(symbol, grid, |x, y, grid| {
        warped_module_vote(warped, symbol, x, y, grid)
    }) - ratio_penalty
        - timing.size_penalty(symbol)
        - grid_penalty(grid)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DataMatrixSamplingGrid {
    pub shift_x: f64,
    pub shift_y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
}

const SAMPLING_CONTRAST_WEIGHT: f64 = 0.12;
const MIN_WARPED_GRID_SCORE: f64 = 0.62;

fn sampling_grids() -> impl Iterator<Item = DataMatrixSamplingGrid> {
    const SHIFTS: [f64; 5] = [-0.18, -0.09, 0.0, 0.09, 0.18];
    const SCALES: [f64; 5] = [0.970, 0.985, 1.0, 1.015, 1.030];
    SHIFTS.into_iter().flat_map(|shift_y| {
        SHIFTS.into_iter().flat_map(move |shift_x| {
            SCALES.into_iter().flat_map(move |scale_y| {
                SCALES
                    .into_iter()
                    .map(move |scale_x| DataMatrixSamplingGrid {
                        shift_x,
                        shift_y,
                        scale_x,
                        scale_y,
                    })
            })
        })
    })
}

fn corner_sampling_grids() -> impl Iterator<Item = DataMatrixSamplingGrid> {
    [DataMatrixSamplingGrid {
        shift_x: 0.0,
        shift_y: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
    }]
    .into_iter()
}

fn grid_penalty(grid: DataMatrixSamplingGrid) -> f64 {
    (grid.shift_x.abs() + grid.shift_y.abs()) * 0.020
        + ((grid.scale_x - 1.0).abs() + (grid.scale_y - 1.0).abs()) * 0.50
}

fn score_data_matrix_symbol<F>(
    symbol: DataMatrixSymbol,
    grid: DataMatrixSamplingGrid,
    mut vote: F,
) -> f64
where
    F: FnMut(usize, usize, DataMatrixSamplingGrid) -> (usize, usize),
{
    let mut scorer = GridScore::default();
    let region_h = symbol.region_rows + 2;
    let region_w = symbol.region_cols + 2;
    if region_h == 0 || region_w == 0 {
        return 0.0;
    }

    for y0 in (0..symbol.rows).step_by(region_h) {
        for x0 in (0..symbol.cols).step_by(region_w) {
            let y_end = (y0 + region_h).min(symbol.rows);
            let x_end = (x0 + region_w).min(symbol.cols);
            for y in y0..y_end {
                scorer.add(&mut vote, x0, y, true, grid, 4.4);
                scorer.add(&mut vote, x_end - 1, y, (y - y0) % 2 == 1, grid, 3.8);
            }
            for x in x0..x_end {
                scorer.add(&mut vote, x, y_end - 1, true, grid, 4.4);
                scorer.add(&mut vote, x, y0, (x - x0) % 2 == 0, grid, 3.8);
            }
        }
    }

    scorer.finish()
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

fn score_data_matrix_outer_symbol<F>(
    symbol: DataMatrixSymbol,
    grid: DataMatrixSamplingGrid,
    mut vote: F,
) -> f64
where
    F: FnMut(usize, usize, DataMatrixSamplingGrid) -> (usize, usize),
{
    let mut scorer = GridScore::default();
    for y in 0..symbol.rows {
        scorer.add(&mut vote, 0, y, true, grid, 4.4);
        scorer.add(&mut vote, symbol.cols - 1, y, y % 2 == 1, grid, 3.8);
    }
    for x in 0..symbol.cols {
        scorer.add(&mut vote, x, symbol.rows - 1, true, grid, 4.4);
        scorer.add(&mut vote, x, 0, x % 2 == 0, grid, 3.8);
    }
    scorer.finish()
}

fn estimate_corner_timing_modules(
    bin: &BinaryImage,
    corners: &[(f64, f64); 4],
) -> (Option<usize>, Option<usize>) {
    let top = estimate_edge_timing_modules(bin, corners[0], corners[1], corners[2], corners[3]);
    let right = estimate_edge_timing_modules(bin, corners[1], corners[3], corners[0], corners[2]);
    (top, right)
}

fn corner_timing_penalty(
    timing_hint: (Option<usize>, Option<usize>),
    symbol: DataMatrixSymbol,
) -> f64 {
    let horizontal = timing_hint
        .0
        .map(|modules| (modules as i32 - symbol.cols as i32).unsigned_abs() as f64)
        .unwrap_or(0.0);
    let vertical = timing_hint
        .1
        .map(|modules| (modules as i32 - symbol.rows as i32).unsigned_abs() as f64)
        .unwrap_or(0.0);
    ((horizontal + vertical) * 0.018).min(0.46)
}

fn estimate_edge_timing_modules(
    bin: &BinaryImage,
    edge_start: (f64, f64),
    edge_end: (f64, f64),
    inward_start: (f64, f64),
    inward_end: (f64, f64),
) -> Option<usize> {
    let edge_len = distance(edge_start, edge_end);
    if edge_len < 16.0 {
        return None;
    }
    let inward = (
        (inward_start.0 + inward_end.0 - edge_start.0 - edge_end.0) * 0.5,
        (inward_start.1 + inward_end.1 - edge_start.1 - edge_end.1) * 0.5,
    );
    let inward_len = inward.0.hypot(inward.1);
    if inward_len <= f64::EPSILON {
        return None;
    }
    let inset = (edge_len / 48.0).clamp(1.0, 8.0);
    let offset = (inward.0 / inward_len * inset, inward.1 / inward_len * inset);
    let samples = edge_len.round().clamp(32.0, 768.0) as usize;
    let min_run = (samples / 220).max(2);
    let runs = binary_run_count(
        (0..samples).map(|idx| {
            let t = (idx as f64 + 0.5) / samples as f64;
            let x = edge_start.0 + (edge_end.0 - edge_start.0) * t + offset.0;
            let y = edge_start.1 + (edge_end.1 - edge_start.1) * t + offset.1;
            bin.is_black(x.round() as i32, y.round() as i32)
        }),
        min_run,
    );
    (6..=160).contains(&runs).then_some(runs)
}

#[derive(Default)]
struct GridScore {
    correct: f64,
    total: f64,
}

impl GridScore {
    fn add<F>(
        &mut self,
        vote: &mut F,
        x: usize,
        y: usize,
        expected: bool,
        grid: DataMatrixSamplingGrid,
        weight: f64,
    ) where
        F: FnMut(usize, usize, DataMatrixSamplingGrid) -> (usize, usize),
    {
        let (black, total) = vote(x, y, grid);
        let actual = score_vote_is_black(black, total);
        if actual == expected {
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

fn module_vote_is_black(black: usize, total: usize) -> bool {
    black * 2 >= total
}

fn score_vote_is_black(black: usize, total: usize) -> bool {
    black * 2 + 1 >= total
}

fn score_sampling_contrast(
    warped: &BinaryImage,
    symbol: DataMatrixSymbol,
    grid: DataMatrixSamplingGrid,
) -> f64 {
    let step = (symbol.rows.max(symbol.cols) / 48).max(1);
    let mut total = 0.0;
    let mut count = 0_usize;

    for y in (0..symbol.rows).step_by(step) {
        for x in (0..symbol.cols).step_by(step) {
            let (black, samples) = warped_module_vote(warped, symbol, x, y, grid);
            let white = samples - black;
            total += black.max(white) as f64 / samples.max(1) as f64;
            count += 1;
        }
    }

    total / count.max(1) as f64
}

#[derive(Clone, Copy, Debug)]
struct DataMatrixTimingEstimate {
    horizontal: Option<usize>,
    vertical: Option<usize>,
    solid_cols: Option<usize>,
    solid_rows: Option<usize>,
}

impl DataMatrixTimingEstimate {
    fn from_warped(warped: &BinaryImage) -> Self {
        Self {
            horizontal: estimate_horizontal_timing_modules(warped),
            vertical: estimate_vertical_timing_modules(warped),
            solid_cols: estimate_modules_from_left_solid_border(warped),
            solid_rows: estimate_modules_from_bottom_solid_border(warped),
        }
    }

    fn size_penalty(self, symbol: DataMatrixSymbol) -> f64 {
        let horizontal = self
            .horizontal
            .map(|modules| (modules as i32 - symbol.cols as i32).unsigned_abs() as f64)
            .unwrap_or(0.0);
        let vertical = self
            .vertical
            .map(|modules| (modules as i32 - symbol.rows as i32).unsigned_abs() as f64)
            .unwrap_or(0.0);
        let solid_cols = self
            .solid_cols
            .map(|modules| (modules as i32 - symbol.cols as i32).unsigned_abs() as f64)
            .unwrap_or(0.0);
        let solid_rows = self
            .solid_rows
            .map(|modules| (modules as i32 - symbol.rows as i32).unsigned_abs() as f64)
            .unwrap_or(0.0);
        ((horizontal + vertical) * 0.014 + (solid_cols + solid_rows) * 0.026).min(0.58)
    }
}

fn estimate_horizontal_timing_modules(warped: &BinaryImage) -> Option<usize> {
    if warped.w < 16 || warped.h < 16 {
        return None;
    }
    let y_limit = ((warped.h as usize + 4) / 5).clamp(4, 96);
    let min_run = (warped.w as usize / 220).max(2);
    let mut best = 0;
    for y in 0..y_limit.min(warped.h as usize) {
        let runs = binary_run_count(
            (0..warped.w as i32).map(|x| warped.is_black(x, y as i32)),
            min_run,
        );
        if (6..=160).contains(&runs) {
            best = best.max(runs);
        }
    }
    (best > 0).then_some(best)
}

fn estimate_vertical_timing_modules(warped: &BinaryImage) -> Option<usize> {
    if warped.w < 16 || warped.h < 16 {
        return None;
    }
    let x_start = (warped.w as usize * 4 / 5).min(warped.w.saturating_sub(1) as usize);
    let min_run = (warped.h as usize / 220).max(2);
    let mut best = 0;
    for x in x_start..warped.w as usize {
        let runs = binary_run_count(
            (0..warped.h as i32).map(|y| warped.is_black(x as i32, y)),
            min_run,
        );
        if (6..=160).contains(&runs) {
            best = best.max(runs);
        }
    }
    (best > 0).then_some(best)
}

fn estimate_modules_from_left_solid_border(warped: &BinaryImage) -> Option<usize> {
    if warped.w < 16 || warped.h < 16 {
        return None;
    }
    let y_start = warped.h as usize / 12;
    let y_end = warped.h as usize * 11 / 12;
    let step = (warped.h as usize / 80).max(1);
    let max_run = (warped.w as usize / 3).max(2);
    let mut runs = Vec::new();
    for y in (y_start..y_end.max(y_start + 1)).step_by(step) {
        let mut run = 0_usize;
        while run < warped.w as usize && warped.is_black(run as i32, y as i32) {
            run += 1;
        }
        if (2..=max_run).contains(&run) {
            runs.push(run);
        }
    }
    modules_from_border_runs(warped.w as usize, &mut runs)
}

fn estimate_modules_from_bottom_solid_border(warped: &BinaryImage) -> Option<usize> {
    if warped.w < 16 || warped.h < 16 {
        return None;
    }
    let x_start = warped.w as usize / 12;
    let x_end = warped.w as usize * 11 / 12;
    let step = (warped.w as usize / 80).max(1);
    let max_run = (warped.h as usize / 3).max(2);
    let mut runs = Vec::new();
    for x in (x_start..x_end.max(x_start + 1)).step_by(step) {
        let mut run = 0_usize;
        while run < warped.h as usize
            && warped.is_black(x as i32, warped.h.saturating_sub(1) as i32 - run as i32)
        {
            run += 1;
        }
        if (2..=max_run).contains(&run) {
            runs.push(run);
        }
    }
    modules_from_border_runs(warped.h as usize, &mut runs)
}

fn modules_from_border_runs(side: usize, runs: &mut [usize]) -> Option<usize> {
    if runs.len() < 4 {
        return None;
    }
    runs.sort_unstable();
    let thickness = runs[runs.len() / 2].max(1);
    let modules = (side as f64 / thickness as f64).round() as usize;
    (6..=160).contains(&modules).then_some(modules)
}

fn binary_run_count<I>(values: I, min_run: usize) -> usize
where
    I: IntoIterator<Item = bool>,
{
    let mut runs: Vec<(bool, usize)> = Vec::new();
    for value in values {
        match runs.last_mut() {
            Some((last, len)) if *last == value => *len += 1,
            _ => runs.push((value, 1)),
        }
    }

    let mut filtered: Vec<bool> = Vec::new();
    for (value, len) in runs {
        if len < min_run {
            continue;
        }
        if filtered.last().copied() != Some(value) {
            filtered.push(value);
        }
    }
    filtered.len()
}

fn warped_module_vote(
    warped: &BinaryImage,
    symbol: DataMatrixSymbol,
    x: usize,
    y: usize,
    grid: DataMatrixSamplingGrid,
) -> (usize, usize) {
    if symbol.cols == 0 || symbol.rows == 0 || warped.w == 0 || warped.h == 0 {
        return (0, 1);
    }

    let cell_w = warped.w as f64 / symbol.cols as f64 * grid.scale_x;
    let cell_h = warped.h as f64 / symbol.rows as f64 * grid.scale_y;
    let center_x =
        warped.w as f64 * 0.5 + (x as f64 + 0.5 - symbol.cols as f64 * 0.5 + grid.shift_x) * cell_w;
    let center_y =
        warped.h as f64 * 0.5 + (y as f64 + 0.5 - symbol.rows as f64 * 0.5 + grid.shift_y) * cell_h;
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

fn mapped_module_vote(
    bin: &BinaryImage,
    module_to_source: &nalgebra::Matrix3<f64>,
    symbol: DataMatrixSymbol,
    x: usize,
    y: usize,
    grid: DataMatrixSamplingGrid,
) -> (usize, usize) {
    if symbol.cols == 0 || symbol.rows == 0 {
        return (0, 1);
    }

    let center_x = symbol.cols as f64 * 0.5
        + (x as f64 + 0.5 - symbol.cols as f64 * 0.5 + grid.shift_x) * grid.scale_x;
    let center_y = symbol.rows as f64 * 0.5
        + (y as f64 + 0.5 - symbol.rows as f64 * 0.5 + grid.shift_y) * grid.scale_y;
    let offsets = [-0.18, 0.0, 0.18];
    let mut black = 0;
    let mut total = 0;

    for oy in offsets {
        for ox in offsets {
            let px = center_x + ox * grid.scale_x;
            let py = center_y + oy * grid.scale_y;
            if sample_mapped_binary(bin, module_to_source, px, py) {
                black += 1;
            }
            total += 1;
        }
    }

    (black, total)
}

fn sample_mapped_binary(
    bin: &BinaryImage,
    module_to_source: &nalgebra::Matrix3<f64>,
    x: f64,
    y: f64,
) -> bool {
    let p = module_to_source * Vector3::new(x, y, 1.0);
    if p.z.abs() <= f64::EPSILON {
        return false;
    }
    let sx = p.x / p.z;
    let sy = p.y / p.z;
    if sx < 0.0
        || sy < 0.0
        || sx > bin.w.saturating_sub(1) as f64
        || sy > bin.h.saturating_sub(1) as f64
    {
        return false;
    }
    bin.is_black(sx.round() as i32, sy.round() as i32)
}

fn image_aspect_penalty(actual_ratio: f64, symbol: DataMatrixSymbol) -> f64 {
    let expected = symbol.cols as f64 / symbol.rows as f64;
    ((actual_ratio / expected).ln().abs() * 0.70).min(0.75)
}

fn corner_aspect_penalty(corners: &[(f64, f64); 4], symbol: DataMatrixSymbol) -> f64 {
    let top = distance(corners[0], corners[1]);
    let bottom = distance(corners[2], corners[3]);
    let left = distance(corners[0], corners[2]);
    let right = distance(corners[1], corners[3]);
    let width = (top + bottom) * 0.5;
    let height = (left + right) * 0.5;
    if height <= f64::EPSILON {
        return 0.75;
    }
    image_aspect_penalty(width / height, symbol)
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampling_recovers_synthetic_square_symbol() {
        let symbol = DATA_MATRIX_SYMBOLS
            .iter()
            .copied()
            .find(|symbol| symbol.rows == 24 && symbol.cols == 24)
            .unwrap();
        let original = synthetic_matrix(symbol);
        let image = render_matrix(&original, 9);
        let sampled = sample_data_matrix_grid(&image).unwrap();
        assert_eq!(sampled.rows, symbol.rows);
        assert_eq!(sampled.cols, symbol.cols);
        assert_eq!(sampled.matrix, original);
    }

    #[test]
    fn sampling_recovers_synthetic_rectangular_symbol() {
        let symbol = DATA_MATRIX_SYMBOLS
            .iter()
            .copied()
            .find(|symbol| symbol.rows == 16 && symbol.cols == 48)
            .unwrap();
        let original = synthetic_matrix(symbol);
        let image = render_matrix(&original, 8);
        let sampled = sample_data_matrix_grid(&image).unwrap();
        assert_eq!(sampled.rows, symbol.rows);
        assert_eq!(sampled.cols, symbol.cols);
        assert_eq!(sampled.matrix, original);
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
        let mut data = vec![255; rows * cols * scale as usize * scale as usize];
        let width = cols as u32 * scale;
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
        BinaryImage::new(width, rows as u32 * scale, data)
    }
}
