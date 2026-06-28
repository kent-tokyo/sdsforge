/// Pure-Rust PDF generation via harumi's `html` feature.
use crate::error::SdsError;
use crate::language::Language;
use crate::schema::SdsRoot;

use super::html::generate_html;

/// Generate a PDF document from an [`SdsRoot`] in the given language.
///
/// Converts the SDS to HTML via [`generate_html`] and renders it to PDF bytes
/// using harumi. A system CJK font is loaded automatically; returns an error if
/// none can be found.
pub fn generate_pdf(sds: &SdsRoot, lang: Language) -> Result<Vec<u8>, SdsError> {
    let html = generate_html(sds, lang)?;
    let font_bytes = load_cjk_font()?;
    let options = harumi::HtmlRenderOptions {
        font_bytes,
        ..Default::default()
    };
    harumi::render_html_to_pdf(&html, options)
        .map_err(|e| SdsError::Extract(format!("PDF render error: {e}")))
}

fn load_cjk_font() -> Result<Vec<u8>, SdsError> {
    #[cfg(target_os = "macos")]
    let candidates: &[&str] = &[
        "/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
    ];
    #[cfg(target_os = "windows")]
    let candidates: &[&str] = &[
        "C:/Windows/Fonts/meiryo.ttc",
        "C:/Windows/Fonts/YuGothM.ttc",
        "C:/Windows/Fonts/msgothic.ttc",
    ];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let candidates: &[&str] = &[
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJKjp-Regular.otf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
    ];
    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            return Ok(data);
        }
    }
    Err(SdsError::Extract(
        "No CJK font found for PDF generation. \
         Install Hiragino (macOS), Meiryo (Windows), or Noto Sans CJK (Linux)."
            .into(),
    ))
}
