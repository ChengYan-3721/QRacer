use image::{DynamicImage, GrayImage, Luma};

/// Binary image used by the detection and geometry pipeline.
///
/// Pixels are stored as 0 for black foreground and 255 for white background.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryImage {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

impl BinaryImage {
    /// Creates a binary image from raw 0/255 bytes.
    pub fn new(w: u32, h: u32, data: Vec<u8>) -> Self {
        debug_assert_eq!(data.len(), (w as usize) * (h as usize));
        Self { w, h, data }
    }

    /// Returns the pixel at `(x, y)`, or white for out-of-bounds coordinates.
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return 255;
        }
        self.data[y as usize * self.w as usize + x as usize]
    }

    /// Returns true when the pixel at `(x, y)` is black.
    pub fn is_black(&self, x: i32, y: i32) -> bool {
        self.get(x, y) < 128
    }

    /// Converts this binary image back to an image crate grayscale image.
    pub fn to_gray_image(&self) -> GrayImage {
        GrayImage::from_raw(self.w, self.h, self.data.clone())
            .expect("BinaryImage dimensions match its buffer")
    }

    /// Converts this binary image to a DynamicImage for GUI preview.
    pub fn to_dynamic_image(&self) -> DynamicImage {
        DynamicImage::ImageLuma8(self.to_gray_image())
    }
}

/// Converts an input image to grayscale, thresholds it with Otsu, and removes
/// one-pixel binary speckles with a 3x3 open/close pass.
pub fn preprocess(img: &DynamicImage) -> BinaryImage {
    let gray = img.to_luma8();
    let binary = otsu_binarize(&gray);
    let opened = morph_open_black(&binary);
    let cleaned = morph_close_black(&opened);

    BinaryImage::new(cleaned.width(), cleaned.height(), cleaned.into_raw())
}

/// Applies only Otsu binarization and keeps black as 0, white as 255.
pub fn otsu_binarize(gray: &GrayImage) -> GrayImage {
    let threshold = imageproc::contrast::otsu_level(gray);
    let mut out = GrayImage::new(gray.width(), gray.height());

    for (x, y, pixel) in gray.enumerate_pixels() {
        let value = if pixel[0] <= threshold { 0 } else { 255 };
        out.put_pixel(x, y, Luma([value]));
    }

    out
}

fn morph_open_black(src: &GrayImage) -> GrayImage {
    dilate_black(&erode_black(src))
}

fn morph_close_black(src: &GrayImage) -> GrayImage {
    erode_black(&dilate_black(src))
}

fn erode_black(src: &GrayImage) -> GrayImage {
    let mut out = GrayImage::from_pixel(src.width(), src.height(), Luma([255]));

    for y in 0..src.height() as i32 {
        for x in 0..src.width() as i32 {
            let mut all_black = true;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if sample_gray(src, x + dx, y + dy) >= 128 {
                        all_black = false;
                    }
                }
            }
            if all_black {
                out.put_pixel(x as u32, y as u32, Luma([0]));
            }
        }
    }

    out
}

fn dilate_black(src: &GrayImage) -> GrayImage {
    let mut out = GrayImage::from_pixel(src.width(), src.height(), Luma([255]));

    for y in 0..src.height() as i32 {
        for x in 0..src.width() as i32 {
            let mut any_black = false;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if sample_gray(src, x + dx, y + dy) < 128 {
                        any_black = true;
                    }
                }
            }
            if any_black {
                out.put_pixel(x as u32, y as u32, Luma([0]));
            }
        }
    }

    out
}

fn sample_gray(src: &GrayImage, x: i32, y: i32) -> u8 {
    if x < 0 || y < 0 || x >= src.width() as i32 || y >= src.height() as i32 {
        return 255;
    }
    src.get_pixel(x as u32, y as u32)[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Luma};
    use qrcodegen::{QrCode, QrCodeEcc};

    #[test]
    fn otsu_separates_pure_bimodal() {
        let mut gray = GrayImage::new(20, 10);
        for y in 0..10 {
            for x in 0..20 {
                let value = if x < 10 { 0 } else { 255 };
                gray.put_pixel(x, y, Luma([value]));
            }
        }

        let binary = otsu_binarize(&gray);
        assert_eq!(binary.get_pixel(0, 0)[0], 0);
        assert_eq!(binary.get_pixel(19, 0)[0], 255);
    }

    #[test]
    fn preprocess_keeps_qr_modules() {
        let qr = QrCode::encode_text("qracer phase 2", QrCodeEcc::Medium).unwrap();
        let img = render_qr(&qr, 6, 4);
        let bin = preprocess(&DynamicImage::ImageLuma8(img));

        let black = bin.data.iter().filter(|&&px| px == 0).count();
        assert!(black > 1000);
    }

    fn render_qr(qr: &QrCode, scale: u32, border: u32) -> GrayImage {
        let size = qr.size() as u32;
        let image_size = (size + border * 2) * scale;
        let mut img: GrayImage = ImageBuffer::from_pixel(image_size, image_size, Luma([255]));

        for y in 0..size {
            for x in 0..size {
                if qr.get_module(x as i32, y as i32) {
                    let start_x = (x + border) * scale;
                    let start_y = (y + border) * scale;
                    for yy in start_y..start_y + scale {
                        for xx in start_x..start_x + scale {
                            img.put_pixel(xx, yy, Luma([0]));
                        }
                    }
                }
            }
        }

        img
    }
}
