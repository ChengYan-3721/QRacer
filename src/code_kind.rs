// 码类型枚举。阶段 1 只用作显示占位，阶段 2 起由 detect 模块自动判定。

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeKind {
    Unknown,
    Qr,
    DataMatrix,
    WxMiniprogram,
    Douyin,
}

impl CodeKind {
    pub const PROCESSABLE: [CodeKind; 4] = [
        CodeKind::Qr,
        CodeKind::DataMatrix,
        CodeKind::WxMiniprogram,
        CodeKind::Douyin,
    ];

    pub const MANUAL_CALIBRATION: [CodeKind; 4] = [
        CodeKind::Qr,
        CodeKind::DataMatrix,
        CodeKind::WxMiniprogram,
        CodeKind::Douyin,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CodeKind::Unknown => "未识别",
            CodeKind::Qr => "二维码 (QR)",
            CodeKind::DataMatrix => "Data Matrix码",
            CodeKind::WxMiniprogram => "小程序码",
            CodeKind::Douyin => "抖音码",
        }
    }

    pub fn can_process(self) -> bool {
        matches!(
            self,
            CodeKind::Qr | CodeKind::DataMatrix | CodeKind::WxMiniprogram | CodeKind::Douyin
        )
    }

    pub fn can_manual_calibrate(self) -> bool {
        matches!(
            self,
            CodeKind::Qr | CodeKind::DataMatrix | CodeKind::WxMiniprogram | CodeKind::Douyin
        )
    }
}

impl fmt::Display for CodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}
