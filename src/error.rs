#[derive(thiserror::Error, Debug)]
#[allow(dead_code)]
pub enum QRacerError {
    #[error("insufficient finder patterns: found {0}")]
    InsufficientFinders(usize),
    #[error("unknown code kind")]
    UnknownCodeKind,
    #[error("QR decode failed: {0}")]
    QrDecode(String),
    #[error("clipboard access failed: {0}")]
    Clipboard(String),
    #[error("perspective correction failed: {0}")]
    Perspective(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image format error: {0}")]
    ImageFormat(String),
}

pub type Result<T> = std::result::Result<T, QRacerError>;
