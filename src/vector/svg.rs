use crate::codec::qr::QrMatrix;
#[cfg(test)]
use crate::pipeline::preprocess::BinaryImage;

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
}
