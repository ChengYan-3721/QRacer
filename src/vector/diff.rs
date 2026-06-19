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
