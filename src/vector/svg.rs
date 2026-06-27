use crate::codec::data_matrix_grid::DataMatrixGrid;
use crate::codec::dy_grid::{DyBadgeStyle, DyGrid, RingSpec};
use crate::codec::qr::QrMatrix;
use crate::codec::wx_grid::WxGrid;
use crate::pipeline::preprocess::BinaryImage;
use crate::vector::diff::DiffResult;
use crate::vector::shapes::polar_sector_path;
use image::{DynamicImage, Rgba, RgbaImage};
use std::collections::HashSet;

const DOUYIN_BLACK_FILL: &str = "#000";

struct DouyinLogoLayer {
    path: &'static str,
    fill: &'static str,
}

const DOUYIN_LOGO_REFERENCE_CENTER: (f64, f64) = (564.67, 176.73);
const DOUYIN_LOGO_REFERENCE_RADIUS: f64 = 73.04;
const DOUYIN_LOGO_LAYERS: &[DouyinLogoLayer] = &[
    DouyinLogoLayer {
        path: "M598.54,155.89c-9.42,0-17.06-7.64-17.06-17.06h-14.21v52.59c0,5.5-4.45,9.95-9.95,9.95s-9.95-4.45-9.95-9.95,4.45-9.95,9.95-9.95h4.26v-14.21h-4.26c-13.35,0-24.16,10.82-24.16,24.16s10.82,24.16,24.16,24.16,24.16-10.82,24.16-24.16v-26.38c5.07,3.31,11,5.07,17.06,5.06h1.42v-14.21h-1.42Z",
        fill: "#fa1e5c",
    },
    DouyinLogoLayer {
        path: "M595.7,153.04c-9.42,0-17.06-7.64-17.06-17.06h-14.21v52.59c0,5.5-4.45,9.95-9.95,9.95s-9.95-4.45-9.95-9.95,4.45-9.95,9.95-9.95h4.26v-14.21h-4.26c-13.35,0-24.16,10.82-24.16,24.16s10.82,24.16,24.16,24.16,24.16-10.82,24.16-24.16v-26.38c5.07,3.31,11,5.07,17.06,5.06h1.42v-14.21h-1.42Z",
        fill: "#5ffdff",
    },
    DouyinLogoLayer {
        path: "M597.12,155.83c-4.71-.39-9.05-2.73-11.98-6.44-3.33-2.62-5.56-6.38-6.27-10.56h-11.61v52.59c0,5.5-4.45,9.95-9.95,9.95-3.35,0-6.47-1.68-8.31-4.48-4.59-3.02-5.86-9.19-2.84-13.78,1.84-2.8,4.96-4.48,8.31-4.48h4.26v-11.37h-1.42c-13.35,0-24.16,10.82-24.17,24.16,0,5.72,2.02,11.25,5.72,15.61,10.19,8.62,25.43,7.35,34.05-2.84,3.69-4.36,5.72-9.89,5.72-15.61v-26.38c5.07,3.31,11,5.07,17.06,5.06h1.42v-11.43Z",
        fill: DOUYIN_BLACK_FILL,
    },
];
const DOUYIN_BULLSEYE_BADGE_PATHS: &[&str] = &[
    "M500.08,86.49c-18.1,0-32.76,14.68-32.76,32.76s14.68,32.76,32.76,32.76,32.76-14.68,32.76-32.76-14.68-32.76-32.76-32.76h0ZM500.08,138.66c-10.71,0-19.4-8.69-19.4-19.4s8.69-19.4,19.4-19.4,19.4,8.69,19.4,19.4-8.69,19.4-19.4,19.4Z",
    "M500.08,167.38c-1.75,0-3.48-.1-5.19-.28-.14,1.39-.28,2.72-.41,3.98,1.84.2,3.7.3,5.59.3,1.78,0,3.55-.09,5.28-.27-.14-1.26-.28-2.59-.43-3.98-1.6.16-3.22.24-4.86.24h.02Z",
    "M454.29,134.05c-1.33.43-2.6.85-3.8,1.25,5.12,15.8,17.59,28.31,33.36,33.49.39-1.21.8-2.48,1.23-3.81-14.56-4.79-26.07-16.34-30.79-30.92h0Z",
    "M514.77,165.08c.43,1.33.85,2.6,1.25,3.8,15.87-5.11,28.44-17.63,33.62-33.47-1.2-.39-2.47-.81-3.8-1.25-4.78,14.64-16.4,26.21-31.06,30.92h-.01Z",
    "M500.08,71.13c1.69,0,3.35.09,4.99.26.15-1.39.29-2.72.43-3.98-1.78-.18-3.59-.28-5.42-.28s-3.69.1-5.5.29c.13,1.26.27,2.59.41,3.98,1.67-.18,3.37-.27,5.09-.27h0Z",
    "M545.86,104.42c1.33-.43,2.6-.84,3.81-1.23-5.14-15.83-17.66-28.36-33.49-33.51-.39,1.2-.81,2.47-1.25,3.8,14.61,4.75,26.18,16.32,30.92,30.94h.01Z",
    "M548.21,119.25c0,1.71-.09,3.4-.27,5.07,1.39.15,2.72.29,3.98.43.19-1.81.29-3.64.29-5.5s-.1-3.62-.28-5.4c-1.26.13-2.59.27-3.98.41.17,1.64.26,3.31.26,4.99h0Z",
    "M485.14,73.51c-.43-1.33-.84-2.6-1.23-3.81-15.76,5.15-28.22,17.62-33.38,33.38,1.21.39,2.48.8,3.81,1.23,4.76-14.53,16.26-26.04,30.8-30.8h0Z",
    "M451.96,119.25c0-1.72.1-3.43.27-5.11-1.39-.14-2.72-.28-3.98-.41-.19,1.81-.29,3.65-.29,5.51s.09,3.61.28,5.38c1.26-.14,2.59-.28,3.98-.43-.17-1.63-.25-3.28-.25-4.96v.02Z",
];
const DOUYIN_BLACK_BORDER_LOCATOR_DISTANCE: f64 = 261.452;
const DOUYIN_BLACK_BORDER_BADGE_OUTER_RADIUS_SCALE: f64 = 1.17;
const DOUYIN_NO_BORDER_LOCATOR_DISTANCE: f64 = 240.529442688416;
// Cell centers land at 1 deg + n * 3 deg in the standard no-border SVG.
const DOUYIN_NO_BORDER_CODE_THETA_OFFSET: f64 = -0.5 * std::f64::consts::PI / 180.0;
const DOUYIN_NO_BORDER_RENDER_RADIUS_OFFSET: f64 = 0.0;
const DOUYIN_NO_BORDER_RENDER_RING_RADIUS_OFFSETS: [f64; 6] = [0.0, 0.125, 0.25, 0.0, 0.0, 0.0];
const DOUYIN_NO_BORDER_RENDER_RING_WIDTH_SCALES: [f64; 6] = [1.00, 1.02, 1.0, 1.0, 0.98, 0.98];
const DOUYIN_NO_BORDER_RING1_LONG_RUN_WIDTH_SCALE: f64 = 1.00;
const DOUYIN_NO_BORDER_MULTI_RUN_RADIAL_OFFSET_SCALES: [f64; 6] =
    [1.05, 0.89, 0.72, 1.00, 1.00, 1.00];
const DOUYIN_NO_BORDER_RUN_TANGENTIAL_OFFSET_SCALES: [f64; 6] = [1.0, 0.75, 1.08, 1.0, 1.0, 1.0];
const DOUYIN_NO_BORDER_SINGLE_DOT_WIDTH_SCALES: [f64; 6] = [0.81, 1.0, 0.81, 1.0, 1.0, 1.0];
const DOUYIN_NO_BORDER_CODE_RUN_WIDTH_SCALE: f64 = 1.04;
const DOUYIN_NO_BORDER_DECORATIVE_RUN_WIDTH_SCALE: f64 = 1.04;
// 装饰环里 run 弧长 ≤ 此倍数 × 线宽即判定为「孤立圆点」（画成 r≈4 小圆），否则为弧段
// （画成圆角条带）。实测干净样本 dot 弧长≈线宽×0.8~1.0、arc 弧长≥线宽×4，间隔极大，
// 1.6 居中可靠区分。参考 samples/无框版1.svg：ring0 7 点 + ring2 2 点皆 len4~5。
const DOUYIN_NO_BORDER_DECORATIVE_DOT_MAX_ARC_SCALE: f64 = 1.6;
const DOUYIN_NO_BORDER_SHORT_RUN_ANGULAR_INSET: f64 = 0.58;
const DOUYIN_NO_BORDER_LONG_RUN_ANGULAR_INSET: f64 = 0.52;
const DOUYIN_NO_BORDER_SHORT_RUN_ANGULAR_INSETS: [f64; 6] = [0.64, 0.28, 0.72, 0.58, 0.58, 0.58];
const DOUYIN_NO_BORDER_LONG_RUN_ANGULAR_INSETS: [f64; 6] = [0.56, 0.28, 0.62, 0.52, 0.52, 0.52];
const DOUYIN_NO_BORDER_STANDARD_RINGS: [(f64, f64, bool); 6] = [
    (228.66, 5.0, true),
    (207.98, 5.0, false),
    (188.59, 5.0, true),
    (171.71, 5.0, false),
    (153.74, 5.0, false),
    (133.24, 5.0, false),
];
const DOUYIN_NO_BORDER_LOGO_PATH: &str = "M504.41,111.46c-5.6-.45-10.01-5.14-10.01-10.85h0s-10.07,0-10.07,0h0s-1.07,0-1.07,0v34.03c0,3.43-2.78,6.22-6.22,6.22s-6.22-2.78-6.22-6.22,2.78-6.22,6.22-6.22c.21,0,.42.01.63.03v-10.07c-.21,0-.42-.03-.63-.03-8.99,0-16.29,7.29-16.29,16.29s7.29,16.29,16.29,16.29,16.29-7.29,16.29-16.29v-16.86c3.17,2.21,6.97,3.57,11.08,3.75v-10.07Z";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrAppearance {
    Standard,
    Wechat,
    EnterpriseWechat,
    Xiaohongshu,
}

impl QrAppearance {
    pub const ALL: [Self; 4] = [
        Self::Standard,
        Self::Wechat,
        Self::EnterpriseWechat,
        Self::Xiaohongshu,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "标准",
            Self::Wechat => "微信样式",
            Self::EnterpriseWechat => "企业微信样式",
            Self::Xiaohongshu => "小红书样式",
        }
    }
}

const ENTERPRISE_WECHAT_DATA_SIDE_RATIO: f64 = 1.98 / 2.83;
const WECHAT_LOGO_REFERENCE_CENTER: (f64, f64) = (57.10, 57.27);
const WECHAT_LOGO_REFERENCE_BADGE_SIZE: f64 = 25.70;
const WECHAT_DATA_SIDE_RATIO: f64 = 0.766_784_452_296_819_8;
const WECHAT_DATA_RADIUS_RATIO: f64 = 0.123_674_911_660_777_39;
const WECHAT_FINDER_OFFSET_RATIO: f64 = -0.106_007_067_137_809_19;
const WECHAT_FINDER_OUTER_SIZE_RATIO: f64 = 7.003_533_568_904_593;
const WECHAT_FINDER_OUTER_RADIUS_RATIO: f64 = 0.713_780_918_727_915_2;
const WECHAT_FINDER_HOLE_OFFSET_RATIO: f64 = 0.858_657_243_816_254_5;
const WECHAT_FINDER_HOLE_SIZE_RATIO: f64 = 5.070_671_378_091_873;
const WECHAT_FINDER_HOLE_RADIUS_RATIO: f64 = 0.363_957_597_173_144_9;
const WECHAT_FINDER_INNER_OFFSET_RATIO: f64 = 1.886_925_795_053_003_5;
const WECHAT_FINDER_INNER_SIZE_RATIO: f64 = 3.014_134_275_618_375;
const WECHAT_FINDER_INNER_RADIUS_RATIO: f64 = 0.392_226_148_409_893_94;
const WECHAT_BADGE_WHITE_SIZE_QR_RATIO: f64 = 0.297_774_806_608_729;
const WECHAT_BADGE_WHITE_CENTER_OFFSET_MODULES: f64 = 0.030_035_335_689_043_7;
const WECHAT_LOGO_SIZE_QR_RATIO: f64 = 0.245_439_786_075_828;
const WECHAT_LOGO_CENTER_OFFSET_X_MODULES: f64 = -0.083_038_869_257_952_8;
const WECHAT_LOGO_CENTER_OFFSET_Y_MODULES: f64 = -0.079_505_300_353_357_4;
const WECHAT_LOGO_PATHS: &[&str] = &[
    "M58.79,57.43c-.34,0-.61.27-.61.61s.27.61.61.61.61-.27.61-.61-.27-.61-.61-.61Z",
    "M52.58,53c-.4,0-.73.33-.73.73s.33.73.73.73.73-.33.73-.73-.33-.73-.73-.73Z",
    "M56.79,54.46c.4,0,.73-.33.73-.73s-.33-.73-.73-.73-.73.33-.73.73.33.73.73.73Z",
    "M66.85,44.42h-19.5c-1.71,0-3.1,1.39-3.1,3.1v19.5c0,1.71,1.39,3.1,3.1,3.1h19.5c1.71,0,3.1-1.39,3.1-3.1v-19.5c0-1.71-1.39-3.1-3.1-3.1ZM54.66,60.75c-.79,0-1.54-.13-2.23-.35l-2.08,1.19.36-2.03c-1.42-.98-2.32-2.47-2.32-4.14,0-2.94,2.81-5.33,6.28-5.33,3.15,0,5.74,1.97,6.2,4.53-.11,0-.22-.01-.33-.01-3.15,0-5.7,2.17-5.7,4.84,0,.45.08.87.21,1.28-.12,0-.25.02-.37.02ZM63.91,62.82l.19,1.81-1.6-1.07c-.61.21-1.28.33-1.98.33-2.89,0-5.24-1.99-5.24-4.45s2.35-4.45,5.24-4.45,5.24,1.99,5.24,4.45c0,1.36-.73,2.55-1.86,3.37Z",
    "M62.3,57.43c-.34,0-.61.27-.61.61s.27.61.61.61.61-.27.61-.61-.27-.61-.61-.61Z",
];

