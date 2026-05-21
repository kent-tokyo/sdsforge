pub mod extractor;
pub mod generator;
pub mod llm;
pub mod validator;

use std::path::Path;

use crate::error::SdsError;
use crate::language::Language;
use crate::schema::SdsRoot;

pub use extractor::InputFormat;
pub use generator::generate_docx;
pub use llm::{AnthropicBackend, LlmBackend, LlmConfig, OpenAiCompatBackend};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Docx,
}

/// Configuration for document conversion.
#[derive(Debug, Clone, Default)]
pub struct ConvertConfig {
    /// Language hint for the source document. `None` = auto-detect.
    pub source_language: Option<Language>,
    /// Language used for section headings in the generated document.
    pub output_language: Language,
}

/// Extract text from a PDF or DOCX file and convert it to [`SdsRoot`] via LLM.
pub async fn convert_to_json<B: LlmBackend>(
    input_path: &Path,
    backend: &B,
    config: &ConvertConfig,
) -> Result<SdsRoot, SdsError> {
    let text = extractor::extract_text(input_path)?;
    if text.trim().is_empty() {
        return Err(SdsError::PdfExtract(
            "No text extracted — document may be image-only or empty".into(),
        ));
    }
    let sds = llm::extract_sds_from_text(backend, &text, config.source_language).await?;
    validator::validate(&sds);
    Ok(sds)
}

/// Convert an [`SdsRoot`] to a document file (currently `.docx` only).
pub fn convert_from_json(
    sds: &SdsRoot,
    output_path: &Path,
    _format: OutputFormat,
    config: &ConvertConfig,
) -> Result<(), SdsError> {
    generate_docx(sds, output_path, config.output_language)
}
