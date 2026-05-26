pub mod extractor;
pub mod generator;
pub mod html;
pub mod llm;
pub mod pdf;
pub mod template;
pub mod validator;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::SdsError;
use crate::language::Language;
use crate::schema::SdsRoot;

// ---------------------------------------------------------------------------
// ConversionReport
// ---------------------------------------------------------------------------

/// Structured report produced alongside every `to-json` conversion.
///
/// Describes what was extracted, what was missing, and any normalisations
/// applied to meet the JIS Z 7253 / MHLW standard.
///
/// The report is `serde`-serialisable so it can be written as JSON by the CLI
/// and consumed as a structured value by web-app / API callers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversionReport {
    /// BCP-47 tag of the detected or user-specified source language
    /// (e.g. `"ja"`, `"en"`, `"zh-CN"`, `"zh-TW"`).
    pub source_language: String,
    /// `true` if the language was inferred from text content rather than
    /// supplied explicitly via `--lang` / `ConvertConfig::source_language`.
    pub language_auto_detected: bool,
    /// MHLW section keys that contain at least one extracted value.
    pub populated_sections: Vec<String>,
    /// MHLW section keys for which no data was found.
    pub empty_sections: Vec<String>,
    /// Notes explaining how source values were normalised to conform to the
    /// current JIS Z 7253 / MHLW standard.
    ///
    /// Example: section heading terminology updated in JIS Z 7253:2019.
    pub standardization_notes: Vec<String>,
    /// Validation warnings emitted by the MHLW schema validator.
    pub warnings: Vec<String>,
}

impl ConversionReport {
    /// Build a `ConversionReport` from a completed [`SdsRoot`] and metadata.
    pub fn from_sds(
        sds: &SdsRoot,
        source_language: Language,
        language_auto_detected: bool,
        warnings: Vec<String>,
    ) -> Self {
        let mut populated = Vec::new();
        let mut empty = Vec::new();

        macro_rules! check_opt {
            ($key:literal, $field:expr) => {
                if $field.is_some() {
                    populated.push($key.to_string());
                } else {
                    empty.push($key.to_string());
                }
            };
        }
        macro_rules! check_vec {
            ($key:literal, $field:expr) => {
                if $field.as_ref().map_or(false, |v: &Vec<_>| !v.is_empty()) {
                    populated.push($key.to_string());
                } else {
                    empty.push($key.to_string());
                }
            };
        }

        check_opt!("Identification",                      sds.identification);
        check_opt!("HazardIdentification",                sds.hazard_identification);
        check_opt!("Composition",                         sds.composition);
        check_opt!("FirstAidMeasures",                    sds.first_aid_measures);
        check_opt!("FireFightingMeasures",                sds.fire_fighting_measures);
        check_opt!("AccidentalReleaseMeasures",           sds.accidental_release_measures);
        check_opt!("HandlingAndStorage",                  sds.handling_and_storage);
        check_opt!("ExposureControlPersonalProtection",   sds.exposure_control_personal_protection);
        check_opt!("PhysicalChemicalProperties",          sds.physical_chemical_properties);
        check_opt!("StabilityReactivity",                 sds.stability_reactivity);
        check_vec!("ToxicologicalInformation",            sds.toxicological_information);
        check_vec!("EcologicalInformation",               sds.ecological_information);
        check_opt!("DisposalConsiderations",              sds.disposal_considerations);
        check_opt!("TransportInformation",                sds.transport_information);
        check_opt!("RegulatoryInformation",               sds.regulatory_information);
        check_opt!("OtherInformation",                    sds.other_information);

        let standardization_notes = match source_language {
            Language::Japanese => vec![
                "Section headings follow JIS Z 7253:2019. \
                 Section 1 uses '化学品及び会社情報' (JIS Z 7253:2019); \
                 older source documents that use '製品及び会社情報' are normalised automatically."
                    .to_string(),
            ],
            Language::English => vec![
                "Section headings follow GHS Rev.10 / ISO 11014.".to_string(),
            ],
            Language::ChineseSimplified => vec![
                "Section headings follow GB/T 16483-2012.".to_string(),
            ],
            Language::ChineseTraditional => vec![
                "Section headings follow CNS 15030.".to_string(),
            ],
        };

        Self {
            source_language: source_language.bcp47().to_string(),
            language_auto_detected,
            populated_sections: populated,
            empty_sections: empty,
            standardization_notes,
            warnings,
        }
    }
}

