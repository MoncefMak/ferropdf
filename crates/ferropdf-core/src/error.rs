#[derive(Debug, thiserror::Error)]
pub enum FerroError {
    #[error("HTML parse error: {0}")]
    HtmlParse(String),
    #[error("CSS parse error: {0}")]
    CssParse(String),
    #[error("Style error: {0}")]
    Style(String),
    #[error("Layout error: {0}")]
    Layout(String),
    #[error("Font error: {0}")]
    Font(String),
    #[error("Image error: {0}")]
    Image(String),
    #[error("PDF write error: {0}")]
    PdfWrite(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FerroError>;

/// A non-fatal warning collected during rendering.
/// Returned alongside the PDF bytes so callers can diagnose issues
/// (e.g. unsupported CSS properties, missing images, invalid selectors).
#[derive(Debug, Clone)]
pub enum RenderWarning {
    /// A CSS property was parsed but is not rendered (e.g. position:absolute).
    UnsupportedCss { property: String, value: String },
    /// A CSS selector could not be parsed.
    InvalidSelector(String),
    /// An image could not be loaded.
    ImageLoadFailed { src: String, reason: String },
    /// An external stylesheet could not be loaded or parsed.
    StylesheetFailed { path: String, reason: String },
    /// A font could not be found for the requested family/weight/style.
    FontNotFound {
        family: String,
        bold: bool,
        italic: bool,
    },
}

impl std::fmt::Display for RenderWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderWarning::UnsupportedCss { property, value } => {
                write!(
                    f,
                    "unsupported CSS: {}:{} (parsed but not rendered)",
                    property, value
                )
            }
            RenderWarning::InvalidSelector(s) => write!(f, "invalid selector: {}", s),
            RenderWarning::ImageLoadFailed { src, reason } => {
                write!(f, "image load failed: {}: {}", src, reason)
            }
            RenderWarning::StylesheetFailed { path, reason } => {
                write!(f, "stylesheet failed: {}: {}", path, reason)
            }
            RenderWarning::FontNotFound {
                family,
                bold,
                italic,
            } => {
                write!(
                    f,
                    "font not found: {} (bold={}, italic={})",
                    family, bold, italic
                )
            }
        }
    }
}
