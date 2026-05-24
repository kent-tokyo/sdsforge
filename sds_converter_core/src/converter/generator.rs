use std::path::Path;

use docx_rs::*;
use serde_json::Value;

use crate::error::SdsError;
use crate::language::Language;
use crate::schema::SdsRoot;

// JIS Z 7253 section names in 4 languages.
// Order: (Japanese, English, ChineseSimplified, ChineseTraditional)
//
// Sources:
//   EN   — GHS Rev.10 (UN) / ISO 11014:2020 / OSHA HazCom 2012
//   zhCN — GB/T 16483-2012 (中国国家标准)
//   zhTW — CNS 15030 (台灣 GHS 標準)
pub(crate) const SECTION_NAMES: &[(&str, &str, &str, &str)] = &[
    ("化学品及び会社情報",         "Identification",                            "化学品及其企业标识",   "化學品與廠商資料"),
    ("危険有害性の要約",           "Hazard(s) Identification",                  "危险性概述",           "危害辨識資料"),
    ("組成及び成分情報",           "Composition / Information on Ingredients",  "成分/组成信息",        "成分辨識資料"),
    ("応急措置",                   "First-Aid Measures",                        "急救措施",             "急救措施"),
    ("火災時の措置",               "Fire-Fighting Measures",                    "消防措施",             "滅火措施"),
    ("漏出時の措置",               "Accidental Release Measures",               "泄漏应急处理",         "洩漏處理方法"),
    ("取扱い及び保管上の注意",     "Handling and Storage",                      "操作处置与储存",       "安全處置與儲存方法"),
    ("ばく露防止及び保護措置",     "Exposure Controls / Personal Protection",   "接触控制/个体防护",    "暴露預防措施"),
    ("物理的及び化学的性質",       "Physical and Chemical Properties",          "理化特性",             "物理及化學性質"),
    ("安定性及び反応性",           "Stability and Reactivity",                  "稳定性和反应性",       "安定性及反應性"),
    ("有害性情報",                 "Toxicological Information",                 "毒理学信息",           "毒性資料"),
    ("環境影響情報",               "Ecological Information",                    "生态学信息",           "生態資料"),
    ("廃棄上の注意",               "Disposal Considerations",                   "废弃处置",             "廢棄處置方法"),
    ("輸送上の注意",               "Transport Information",                     "运输信息",             "運送資料"),
    ("適用法令",                   "Regulatory Information",                    "法规信息",             "法規資料"),
    ("その他の情報",               "Other Information",                         "其他信息",             "其他資料"),
];

// Mapping: SdsRoot JSON key → section index (0-based matching SECTION_NAMES)
pub(crate) const SECTION_KEYS: &[&str] = &[
    "Identification",
    "HazardIdentification",
    "Composition",
    "FirstAidMeasures",
    "FireFightingMeasures",
    "AccidentalReleaseMeasures",
    "HandlingAndStorage",
    "ExposureControlPersonalProtection",
    "PhysicalChemicalProperties",
    "StabilityReactivity",
    "ToxicologicalInformation",
    "EcologicalInformation",
    "DisposalConsiderations",
    "TransportInformation",
    "RegulatoryInformation",
    "OtherInformation",
];

pub(crate) const DOCUMENT_TITLE: &[&str] = &[
    "安全データシート",         // Japanese (JIS Z 7253)
    "Safety Data Sheet",        // English (GHS/ISO 11014)
    "安全技术说明书",           // ChineseSimplified (GB/T 16483)
    "安全資料表",               // ChineseTraditional (CNS 15030)
];

pub(crate) fn lang_index(lang: Language) -> usize {
    match lang {
        Language::Japanese => 0,
        Language::English => 1,
        Language::ChineseSimplified => 2,
        Language::ChineseTraditional => 3,
    }
}

pub(crate) fn section_name(section_idx: usize, lang: Language) -> &'static str {
    let row = &SECTION_NAMES[section_idx];
    match lang {
        Language::Japanese => row.0,
        Language::English => row.1,
        Language::ChineseSimplified => row.2,
        Language::ChineseTraditional => row.3,
    }
}

// ---------------------------------------------------------------------------
// DOCX generation
// ---------------------------------------------------------------------------

