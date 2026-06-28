use thiserror::Error;

#[derive(Debug, Error)]
pub enum SdsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Extraction failed: {0}")]
    Extract(String),

    #[error("DOCX error: {0}")]
    Docx(String),

    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("LLM API error: {status} - {message}")]
    LlmApi { status: u16, message: String },

    #[error("LLM response parse error: {0}")]
    LlmParse(String),

    #[error("PDF appears to be image-only (OCR unavailable): {0}")]
    ImageOnlyPdf(String),
}

impl SdsError {
    /// Returns a sanitized error message safe for external/client display.
    /// Strips verbose provider error bodies to avoid information disclosure.
    pub fn display_safe(&self) -> String {
        match self {
            SdsError::LlmApi { status, .. } => {
                format!("LLM request failed (HTTP {})", status)
            }
            other => other.to_string(),
        }
    }
}