#[derive(Debug, Clone, Copy)]
enum DouyinBlackBorderStaticMarks {
    DouyinLogo,
    Bullseye,
}

#[derive(Debug, Clone, Copy)]
struct DouyinBlackBorderBadgeGeometry {
    cx: f64,
    cy: f64,
    inner_radius: f64,
    outer_radius: f64,
}

#[derive(Debug, Clone, Copy)]
struct DouyinBlackBorderLayout {
    viewbox: (f64, f64),
    center: (f64, f64),
    code_theta_offset: f64,
    static_marks: DouyinBlackBorderStaticMarks,
    badge: DouyinBlackBorderBadgeGeometry,
    locators: [(f64, f64); 3],
    black_fill: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct DouyinNoBorderBadgeGeometry {
    cx: f64,
    cy: f64,
    outer_radius: f64,
    inner_radius: f64,
}

#[derive(Debug, Clone, Copy)]
struct DouyinNoBorderLayout {
    viewbox: (f64, f64),
    center: (f64, f64),
    code_theta_offset: f64,
    badge: DouyinNoBorderBadgeGeometry,
    locators: [(f64, f64); 3],
    locator_radii: (f64, f64, f64),
    black_fill: &'static str,
}

const DOUYIN_BLACK_BORDER_72_LAYOUT: DouyinBlackBorderLayout = DouyinBlackBorderLayout {
    viewbox: (742.05, 742.05),
    center: (371.02, 371.02),
    code_theta_offset: 5.0 * std::f64::consts::PI / 180.0,
    static_marks: DouyinBlackBorderStaticMarks::DouyinLogo,
    badge: DouyinBlackBorderBadgeGeometry {
        cx: 564.67,
        cy: 176.84,
        inner_radius: 73.04,
        outer_radius: 85.35,
    },
    locators: [(186.18, 186.16), (186.17, 555.87), (555.88, 555.87)],
    black_fill: DOUYIN_BLACK_FILL,
};

const DOUYIN_BLACK_BORDER_120_LAYOUT: DouyinBlackBorderLayout = DouyinBlackBorderLayout {
    viewbox: (715.47, 715.47),
    center: (366.24, 352.40),
    code_theta_offset: 3.0 * std::f64::consts::PI / 180.0,
    static_marks: DouyinBlackBorderStaticMarks::DouyinLogo,
    badge: DouyinBlackBorderBadgeGeometry {
        cx: 559.89,
        cy: 158.22,
        inner_radius: 73.04,
        outer_radius: 85.35,
    },
    locators: [(181.40, 167.54), (181.39, 537.25), (551.10, 537.25)],
    black_fill: DOUYIN_BLACK_FILL,
};

const DOUYIN_BLACK_BORDER_BULLSEYE_BADGE_LAYOUT: DouyinBlackBorderLayout =
    DouyinBlackBorderLayout {
        viewbox: (626.65, 628.84),
        center: (306.45, 314.43),
        code_theta_offset: 2.5 * std::f64::consts::PI / 180.0,
        static_marks: DouyinBlackBorderStaticMarks::Bullseye,
        badge: DouyinBlackBorderBadgeGeometry {
            cx: 500.09,
            cy: 119.25,
            inner_radius: 73.04,
            outer_radius: 85.35,
        },
        locators: [(121.60, 128.57), (121.59, 498.28), (491.30, 498.28)],
        black_fill: DOUYIN_BLACK_FILL,
    };

const DOUYIN_NO_BORDER_LAYOUT: DouyinNoBorderLayout = DouyinNoBorderLayout {
    viewbox: (607.34, 615.94),
    center: (304.32, 307.63),
    code_theta_offset: DOUYIN_NO_BORDER_CODE_THETA_OFFSET,
    badge: DouyinNoBorderBadgeGeometry {
        cx: 483.49,
        cy: 128.31,
        outer_radius: 58.02,
        inner_radius: 47.34,
    },
    locators: [(134.24, 137.55), (134.24, 477.71), (474.40, 477.71)],
    locator_radii: (29.01, 18.43, 8.13),
    black_fill: DOUYIN_BLACK_FILL,
};

pub fn qr_matrix_to_svg(matrix: &QrMatrix, module_mm: f64) -> String {
    qr_matrix_to_svg_with_appearance(matrix, module_mm, QrAppearance::Standard)
}

pub fn qr_matrix_to_svg_with_appearance(
    matrix: &QrMatrix,
    module_mm: f64,
    appearance: QrAppearance,
) -> String {
    match appearance {
        QrAppearance::Standard => qr_matrix_to_standard_svg(matrix, module_mm),
        QrAppearance::Wechat | QrAppearance::Xiaohongshu | QrAppearance::EnterpriseWechat => {
            qr_matrix_to_styled_svg(matrix, module_mm, appearance)
        }
    }
}

fn qr_matrix_to_standard_svg(matrix: &QrMatrix, module_mm: f64) -> String {
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
        for (x, _) in row.iter().enumerate() {
            if standard_qr_module_is_black(matrix, size, x, y) {
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

fn qr_matrix_to_styled_svg(matrix: &QrMatrix, module_mm: f64, appearance: QrAppearance) -> String {
    let size = matrix.len();
    let module_mm = module_mm.max(0.01);
    let canvas = size as f64 * module_mm;
    let view_min = if appearance == QrAppearance::Wechat {
        module_mm * WECHAT_FINDER_OFFSET_RATIO
    } else {
        0.0
    };
    let view_size = canvas - view_min;
    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{view_size:.3}mm" height="{view_size:.3}mm" viewBox="{view_min:.3} {view_min:.3} {view_size:.3} {view_size:.3}" shape-rendering="geometricPrecision">"#
    ));
    svg.push_str(&format!(
        r##"<rect x="{view_min:.3}" y="{view_min:.3}" width="{view_size:.3}" height="{view_size:.3}" fill="#fff"/>"##
    ));

    for (y, row) in matrix.iter().enumerate() {
        for (x, &is_black) in row.iter().enumerate() {
            if !is_black || styled_qr_matrix_module_is_reserved(size, x, y, appearance) {
                continue;
            }
            push_qr_styled_module(&mut svg, x, y, module_mm, appearance);
        }
    }

    if size >= 7 {
        for (x, y) in qr_finder_origins(size) {
            match appearance {
                QrAppearance::Wechat => push_wechat_qr_finder(
                    &mut svg,
                    x as f64 * module_mm,
                    y as f64 * module_mm,
                    module_mm,
                ),
                QrAppearance::Xiaohongshu => push_xiaohongshu_qr_finder(
                    &mut svg,
                    x as f64 * module_mm,
                    y as f64 * module_mm,
                    module_mm,
                ),
                QrAppearance::Standard | QrAppearance::EnterpriseWechat => {}
            }
        }
    }

    if appearance == QrAppearance::EnterpriseWechat {
        push_enterprise_wechat_qr_fixed_modules(&mut svg, size, module_mm);
    }

    if appearance == QrAppearance::Wechat && size >= 21 {
        push_wechat_qr_badge(&mut svg, size as f64 * module_mm, module_mm);
    }

    svg.push_str("</svg>");
    svg
}

fn push_qr_styled_module(
    svg: &mut String,
    x: usize,
    y: usize,
    module: f64,
    appearance: QrAppearance,
) {
    let cx = (x as f64 + 0.5) * module;
    let cy = (y as f64 + 0.5) * module;
    match appearance {
        QrAppearance::Wechat => {
            let side = module * WECHAT_DATA_SIDE_RATIO;
            let radius = module * WECHAT_DATA_RADIUS_RATIO;
            svg.push_str(&format!(
                r##"<rect x="{:.3}" y="{:.3}" width="{side:.3}" height="{side:.3}" rx="{radius:.3}" ry="{radius:.3}" fill="#000"/>"##,
                cx - side * 0.5,
                cy - side * 0.5,
            ));
        }
        QrAppearance::Xiaohongshu => {
            let radius = module * 0.31;
            svg.push_str(&format!(
                r##"<circle cx="{cx:.3}" cy="{cy:.3}" r="{radius:.3}" fill="#000"/>"##
            ));
        }
        QrAppearance::EnterpriseWechat => {
            let side = module * ENTERPRISE_WECHAT_DATA_SIDE_RATIO;
            svg.push_str(&format!(
                r##"<rect x="{:.3}" y="{:.3}" width="{side:.3}" height="{side:.3}" fill="#000"/>"##,
                cx - side * 0.5,
                cy - side * 0.5,
            ));
        }
        QrAppearance::Standard => {}
    }
}

fn push_enterprise_wechat_qr_fixed_modules(svg: &mut String, size: usize, module: f64) {
    for y in 0..size {
        for x in 0..size {
            if !enterprise_wechat_fixed_module_is_black(size, x, y) {
                continue;
            }
            let px = x as f64 * module;
            let py = y as f64 * module;
            svg.push_str(&format!(
                r##"<rect x="{px:.3}" y="{py:.3}" width="{module:.3}" height="{module:.3}" fill="#000"/>"##
            ));
        }
    }
}

fn push_wechat_qr_finder(svg: &mut String, x: f64, y: f64, module: f64) {
    let outer_x = x + module * WECHAT_FINDER_OFFSET_RATIO;
    let outer_y = y + module * WECHAT_FINDER_OFFSET_RATIO;
    let outer = module * WECHAT_FINDER_OUTER_SIZE_RATIO;
    let outer_radius = module * WECHAT_FINDER_OUTER_RADIUS_RATIO;
    svg.push_str(&format!(
        r##"<rect x="{outer_x:.3}" y="{outer_y:.3}" width="{outer:.3}" height="{outer:.3}" rx="{outer_radius:.3}" ry="{outer_radius:.3}" fill="#000"/>"##
    ));
    svg.push_str(&format!(
        r##"<rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" rx="{:.3}" ry="{:.3}" fill="#fff"/>"##,
        x + module * WECHAT_FINDER_HOLE_OFFSET_RATIO,
        y + module * WECHAT_FINDER_HOLE_OFFSET_RATIO,
        module * WECHAT_FINDER_HOLE_SIZE_RATIO,
        module * WECHAT_FINDER_HOLE_SIZE_RATIO,
        module * WECHAT_FINDER_HOLE_RADIUS_RATIO,
        module * WECHAT_FINDER_HOLE_RADIUS_RATIO,
    ));
    svg.push_str(&format!(
        r##"<rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" rx="{:.3}" ry="{:.3}" fill="#000"/>"##,
        x + module * WECHAT_FINDER_INNER_OFFSET_RATIO,
        y + module * WECHAT_FINDER_INNER_OFFSET_RATIO,
        module * WECHAT_FINDER_INNER_SIZE_RATIO,
        module * WECHAT_FINDER_INNER_SIZE_RATIO,
        module * WECHAT_FINDER_INNER_RADIUS_RATIO,
        module * WECHAT_FINDER_INNER_RADIUS_RATIO,
    ));
}

fn push_xiaohongshu_qr_finder(svg: &mut String, x: f64, y: f64, module: f64) {
    let cx = x + module * 3.5;
    let cy = y + module * 3.5;
    svg.push_str(&format!(
        r##"<circle cx="{cx:.3}" cy="{cy:.3}" r="{:.3}" fill="#000"/>"##,
        module * 3.50,
    ));
    svg.push_str(&format!(
        r##"<circle cx="{cx:.3}" cy="{cy:.3}" r="{:.3}" fill="#fff"/>"##,
        module * 2.50,
    ));
    svg.push_str(&format!(
        r##"<circle cx="{cx:.3}" cy="{cy:.3}" r="{:.3}" fill="#000"/>"##,
        module * 1.50,
    ));
}

fn push_wechat_qr_badge(svg: &mut String, canvas: f64, module: f64) {
    let center = canvas * 0.5;
    let white_center = center + module * WECHAT_BADGE_WHITE_CENTER_OFFSET_MODULES;
    let logo_center_x = center + module * WECHAT_LOGO_CENTER_OFFSET_X_MODULES;
    let logo_center_y = center + module * WECHAT_LOGO_CENTER_OFFSET_Y_MODULES;
    let white_size = canvas * WECHAT_BADGE_WHITE_SIZE_QR_RATIO;
    let black_size = canvas * WECHAT_LOGO_SIZE_QR_RATIO;
    svg.push_str(&format!(
        r##"<rect x="{:.3}" y="{:.3}" width="{white_size:.3}" height="{white_size:.3}" fill="#fff"/>"##,
        white_center - white_size * 0.5,
        white_center - white_size * 0.5,
    ));
    push_wechat_logo_paths(svg, logo_center_x, logo_center_y, black_size);
}

fn push_wechat_logo_paths(svg: &mut String, cx: f64, cy: f64, badge_size: f64) {
    let scale = badge_size / WECHAT_LOGO_REFERENCE_BADGE_SIZE;
    let tx = cx - WECHAT_LOGO_REFERENCE_CENTER.0 * scale;
    let ty = cy - WECHAT_LOGO_REFERENCE_CENTER.1 * scale;
    svg.push_str(&format!(
        r#"<g transform="matrix({scale:.6} 0 0 {scale:.6} {tx:.6} {ty:.6})">"#
    ));
    for path in WECHAT_LOGO_PATHS {
        svg.push_str(&format!(r##"<path d="{path}" fill="#000"/>"##));
    }
    svg.push_str("</g>");
}

fn styled_qr_matrix_module_is_reserved(
    size: usize,
    x: usize,
    y: usize,
    appearance: QrAppearance,
) -> bool {
    match appearance {
        QrAppearance::Wechat | QrAppearance::Xiaohongshu => is_qr_finder_area(size, x, y),
        QrAppearance::EnterpriseWechat => is_enterprise_wechat_fixed_module_area(size, x, y),
        QrAppearance::Standard => false,
    }
}

fn is_enterprise_wechat_fixed_module_area(size: usize, x: usize, y: usize) -> bool {
    standard_qr_finder_module(size, x, y).is_some()
        || standard_qr_alignment_module(size, x, y).is_some()
}

fn enterprise_wechat_fixed_module_is_black(size: usize, x: usize, y: usize) -> bool {
    standard_qr_finder_module(size, x, y)
        .or_else(|| standard_qr_alignment_module(size, x, y))
        .unwrap_or(false)
}

fn is_qr_finder_area(size: usize, x: usize, y: usize) -> bool {
    size >= 7 && ((x < 7 && y < 7) || (x >= size - 7 && y < 7) || (x < 7 && y >= size - 7))
}

fn standard_qr_module_is_black(matrix: &QrMatrix, size: usize, x: usize, y: usize) -> bool {
    standard_qr_finder_module(size, x, y).unwrap_or_else(|| matrix[y][x])
}

fn standard_qr_finder_module(size: usize, x: usize, y: usize) -> Option<bool> {
    if size < 7 {
        return None;
    }

    let local = if x < 7 && y < 7 {
        Some((x, y))
    } else if x >= size - 7 && y < 7 {
        Some((x - (size - 7), y))
    } else if x < 7 && y >= size - 7 {
        Some((x, y - (size - 7)))
    } else {
        None
    }?;

    Some(qr_standard_finder_expected(local.0, local.1))
}

fn standard_qr_alignment_module(size: usize, x: usize, y: usize) -> Option<bool> {
    let version = qr_version_for_size(size)?;
    if version == 1 {
        return None;
    }

    let centers = qr_alignment_pattern_centers(version, size);
    for &cy in &centers {
        for &cx in &centers {
            if alignment_pattern_overlaps_finder(size, cx, cy) {
                continue;
            }
            let dx = x.abs_diff(cx);
            let dy = y.abs_diff(cy);
            if dx <= 2 && dy <= 2 {
                let ring = dx.max(dy);
                return Some(ring == 2 || ring == 0);
            }
        }
    }
    None
}

fn qr_version_for_size(size: usize) -> Option<usize> {
    if !(21..=177).contains(&size) || !(size - 21).is_multiple_of(4) {
        return None;
    }
    Some((size - 21) / 4 + 1)
}

fn qr_alignment_pattern_centers(version: usize, size: usize) -> Vec<usize> {
    if version == 1 {
        return Vec::new();
    }

    let count = version / 7 + 2;
    let step = if version == 32 {
        26
    } else {
        ((version * 4 + count * 2 + 1) / (count * 2 - 2)) * 2
    };

    let mut centers = vec![0; count];
    centers[0] = 6;
    let mut pos = size - 7;
    for index in (1..count).rev() {
        centers[index] = pos;
        pos = pos.saturating_sub(step);
    }
    centers
}

fn alignment_pattern_overlaps_finder(size: usize, cx: usize, cy: usize) -> bool {
    let far = size - 7;
    (cx == 6 && (cy == 6 || cy == far)) || (cx == far && cy == 6)
}

fn qr_standard_finder_expected(x: usize, y: usize) -> bool {
    let dx = (x as i32 - 3).abs();
    let dy = (y as i32 - 3).abs();
    dx.max(dy) != 2
}

fn qr_finder_origins(size: usize) -> [(usize, usize); 3] {
    [(0, 0), (size - 7, 0), (0, size - 7)]
}

pub fn qr_matrix_to_preview_image(
    matrix: &QrMatrix,
    appearance: QrAppearance,
    diff: Option<&DiffResult>,
    show_diff: bool,
    scale: u32,
    border: u32,
) -> DynamicImage {
    let modules = matrix.len() as u32;
    let scale = scale.max(1);
    let style_pad = qr_preview_style_padding(appearance, scale);
    let image_size = (modules + border * 2).max(1) * scale + style_pad;
    let mut image = RgbaImage::from_pixel(image_size, image_size, Rgba([255, 255, 255, 255]));

    for (module_y, row) in matrix.iter().enumerate() {
        for (module_x, &is_black) in row.iter().enumerate() {
            let should_paint = match appearance {
                QrAppearance::Standard => {
                    standard_qr_module_is_black(matrix, matrix.len(), module_x, module_y)
                }
                QrAppearance::Wechat
                | QrAppearance::Xiaohongshu
                | QrAppearance::EnterpriseWechat => is_black,
            };
            if !should_paint {
                continue;
            }
            if styled_qr_matrix_module_is_reserved(matrix.len(), module_x, module_y, appearance) {
                continue;
            }
            let module_x = module_x as u32;
            let module_y = module_y as u32;
            let start_x = style_pad + (module_x + border) * scale;
            let start_y = style_pad + (module_y + border) * scale;
            let black = Rgba([0, 0, 0, 255]);

            match appearance {
                QrAppearance::Standard => paint_filled_rect_px(
                    &mut image,
                    start_x as f64,
                    start_y as f64,
                    scale as f64,
                    scale as f64,
                    black,
                ),
                QrAppearance::Wechat => paint_filled_round_rect(
                    &mut image,
                    start_x as f64 + scale as f64 * (1.0 - WECHAT_DATA_SIDE_RATIO) * 0.5,
                    start_y as f64 + scale as f64 * (1.0 - WECHAT_DATA_SIDE_RATIO) * 0.5,
                    scale as f64 * WECHAT_DATA_SIDE_RATIO,
                    scale as f64 * WECHAT_DATA_SIDE_RATIO,
                    scale as f64 * WECHAT_DATA_RADIUS_RATIO,
                    black,
                ),
                QrAppearance::Xiaohongshu => paint_filled_circle(
                    &mut image,
                    (
                        start_x as f64 + scale as f64 * 0.5,
                        start_y as f64 + scale as f64 * 0.5,
                    ),
                    scale as f64 * 0.31,
                    black,
                ),
                QrAppearance::EnterpriseWechat => paint_filled_rect_px(
                    &mut image,
                    start_x as f64 + scale as f64 * (1.0 - ENTERPRISE_WECHAT_DATA_SIDE_RATIO) * 0.5,
                    start_y as f64 + scale as f64 * (1.0 - ENTERPRISE_WECHAT_DATA_SIDE_RATIO) * 0.5,
                    scale as f64 * ENTERPRISE_WECHAT_DATA_SIDE_RATIO,
                    scale as f64 * ENTERPRISE_WECHAT_DATA_SIDE_RATIO,
                    black,
                ),
            }
        }
    }

    if appearance != QrAppearance::Standard && matrix.len() >= 7 {
        for (x, y) in qr_finder_origins(matrix.len()) {
            let px = style_pad + (x as u32 + border) * scale;
            let py = style_pad + (y as u32 + border) * scale;
            match appearance {
                QrAppearance::Wechat => {
                    paint_wechat_qr_finder(&mut image, px as f64, py as f64, scale as f64)
                }
                QrAppearance::Xiaohongshu => {
                    paint_xiaohongshu_qr_finder(&mut image, px as f64, py as f64, scale as f64)
                }
                QrAppearance::Standard | QrAppearance::EnterpriseWechat => {}
            }
        }
    }

    if appearance == QrAppearance::EnterpriseWechat {
        let qr_origin = style_pad as f64 + border as f64 * scale as f64;
        paint_enterprise_wechat_qr_fixed_modules(
            &mut image,
            qr_origin,
            qr_origin,
            scale as f64,
            matrix.len(),
        );
    }

    if appearance == QrAppearance::Wechat && matrix.len() >= 21 {
        let qr_origin = style_pad as f64 + border as f64 * scale as f64;
        paint_wechat_qr_badge(
            &mut image,
            qr_origin,
            qr_origin,
            modules as f64 * scale as f64,
            scale as f64,
        );
    }

    if let Some(diff) = diff.filter(|_| show_diff) {
        let missing_set: HashSet<(u32, u32)> = diff.missing_in_generated.iter().copied().collect();
        let extra_set: HashSet<(u32, u32)> = diff.extra_in_generated.iter().copied().collect();
        for (modules, color) in [
            (&missing_set, Rgba([220, 32, 32, 255])),
            (&extra_set, Rgba([32, 96, 220, 255])),
        ] {
            for &(module_x, module_y) in modules {
                let start_x = style_pad + (module_x + border) * scale;
                let start_y = style_pad + (module_y + border) * scale;
                paint_filled_rect_px(
                    &mut image,
                    start_x as f64,
                    start_y as f64,
                    scale as f64,
                    scale as f64,
                    color,
                );
            }
        }
    }

    DynamicImage::ImageRgba8(image)
}

pub fn data_matrix_grid_to_svg(grid: &DataMatrixGrid) -> String {
    let module_mm = 1.0_f64;
    let width = grid.cols as f64 * module_mm;
    let height = grid.rows as f64 * module_mm;
    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.3}mm" height="{height:.3}mm" viewBox="0 0 {width:.3} {height:.3}" shape-rendering="crispEdges">"#
    ));
    svg.push_str(&format!(
        r##"<rect x="0" y="0" width="{width:.3}" height="{height:.3}" fill="#fff"/>"##
    ));

    for (y, row) in grid.matrix.iter().enumerate() {
        for (x, &is_black) in row.iter().enumerate() {
            if !is_black {
                continue;
            }
            let px = x as f64 * module_mm;
            let py = y as f64 * module_mm;
            svg.push_str(&format!(
                r##"<rect x="{px:.3}" y="{py:.3}" width="{module_mm:.3}" height="{module_mm:.3}" fill="#000"/>"##
            ));
        }
    }

    svg.push_str("</svg>");
    svg
}

pub fn data_matrix_grid_to_preview_image(grid: &DataMatrixGrid, size: u32) -> DynamicImage {
    data_matrix_grid_to_diff_preview_image(grid, &BinaryImage::new(0, 0, Vec::new()), false, size).0
}

pub fn data_matrix_grid_to_diff_preview_image(
    grid: &DataMatrixGrid,
    source: &BinaryImage,
    show_diff: bool,
    size: u32,
) -> (DynamicImage, u32) {
    let rows = grid.rows.max(1) as u32;
    let cols = grid.cols.max(1) as u32;
    let scale = (size.max(1) / rows.max(cols)).max(2);
    let width = cols * scale;
    let height = rows * scale;
    let mut image = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));
    let mut diff_count = 0_u32;

    for module_y in 0..rows as usize {
        for module_x in 0..cols as usize {
            let generated_black = grid
                .matrix
                .get(module_y)
                .and_then(|row| row.get(module_x))
                .copied()
                .unwrap_or(false);
            if generated_black {
                paint_filled_rect_px(
                    &mut image,
                    module_x as f64 * scale as f64,
                    module_y as f64 * scale as f64,
                    scale as f64,
                    scale as f64,
                    Rgba([0, 0, 0, 255]),
                );
            }

            if source.w == 0 || source.h == 0 {
                continue;
            }

            let source_black =
                data_matrix_preview_source_module_is_black(source, grid, module_x, module_y);
            if source_black == generated_black {
                continue;
            }

            diff_count += 1;
            if show_diff {
                let color = if source_black {
                    Rgba([220, 32, 32, 255])
                } else {
                    Rgba([32, 96, 220, 255])
                };
                paint_filled_rect_px(
                    &mut image,
                    module_x as f64 * scale as f64,
                    module_y as f64 * scale as f64,
                    scale as f64,
                    scale as f64,
                    color,
                );
            }
        }
    }

    (DynamicImage::ImageRgba8(image), diff_count)
}

