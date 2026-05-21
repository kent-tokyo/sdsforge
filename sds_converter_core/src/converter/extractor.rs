use std::path::Path;

use crate::error::SdsError;

pub enum InputFormat {
    Pdf,
    Docx,
    Txt,
}

pub fn detect_format(path: &Path) -> Result<InputFormat, SdsError> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("pdf") => Ok(InputFormat::Pdf),
        Some("docx") => Ok(InputFormat::Docx),
        Some("txt") => Ok(InputFormat::Txt),
        Some(ext) => Err(SdsError::UnsupportedFormat(ext.to_string())),
        None => Err(SdsError::UnsupportedFormat("(no extension)".to_string())),
    }
}

pub fn extract_text(path: &Path) -> Result<String, SdsError> {
    match detect_format(path)? {
        InputFormat::Pdf => extract_text_from_pdf(path),
        InputFormat::Docx => extract_text_from_docx(path),
        InputFormat::Txt => {
            std::fs::read_to_string(path).map_err(|e| SdsError::PdfExtract(e.to_string()))
        }
    }
}

pub fn extract_text_from_pdf(path: &Path) -> Result<String, SdsError> {
    pdf_extract::extract_text(path).map_err(|e| SdsError::PdfExtract(e.to_string()))
}

pub fn extract_text_from_docx(path: &Path) -> Result<String, SdsError> {
    let docx = docx_rust::DocxFile::from_file(path)
        .map_err(|e| SdsError::Docx(format!("open failed: {e:?}")))?;
    let docx = docx
        .parse()
        .map_err(|e| SdsError::Docx(format!("parse failed: {e:?}")))?;
    Ok(docx.document.body.text())
}
