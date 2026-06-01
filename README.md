# sds-converter

GUI + CLI tool for **bidirectional conversion** between Safety Data Sheet (SDS) documents (Word/PDF) and the Japanese Ministry of Health, Labour and Welfare (MHLW) standard JSON format.

Supports documents in **Japanese**, **English**, **Simplified Chinese**, and **Traditional Chinese**.

[日本語](README_ja.md) | [中文](README_zh.md)

---

## Download

| Platform | Download |
|---|---|
| **macOS** (Homebrew) | `brew tap kent-tokyo/sds-converter && brew install --cask sds-converter` |
| **macOS** (Direct — Universal, Apple Silicon + Intel) | [sds-converter-macos.zip](https://github.com/kent-tokyo/sds-converter/releases/latest/download/sds-converter-macos.zip) |
| **Windows** (Portable .exe — no install required) | [sds-converter-windows-portable.zip](https://github.com/kent-tokyo/sds-converter/releases/latest/download/sds-converter-windows-portable.zip) |
| **Rust / CLI** | `cargo install sds-converter` |

→ [All releases & changelogs](https://github.com/kent-tokyo/sds-converter/releases)

> **Windows note:** If Windows SmartScreen shows "Windows protected your PC", click **"More info" → "Run anyway"**.

---

## GUI

Launch the graphical interface by running `sds-converter` **without any arguments**, or double-click the downloaded app:

```bash
sds-converter
```

The GUI window opens with five tabs:

| Tab | Function |
|---|---|
| **Convert** | SDS document (PDF/DOCX/XLSX/HTML/URL) → MHLW standard JSON |
| **Generate** | MHLW JSON → DOCX / HTML / PDF (with optional DOCX template) |
| **Validate** | Structural validation of MHLW JSON with colored OK/Warning/Error results |
| **Extract Text** | Raw text extraction from documents — no LLM API required |
| **Settings** | API key, model name, base URL, quality, language, UI language |

| Convert tab | Generate tab | Extract Text tab |
|---|---|---|
| ![Convert tab](docs/tab_convert.png) | ![Generate tab](docs/tab_generate.png) | ![Extract Text tab](docs/tab_extract.png) |

**Drag & drop** files onto any tab to fill the input field automatically.
Settings are saved to `~/.config/sds-converter/config.toml` and restored on next launch.

---

## Features

- **SDS document → JSON**: Extracts text from PDF/DOCX/XLSX/TXT/**HTML/URL** and converts it to the MHLW SDS data exchange format v1.0 via LLM API. Parallel extraction with automatic retry. PDF extraction uses a 3-tier fallback: `pdf-extract` → `pdftotext` (CID/Shift-JIS fonts) → `pdftoppm`+`tesseract` OCR or Claude Vision API (scanned PDFs).
- **JSON → DOCX**: Generates a JIS Z 7253-compliant 16-section Word document from the standard JSON, with localized section headings.
- **JSON → HTML**: Generates a self-contained UTF-8 HTML5 document with inline CSS and `@media print` support (`to-html`).
- **JSON → PDF**: Converts to PDF via LibreOffice CLI (`to-pdf`). Requires `soffice` in PATH.
- **GHS/CAS validation**: Validates H-codes (H200–H420) and P-codes (P101–P503) against GHS Rev.10. Validates CAS number format and check-digit. Optional PubChem enrichment (`--enrich`) for composition cross-checking.
- **Multi-country SDS support**: Auto-infers source country from `--lang` (zh-cn→China, zh-tw→Taiwan, ja→Japan). Override with `--country cn|tw|kr|jp`. Injects country-specific extraction rules into the LLM prompt — China (GB/T 16483): 24h emergency contact, GBZ 2 OEL, GB 13690 regulatory refs; Taiwan (CNS 15030): CNS headings, NERC contact; Korea (K-GHS Rev.6): KEC number, KOSHA reference, K-REACH status. Country-specific validation (`validate_country()`) and compliance gap reports (`ComplianceDiffReport`) included in `ConversionReport`.
- **Validation-driven correction pass**: `--correct` flag activates a second targeted LLM call to fix invalid GHS H/P-codes found by the validator, plus deterministic CAS check-digit correction without an LLM call.
- **Multilingual**: Handles source documents in `ja` / `en` / `zh-CN` / `zh-TW`.
- **Extensible LLM backend**: Ships with Anthropic Claude, OpenAI GPT, Google Gemini, Mistral, Groq, and Cohere backends. Bring your own by implementing `LlmBackend`.
- **Library + CLI**: Use as a Rust library or as a standalone command-line tool.
- **Security hardened REST server**: Bearer token auth with timing-safe comparison (`constant_time_eq`), SSRF protection with full IPv6 coverage (`fc00::/7`, `fe80::/10`, IPv4-mapped), redirect-disabled HTTP client, and 50 MB upload cap.

---

## Why LLM?

SDS documents are **unstructured prose**, not spreadsheets. A single product's SDS might differ from another in:

- **Section order** — manufacturers rearrange sections freely within standards
- **Field labeling** — the same data appears under different headings across JIS Z 7253, GHS/OSHA HazCom, GB/T 16483, CNS 15030, and company-specific layouts
- **Text style** — concentrations as `"≥99.5%"`, `"99.5% or higher"`, or `"content: approximately 100%"` all mean the same thing
- **Language mixing** — Japanese SDS often embed English chemical names and CAS numbers mid-sentence

The MHLW standard JSON schema has **~200 deeply nested fields**. Writing rule-based parsers for every document variation is impractical. An LLM reads the document as a human would, maps free-form text to the correct schema fields regardless of format, and handles multilingual documents natively.

The `LlmBackend` trait keeps the extraction engine swappable — you can use Claude, GPT-4o, Gemini, or any future model without changing the rest of the tool.

---

## Quick Start

```bash
# Install the CLI
cargo install sds-converter

# Convert PDF → MHLW standard JSON
export ANTHROPIC_API_KEY=sk-ant-...
sds-converter to-json --input input.pdf --output output.json

# Convert from a URL directly
sds-converter to-json --input https://example.com/sds.html --output output.json

# Convert JSON → Word document
sds-converter to-docx --input output.json --output result.docx --lang ja

# Convert JSON → HTML (printable, A4)
sds-converter to-html --input output.json --output result.html --lang ja

# Convert JSON → PDF (requires LibreOffice)
sds-converter to-pdf --input output.json --output result.pdf --lang ja

# Validate JSON + check GHS codes and CAS numbers
sds-converter validate --input output.json

# Validate and cross-check CAS numbers against PubChem
sds-converter to-json --input input.pdf --output output.json --enrich

# Convert a Chinese SDS (GB/T 16483) with explicit country and correction pass
sds-converter to-json --input input.pdf --output output.json --lang zh-cn --country cn --correct
```

See the [`sds-converter` CLI README](./sds_converter/README.md) for full CLI reference and the [`sds-converter-core` README](./sds_converter_core/README.md) for the Rust library API.

---

## Language Support

| Language | `--lang` | Source documents | Output DOCX headings |
|---|---|---|---|
| Japanese | `ja` | JIS Z 7253 compliant SDS | JIS Z 7253 |
| English | `en` | GHS/OSHA HazCom format | GHS Rev.10 / ISO 11014 |
| Simplified Chinese | `zh-cn` | GB/T 16483 format | GB/T 16483-2012 |
| Traditional Chinese | `zh-tw` | CNS 15030 format | CNS 15030 |

---

## Comparison with Alternatives

### Open-source tools

| | **sds-converter** (this) | [sds_parser](https://github.com/astepe/sds_parser) | [tungsten](https://github.com/CrucibleSDS/tungsten) |
|---|---|---|---|
| Language | Rust | Python | Python |
| AI/LLM | Yes (pluggable) | No (regex) | No (rule-based) |
| MHLW JSON | Yes | No | No |
| Bidirectional | Yes (DOCX + HTML + PDF) | No | No |
| HTML/URL input | Yes | No | No |
| GHS/CAS validation | Yes | No | No |
| Multilingual | ja / en / zh-CN / zh-TW | Limited | English only |

### Commercial products (Japan)

| | **sds-converter** (this) | [SDS Meister](https://www.kcs.co.jp/ja/service/ind/general/chemical/sds.html) | [SmartSDS](https://smartsds.jp/) | [Dr.EHS Chemical](https://www.iad.co.jp/drehs/chemical2/) |
|---|---|---|---|---|
| Provider | — | さくらケーシーエス | テクノヒル | アイアンドディー |
| AI | Yes (your API key) | No | Yes (translation) | AI-OCR |
| MHLW JSON | Yes | Yes | Yes | Yes |
| Source PDF → JSON | Yes | No (authoring only) | Partial (JP only) | Yes |
| Open-source | Yes (MIT/Apache-2.0) | No | No | No |

### Commercial products (Global)

| | **sds-converter** (this) | [Affinda](https://www.affinda.com/documents/material-safety-data-sheet) | [SDS Manager API](https://sdsmanager.com/) | [safetydatasheetapi.com](https://safetydatasheetapi.com/) | [EcoOnline](https://www.ecoonline.com/) |
|---|---|---|---|---|---|
| AI/LLM | Pluggable LLM | LLM (adaptive) | NLP/ML | ML + OCR | AI/NLP |
| Input | PDF / DOCX | PDF / Word | PDF | PDF (incl. scanned) | PDF |
| Output | MHLW JSON + DOCX | Custom JSON | JSON / XML | JSON / XML / CSV | Internal only |
| Open-source | Yes | No | No | No | No |

**Key advantages:** the only open-source solution that supports the MHLW standard JSON, bidirectional conversion (JSON → DOCX/HTML/PDF), local execution without cloud subscriptions, GHS Rev.10 validation, PubChem enrichment, and a pluggable LLM backend.

---

## For Developers

| Crate | Description |
|---|---|
| [`sds-converter`](https://crates.io/crates/sds-converter) | CLI + GUI binary — this tool |
| [`sds-converter-core`](https://crates.io/crates/sds-converter-core) | Rust library — LLM extraction, DOCX/HTML generation, MHLW schema |

```toml
# Cargo.toml
[dependencies]
sds-converter-core = "0.3"
```

---

## Roadmap

### Next (0.3.x)
- [ ] DOCX table layout — Section 3 Composition (4-column), Section 2 H/P codes (2-column), Section 9 physical properties (2-column)

### Completed in 0.3.8 / 0.2.8
- [x] QC r27: S2-HAZARD-NO-PICTOGRAM (MED) — active signal word + H-codes but Pictogram list empty (detects image-only pictograms in PDFs)
- [x] QC r27: S3-CONC-UNIT-NO-VALUE (MED) — mixture component has concentration unit (%) but no numeric value extracted
- [x] QC r27: false-positive fixes — `危險` (zh-tw Danger) and `Not applicable` (en) added to valid signal words; S14 UN number, packing group, and shipping name detection extended for Traditional/Simplified Chinese formats
- [x] New tool `tools/roundtrip_random30.py` — balanced random-sample roundtrip test (seed-controlled, any n, per-rule ranking report)
- [x] Round-trip test r27 baseline (seed=42, n=30): 30/30 JSON ✓, 30/30 DOCX ✓, CRIT=0, HIGH=14, MED=239

### Completed in 0.3.6 / 0.2.6 – 0.3.7 / 0.2.7
- [x] QC r24: 5 new rule-based checks (S1-ZH-NO-EMERGENCY, S7-FLAMMABLE-STORAGE-TEMP, S8-NO-ENG-CONTROLS, S10-NO-INCOMPATIBLE, CROSS-STALE-DATE)
- [x] QC r24: S8-OEL-NO-NUMERIC false-positive fixes — Chinese unit-before-value format, additional "no OEL" exemption phrases
- [x] QC r24: S5-EMPTY threshold 30→15 chars (reduces false positives for brief Chinese firefighting sections)
- [x] Round-trip test: JSONL parsing fix, validator string-array handling; r24 baseline 30/30 success, CRIT=0, HIGH=9, MED=176
- [x] QC r25: fix S2-EXPLOSIVE-NO-GHS01 / S2-ENV-NO-GHS09 false-negatives (substring "01"/"09" in dates/H-codes); new S3-NAME-IS-CAS (HIGH) and S16-REVISION-BEFORE-ISSUE (HIGH)
- [x] Round-trip test r25 baseline: 30/30 success, CRIT=0, HIGH=13, MED=175
- [x] QC r26: S2-FLAMMABLE-NO-GHS02, S2-CORROSIVE-NO-GHS05, S2-ACUTETOX-NO-GHS06 (all MED) — pictogram/H-code consistency for flammable, corrosive, and acute-tox Cat 1–3; S4-H314-NO-REMOVE-CLOTHING (MED) — P361 compliance
- [x] Round-trip test r26 baseline: 30/30 success, CRIT=0, HIGH=14, MED=181
- [x] LLM prompt: Section 1 Use fallback — source phrase captured when Section 1.2 exists but no specific use is listed (e.g. `'无相关详细资料'`)
- [x] LLM prompt: Section 8 OEL "not required" detection — `不要求` / `无需监控` / `不适用` and similar phrases now stored in `AdditionalInfo.FullText` instead of being silently omitted
- [x] LLM prompt: Section 9 Densities always extracted; VapourPressure added for flammable/volatile products (H224/H225/H226/H330–H332)
- [x] LLM prompt: Section 12 `PersistenceDegradability.BiologicalDegradability` always populated when the source subsection exists

### Completed in 0.3.5 / 0.2.5
- [x] Multi-country SDS support (`--country cn|tw|kr|jp`) with country-specific LLM extraction rules and compliance gap reports
- [x] Validation-driven correction pass (`--correct`) — second LLM call fixes invalid H/P-codes; deterministic CAS check-digit correction
- [x] CAS concatenation normalization — splits multi-CAS strings delimited by `\n`, comma, or semicolon
- [x] Non-hazardous product stub — inserts minimal `HazardIdentification` when LLM omits it for non-hazardous products
- [x] Expanded H-code mapping table with zh-cn/zh-tw phrases and multi-hazard split instruction
- [x] P-code annotation disambiguation — strips bracketed H-codes (e.g. `[H315]`) from P-code fields
- [x] Vision path CRITICAL instruction parity with text path
- [x] Validator enhancements: date-in-concentration detection, placeholder product name detection, classification completeness, H290 Chinese keyword cross-check, mixture-aware AcuteToxicity cross-check

### Planned
- [x] GUI application (eframe/egui) — Convert / Generate / Validate / Extract Text / Settings tabs with drag-and-drop, persistent config, and 3-language UI
- [x] Published to crates.io (`sds-converter-core` + `sds-converter`)
- [ ] GHS pictogram embedding in HTML and DOCX output

### External dependency
- [x] Pure-Rust PDF generation — `harumi::render_html_to_pdf` now available in [`harumi`](https://crates.io/crates/harumi) v0.4.0 (`html` feature)
- [x] OCR support for scanned PDFs — `pdftoppm` + `tesseract` CLI fallback (auto-detected when text extraction yields < 200 chars)
- [x] `pdftotext` fallback for Japanese CID font PDFs — handles Shift-JIS encoded PDFs that cause `pdf-extract` to panic
- [x] Schema compatibility hardening (v0.3.3) — `flex_vec_string_opt` for `CASno.FullText`, coerce `Colour`/`Odour`/`PhysicalState` objects to strings, remove deprecated `-utf8` flag from `pdftotext` fallback

---

## References

- [MHLW — SDS Standard Data Exchange Format (official page)](https://www.mhlw.go.jp/stf/newpage_56484.html) (Japanese)
- [SDS Data Exchange Format Developer Manual (PDF)](https://www.mhlw.go.jp/content/11305000/001467068.pdf) (Japanese)
- [JSON Quality Check Manual — all 53 rules explained by section](docs/quality-check.md) ([日本語](docs/quality-check_ja.md) / [中文](docs/quality-check_zh.md))

---

## License

Licensed under either of:
- Apache License, Version 2.0
- MIT License

at your option.