fn data_matrix_preview_source_module_is_black(
    source: &BinaryImage,
    grid: &DataMatrixGrid,
    x: usize,
    y: usize,
) -> bool {
    let (black, total) = data_matrix_preview_source_module_vote(source, grid, x, y);
    black * 2 >= total
}

fn data_matrix_preview_source_module_vote(
    source: &BinaryImage,
    grid: &DataMatrixGrid,
    x: usize,
    y: usize,
) -> (usize, usize) {
    if grid.cols == 0 || grid.rows == 0 || source.w == 0 || source.h == 0 {
        return (0, 1);
    }

    let cell_w = source.w as f64 / grid.cols as f64 * grid.sampling.scale_x;
    let cell_h = source.h as f64 / grid.rows as f64 * grid.sampling.scale_y;
    let center_x = source.w as f64 * 0.5
        + (x as f64 + 0.5 - grid.cols as f64 * 0.5 + grid.sampling.shift_x) * cell_w;
    let center_y = source.h as f64 * 0.5
        + (y as f64 + 0.5 - grid.rows as f64 * 0.5 + grid.sampling.shift_y) * cell_h;
    let offsets = [-0.18, 0.0, 0.18];

    let mut black = 0;
    let mut total = 0;
    for oy in offsets {
        for ox in offsets {
            let px = (center_x + ox * cell_w)
                .round()
                .clamp(0.0, source.w.saturating_sub(1) as f64) as i32;
            let py = (center_y + oy * cell_h)
                .round()
                .clamp(0.0, source.h.saturating_sub(1) as f64) as i32;
            if source.is_black(px, py) {
                black += 1;
            }
            total += 1;
        }
    }

    (black, total)
}

fn qr_preview_style_padding(appearance: QrAppearance, scale: u32) -> u32 {
    if appearance == QrAppearance::Wechat {
        (-WECHAT_FINDER_OFFSET_RATIO * scale as f64).ceil() as u32
    } else {
        0
    }
}

fn paint_wechat_qr_finder(image: &mut RgbaImage, x: f64, y: f64, scale: f64) {
    let black = Rgba([0, 0, 0, 255]);
    let white = Rgba([255, 255, 255, 255]);
    paint_filled_round_rect(
        image,
        x + scale * WECHAT_FINDER_OFFSET_RATIO,
        y + scale * WECHAT_FINDER_OFFSET_RATIO,
        scale * WECHAT_FINDER_OUTER_SIZE_RATIO,
        scale * WECHAT_FINDER_OUTER_SIZE_RATIO,
        scale * WECHAT_FINDER_OUTER_RADIUS_RATIO,
        black,
    );
    paint_filled_round_rect(
        image,
        x + scale * WECHAT_FINDER_HOLE_OFFSET_RATIO,
        y + scale * WECHAT_FINDER_HOLE_OFFSET_RATIO,
        scale * WECHAT_FINDER_HOLE_SIZE_RATIO,
        scale * WECHAT_FINDER_HOLE_SIZE_RATIO,
        scale * WECHAT_FINDER_HOLE_RADIUS_RATIO,
        white,
    );
    paint_filled_round_rect(
        image,
        x + scale * WECHAT_FINDER_INNER_OFFSET_RATIO,
        y + scale * WECHAT_FINDER_INNER_OFFSET_RATIO,
        scale * WECHAT_FINDER_INNER_SIZE_RATIO,
        scale * WECHAT_FINDER_INNER_SIZE_RATIO,
        scale * WECHAT_FINDER_INNER_RADIUS_RATIO,
        black,
    );
}

