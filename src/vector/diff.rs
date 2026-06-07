#[cfg(test)]
use std::collections::HashSet;

#[cfg(test)]
use image::{DynamicImage, Rgba, RgbaImage};

use crate::codec::qr::QrMatrix;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffResult {
    pub diff_modules: Vec<(u32, u32)>,
    pub missing_in_generated: Vec<(u32, u32)>,
    pub extra_in_generated: Vec<(u32, u32)>,
    pub diff_count: u32,
}

pub fn compute_matrix_diff(
    reference_matrix: &QrMatrix,
    generated_matrix: &QrMatrix,
) -> Option<DiffResult> {
    let modules = generated_matrix.len();
    if !is_square_matrix(reference_matrix, modules) || !is_square_matrix(generated_matrix, modules)
    {
        return None;
    }

    let mut diff_modules = Vec::new();
    let mut missing_in_generated = Vec::new();
    let mut extra_in_generated = Vec::new();
    for y in 0..modules {
        for x in 0..modules {
            let actual = reference_matrix[y][x];
            let expected = generated_matrix[y][x];
            if actual != expected {
                let module = (x as u32, y as u32);
                diff_modules.push(module);
                if actual {
                    missing_in_generated.push(module);
                } else {
                    extra_in_generated.push(module);
                }
            }
        }
    }

    Some(DiffResult {
        diff_count: diff_modules.len() as u32,
        diff_modules,
        missing_in_generated,
        extra_in_generated,
    })
}

fn is_square_matrix(matrix: &QrMatrix, modules: usize) -> bool {
    matrix.len() == modules && matrix.iter().all(|row| row.len() == modules)
}

#[cfg(test)]
pub fn render_qr_diff_preview(
    matrix: &QrMatrix,
    diff: Option<&DiffResult>,
    show_diff: bool,
    scale: u32,
    border: u32,
) -> DynamicImage {
    let modules = matrix.len() as u32;
    let scale = scale.max(1);
    let image_size = (modules + border * 2).max(1) * scale;
    let mut image = RgbaImage::from_pixel(image_size, image_size, Rgba([255, 255, 255, 255]));
    let missing_set: HashSet<(u32, u32)> = diff
        .filter(|_| show_diff)
        .map(|diff| diff.missing_in_generated.iter().copied().collect())
        .unwrap_or_default();
    let extra_set: HashSet<(u32, u32)> = diff
        .filter(|_| show_diff)
        .map(|diff| diff.extra_in_generated.iter().copied().collect())
        .unwrap_or_default();

    for (module_y, row) in matrix.iter().enumerate() {
        for (module_x, &is_black) in row.iter().enumerate() {
            let module_x = module_x as u32;
            let module_y = module_y as u32;
            let color = if missing_set.contains(&(module_x, module_y)) {
                Rgba([220, 32, 32, 255])
            } else if extra_set.contains(&(module_x, module_y)) {
                Rgba([32, 96, 220, 255])
            } else if is_black {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };

            if color[0] == 255 && color[1] == 255 && color[2] == 255 {
                continue;
            }

            let start_x = (module_x + border) * scale;
            let start_y = (module_y + border) * scale;
            for y in start_y..start_y + scale {
                for x in start_x..start_x + scale {
                    image.put_pixel(x, y, color);
                }
            }
        }
    }

    DynamicImage::ImageRgba8(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_matrix_has_zero_diff() {
        let matrix = vec![
            vec![true, false, true],
            vec![false, true, false],
            vec![true, false, true],
        ];

        let diff = compute_matrix_diff(&matrix, &matrix).unwrap();

        assert_eq!(diff.diff_count, 0);
        assert!(diff.missing_in_generated.is_empty());
        assert!(diff.extra_in_generated.is_empty());
    }

    #[test]
    fn changed_module_is_reported() {
        let matrix = vec![
            vec![true, false, true],
            vec![false, true, false],
            vec![true, false, true],
        ];
        let mut changed = matrix.clone();
        changed[1][1] = false;

        let diff = compute_matrix_diff(&matrix, &changed).unwrap();

        assert_eq!(diff.diff_count, 1);
        assert_eq!(diff.diff_modules, vec![(1, 1)]);
        assert_eq!(diff.missing_in_generated, vec![(1, 1)]);
        assert!(diff.extra_in_generated.is_empty());
    }

    #[test]
    fn matrix_diff_uses_reference_as_original() {
        let reference = vec![vec![true, false], vec![false, true]];
        let generated = vec![vec![false, true], vec![false, true]];

        let diff = compute_matrix_diff(&reference, &generated).unwrap();

        assert_eq!(diff.diff_count, 2);
        assert_eq!(diff.missing_in_generated, vec![(0, 0)]);
        assert_eq!(diff.extra_in_generated, vec![(1, 0)]);
    }

    #[test]
    fn matrix_diff_rejects_dimension_mismatch() {
        let reference = vec![vec![true, false]];
        let generated = vec![vec![true, false], vec![false, true]];

        assert!(compute_matrix_diff(&reference, &generated).is_none());
    }

    #[test]
    fn diff_preview_marks_missing_red_and_extra_blue() {
        let matrix = vec![vec![true, false], vec![false, true]];
        let diff = DiffResult {
            diff_modules: vec![(1, 0), (0, 1)],
            missing_in_generated: vec![(1, 0)],
            extra_in_generated: vec![(0, 1)],
            diff_count: 2,
        };
        let image = render_qr_diff_preview(&matrix, Some(&diff), true, 4, 0).to_rgba8();

        assert_eq!(image.get_pixel(5, 1), &Rgba([220, 32, 32, 255]));
        assert_eq!(image.get_pixel(1, 5), &Rgba([32, 96, 220, 255]));
        assert_eq!(image.get_pixel(1, 1), &Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn diff_preview_can_hide_diff_colors() {
        let matrix = vec![vec![true, false]];
        let diff = DiffResult {
            diff_modules: vec![(0, 0)],
            missing_in_generated: vec![(0, 0)],
            extra_in_generated: Vec::new(),
            diff_count: 1,
        };
        let image = render_qr_diff_preview(&matrix, Some(&diff), false, 4, 0).to_rgba8();

        assert_eq!(image.get_pixel(1, 1), &Rgba([0, 0, 0, 255]));
    }
}