pub use extractor::InputFormat;
pub use generator::generate_docx;
pub use pdf::generate_pdf;
pub use llm::{
    openai_compat_url, AnthropicBackend, AnyBackend, build_any_backend,
    extract_sds_from_pdf_vision, LlmBackend, LlmConfig, OpenAiCompatBackend,
};
pub use template::fill_template;

/// Configuration for document conversion.
#[derive(Debug, Clone)]
pub struct ConvertConfig {
    /// Language hint for the source document. `None` = auto-detect.
    pub source_language: Option<Language>,
    /// Language used for section headings in the generated document.
    pub output_language: Language,
    /// Maximum characters of extracted text sent to the LLM (quality control).
    /// Defaults to 80,000 to capture all 16 SDS sections including transport/regulatory.
    /// CLI overrides this via `--quality` (low=15k, medium=30k, high=60k).
    pub max_chars: usize,
}

impl Default for ConvertConfig {
    fn default() -> Self {
        Self {
            source_language: None,
            output_language: Language::default(),
            max_chars: 80_000,
        }
    }
}

/// Extract text from a PDF or DOCX file and convert it to [`SdsRoot`] via LLM.
///
/// Returns `(SdsRoot, [`ConversionReport`])` with full extraction metadata.
/// Use [`convert_to_json`] for the simpler `(SdsRoot, Vec<String>)` interface.
pub async fn convert_to_json_with_report<B: LlmBackend + Sync>(
    input_path: &Path,
    backend: &B,
    config: &ConvertConfig,
) -> Result<(SdsRoot, ConversionReport), SdsError> {
    let text = extractor::extract_text_limited(input_path, config.max_chars).await?;
    if text.trim().is_empty() {
        return Err(SdsError::Extract(
            "No text extracted — document may be image-only or empty".into(),
        ));
    }
    let language_auto_detected = config.source_language.is_none();
    let (sds, mut warnings) =
        llm::extract_sds_from_text(backend, &text, config.source_language).await?;
    let validation_warnings = validator::validate(&sds);
    warnings.extend(validation_warnings);

    // Determine effective source language for the report.
    let effective_lang = config.source_language.unwrap_or_else(|| {
        crate::language::detect_language(&text)
    });
    let report = ConversionReport::from_sds(&sds, effective_lang, language_auto_detected, warnings);
    Ok((sds, report))
}

/// Extract text from a PDF or DOCX file and convert it to [`SdsRoot`] via LLM.
///
/// Returns `(SdsRoot, Vec<String>)` where the `Vec` contains any warnings:
/// sections skipped due to schema mismatch, and structural validation issues.
///
/// For richer metadata (detected language, empty sections, standardisation notes)
/// use [`convert_to_json_with_report`] instead.
pub async fn convert_to_json<B: LlmBackend + Sync>(
    input_path: &Path,
    backend: &B,
    config: &ConvertConfig,
) -> Result<(SdsRoot, Vec<String>), SdsError> {
    let (sds, report) = convert_to_json_with_report(input_path, backend, config).await?;
    Ok((sds, report.warnings))
}

/// Convert raw document bytes to [`SdsRoot`] via LLM, returning a [`ConversionReport`].
///
/// This is the primary entry point for web / API callers that receive file bytes directly.
/// For the simpler `(SdsRoot, Vec<String>)` interface use [`convert_bytes_to_json`].
pub async fn convert_bytes_to_json_with_report<B: LlmBackend + Sync>(
    data: &[u8],
    filename: &str,
    backend: &B,
    config: &ConvertConfig,
) -> Result<(SdsRoot, ConversionReport), SdsError> {
    let suffix = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_ascii_lowercase()))
        .unwrap_or_default();

    let data_owned = data.to_vec();
    let tmp = tokio::task::spawn_blocking(move || {
        use std::io::Write as _;
        let mut f = tempfile::Builder::new()
            .suffix(&suffix)
            .tempfile()
            .map_err(|e| SdsError::Extract(format!("tempfile create: {e}")))?;
        f.write_all(&data_owned)
            .map_err(|e| SdsError::Extract(format!("tempfile write: {e}")))?;
        f.flush()
            .map_err(|e| SdsError::Extract(format!("tempfile flush: {e}")))?;
        Ok::<_, SdsError>(f.into_temp_path())
    })
    .await
    .map_err(|e| SdsError::Extract(format!("spawn_blocking panicked: {e}")))??;

    convert_to_json_with_report(tmp.as_ref(), backend, config).await
}