fn paint_xiaohongshu_qr_finder(image: &mut RgbaImage, x: f64, y: f64, scale: f64) {
    let center = (x + scale * 3.5, y + scale * 3.5);
    paint_filled_circle(image, center, scale * 3.50, Rgba([0, 0, 0, 255]));
    paint_filled_circle(image, center, scale * 2.50, Rgba([255, 255, 255, 255]));
    paint_filled_circle(image, center, scale * 1.50, Rgba([0, 0, 0, 255]));
}

fn paint_enterprise_wechat_qr_fixed_modules(
    image: &mut RgbaImage,
    x: f64,
    y: f64,
    scale: f64,
    modules: usize,
) {
    let black = Rgba([0, 0, 0, 255]);
    for module_y in 0..modules {
        for module_x in 0..modules {
            if !enterprise_wechat_fixed_module_is_black(modules, module_x, module_y) {
                continue;
            }
            paint_filled_rect_px(
                image,
                x + module_x as f64 * scale,
                y + module_y as f64 * scale,
                scale,
                scale,
                black,
            );
        }
    }
}

fn paint_wechat_qr_badge(image: &mut RgbaImage, x: f64, y: f64, canvas: f64, module: f64) {
    let center_x = x + canvas * 0.5;
    let center_y = y + canvas * 0.5;
    let white_center_x = center_x + module * WECHAT_BADGE_WHITE_CENTER_OFFSET_MODULES;
    let white_center_y = center_y + module * WECHAT_BADGE_WHITE_CENTER_OFFSET_MODULES;
    let logo_center_x = center_x + module * WECHAT_LOGO_CENTER_OFFSET_X_MODULES;
    let logo_center_y = center_y + module * WECHAT_LOGO_CENTER_OFFSET_Y_MODULES;
    let white_size = canvas * WECHAT_BADGE_WHITE_SIZE_QR_RATIO;
    let black_size = canvas * WECHAT_LOGO_SIZE_QR_RATIO;
    paint_filled_rect_px(
        image,
        white_center_x - white_size * 0.5,
        white_center_y - white_size * 0.5,
        white_size,
        white_size,
        Rgba([255, 255, 255, 255]),
    );
    paint_filled_round_rect(
        image,
        logo_center_x - black_size * 0.5,
        logo_center_y - black_size * 0.5,
        black_size,
        black_size,
        black_size * 0.10,
        Rgba([0, 0, 0, 255]),
    );
    paint_wechat_logo(image, (logo_center_x, logo_center_y), black_size * 0.42);
}

fn paint_wechat_logo(image: &mut RgbaImage, center: (f64, f64), radius: f64) {
    let white = Rgba([255, 255, 255, 255]);
    let black = Rgba([0, 0, 0, 255]);
    let big = radius;
    let small = radius * 0.82;
    let big_center = (center.0 - radius * 0.22, center.1 - radius * 0.10);
    let small_center = (center.0 + radius * 0.28, center.1 + radius * 0.20);
    paint_filled_ellipse(image, big_center, big * 0.74, big * 0.52, white);
    paint_filled_ellipse(image, small_center, small * 0.74, small * 0.52, white);
    for (eye_x, eye_y, r) in [
        (
            big_center.0 - big * 0.24,
            big_center.1 - big * 0.10,
            big * 0.065,
        ),
        (
            big_center.0 + big * 0.20,
            big_center.1 - big * 0.10,
            big * 0.065,
        ),
        (
            small_center.0 - small * 0.20,
            small_center.1 - small * 0.08,
            small * 0.070,
        ),
        (
            small_center.0 + small * 0.20,
            small_center.1 - small * 0.08,
            small * 0.070,
        ),
    ] {
        paint_filled_circle(image, (eye_x, eye_y), r, black);
    }
}

pub fn wx_grid_to_svg(grid: &WxGrid) -> String {
    let canvas = (grid.r_max * 2.0).max(1.0);
    let center = grid.r_max;
    let radial_step = (grid.r_max - grid.r_min) / grid.points_per_line.max(1) as f64;
    let stroke_width = radial_step;

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{canvas:.3}mm" height="{canvas:.3}mm" viewBox="0 0 {canvas:.3} {canvas:.3}" shape-rendering="geometricPrecision">"#
    ));
    svg.push_str(&format!(
        r##"<rect x="0" y="0" width="{canvas:.3}" height="{canvas:.3}" fill="#fff"/>"##
    ));
    if grid.lines == 0 || grid.points_per_line == 0 || grid.r_max <= grid.r_min {
        svg.push_str("</svg>");
        return svg;
    }

    let theta_step = std::f64::consts::TAU / grid.lines as f64;
    for line in 0..grid.lines {
        let theta = grid.theta_offset + (line as f64 + 0.5) * theta_step;
        let angle = theta.to_degrees();
        let mut point = 0;
        while point < grid.points_per_line {
            if !grid.sample(line, point) {
                point += 1;
                continue;
            }

            let start = point;
            while point + 1 < grid.points_per_line && grid.sample(line, point + 1) {
                point += 1;
            }
            let end = point;

            let r_mid = grid.r_min + ((start + end) as f64 * 0.5 + 0.5) * radial_step;
            let p_mid = polar_point(center, center, r_mid, theta);
            let length = (end - start + 1) as f64 * radial_step;
            svg.push_str(&format!(
                r##"<rect x="{:.3}" y="{:.3}" width="{length:.3}" height="{stroke_width:.3}" rx="{:.3}" fill="#000" transform="rotate({angle:.3} {:.3} {:.3})"/>"##,
                p_mid.0 - length * 0.5,
                p_mid.1 - stroke_width * 0.5,
                stroke_width * 0.5,
                p_mid.0,
                p_mid.1
            ));
            point += 1;
        }
    }

    for finder in grid.finders {
        let cx = center + finder.cx - grid.center.0;
        let cy = center + finder.cy - grid.center.1;
        svg.push_str(&format!(
            r##"<circle cx="{cx:.3}" cy="{cy:.3}" r="{:.3}" fill="#000"/>"##,
            finder.r_outer
        ));
        svg.push_str(&format!(
            r##"<circle cx="{cx:.3}" cy="{cy:.3}" r="{:.3}" fill="#fff"/>"##,
            finder.r_outer * 0.62
        ));
        svg.push_str(&format!(
            r##"<circle cx="{cx:.3}" cy="{cy:.3}" r="{:.3}" fill="#000"/>"##,
            finder.r_outer * 0.18
        ));
    }

    if let Some(badge) = grid.badge {
        let cx = center + badge.cx - grid.center.0;
        let cy = center + badge.cy - grid.center.1;
        let fill = rgb_hex(badge.color);
        svg.push_str(&format!(
            r##"<circle cx="{cx:.3}" cy="{cy:.3}" r="{:.3}" fill="{fill}"/>"##,
            badge.radius
        ));
        svg.push_str(&mini_program_logo_path(cx, cy, badge.radius));
    }

    svg.push_str("</svg>");
    svg
}

pub fn wx_grid_to_preview_image(grid: &WxGrid, size: u32) -> DynamicImage {
    let size = size.max(1);
    let mut image = RgbaImage::from_pixel(size, size, Rgba([255, 255, 255, 255]));
    if grid.lines == 0 || grid.points_per_line == 0 || grid.r_max <= grid.r_min {
        return DynamicImage::ImageRgba8(image);
    }

    let center = (size as f64 - 1.0) * 0.5;
    let scale = (size as f64 - 1.0) / (grid.r_max * 2.0).max(1.0);
    let radial_step = (grid.r_max - grid.r_min) / grid.points_per_line as f64;
    let stroke_radius = radial_step * scale * 0.5;
    let theta_step = std::f64::consts::TAU / grid.lines as f64;
    for line in 0..grid.lines {
        let theta = grid.theta_offset + (line as f64 + 0.5) * theta_step;
        let mut point = 0;
        while point < grid.points_per_line {
            if !grid.sample(line, point) {
                point += 1;
                continue;
            }

            let start = point;
            while point + 1 < grid.points_per_line && grid.sample(line, point + 1) {
                point += 1;
            }
            let end = point;

            let r_start = grid.r_min + (start as f64 + 0.5) * radial_step;
            let r_end = grid.r_min + (end as f64 + 0.5) * radial_step;
            let p_start = scaled_polar_point(center, scale, r_start, theta);
            let p_end = scaled_polar_point(center, scale, r_end, theta);
            paint_capsule(
                &mut image,
                p_start,
                p_end,
                stroke_radius,
                Rgba([0, 0, 0, 255]),
            );
            point += 1;
        }
    }

    for finder in grid.finders {
        let cx = center + (finder.cx - grid.center.0) * scale;
        let cy = center + (finder.cy - grid.center.1) * scale;
        paint_filled_circle(
            &mut image,
            (cx, cy),
            finder.r_outer * scale,
            Rgba([0, 0, 0, 255]),
        );
        paint_filled_circle(
            &mut image,
            (cx, cy),
            finder.r_outer * scale * 0.62,
            Rgba([255, 255, 255, 255]),
        );
        paint_filled_circle(
            &mut image,
            (cx, cy),
            finder.r_outer * scale * 0.18,
            Rgba([0, 0, 0, 255]),
        );
    }

    if let Some(badge) = grid.badge {
        let cx = center + (badge.cx - grid.center.0) * scale;
        let cy = center + (badge.cy - grid.center.1) * scale;
        let radius = badge.radius * scale;
        paint_filled_circle(
            &mut image,
            (cx, cy),
            radius,
            Rgba([badge.color[0], badge.color[1], badge.color[2], 255]),
        );
        paint_mini_program_logo(&mut image, (cx, cy), radius);
    }

    DynamicImage::ImageRgba8(image)
}

pub fn wx_grid_to_diff_preview_image(
    grid: &WxGrid,
    source: &BinaryImage,
    show_diff: bool,
    size: u32,
) -> (DynamicImage, u32) {
    let mut image = wx_grid_to_preview_image(grid, size).to_rgba8();
    if grid.lines == 0 || grid.points_per_line == 0 || grid.r_max <= grid.r_min {
        return (DynamicImage::ImageRgba8(image), 0);
    }

    let preview_center = (image.width() as f64 - 1.0) * 0.5;
    let scale = (image.width() as f64 - 1.0) / (grid.r_max * 2.0).max(1.0);
    let mut diff_count = 0_u32;

    for y in 0..image.height() {
        for x in 0..image.width() {
            let source_point = (
                grid.center.0 + (x as f64 - preview_center) / scale,
                grid.center.1 + (y as f64 - preview_center) / scale,
            );
            if is_wx_diff_ignored(grid, source_point) {
                continue;
            }

            let generated = image.get_pixel(x, y).0;
            let generated_black = generated[0] < 96 && generated[1] < 96 && generated[2] < 96;
            let original_black =
                source.is_black(source_point.0.round() as i32, source_point.1.round() as i32);
            if original_black == generated_black {
                continue;
            }

            diff_count += 1;
            if show_diff {
                let color = if original_black {
                    Rgba([220, 32, 32, 255])
                } else {
                    Rgba([32, 96, 220, 255])
                };
                image.put_pixel(x, y, color);
            }
        }
    }

    (DynamicImage::ImageRgba8(image), diff_count)
}

pub fn dy_grid_to_svg(grid: &DyGrid) -> String {
    if grid.has_border {
        return dy_black_border_grid_to_svg(grid);
    }

    dy_no_border_grid_to_svg(grid)
}

fn dy_no_border_grid_to_svg(grid: &DyGrid) -> String {
    let layout = DOUYIN_NO_BORDER_LAYOUT;
    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {:.2} {:.2}" shape-rendering="geometricPrecision">"#,
        layout.viewbox.0, layout.viewbox.1
    ));

    if grid.points_per_ring == 0 || grid.rings.is_empty() {
        svg.push_str(&standard_no_border_static_marks_group(layout));
        svg.push_str("</svg>");
        return svg;
    }

    let scale = DOUYIN_NO_BORDER_LOCATOR_DISTANCE / grid_locator_distance(grid).max(1.0);
    let theta_step = std::f64::consts::TAU / grid.points_per_ring as f64;
    svg.push_str(r#"<g id="a">"#);
    // 装饰环改由 grid.decorative_rings（720 点高密度弧段）渲染，这里跳过。
    for (ring_idx, ring) in grid.rings.iter().enumerate() {
        if ring.is_decoration {
            continue;
        }
        let render_ring = no_border_standard_render_ring(grid, ring_idx).unwrap_or(RingSpec {
            r_inner: ring.r_inner * scale,
            r_outer: ring.r_outer * scale,
            is_decoration: ring.is_decoration,
        });
        for run in dy_sample_runs(grid, ring_idx as u32) {
            let Some(mark) = dy_no_border_mark_geometry_for_ring(
                ring_idx,
                layout.code_theta_offset + grid.ring_theta_delta(ring_idx),
                &render_ring,
                run,
                theta_step,
            ) else {
                continue;
            };
            let mark = no_border_apply_render_ring_width_scale(mark, ring_idx, run);
            let mark = no_border_apply_run_offsets(mark, grid, ring_idx, run, scale, theta_step);
            if run.len() == 1 {
                let p = polar_point(
                    layout.center.0,
                    layout.center.1,
                    mark.radius,
                    mark.theta_mid(),
                );
                svg.push_str(&format!(
                    r##"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" style="fill:{};"/>"##,
                    p.0,
                    p.1,
                    mark.stroke_width * 0.5 * no_border_single_dot_width_scale(ring_idx),
                    layout.black_fill,
                ));
            } else {
                svg.push_str(&format!(
                    r##"<path d="{}" style="fill:{};"/>"##,
                    rounded_arc_bar_path(
                        layout.center.0,
                        layout.center.1,
                        mark.radius,
                        mark.stroke_width,
                        mark.theta_start,
                        mark.theta_end,
                    ),
                    layout.black_fill,
                ));
            }
        }
    }
    // 装饰环（ring0/ring2）：720 点高密度采样的黑弧段。渲染风格与编码环一致——
    // 弧段用标准环线宽（10.4，与编码同）+ 圆角端点（rounded_arc_bar_path）；孤立圆点
    // （弧长 ≤ 1.6×线宽）画成圆形，半径用基础 radial_step×0.5×0.81≈4（比编码点 r=5
    // 略小，与 samples/无框版1.svg 的装饰环黑点一致）。
    for decorative in &grid.decorative_rings {
        let points = decorative.points_per_ring;
        if points == 0 {
            continue;
        }
        let ring_idx = no_border_decorative_ring_index(&decorative.ring, scale);
        let render_ring = no_border_standard_render_ring(grid, ring_idx).unwrap_or(RingSpec {
            r_inner: decorative.ring.r_inner * scale,
            r_outer: decorative.ring.r_outer * scale,
            is_decoration: true,
        });
        let radial_step = (render_ring.r_outer - render_ring.r_inner).max(0.01);
        let radius = (render_ring.r_inner + render_ring.r_outer) * 0.5;
        // 弧段线宽 = 标准 radial_step ×1.04 × 该环宽度系数（ring0/2 均≈1.0），与编码环同。
        let bar_width = radial_step
            * DOUYIN_NO_BORDER_DECORATIVE_RUN_WIDTH_SCALE
            * no_border_render_ring_width_scale(ring_idx);
        let dtheta_step = std::f64::consts::TAU / points as f64;
        for run in dy_runs_from_samples(points, |point| decorative.sample(point)) {
            if run.len() == 0 {
                continue;
            }
            // 直接用 run 边界算角度，与旧直角扇形同跨度（不套编码环的大角度内缩），保弧长不变。
            let theta_start = decorative.theta_offset + run.start as f64 * dtheta_step;
            let theta_end = decorative.theta_offset + run.end as f64 * dtheta_step;
            let arc_len = run.len() as f64 * dtheta_step * radius;
            if arc_len <= bar_width * DOUYIN_NO_BORDER_DECORATIVE_DOT_MAX_ARC_SCALE {
                // 孤立圆点：半径取基础 radial_step（不乘弧段加宽 1.04）×0.81 ≈ 4。
                let p = polar_point(
                    layout.center.0,
                    layout.center.1,
                    radius,
                    (theta_start + theta_end) * 0.5,
                );
                svg.push_str(&format!(
                    r##"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" style="fill:{};"/>"##,
                    p.0,
                    p.1,
                    radial_step * 0.5 * no_border_single_dot_width_scale(ring_idx),
                    layout.black_fill,
                ));
            } else {
                // 圆角帽在传入角度外侧凸出 half_width；为保持与原直角扇形相同的弧长
                // （端点不外延），把传入角度向内缩 δ=half_width/radius，使圆角顶点正好
                // 落回 theta_start/theta_end。span 不足 2δ 时夹到半跨防反转。
                let cap_inset = (bar_width * 0.5 / radius).min((theta_end - theta_start) * 0.5);
                svg.push_str(&format!(
                    r##"<path d="{}" style="fill:{};"/>"##,
                    rounded_arc_bar_path(
                        layout.center.0,
                        layout.center.1,
                        radius,
                        bar_width,
                        theta_start + cap_inset,
                        theta_end - cap_inset,
                    ),
                    layout.black_fill,
                ));
            }
        }
    }
    svg.push_str("</g>");
    svg.push_str(&standard_no_border_static_marks_group(layout));
    svg.push_str("</svg>");
    svg
}

