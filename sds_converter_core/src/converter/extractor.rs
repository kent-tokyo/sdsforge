use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::SdsError;

/// Default maximum characters to send to the LLM — consistent with ConvertConfig::default().
const DEFAULT_MAX_LLM_CHARS: usize = 80_000;

pub enum InputFormat {
    Pdf,
    Docx,
    Txt,
    Xlsx,
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
        Some("xlsx") | Some("xls") | Some("xlsm") => Ok(InputFormat::Xlsx),
        Some(ext) => Err(SdsError::UnsupportedFormat(ext.to_string())),
        None => Err(SdsError::UnsupportedFormat("(no extension)".to_string())),
    }
}

pub async fn extract_text(path: &Path) -> Result<String, SdsError> {
    extract_text_limited(path, DEFAULT_MAX_LLM_CHARS).await
}

/// Like [`extract_text`] but truncates to `max_chars` after cleaning.
pub async fn extract_text_limited(path: &Path, max_chars: usize) -> Result<String, SdsError> {
    let raw = match detect_format(path)? {
        InputFormat::Pdf => {
            // Run synchronous PDF parsing on a blocking thread to avoid stalling async workers.
            let path = path.to_path_buf();
            tokio::task::spawn_blocking(move || {
                pdf_extract::extract_text(&path).map_err(|e| SdsError::Extract(e.to_string()))
            })
            .await
            .unwrap_or_else(|e| Err(SdsError::Extract(e.to_string())))?
        }
        InputFormat::Docx => extract_text_from_docx(path)?,
        InputFormat::Txt => {
            std::fs::read_to_string(path).map_err(|e| SdsError::Extract(e.to_string()))?
        }
        InputFormat::Xlsx => extract_text_from_xlsx(path)?,
    };
    Ok(clean_extracted_text(&raw, max_chars))
}

/// Clean and condense raw extracted text before sending to the LLM.
///
/// Three passes:
///   1. Remove separator lines, collapse blank runs, strip control chars.
///   2. Deduplicate repeated short lines (PDF page headers/footers).
///   3. Truncate to `max_chars` at a UTF-8 char boundary.
pub fn clean_extracted_text(text: &str, max_chars: usize) -> String {
    // Pass 1 — noise removal
    let mut out = String::with_capacity(text.len().min(max_chars + 1024));
    let mut blank_run = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();

        // Drop control characters and zero-width spaces but keep CJK / Latin content
        let trimmed: String = trimmed
            .chars()
            .filter(|&c| c >= ' ' || c == '\t')
            .collect();
        let trimmed = trimmed.trim();

        // Drop lines that are purely visual separators (─━=─-*•· etc.)
        if !trimmed.is_empty()
            && trimmed.chars().all(|c| {
                matches!(c,
                    '-' | '=' | '_' | '*' | '─' | '━' | '╌' | '╍'
                    | '┄' | '┅' | '┈' | '┉' | '╴' | '╶' | '╸'
                    | '·' | '•' | '~' | '/' | '\\' | '|' | '+' | '#'
                )
            })
            && trimmed.len() >= 3
        {
            continue;
        }

        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push('\n');
            }
        } else {
            blank_run = 0;
            out.push_str(trimmed);
            out.push('\n');
        }
    }

    // Pass 2 — deduplicate repeated short lines (page headers / footers)
    // Any line ≤ 80 chars appearing 3+ times is treated as a repeated header/footer.
    {
        let mut freq: HashMap<String, usize> = HashMap::new();
        for line in out.lines() {
            if line.len() <= 80 {
                *freq.entry(line.to_string()).or_default() += 1;
            }
        }
        let mut first_seen: HashSet<String> = HashSet::new();
        let mut deduped = String::with_capacity(out.len());
        for line in out.lines() {
            let count = freq.get(line).copied().unwrap_or(1);
            if line.len() <= 80 && count >= 3 {
                if first_seen.insert(line.to_string()) {
                    deduped.push_str(line);
                    deduped.push('\n');
                }
            } else {
                deduped.push_str(line);
                deduped.push('\n');
            }
        }
        out = deduped;
    }

    // Pass 3 — truncate to max_chars at a valid UTF-8 char boundary
    if out.len() > max_chars {
        let mut at = max_chars;
        while at > 0 && !out.is_char_boundary(at) {
            at -= 1;
        }
        out.truncate(at);
        out.push_str("\n[テキスト省略]\n");
    }

    out
}

pub fn extract_text_from_docx(path: &Path) -> Result<String, SdsError> {
    let docx = docx_rust::DocxFile::from_file(path)
        .map_err(|e| SdsError::Docx(format!("open failed: {e:?}")))?;
    let docx = docx
        .parse()
        .map_err(|e| SdsError::Docx(format!("parse failed: {e:?}")))?;
    Ok(docx.document.body.text())
}

pub fn extract_text_from_xlsx(path: &Path) -> Result<String, SdsError> {
    use calamine::{open_workbook_auto, Reader};
    let mut wb = open_workbook_auto(path)
        .map_err(|e| SdsError::Extract(format!("xlsx open failed: {e}")))?;
    let mut out = String::new();
    for sheet_name in wb.sheet_names().to_owned() {
        if let Ok(range) = wb.worksheet_range(&sheet_name) {
            for row in range.rows() {
                let cells: Vec<String> = row
                    .iter()
                    .map(|c| c.to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !cells.is_empty() {
                    out.push_str(&cells.join("\t"));
                    out.push('\n');
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separator_lines_are_dropped() {
        let input = "Section 1\n---\nContent\n===\nMore content\n";
        let result = clean_extracted_text(input, 1000);
        assert!(!result.contains("---"));
        assert!(!result.contains("==="));
        assert!(result.contains("Section 1"));
        assert!(result.contains("Content"));
    }

    #[test]
    fn multiple_blank_lines_collapse_to_one() {
        let input = "Line A\n\n\n\nLine B\n";
        let result = clean_extracted_text(input, 1000);
        // Should have at most one blank line between A and B
        assert!(!result.contains("\n\n\n"));
        assert!(result.contains("Line A"));
        assert!(result.contains("Line B"));
    }

    #[test]
    fn cjk_content_passes_through() {
        let input = "第1節 化学品の名称\n製品名：テスト化学物質\n";
        let result = clean_extracted_text(input, 1000);
        assert!(result.contains("第1節"));
        assert!(result.contains("テスト化学物質"));
    }

    #[test]
    fn truncation_lands_on_utf8_boundary() {
        let input: String = "あ".repeat(100);
        let result = clean_extracted_text(&input, 10);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    #[test]
    fn repeated_header_lines_deduplicated() {
        let header = "Company Inc. SDS";
        let mut input = String::new();
        for i in 0..10 {
            input.push_str(header);
            input.push('\n');
            input.push_str(&format!("Section {i} content\n"));
        }
        let result = clean_extracted_text(&input, 10_000);
        let count = result.matches(header).count();
        assert_eq!(count, 1, "header appeared {count} times, expected 1");
    }

    #[test]
    fn short_non_repeated_lines_kept() {
        let input = "Line A\nLine B\nLine C\n";
        let result = clean_extracted_text(input, 1000);
        assert!(result.contains("Line A"));
        assert!(result.contains("Line B"));
        assert!(result.contains("Line C"));
    }
}
