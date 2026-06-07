// 图像 IO + egui 纹理转换辅助。
//
// 阶段 1 范围：
//   - 从系统剪贴板读取图像（arboard）
//   - 从文件对话框选择图像文件（rfd）
//   - 把 image::DynamicImage 转成 egui::ColorImage 给 GUI 显示

use anyhow::{Context, Result, anyhow};
use image::DynamicImage;
use std::path::Path;

/// 从剪贴板读取图像。
///
/// arboard 的 get_image() 返回 RGBA 原始像素 + 宽高。
/// 这里把它包装成 image::DynamicImage，统一项目内部图像类型。
pub fn read_clipboard_image() -> Result<DynamicImage> {
    let mut cb = arboard::Clipboard::new().context("无法访问系统剪贴板")?;
    let img = cb.get_image().context("剪贴板内没有图像")?;

    // arboard 给的是 RGBA8，bytes 是 Cow<[u8]>，长度应等于 width*height*4
    let w = img.width as u32;
    let h = img.height as u32;
    let buf = image::RgbaImage::from_raw(w, h, img.bytes.into_owned())
        .ok_or_else(|| anyhow!("剪贴板图像数据长度异常"))?;

    Ok(DynamicImage::ImageRgba8(buf))
}

/// 弹文件对话框，让用户选一张图。返回 Ok(None) 表示用户取消。
pub fn open_image_dialog() -> Result<Option<DynamicImage>> {
    let file = rfd::FileDialog::new()
        .add_filter("图像", &["png", "jpg", "jpeg", "bmp", "webp"])
        .add_filter("所有文件", &["*"])
        .set_title("选择二维码 / 小程序码 / 抖音码截图")
        .pick_file();

    let Some(path) = file else {
        return Ok(None);
    };

    let img = load_image_from_path(&path)?;
    Ok(Some(img))
}

fn load_image_from_path(path: &Path) -> Result<DynamicImage> {
    let bytes =
        std::fs::read(path).with_context(|| format!("无法读取图像文件：{}", path.display()))?;

    match image::load_from_memory(&bytes) {
        Ok(img) => Ok(img),
        Err(content_error) => image::open(path).with_context(|| {
            format!(
                "无法打开图像文件：{}（按文件头识别失败：{}）",
                path.display(),
                content_error
            )
        }),
    }
}

/// 把 image::DynamicImage 转为 egui::ColorImage（egui 的 CPU 端图像表示）。
///
/// 后续可以用 ctx.load_texture() 把它上传成 TextureHandle 显示。
/// egui 期望 RGBA8 unmultiplied。
pub fn to_color_image(img: &DynamicImage) -> egui::ColorImage {
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, ImageFormat, Rgba};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn load_image_from_path_uses_file_header_before_extension() {
        let path = temp_image_path("jpg");
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_fn(3, 2, |x, y| {
            Rgba([(x * 40) as u8, (y * 80) as u8, 128, 255])
        }));
        image
            .save_with_format(&path, ImageFormat::Png)
            .expect("png test image should save with mismatched extension");

        let loaded = load_image_from_path(&path).expect("image should load from file header");

        assert_eq!(loaded.width(), 3);
        assert_eq!(loaded.height(), 2);
        let _ = std::fs::remove_file(path);
    }

    fn temp_image_path(extension: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "qracer-mismatched-image-{}-{nanos}.{extension}",
            std::process::id()
        ))
    }
}