fn dy_black_border_grid_to_svg(grid: &DyGrid) -> String {
    let layout = douyin_black_border_layout(grid);
    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {:.2} {:.2}" shape-rendering="geometricPrecision">"#,
        layout.viewbox.0, layout.viewbox.1
    ));

    if grid.points_per_ring == 0 || grid.rings.is_empty() {
        if let Some(group) = standard_black_border_static_marks_group(layout) {
            svg.push_str(&group);
        }
        svg.push_str("</svg>");
        return svg;
    }

    let scale = DOUYIN_BLACK_BORDER_LOCATOR_DISTANCE / grid_locator_distance(grid).max(1.0);
    let black_fill = layout.black_fill;
    svg.push_str(r#"<g id="a">"#);
    if let Some(outer_frame) = &grid.outer_frame {
        for segment in &outer_frame.segments {
            svg.push_str(&format!(
                r##"<path d="{}" style="fill:{black_fill};"/>"##,
                polar_sector_path(
                    layout.center.0,
                    layout.center.1,
                    outer_frame.ring.r_inner * scale,
                    outer_frame.ring.r_outer * scale,
                    segment.theta_start,
                    segment.theta_end,
                )
            ));
        }
    }
    for decorative in &grid.decorative_rings {
        let points = decorative.points_per_ring;
        if points == 0 {
            continue;
        }
        let theta_step = std::f64::consts::TAU / points as f64;
        for run in dy_runs_from_samples(points, |point| decorative.sample(point)) {
            let Some(mark) = dy_mark_geometry(
                true,
                decorative.theta_offset,
                &decorative.ring,
                run,
                theta_step,
            ) else {
                continue;
            };
            svg.push_str(&format!(
                r##"<path d="{}" style="fill:{black_fill};"/>"##,
                polar_sector_path(
                    layout.center.0,
                    layout.center.1,
                    mark.r_inner * scale,
                    mark.r_outer * scale,
                    mark.theta_start,
                    mark.theta_end,
                )
            ));
        }
    }
    svg.push_str("</g>");
    if let Some(group) = standard_black_border_static_marks_group(layout) {
        svg.push_str(&group);
    }

    let theta_step = std::f64::consts::TAU / grid.points_per_ring as f64;
    let code_theta_offset = layout.code_theta_offset;
    svg.push_str(r#"<g id="c">"#);
    for (ring_idx, ring) in grid.rings.iter().enumerate() {
        for run in dy_sample_runs(grid, ring_idx as u32) {
            let Some(mark) = dy_mark_geometry(true, code_theta_offset, ring, run, theta_step)
            else {
                continue;
            };
            svg.push_str(&format!(
                r##"<path d="{}" style="fill:{black_fill};"/>"##,
                polar_sector_path(
                    layout.center.0,
                    layout.center.1,
                    mark.r_inner * scale,
                    mark.r_outer * scale,
                    mark.theta_start,
                    mark.theta_end,
                )
            ));
        }
    }
    svg.push_str("</g>");
    svg.push_str("</svg>");
    svg
}

pub fn dy_grid_to_preview_image(grid: &DyGrid, size: u32) -> DynamicImage {
    let size = size.max(1);
    if !grid.has_border {
        return dy_no_border_grid_to_preview_image(grid, size);
    }

    let mut image = RgbaImage::from_pixel(size, size, Rgba([255, 255, 255, 255]));
    if grid.points_per_ring == 0 || grid.rings.is_empty() {
        return DynamicImage::ImageRgba8(image);
    }

    let preview_center = (size as f64 - 1.0) * 0.5;
    let r_outer = dy_outer_radius(grid).unwrap_or(1.0);
    let scale = (size as f64 - 1.0) / (r_outer * 2.3).max(1.0);
    if grid.has_border {
        if let Some(outer_frame) = &grid.outer_frame {
            for segment in &outer_frame.segments {
                paint_ring_sector(
                    &mut image,
                    preview_center,
                    scale,
                    RasterSector {
                        r_inner: outer_frame.ring.r_inner,
                        r_outer: outer_frame.ring.r_outer,
                        theta_start: segment.theta_start,
                        theta_end: segment.theta_end,
                    },
                    Rgba([0, 0, 0, 255]),
                );
            }
        }
        for decorative in &grid.decorative_rings {
            let points = decorative.points_per_ring;
            if points == 0 {
                continue;
            }
            let theta_step = std::f64::consts::TAU / points as f64;
            for run in dy_runs_from_samples(points, |point| decorative.sample(point)) {
                let Some(mark) = dy_mark_geometry(
                    true,
                    decorative.theta_offset,
                    &decorative.ring,
                    run,
                    theta_step,
                ) else {
                    continue;
                };
                paint_ring_sector(
                    &mut image,
                    preview_center,
                    scale,
                    RasterSector {
                        r_inner: mark.r_inner,
                        r_outer: mark.r_outer,
                        theta_start: mark.theta_start,
                        theta_end: mark.theta_end,
                    },
                    Rgba([0, 0, 0, 255]),
                );
            }
        }
    }
    for finder in &grid.finders {
        let cx = preview_center + (finder.cx - grid.center.0) * scale;
        let cy = preview_center + (finder.cy - grid.center.1) * scale;
        let outer = finder.outer_radius() * scale;
        paint_filled_circle(&mut image, (cx, cy), outer, Rgba([0, 0, 0, 255]));
        paint_filled_circle(
            &mut image,
            (cx, cy),
            outer * 0.62,
            Rgba([255, 255, 255, 255]),
        );
        paint_filled_circle(&mut image, (cx, cy), outer * 0.18, Rgba([0, 0, 0, 255]));
    }

    if let Some(badge) = grid.badge {
        let cx = preview_center + (badge.cx - grid.center.0) * scale;
        let cy = preview_center + (badge.cy - grid.center.1) * scale;
        let radius = badge.radius * scale;
        let outer_radius = if grid.has_border {
            radius * DOUYIN_BLACK_BORDER_BADGE_OUTER_RADIUS_SCALE
        } else {
            radius
        };
        let inner_radius = if grid.has_border {
            radius
        } else {
            radius * 0.78
        };
        paint_filled_circle(&mut image, (cx, cy), outer_radius, Rgba([0, 0, 0, 255]));
        paint_filled_circle(
            &mut image,
            (cx, cy),
            inner_radius,
            Rgba([255, 255, 255, 255]),
        );
        match grid.badge_style {
            DyBadgeStyle::DouyinLogo => paint_douyin_logo(&mut image, (cx, cy), radius),
            DyBadgeStyle::Bullseye => paint_douyin_bullseye_badge(&mut image, (cx, cy), radius),
        }
    }

    if let Some(logo) = grid.center_logo {
        let cx = preview_center + (logo.cx - grid.center.0) * scale;
        let cy = preview_center + (logo.cy - grid.center.1) * scale;
        paint_filled_circle(
            &mut image,
            (cx, cy),
            logo.radius * scale,
            Rgba([242, 48, 64, 255]),
        );
    }

    let theta_step = std::f64::consts::TAU / grid.points_per_ring as f64;
    for (ring_idx, ring) in grid.rings.iter().enumerate() {
        for run in dy_sample_runs(grid, ring_idx as u32) {
            let Some(mark) =
                dy_mark_geometry(grid.has_border, grid.theta_offset, ring, run, theta_step)
            else {
                continue;
            };
            if grid.has_border {
                paint_ring_sector(
                    &mut image,
                    preview_center,
                    scale,
                    RasterSector {
                        r_inner: mark.r_inner,
                        r_outer: mark.r_outer,
                        theta_start: mark.theta_start,
                        theta_end: mark.theta_end,
                    },
                    Rgba([0, 0, 0, 255]),
                );
            } else if run.len() == 1 {
                let mark =
                    mark.with_radial_offset(dy_run_radial_offset(grid, ring_idx as u32, run));
                let point =
                    scaled_polar_point(preview_center, scale, mark.radius, mark.theta_mid());
                paint_filled_circle(
                    &mut image,
                    point,
                    mark.stroke_width * scale * 0.5,
                    Rgba([0, 0, 0, 255]),
                );
            } else {
                paint_arc_stroke(
                    &mut image,
                    preview_center,
                    scale,
                    RasterArcStroke {
                        radius: mark.radius,
                        theta_start: mark.theta_start,
                        theta_end: mark.theta_end,
                        stroke_radius: mark.stroke_width * scale * 0.5,
                    },
                    Rgba([0, 0, 0, 255]),
                );
            }
        }
    }

    DynamicImage::ImageRgba8(image)
}

fn dy_no_border_grid_to_preview_image(grid: &DyGrid, size: u32) -> DynamicImage {
    let mut image = RgbaImage::from_pixel(size, size, Rgba([255, 255, 255, 255]));
    let layout = DOUYIN_NO_BORDER_LAYOUT;
    let transform = preview_fit_transform(layout.viewbox, size);

    if grid.points_per_ring != 0 && !grid.rings.is_empty() {
        let svg_scale = DOUYIN_NO_BORDER_LOCATOR_DISTANCE / grid_locator_distance(grid).max(1.0);
        let render_scale = transform.scale;
        let center = transform.point(layout.center);
        let theta_step = std::f64::consts::TAU / grid.points_per_ring as f64;
        for (ring_idx, ring) in grid.rings.iter().enumerate() {
            let render_ring = no_border_standard_render_ring(grid, ring_idx).unwrap_or(RingSpec {
                r_inner: ring.r_inner * svg_scale,
                r_outer: ring.r_outer * svg_scale,
                is_decoration: ring.is_decoration,
            });
            for run in dy_sample_runs(grid, ring_idx as u32) {
                let Some(mark) = dy_no_border_mark_geometry_for_ring(
                    ring_idx,
                    layout.code_theta_offset + grid.ring_theta_delta(ring_idx),
                    &render_ring,
                    run,
                    theta_step,
                ) else {
                    continue;
                };
                let mark = no_border_apply_render_ring_width_scale(mark, ring_idx, run);
                let mark =
                    no_border_apply_run_offsets(mark, grid, ring_idx, run, svg_scale, theta_step);
                if run.len() == 1 {
                    let point =
                        polar_point_px(center, mark.radius * render_scale, mark.theta_mid());
                    paint_filled_circle(
                        &mut image,
                        point,
                        mark.stroke_width
                            * render_scale
                            * 0.5
                            * no_border_single_dot_width_scale(ring_idx),
                        Rgba([0, 0, 0, 255]),
                    );
                } else {
                    paint_rounded_arc_bar(
                        &mut image,
                        center,
                        render_scale,
                        RasterArcStroke {
                            radius: mark.radius,
                            theta_start: mark.theta_start,
                            theta_end: mark.theta_end,
                            stroke_radius: mark.stroke_width * render_scale * 0.5,
                        },
                        Rgba([0, 0, 0, 255]),
                    );
                }
            }
        }
    }

    paint_no_border_static_marks(&mut image, layout, transform);
    DynamicImage::ImageRgba8(image)
}

pub fn dy_grid_to_diff_preview_image(
    grid: &DyGrid,
    source: &BinaryImage,
    show_diff: bool,
    size: u32,
) -> (DynamicImage, u32) {
    if !grid.has_border {
        return dy_no_border_grid_to_diff_preview_image(grid, source, show_diff, size);
    }

    let mut image = dy_grid_to_preview_image(grid, size).to_rgba8();
    if grid.points_per_ring == 0 || grid.rings.is_empty() {
        return (DynamicImage::ImageRgba8(image), 0);
    }

    let preview_center = (image.width() as f64 - 1.0) * 0.5;
    let r_outer = dy_outer_radius(grid).unwrap_or(1.0);
    let scale = (image.width() as f64 - 1.0) / (r_outer * 2.3).max(1.0);
    let mut diff_count = 0_u32;

    for y in 0..image.height() {
        for x in 0..image.width() {
            let source_point = (
                grid.center.0 + (x as f64 - preview_center) / scale,
                grid.center.1 + (y as f64 - preview_center) / scale,
            );
            if is_dy_diff_ignored(grid, source_point) {
                continue;
            }

            let generated = image.get_pixel(x, y).0;
            let generated_black = generated[0] < 96 && generated[1] < 96 && generated[2] < 96;
            let original_black =
                source.is_black(source_point.0.round() as i32, source_point.1.round() as i32);
            if original_black == generated_black {
                continue;
            }

            diff_count += 1;
            if show_diff {
                let color = if original_black {
                    Rgba([220, 32, 32, 255])
                } else {
                    Rgba([32, 96, 220, 255])
                };
                image.put_pixel(x, y, color);
            }
        }
    }

    (DynamicImage::ImageRgba8(image), diff_count)
}

fn dy_no_border_grid_to_diff_preview_image(
    grid: &DyGrid,
    source: &BinaryImage,
    show_diff: bool,
    size: u32,
) -> (DynamicImage, u32) {
    let mut image = dy_no_border_grid_to_preview_image(grid, size.max(1)).to_rgba8();
    if grid.points_per_ring == 0 || grid.rings.is_empty() {
        return (DynamicImage::ImageRgba8(image), 0);
    }

    let layout = DOUYIN_NO_BORDER_LAYOUT;
    let transform = preview_fit_transform(layout.viewbox, image.width().max(1));
    let svg_scale = DOUYIN_NO_BORDER_LOCATOR_DISTANCE / grid_locator_distance(grid).max(1.0);
    let rotation = grid.theta_offset - layout.code_theta_offset;
    let mut diff_count = 0_u32;

    for y in 0..image.height() {
        for x in 0..image.width() {
            let layout_point = transform.inverse_point((x as f64 + 0.5, y as f64 + 0.5));
            let source_point =
                no_border_layout_to_source_point(grid, layout, svg_scale, rotation, layout_point);
            if is_dy_diff_ignored(grid, source_point) {
                continue;
            }

            let generated = image.get_pixel(x, y).0;
            let generated_black = generated[0] < 96 && generated[1] < 96 && generated[2] < 96;
            let original_black =
                source.is_black(source_point.0.round() as i32, source_point.1.round() as i32);
            if original_black == generated_black {
                continue;
            }

            diff_count += 1;
            if show_diff {
                let color = if original_black {
                    Rgba([220, 32, 32, 255])
                } else {
                    Rgba([32, 96, 220, 255])
                };
                image.put_pixel(x, y, color);
            }
        }
    }

    (DynamicImage::ImageRgba8(image), diff_count)
}

fn is_wx_diff_ignored(grid: &WxGrid, point: (f64, f64)) -> bool {
    let radius = (point.0 - grid.center.0).hypot(point.1 - grid.center.1);
    if radius < grid.r_min * 0.96 {
        return true;
    }
    if radius > grid.r_max * 1.02 {
        return true;
    }

    grid.badge
        .is_some_and(|badge| (point.0 - badge.cx).hypot(point.1 - badge.cy) <= badge.radius * 1.08)
}

fn rgb_hex(color: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
}

fn douyin_logo_markup(cx: f64, cy: f64, radius: f64) -> String {
    let scale = radius / DOUYIN_LOGO_REFERENCE_RADIUS;
    let (ref_cx, ref_cy) = DOUYIN_LOGO_REFERENCE_CENTER;
    let mut markup = String::new();
    for layer in DOUYIN_LOGO_LAYERS {
        let path = layer.path;
        let fill = layer.fill;
        markup.push_str(&format!(
            r##"<path d="{path}" fill="{fill}" fill-rule="evenodd" transform="translate({cx:.3} {cy:.3}) scale({scale:.6}) translate(-{ref_cx:.2} -{ref_cy:.2})"/>"##
        ));
    }
    markup
}

fn douyin_black_border_layout(grid: &DyGrid) -> DouyinBlackBorderLayout {
    if grid.badge_style == DyBadgeStyle::Bullseye {
        return DOUYIN_BLACK_BORDER_BULLSEYE_BADGE_LAYOUT;
    }
    if grid.points_per_ring == 120 {
        DOUYIN_BLACK_BORDER_120_LAYOUT
    } else {
        DOUYIN_BLACK_BORDER_72_LAYOUT
    }
}

fn standard_black_border_static_marks_group(layout: DouyinBlackBorderLayout) -> Option<String> {
    let mut group = String::from(r#"<g id="b">"#);
    group.push_str(&black_border_badge_markup(layout));
    for (cx, cy) in layout.locators {
        group.push_str(&black_border_locator_markup(cx, cy, layout.black_fill));
    }
    group.push_str("</g>");
    Some(group)
}

fn standard_no_border_static_marks_group(layout: DouyinNoBorderLayout) -> String {
    let mut group = String::from(r#"<g id="b">"#);
    for (cx, cy) in layout.locators {
        group.push_str(&no_border_locator_markup(cx, cy, layout));
    }
    group.push_str(&no_border_badge_markup(layout));
    group.push_str("</g>");
    group
}

fn black_border_badge_markup(layout: DouyinBlackBorderLayout) -> String {
    let badge = layout.badge;
    let mut markup = format!(
        r##"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" style="fill:{};"/><circle cx="{:.2}" cy="{:.2}" r="{:.2}" style="fill:#fff;"/>"##,
        badge.cx,
        badge.cy,
        badge.outer_radius,
        layout.black_fill,
        badge.cx,
        badge.cy,
        badge.inner_radius
    );

    match layout.static_marks {
        DouyinBlackBorderStaticMarks::DouyinLogo => {
            markup.push_str(&douyin_logo_markup(badge.cx, badge.cy, badge.inner_radius));
        }
        DouyinBlackBorderStaticMarks::Bullseye => {
            for path in DOUYIN_BULLSEYE_BADGE_PATHS {
                markup.push_str(&format!(
                    r##"<path d="{path}" style="fill:{};"/>"##,
                    layout.black_fill
                ));
            }
            markup.push_str(&format!(
                r##"<circle cx="500.08" cy="119.25" r="8.05" style="fill:{};"/>"##,
                layout.black_fill
            ));
        }
    }

    markup
}

fn no_border_badge_markup(layout: DouyinNoBorderLayout) -> String {
    let badge = layout.badge;
    format!(
        r##"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" style="fill:{};"/><circle cx="{:.2}" cy="{:.2}" r="{:.2}" style="fill:#fff;"/><path d="{}" style="fill-rule:evenodd;"/>"##,
        badge.cx,
        badge.cy,
        badge.outer_radius,
        layout.black_fill,
        badge.cx,
        badge.cy,
        badge.inner_radius,
        DOUYIN_NO_BORDER_LOGO_PATH
    )
}

fn black_border_locator_markup(cx: f64, cy: f64, black_fill: &str) -> String {
    format!(
        r##"<circle cx="{cx:.2}" cy="{cy:.2}" r="36.85" style="fill:#fff;"/><circle cx="{cx:.2}" cy="{cy:.2}" r="28.71" style="fill:{black_fill};"/><circle cx="{cx:.2}" cy="{cy:.2}" r="18.24" style="fill:#fff;"/><circle cx="{cx:.2}" cy="{cy:.2}" r="8.05" style="fill:{black_fill};"/>"##
    )
}

fn no_border_locator_markup(cx: f64, cy: f64, layout: DouyinNoBorderLayout) -> String {
    let (outer, middle, inner) = layout.locator_radii;
    let black_fill = layout.black_fill;
    format!(
        r##"<circle cx="{cx:.2}" cy="{cy:.2}" r="{outer:.2}" style="fill:{black_fill};"/><circle cx="{cx:.2}" cy="{cy:.2}" r="{middle:.2}" style="fill:#fff;"/><circle cx="{cx:.2}" cy="{cy:.2}" r="{inner:.2}" style="fill:{black_fill};"/>"##
    )
}

fn grid_locator_distance(grid: &DyGrid) -> f64 {
    grid.finders
        .iter()
        .map(|finder| (finder.cx - grid.center.0).hypot(finder.cy - grid.center.1))
        .sum::<f64>()
        / grid.finders.len() as f64
}

fn dy_outer_radius(grid: &DyGrid) -> Option<f64> {
    grid.rings
        .iter()
        .map(|ring| ring.r_outer)
        .chain(grid.outer_frame.as_ref().map(|outer| outer.ring.r_outer))
        .chain(
            grid.decorative_rings
                .iter()
                .map(|decorative| decorative.ring.r_outer),
        )
        .max_by(f64::total_cmp)
}

fn mini_program_logo_path(cx: f64, cy: f64, radius: f64) -> String {
    let scale = radius / 40.0;
    const STANDARD_S_PATH: &str = "M333.06,347.8c-.02,1.02-.22,1.9-.54,2.64-.62,1.39-1.77,2.48-3.15,3.22-1.49,.8-3.22,1.19-4.87,1.16-1.09-.02-2.09-.21-2.91-.58-1.62-.72-2.82-1.84-3.56-3.13-.61-1.07-.9-2.26-.82-3.42,.07-1.15,.5-2.28,1.29-3.25,1-1.22,2.61-2.26,4.88-2.88,2.17-.6,3.44-2.84,2.84-5.02-.6-2.17-2.85-3.44-5.02-2.84-4.02,1.12-7,3.12-9.01,5.57-1.95,2.38-2.97,5.13-3.14,7.91-.17,2.76,.49,5.56,1.91,8.03,1.55,2.69,4.02,5.01,7.32,6.47,1.86,.83,3.96,1.26,6.07,1.3,3.01,.05,6.17-.66,8.91-2.14,2.85-1.53,5.28-3.9,6.7-7.08,.77-1.74,1.23-3.67,1.26-5.8l.38-21.11c.02-1.02,.22-1.9,.54-2.64,.62-1.39,1.77-2.48,3.15-3.22,1.49-.8,3.22-1.19,4.87-1.16,1.09,.02,2.09,.21,2.91,.58,1.62,.72,2.82,1.84,3.56,3.13,.61,1.07,.9,2.26,.82,3.42-.07,1.15-.5,2.28-1.29,3.25-1,1.22-2.61,2.26-4.88,2.88-2.17,.6-3.44,2.84-2.84,5.02,.6,2.17,2.85,3.44,5.02,2.84,4.02-1.12,7-3.12,9.01-5.57,1.95-2.38,2.97-5.13,3.14-7.91,.17-2.76-.49-5.56-1.91-8.03-1.55-2.69-4.02-5.01-7.32-6.47-1.86-.83-3.96-1.26-6.07-1.3-3.01-.05-6.17,.66-8.91,2.14-2.85,1.53-5.28,3.9-6.7,7.08-.77,1.74-1.23,3.67-1.26,5.8l-.38,21.11";
    format!(
        r##"<path d="{STANDARD_S_PATH}" fill="#fff" transform="translate({cx:.3} {cy:.3}) scale({scale:.6}) translate(-337.33 -337.33)"/>"##
    )
}

#[derive(Debug, Clone, Copy)]
struct PreviewFitTransform {
    scale: f64,
    offset: (f64, f64),
}

impl PreviewFitTransform {
    fn point(self, point: (f64, f64)) -> (f64, f64) {
        (
            self.offset.0 + point.0 * self.scale,
            self.offset.1 + point.1 * self.scale,
        )
    }

    fn inverse_point(self, point: (f64, f64)) -> (f64, f64) {
        (
            (point.0 - self.offset.0) / self.scale,
            (point.1 - self.offset.1) / self.scale,
        )
    }

    fn radius(self, radius: f64) -> f64 {
        radius * self.scale
    }
}

fn preview_fit_transform(viewbox: (f64, f64), size: u32) -> PreviewFitTransform {
    let side = size.max(1) as f64 - 1.0;
    let scale = side / viewbox.0.max(viewbox.1).max(1.0);
    PreviewFitTransform {
        scale,
        offset: (
            (side - viewbox.0 * scale) * 0.5,
            (side - viewbox.1 * scale) * 0.5,
        ),
    }
}

fn no_border_layout_to_source_point(
    grid: &DyGrid,
    layout: DouyinNoBorderLayout,
    svg_scale: f64,
    rotation: f64,
    point: (f64, f64),
) -> (f64, f64) {
    let dx = point.0 - layout.center.0;
    let dy = point.1 - layout.center.1;
    let radius = dx.hypot(dy) / svg_scale.max(f64::EPSILON);
    let theta = dy.atan2(dx) + rotation;
    (
        grid.center.0 + radius * theta.cos(),
        grid.center.1 + radius * theta.sin(),
    )
}

fn paint_no_border_static_marks(
    image: &mut RgbaImage,
    layout: DouyinNoBorderLayout,
    transform: PreviewFitTransform,
) {
    let black = Rgba([0, 0, 0, 255]);
    let white = Rgba([255, 255, 255, 255]);
    let (outer, middle, inner) = layout.locator_radii;

    for locator in layout.locators {
        let center = transform.point(locator);
        paint_filled_circle(image, center, transform.radius(outer), black);
        paint_filled_circle(image, center, transform.radius(middle), white);
        paint_filled_circle(image, center, transform.radius(inner), black);
    }

    let badge = layout.badge;
    let center = transform.point((badge.cx, badge.cy));
    paint_filled_circle(image, center, transform.radius(badge.outer_radius), black);
    paint_filled_circle(image, center, transform.radius(badge.inner_radius), white);
    paint_douyin_logo_shape(
        image,
        center,
        transform.radius(badge.inner_radius),
        Rgba([0, 0, 0, 255]),
    );
}

fn polar_point(cx: f64, cy: f64, radius: f64, theta: f64) -> (f64, f64) {
    (cx + radius * theta.cos(), cy + radius * theta.sin())
}

fn polar_point_px(center: (f64, f64), radius: f64, theta: f64) -> (f64, f64) {
    (
        center.0 + radius * theta.cos(),
        center.1 + radius * theta.sin(),
    )
}

fn scaled_polar_point(center: f64, scale: f64, radius: f64, theta: f64) -> (f64, f64) {
    (
        center + radius * scale * theta.cos(),
        center + radius * scale * theta.sin(),
    )
}

#[derive(Debug, Clone, Copy)]
struct DyRun {
    start: u32,
    end: u32,
}

impl DyRun {
    fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }
}

#[derive(Debug, Clone, Copy)]
struct DyMarkGeometry {
    radius: f64,
    stroke_width: f64,
    r_inner: f64,
    r_outer: f64,
    theta_start: f64,
    theta_end: f64,
}

impl DyMarkGeometry {
    fn theta_mid(self) -> f64 {
        (self.theta_start + self.theta_end) * 0.5
    }

    fn with_radial_offset(mut self, offset: f64) -> Self {
        if offset.abs() <= f64::EPSILON {
            return self;
        }

        let half_width = self.stroke_width * 0.5;
        self.radius = (self.radius + offset).max(half_width);
        self.r_inner = (self.radius - half_width).max(0.0);
        self.r_outer = self.radius + half_width;
        self
    }

    fn with_angular_offset(mut self, offset: f64) -> Self {
        if offset.abs() <= f64::EPSILON {
            return self;
        }

        self.theta_start += offset;
        self.theta_end += offset;
        self
    }
}

fn dy_sample_runs(grid: &DyGrid, ring: u32) -> Vec<DyRun> {
    let points = grid.points_per_ring;
    dy_runs_from_samples(points, |point| grid.sample(ring, point))
}

fn dy_run_radial_offset(grid: &DyGrid, ring: u32, run: DyRun) -> f64 {
    if !no_border_grid_has_radial_offsets(grid) {
        return 0.0;
    }

    let points = grid.points_per_ring as usize;
    let ring_offset = ring as usize * points;
    if ring_offset + points > grid.sample_radial_offsets.len() {
        return 0.0;
    }

    let mut sum = 0.0;
    let mut total = 0_u32;
    for offset in 0..run.len() {
        let point = ((run.start + offset) % grid.points_per_ring) as usize;
        sum += grid.sample_radial_offsets[ring_offset + point];
        total += 1;
    }

    if total == 0 {
        0.0
    } else {
        sum / f64::from(total)
    }
}

fn dy_run_tangential_offset(grid: &DyGrid, ring: u32, run: DyRun) -> f64 {
    if !no_border_grid_has_tangential_offsets(grid) {
        return 0.0;
    }

    let points = grid.points_per_ring as usize;
    let ring_offset = ring as usize * points;
    if ring_offset + points > grid.sample_tangential_offsets.len() {
        return 0.0;
    }

    let mut sum = 0.0;
    let mut total = 0_u32;
    for offset in 0..run.len() {
        let point = ((run.start + offset) % grid.points_per_ring) as usize;
        sum += grid.sample_tangential_offsets[ring_offset + point];
        total += 1;
    }

    if total == 0 {
        0.0
    } else {
        sum / f64::from(total)
    }
}

fn no_border_grid_has_radial_offsets(grid: &DyGrid) -> bool {
    !grid.has_border
        && grid.points_per_ring != 0
        && grid.sample_radial_offsets.len() == grid.samples.len()
}

fn no_border_grid_has_tangential_offsets(grid: &DyGrid) -> bool {
    !grid.has_border
        && grid.points_per_ring != 0
        && grid.sample_tangential_offsets.len() == grid.samples.len()
}

fn no_border_standard_render_ring(grid: &DyGrid, ring_idx: usize) -> Option<RingSpec> {
    let radius_offset =
        DOUYIN_NO_BORDER_RENDER_RADIUS_OFFSET + no_border_render_ring_radius_offset(grid, ring_idx);
    no_border_standard_render_ring_with_radius_offset(ring_idx, radius_offset)
}

/// 把一条装饰环（采样态）按中心半径就近匹配到标准环表中的装饰环序号（ring0/ring2），
/// 以便复用编码环的标准几何/线宽/圆点逻辑渲染。`scale` 把采样半径映回 layout 单位。
fn no_border_decorative_ring_index(ring: &RingSpec, scale: f64) -> usize {
    let mid = (ring.r_inner + ring.r_outer) * 0.5 * scale;
    DOUYIN_NO_BORDER_STANDARD_RINGS
        .iter()
        .enumerate()
        .filter(|(_, spec)| spec.2)
        .min_by(|(_, a), (_, b)| {
            (a.0 - mid)
                .abs()
                .partial_cmp(&(b.0 - mid).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn no_border_apply_render_ring_width_scale(
    mut mark: DyMarkGeometry,
    ring_idx: usize,
    run: DyRun,
) -> DyMarkGeometry {
    if run.len() <= 1 {
        return mark;
    }

    let width_scale = no_border_render_ring_width_scale_for_run(ring_idx, run);
    if (width_scale - 1.0).abs() <= f64::EPSILON {
        return mark;
    }

    mark.stroke_width *= width_scale;
    let half_width = mark.stroke_width * 0.5;
    mark.r_inner = (mark.radius - half_width).max(0.0);
    mark.r_outer = mark.radius + half_width;
    mark
}

fn no_border_apply_run_offsets(
    mark: DyMarkGeometry,
    grid: &DyGrid,
    ring_idx: usize,
    run: DyRun,
    radial_scale: f64,
    theta_step: f64,
) -> DyMarkGeometry {
    let radial_offset = if grid
        .rings
        .get(ring_idx)
        .is_some_and(|ring| ring.is_decoration && run.len() <= 1)
    {
        0.0
    } else {
        dy_run_radial_offset(grid, ring_idx as u32, run)
            * radial_scale
            * no_border_run_radial_offset_scale(ring_idx, run)
    };

    mark.with_radial_offset(radial_offset).with_angular_offset(
        dy_run_tangential_offset(grid, ring_idx as u32, run)
            * theta_step
            * no_border_run_tangential_offset_scale(ring_idx),
    )
}

fn no_border_render_ring_radius_offset(grid: &DyGrid, ring_idx: usize) -> f64 {
    grid.render_ring_radius_offsets
        .get(ring_idx)
        .copied()
        .or_else(|| {
            DOUYIN_NO_BORDER_RENDER_RING_RADIUS_OFFSETS
                .get(ring_idx)
                .copied()
        })
        .unwrap_or_default()
}

fn no_border_render_ring_width_scale(ring_idx: usize) -> f64 {
    DOUYIN_NO_BORDER_RENDER_RING_WIDTH_SCALES
        .get(ring_idx)
        .copied()
        .unwrap_or(1.0)
}

fn no_border_render_ring_width_scale_for_run(ring_idx: usize, run: DyRun) -> f64 {
    if ring_idx == 1 && run.len() > 2 {
        return DOUYIN_NO_BORDER_RING1_LONG_RUN_WIDTH_SCALE;
    }

    no_border_render_ring_width_scale(ring_idx)
}

fn no_border_single_dot_width_scale(ring_idx: usize) -> f64 {
    DOUYIN_NO_BORDER_SINGLE_DOT_WIDTH_SCALES
        .get(ring_idx)
        .copied()
        .unwrap_or(1.0)
}

fn no_border_run_radial_offset_scale(ring_idx: usize, run: DyRun) -> f64 {
    if run.len() <= 1 {
        return 1.0;
    }

    DOUYIN_NO_BORDER_MULTI_RUN_RADIAL_OFFSET_SCALES
        .get(ring_idx)
        .copied()
        .unwrap_or(0.0)
}

fn no_border_run_tangential_offset_scale(ring_idx: usize) -> f64 {
    DOUYIN_NO_BORDER_RUN_TANGENTIAL_OFFSET_SCALES
        .get(ring_idx)
        .copied()
        .unwrap_or(0.0)
}

fn no_border_short_run_angular_inset(ring_idx: usize) -> f64 {
    DOUYIN_NO_BORDER_SHORT_RUN_ANGULAR_INSETS
        .get(ring_idx)
        .copied()
        .unwrap_or(DOUYIN_NO_BORDER_SHORT_RUN_ANGULAR_INSET)
}

fn no_border_long_run_angular_inset(ring_idx: usize) -> f64 {
    DOUYIN_NO_BORDER_LONG_RUN_ANGULAR_INSETS
        .get(ring_idx)
        .copied()
        .unwrap_or(DOUYIN_NO_BORDER_LONG_RUN_ANGULAR_INSET)
}

fn no_border_standard_render_ring_with_radius_offset(
    ring_idx: usize,
    radius_offset: f64,
) -> Option<RingSpec> {
    DOUYIN_NO_BORDER_STANDARD_RINGS
        .get(ring_idx)
        .map(|&(radius, half_width, is_decoration)| RingSpec {
            r_inner: (radius + radius_offset - half_width).max(0.0),
            r_outer: radius + radius_offset + half_width,
            is_decoration,
        })
}

fn dy_runs_from_samples(points: u32, mut is_black: impl FnMut(u32) -> bool) -> Vec<DyRun> {
    if points == 0 {
        return Vec::new();
    }

    let Some(first_white) = (0..points).find(|&point| !is_black(point)) else {
        return vec![DyRun {
            start: 0,
            end: points,
        }];
    };
    let base = first_white + 1;
    let mut runs = Vec::new();
    let mut run_start: Option<u32> = None;

    for offset in 0..points {
        let point = (base + offset) % points;
        if is_black(point) {
            run_start.get_or_insert(offset);
        } else if let Some(start) = run_start.take() {
            runs.push(DyRun {
                start: base + start,
                end: base + offset,
            });
        }
    }

    if let Some(start) = run_start {
        runs.push(DyRun {
            start: base + start,
            end: base + points,
        });
    }

    runs
}

fn dy_mark_geometry(
    has_border: bool,
    theta_offset: f64,
    ring: &RingSpec,
    run: DyRun,
    theta_step: f64,
) -> Option<DyMarkGeometry> {
    let run_len = run.end.checked_sub(run.start)?;
    if run_len == 0 {
        return None;
    }

    let radial_step = (ring.r_outer - ring.r_inner).max(0.01);
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    if has_border {
        let angular_inset = theta_step * if run_len == 1 { 0.04 } else { 0.01 };
        let theta_start = theta_offset + run.start as f64 * theta_step + angular_inset;
        let theta_end = theta_offset + run.end as f64 * theta_step - angular_inset;
        if theta_end <= theta_start {
            return None;
        }

        return Some(DyMarkGeometry {
            radius,
            stroke_width: radial_step,
            r_inner: ring.r_inner,
            r_outer: ring.r_outer,
            theta_start,
            theta_end,
        });
    }

    dy_no_border_mark_geometry_with_insets(
        theta_offset,
        ring,
        run,
        theta_step,
        DOUYIN_NO_BORDER_SHORT_RUN_ANGULAR_INSET,
        DOUYIN_NO_BORDER_LONG_RUN_ANGULAR_INSET,
    )
}

fn dy_no_border_mark_geometry_for_ring(
    ring_idx: usize,
    theta_offset: f64,
    ring: &RingSpec,
    run: DyRun,
    theta_step: f64,
) -> Option<DyMarkGeometry> {
    dy_no_border_mark_geometry_with_insets(
        theta_offset,
        ring,
        run,
        theta_step,
        no_border_short_run_angular_inset(ring_idx),
        no_border_long_run_angular_inset(ring_idx),
    )
}

fn dy_no_border_mark_geometry_with_insets(
    theta_offset: f64,
    ring: &RingSpec,
    run: DyRun,
    theta_step: f64,
    short_run_angular_inset: f64,
    long_run_angular_inset: f64,
) -> Option<DyMarkGeometry> {
    let run_len = run.end.checked_sub(run.start)?;
    if run_len == 0 {
        return None;
    }

    let radial_step = (ring.r_outer - ring.r_inner).max(0.01);
    let radius = (ring.r_inner + ring.r_outer) * 0.5;
    let stroke_width = if ring.is_decoration && run_len > 1 {
        radial_step * DOUYIN_NO_BORDER_DECORATIVE_RUN_WIDTH_SCALE
    } else if run_len > 1 {
        radial_step * DOUYIN_NO_BORDER_CODE_RUN_WIDTH_SCALE
    } else {
        radial_step
    };
    let angular_inset = theta_step
        * if run_len == 1 {
            0.26
        } else if run_len == 2 {
            short_run_angular_inset
        } else {
            long_run_angular_inset
        };
    let theta_start = theta_offset + run.start as f64 * theta_step + angular_inset;
    let theta_end = theta_offset + run.end as f64 * theta_step - angular_inset;
    if theta_end <= theta_start {
        return None;
    }

    let half_width = stroke_width * 0.5;
    Some(DyMarkGeometry {
        radius,
        stroke_width,
        r_inner: (radius - half_width).max(0.0),
        r_outer: radius + half_width,
        theta_start,
        theta_end,
    })
}

fn rounded_arc_bar_path(
    cx: f64,
    cy: f64,
    radius: f64,
    stroke_width: f64,
    theta_start: f64,
    theta_end: f64,
) -> String {
    let half_width = stroke_width * 0.5;
    let r_outer = radius + half_width;
    let r_inner = (radius - half_width).max(0.0);
    let outer_start = polar_point(cx, cy, r_outer, theta_start);
    let outer_end = polar_point(cx, cy, r_outer, theta_end);
    let inner_end = polar_point(cx, cy, r_inner, theta_end);
    let inner_start = polar_point(cx, cy, r_inner, theta_start);
    let large_arc = i32::from((theta_end - theta_start).abs() > std::f64::consts::PI);

    format!(
        "M {:.3} {:.3} A {r_outer:.3} {r_outer:.3} 0 {large_arc} 1 {:.3} {:.3} A {half_width:.3} {half_width:.3} 0 0 1 {:.3} {:.3} A {r_inner:.3} {r_inner:.3} 0 {large_arc} 0 {:.3} {:.3} A {half_width:.3} {half_width:.3} 0 0 1 {:.3} {:.3} Z",
        outer_start.0,
        outer_start.1,
        outer_end.0,
        outer_end.1,
        inner_end.0,
        inner_end.1,
        inner_start.0,
        inner_start.1,
        outer_start.0,
        outer_start.1
    )
}

fn paint_capsule(
    image: &mut RgbaImage,
    start: (f64, f64),
    end: (f64, f64),
    radius: f64,
    color: Rgba<u8>,
) {
    let min_x = ((start.0.min(end.0) - radius).floor() as i32).max(0);
    let max_x = ((start.0.max(end.0) + radius).ceil() as i32).min(image.width() as i32 - 1);
    let min_y = ((start.1.min(end.1) - radius).floor() as i32).max(0);
    let max_y = ((start.1.max(end.1) + radius).ceil() as i32).min(image.height() as i32 - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            if distance_to_segment((px, py), start, end) <= radius {
                image.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn paint_arc_stroke(
    image: &mut RgbaImage,
    center: f64,
    scale: f64,
    stroke: RasterArcStroke,
    color: Rgba<u8>,
) {
    let span = (stroke.theta_end - stroke.theta_start).abs();
    if span <= f64::EPSILON || stroke.stroke_radius <= 0.0 {
        return;
    }

    let arc_len = span * stroke.radius * scale;
    let steps = (arc_len / (stroke.stroke_radius.max(1.0) * 0.75))
        .ceil()
        .clamp(2.0, 96.0) as u32;
    let mut previous = scaled_polar_point(center, scale, stroke.radius, stroke.theta_start);
    for step in 1..=steps {
        let t = step as f64 / steps as f64;
        let theta = stroke.theta_start + (stroke.theta_end - stroke.theta_start) * t;
        let next = scaled_polar_point(center, scale, stroke.radius, theta);
        paint_capsule(image, previous, next, stroke.stroke_radius, color);
        previous = next;
    }
}

fn paint_rounded_arc_bar(
    image: &mut RgbaImage,
    center: (f64, f64),
    scale: f64,
    stroke: RasterArcStroke,
    color: Rgba<u8>,
) {
    let span = (stroke.theta_end - stroke.theta_start).abs();
    let stroke_radius = stroke.stroke_radius;
    if span <= f64::EPSILON || stroke.radius <= 0.0 || stroke_radius <= 0.0 {
        return;
    }

    let radius = stroke.radius * scale;
    let outer = radius + stroke_radius;
    let min_x = (center.0 - outer).floor().max(0.0) as i32;
    let max_x = (center.0 + outer).ceil().min(image.width() as f64 - 1.0) as i32;
    let min_y = (center.1 - outer).floor().max(0.0) as i32;
    let max_y = (center.1 + outer).ceil().min(image.height() as f64 - 1.0) as i32;
    let start = polar_point_px(center, radius, stroke.theta_start);
    let end = polar_point_px(center, radius, stroke.theta_end);
    let radius2 = stroke_radius * stroke_radius;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let point = (x as f64 + 0.5, y as f64 + 0.5);
            let dx = point.0 - center.0;
            let dy = point.1 - center.1;
            let point_radius = dx.hypot(dy);
            let radial_distance = (point_radius - radius).abs();
            if radial_distance <= stroke_radius
                && angle_in_span(dy.atan2(dx), stroke.theta_start, stroke.theta_end)
            {
                image.put_pixel(x as u32, y as u32, color);
                continue;
            }

            let start_dx = point.0 - start.0;
            let start_dy = point.1 - start.1;
            let end_dx = point.0 - end.0;
            let end_dy = point.1 - end.1;
            if start_dx * start_dx + start_dy * start_dy <= radius2
                || end_dx * end_dx + end_dy * end_dy <= radius2
            {
                image.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn paint_mini_program_logo(image: &mut RgbaImage, center: (f64, f64), radius: f64) {
    let white = Rgba([255, 255, 255, 255]);
    let stroke = radius * 0.10;
    let mut points = Vec::new();

    push_cubic_points(
        &mut points,
        logo_point(center, radius, -0.36, 0.22),
        logo_point(center, radius, -0.52, 0.42),
        logo_point(center, radius, -0.10, 0.55),
        logo_point(center, radius, 0.01, 0.25),
        18,
    );
    push_cubic_points(
        &mut points,
        logo_point(center, radius, 0.01, 0.25),
        logo_point(center, radius, 0.05, 0.10),
        logo_point(center, radius, 0.04, -0.06),
        logo_point(center, radius, 0.04, -0.24),
        10,
    );
    push_cubic_points(
        &mut points,
        logo_point(center, radius, 0.04, -0.24),
        logo_point(center, radius, 0.08, -0.54),
        logo_point(center, radius, 0.52, -0.42),
        logo_point(center, radius, 0.38, -0.12),
        18,
    );

    for pair in points.windows(2) {
        paint_capsule(image, pair[0], pair[1], stroke, white);
    }
}

fn paint_douyin_logo(image: &mut RgbaImage, center: (f64, f64), radius: f64) {
    let offset = radius * 0.04;
    paint_douyin_logo_shape(
        image,
        (center.0 + offset, center.1 + offset),
        radius,
        Rgba([250, 30, 92, 255]),
    );
    paint_douyin_logo_shape(
        image,
        (center.0 - offset, center.1 - offset),
        radius,
        Rgba([95, 253, 255, 255]),
    );
    paint_douyin_logo_shape(image, center, radius, Rgba([0, 0, 0, 255]));
}

fn paint_douyin_logo_shape(
    image: &mut RgbaImage,
    center: (f64, f64),
    radius: f64,
    color: Rgba<u8>,
) {
    let stroke = radius * 0.095;

    paint_capsule(
        image,
        logo_point(center, radius, 0.10, -0.48),
        logo_point(center, radius, 0.10, 0.22),
        stroke,
        color,
    );
    paint_capsule(
        image,
        logo_point(center, radius, 0.10, -0.46),
        logo_point(center, radius, 0.44, -0.25),
        stroke,
        color,
    );
    paint_capsule(
        image,
        logo_point(center, radius, 0.40, -0.25),
        logo_point(center, radius, 0.54, -0.12),
        stroke * 0.72,
        color,
    );
    paint_filled_circle(
        image,
        logo_point(center, radius, -0.18, 0.34),
        radius * 0.21,
        color,
    );
    paint_capsule(
        image,
        logo_point(center, radius, -0.18, 0.34),
        logo_point(center, radius, 0.08, 0.22),
        stroke,
        color,
    );
}

fn paint_douyin_bullseye_badge(image: &mut RgbaImage, center: (f64, f64), radius: f64) {
    let black = Rgba([0, 0, 0, 255]);
    let white = Rgba([255, 255, 255, 255]);

    paint_filled_circle(image, center, radius * 0.45, black);
    paint_filled_circle(image, center, radius * 0.27, white);
    paint_filled_circle(image, center, radius * 0.11, black);

    for idx in 0..8 {
        let theta = idx as f64 * std::f64::consts::TAU / 8.0;
        let (sin, cos) = theta.sin_cos();
        let start = (
            center.0 + cos * radius * 0.61,
            center.1 + sin * radius * 0.61,
        );
        let end = (
            center.0 + cos * radius * 0.71,
            center.1 + sin * radius * 0.71,
        );
        paint_capsule(image, start, end, radius * 0.018, black);
    }
}

fn paint_ring_sector(
    image: &mut RgbaImage,
    center: f64,
    scale: f64,
    sector: RasterSector,
    color: Rgba<u8>,
) {
    let max_radius = sector.r_outer * scale;
    let min_x = (center - max_radius).floor().max(0.0) as i32;
    let max_x = (center + max_radius).ceil().min(image.width() as f64 - 1.0) as i32;
    let min_y = (center - max_radius).floor().max(0.0) as i32;
    let max_y = (center + max_radius)
        .ceil()
        .min(image.height() as f64 - 1.0) as i32;
    let inner = sector.r_inner * scale;
    let outer = sector.r_outer * scale;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f64 + 0.5 - center;
            let dy = y as f64 + 0.5 - center;
            let radius = dx.hypot(dy);
            if radius < inner || radius > outer {
                continue;
            }
            let theta = normalize_angle(dy.atan2(dx));
            if angle_in_span(theta, sector.theta_start, sector.theta_end) {
                image.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RasterSector {
    r_inner: f64,
    r_outer: f64,
    theta_start: f64,
    theta_end: f64,
}

#[derive(Debug, Clone, Copy)]
struct RasterArcStroke {
    radius: f64,
    theta_start: f64,
    theta_end: f64,
    stroke_radius: f64,
}

fn is_dy_diff_ignored(grid: &DyGrid, point: (f64, f64)) -> bool {
    let radius = (point.0 - grid.center.0).hypot(point.1 - grid.center.1);
    let Some(r_outer) = dy_outer_radius(grid) else {
        return true;
    };
    let Some(inner_ring) = grid.rings.last() else {
        return true;
    };
    if radius > r_outer * 1.03 || radius < inner_ring.r_inner * 0.96 {
        return true;
    }

    grid.finders.iter().any(|finder| {
        (point.0 - finder.cx).hypot(point.1 - finder.cy) <= finder.outer_radius() * 1.25
    }) || grid.badge.is_some_and(|badge| {
        let scale = if grid.has_border {
            DOUYIN_BLACK_BORDER_BADGE_OUTER_RADIUS_SCALE * 1.02
        } else {
            1.08
        };
        (point.0 - badge.cx).hypot(point.1 - badge.cy) <= badge.radius * scale
    }) || grid
        .center_logo
        .is_some_and(|logo| (point.0 - logo.cx).hypot(point.1 - logo.cy) <= logo.radius * 1.06)
}

fn normalize_angle(theta: f64) -> f64 {
    theta.rem_euclid(std::f64::consts::TAU)
}

fn angle_in_span(theta: f64, start: f64, end: f64) -> bool {
    let theta = normalize_angle(theta);
    let start = normalize_angle(start);
    let end = normalize_angle(end);
    if start <= end {
        theta >= start && theta <= end
    } else {
        theta >= start || theta <= end
    }
}

fn logo_point(center: (f64, f64), radius: f64, x: f64, y: f64) -> (f64, f64) {
    (center.0 + radius * x, center.1 + radius * y)
}

fn push_cubic_points(
    points: &mut Vec<(f64, f64)>,
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    steps: u32,
) {
    let start = if points.is_empty() { 0 } else { 1 };
    for step in start..=steps {
        let t = step as f64 / steps as f64;
        let mt = 1.0 - t;
        points.push((
            mt.powi(3) * p0.0
                + 3.0 * mt.powi(2) * t * p1.0
                + 3.0 * mt * t.powi(2) * p2.0
                + t.powi(3) * p3.0,
            mt.powi(3) * p0.1
                + 3.0 * mt.powi(2) * t * p1.1
                + 3.0 * mt * t.powi(2) * p2.1
                + t.powi(3) * p3.1,
        ));
    }
}

fn paint_filled_rect_px(
    image: &mut RgbaImage,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: Rgba<u8>,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let min_x = x.floor().max(0.0) as i32;
    let max_x = (x + width).ceil().min(image.width() as f64) as i32 - 1;
    let min_y = y.floor().max(0.0) as i32;
    let max_y = (y + height).ceil().min(image.height() as f64) as i32 - 1;
    if max_x < min_x || max_y < min_y {
        return;
    }

    for yy in min_y..=max_y {
        for xx in min_x..=max_x {
            image.put_pixel(xx as u32, yy as u32, color);
        }
    }
}

fn paint_filled_round_rect(
    image: &mut RgbaImage,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
    color: Rgba<u8>,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let radius = radius.max(0.0).min(width.min(height) * 0.5);
    let min_x = x.floor().max(0.0) as i32;
    let max_x = (x + width).ceil().min(image.width() as f64) as i32 - 1;
    let min_y = y.floor().max(0.0) as i32;
    let max_y = (y + height).ceil().min(image.height() as f64) as i32 - 1;
    if max_x < min_x || max_y < min_y {
        return;
    }

    for yy in min_y..=max_y {
        for xx in min_x..=max_x {
            let px = xx as f64 + 0.5;
            let py = yy as f64 + 0.5;
            let dx = if px < x + radius {
                x + radius - px
            } else if px > x + width - radius {
                px - (x + width - radius)
            } else {
                0.0
            };
            let dy = if py < y + radius {
                y + radius - py
            } else if py > y + height - radius {
                py - (y + height - radius)
            } else {
                0.0
            };
            if dx * dx + dy * dy <= radius * radius {
                image.put_pixel(xx as u32, yy as u32, color);
            }
        }
    }
}

fn paint_filled_ellipse(
    image: &mut RgbaImage,
    center: (f64, f64),
    radius_x: f64,
    radius_y: f64,
    color: Rgba<u8>,
) {
    if radius_x <= 0.0 || radius_y <= 0.0 {
        return;
    }
    let min_x = (center.0 - radius_x).floor().max(0.0) as i32;
    let max_x = (center.0 + radius_x).ceil().min(image.width() as f64) as i32 - 1;
    let min_y = (center.1 - radius_y).floor().max(0.0) as i32;
    let max_y = (center.1 + radius_y).ceil().min(image.height() as f64) as i32 - 1;
    if max_x < min_x || max_y < min_y {
        return;
    }

    for yy in min_y..=max_y {
        for xx in min_x..=max_x {
            let dx = (xx as f64 + 0.5 - center.0) / radius_x;
            let dy = (yy as f64 + 0.5 - center.1) / radius_y;
            if dx * dx + dy * dy <= 1.0 {
                image.put_pixel(xx as u32, yy as u32, color);
            }
        }
    }
}

fn paint_filled_circle(image: &mut RgbaImage, center: (f64, f64), radius: f64, color: Rgba<u8>) {
    let min_x = ((center.0 - radius).floor() as i32).max(0);
    let max_x = ((center.0 + radius).ceil() as i32).min(image.width() as i32 - 1);
    let min_y = ((center.1 - radius).floor() as i32).max(0);
    let max_y = ((center.1 + radius).ceil() as i32).min(image.height() as i32 - 1);
    let radius2 = radius * radius;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f64 + 0.5 - center.0;
            let dy = y as f64 + 0.5 - center.1;
            if dx * dx + dy * dy <= radius2 {
                image.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn distance_to_segment(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    let vx = end.0 - start.0;
    let vy = end.1 - start.1;
    let wx = point.0 - start.0;
    let wy = point.1 - start.1;
    let len2 = vx * vx + vy * vy;
    if len2 <= f64::EPSILON {
        return wx.hypot(wy);
    }

    let t = ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0);
    let closest = (start.0 + t * vx, start.1 + t * vy);
    (point.0 - closest.0).hypot(point.1 - closest.1)
}