/// Convert raw document bytes to [`SdsRoot`] via LLM.
///
/// Writes the bytes to a temporary file (preserving the original extension for format
/// detection), then delegates to [`convert_bytes_to_json_with_report`].
///
/// For richer metadata use [`convert_bytes_to_json_with_report`] instead.
pub async fn convert_bytes_to_json<B: LlmBackend + Sync>(
    data: &[u8],
    filename: &str,
    backend: &B,
    config: &ConvertConfig,
) -> Result<(SdsRoot, Vec<String>), SdsError> {
    let (sds, report) =
        convert_bytes_to_json_with_report(data, filename, backend, config).await?;
    Ok((sds, report.warnings))
}

/// Convert an [`SdsRoot`] to a `.docx` file using the built-in layout.
pub fn convert_from_json(
    sds: &SdsRoot,
    output_path: &Path,
    config: &ConvertConfig,
) -> Result<(), SdsError> {
    generate_docx(sds, output_path, config.output_language)
}

/// Fetch an HTML page from `url`, extract its text, and convert it to [`SdsRoot`] via LLM.
pub async fn convert_url_to_json<B: LlmBackend + Sync>(
    url: &str,
    backend: &B,
    config: &ConvertConfig,
) -> Result<(SdsRoot, Vec<String>), SdsError> {
    let text = extractor::extract_text_from_url_limited(url, config.max_chars).await?;
    if text.trim().is_empty() {
        return Err(SdsError::Extract(
            "No text extracted from URL — page may be empty or JavaScript-rendered".into(),
        ));
    }
    let (sds, mut warnings) =
        llm::extract_sds_from_text(backend, &text, config.source_language).await?;
    let validation_warnings = validator::validate(&sds);
    warnings.extend(validation_warnings);
    Ok((sds, warnings))
}

/// Read a PDF file and extract SDS data using Anthropic's native PDF document API.
///
/// This bypasses text extraction entirely — the raw PDF bytes are base64-encoded and passed
/// directly to the model as a document content block. Use this as a fallback when
/// [`convert_to_json`] fails with [`crate::error::SdsError::ImageOnlyPdf`] (i.e. the PDF is
/// image-only and tesseract is not installed).
///
/// Size limit: 32 MB. Requires an Anthropic API key and a `claude-*` model.
pub async fn convert_pdf_to_json_vision(
    input_path: &Path,
    api_key: &str,
    llm_config: &LlmConfig,
    config: &ConvertConfig,
) -> Result<(SdsRoot, Vec<String>), SdsError> {
    let path_owned = input_path.to_path_buf();
    let bytes = tokio::task::spawn_blocking(move || std::fs::read(&path_owned))
        .await
        .map_err(|e| SdsError::Extract(format!("spawn_blocking panicked: {e}")))?
        .map_err(|e| SdsError::Extract(format!("reading PDF: {e}")))?;
    let (sds, mut warnings) =
        llm::extract_sds_from_pdf_vision(api_key, llm_config, &bytes, config.source_language)
            .await?;
    let validation_warnings = validator::validate(&sds);
    warnings.extend(validation_warnings);
    Ok((sds, warnings))
}

/// Fill a Word template (`.docx`) with data from an [`SdsRoot`].
///
/// Placeholders in the template use `{{FieldName}}` syntax where `FieldName` is
/// the leaf key from the MHLW JSON schema (e.g. `{{TradeNameJP}}`,
/// `{{CompanyName}}`). Full dot-path keys are also accepted for disambiguation
/// (e.g. `{{Identification.SupplierInformation.CompanyName}}`).
pub fn convert_from_template(
    sds: &SdsRoot,
    template_path: &Path,
    output_path: &Path,
) -> Result<(), SdsError> {
    fill_template(sds, template_path, output_path)
}