/// Generates a JIS Z 7253-compliant 16-section .docx file from SDS data.
pub fn generate_docx(sds: &SdsRoot, output_path: &Path, lang: Language) -> Result<(), SdsError> {
    assert_eq!(
        SECTION_NAMES.len(),
        SECTION_KEYS.len(),
        "SECTION_NAMES and SECTION_KEYS must have the same length"
    );
    let title = DOCUMENT_TITLE[lang_index(lang)];
    let root_val = serde_json::to_value(sds)
        .map_err(|e| SdsError::Docx(format!("serialize error: {e}")))?;

    let mut doc = Docx::new();
    doc = doc.add_paragraph(
        Paragraph::new().add_run(Run::new().add_text(title).bold().size(32)),
    );

    // Datasheet metadata block (date, version — not a numbered section)
    if let Some(ds) = root_val.get("Datasheet") {
        doc = render_object_fields(doc, ds, 0);
    }

    // 16 numbered sections
    for (i, key) in SECTION_KEYS.iter().enumerate() {
        let heading = format!("{}. {}", i + 1, section_name(i, lang));
        doc = doc.add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text(heading).bold().size(24)),
        );

        if let Some(val) = root_val.get(*key) {
            doc = render_value(doc, val, 0);
        }
    }

    let file = std::fs::File::create(output_path)?;
    doc.build()
        .pack(file)
        .map_err(|e| SdsError::Docx(format!("pack failed: {e:?}")))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Recursive JSON-value renderer
// ---------------------------------------------------------------------------

fn render_value(doc: Docx, val: &Value, indent: usize) -> Docx {
    match val {
        Value::Object(_) => render_object_fields(doc, val, indent),
        Value::Array(items) => {
            let mut d = doc;
            for (i, item) in items.iter().enumerate() {
                match item {
                    Value::Object(_) => {
                        // Array of structs: add a small separator
                        d = d.add_paragraph(
                            Paragraph::new().add_run(
                                Run::new().add_text(format!("[{}]", i + 1)).bold(),
                            ),
                        );
                        d = render_value(d, item, indent + 1);
                    }
                    Value::String(s) => {
                        d = add_leaf(d, &format!("- {s}"), indent);
                    }
                    other => {
                        d = add_leaf(d, &format!("- {}", value_to_text(other)), indent);
                    }
                }
            }
            d
        }
        _ => add_leaf(doc, &value_to_text(val), indent),
    }
}

fn render_object_fields(doc: Docx, val: &Value, indent: usize) -> Docx {
    let Value::Object(map) = val else { return doc };
    let mut d = doc;
    for (key, child) in map {
        // Skip AdditionalInfo unless it has content — keep output concise
        if key == "AdditionalInfo" {
            if let Some(full_text) = child.get("FullText") {
                d = add_label_value(d, key, &value_to_text(full_text), indent);
            }
            continue;
        }
        match child {
            Value::Object(_) => {
                d = add_label(d, key, indent);
                d = render_value(d, child, indent + 1);
            }
            Value::Array(items) if items.is_empty() => {}
            Value::Array(_) => {
                d = add_label(d, key, indent);
                d = render_value(d, child, indent + 1);
            }
            Value::Null => {}
            leaf => {
                d = add_label_value(d, key, &value_to_text(leaf), indent);
            }
        }
    }
    d
}

fn value_to_text(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn indent_twips(indent: usize) -> i32 {
    (indent as i32) * 360 // 360 twips ≈ 0.25 inch per level
}

fn add_label(doc: Docx, label: &str, indent: usize) -> Docx {
    doc.add_paragraph(
        Paragraph::new()
            .indent(Some(indent_twips(indent)), None, None, None)
            .add_run(Run::new().add_text(label).bold()),
    )
}

fn add_leaf(doc: Docx, text: &str, indent: usize) -> Docx {
    doc.add_paragraph(
        Paragraph::new()
            .indent(Some(indent_twips(indent)), None, None, None)
            .add_run(Run::new().add_text(text)),
    )
}

fn add_label_value(doc: Docx, label: &str, value: &str, indent: usize) -> Docx {
    if value.is_empty() {
        return doc;
    }
    doc.add_paragraph(
        Paragraph::new()
            .indent(Some(indent_twips(indent)), None, None, None)
            .add_run(Run::new().add_text(format!("{label}: ")).bold())
            .add_run(Run::new().add_text(value)),
    )
}
