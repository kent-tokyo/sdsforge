# sdsforge

GUI + CLI tool for **bidirectional conversion** between Safety Data Sheet (SDS) documents (Word/PDF) and the Japanese Ministry of Health, Labour and Welfare (MHLW) standard JSON format.

Supports **Japanese**, **English**, **Simplified Chinese**, and **Traditional Chinese**.

[日本語](README_ja.md) | [中文](README_zh.md)

> **Embedding in your Rust project?** Use [`sdsforge-core`](https://crates.io/crates/sdsforge-core) directly.
>
> **Migrating from `sdsconv`?** See [`docs/migration-from-sdsconv.md`](../docs/migration-from-sdsconv.md). The old `sdsconv` binary still works during the deprecation window — it forwards to `sdsforge` with a warning.

---

## Download

| Platform | Download |
|---|---|
| **macOS** (Universal — Apple Silicon + Intel) | [sdsforge-macos.zip](https://github.com/kent-tokyo/sdsforge/releases/latest/download/sdsforge-macos.zip) |
| **Windows** (Portable .exe — no install required) | [sdsforge-windows-portable.zip](https://github.com/kent-tokyo/sdsforge/releases/latest/download/sdsforge-windows-portable.zip) |
| **Rust / CLI** | `cargo install sdsforge` |

→ [All releases & changelogs](https://github.com/kent-tokyo/sdsforge/releases)

---

## GUI Mode

Launch the graphical interface by running `sdsforge` **without any arguments**:

```bash
sdsforge
```

The GUI window (820×640) opens with five tabs:

| Tab | Function |
|---|---|
| **Convert** | SDS document (PDF/DOCX/XLSX/HTML/URL) → MHLW standard JSON |
| **Render Document** | MHLW JSON → DOCX / HTML / PDF (with optional DOCX template) |
| **Validate** | Structural validation of MHLW JSON with colored OK/Warning/Error results |
| **Extract Text** | Raw text extraction from documents — no LLM API required |
| **Settings** | API key, model name, base URL, quality, language, UI language |

| Convert tab | Render tab | Extract Text tab |
|---|---|---|
| ![Convert tab](../docs/tab_convert.png) | ![Render tab](../docs/tab_generate.png) | ![Extract Text tab](../docs/tab_extract.png) |

**Drag & drop** files onto any tab to fill the input field automatically.

Settings are saved to your OS config directory under `sdsforge/config.toml`
(`~/Library/Application Support/sdsforge/config.toml` on macOS,
`~/.config/sdsforge/config.toml` on Linux), migrated automatically from the
pre-rename `sdsconv/config.toml` location the first time you launch this
version if the new file doesn't exist yet — your saved API key and settings
carry over, and the old file is left untouched.
The GUI and CLI share the same conversion engine (`tasks.rs`), so results are identical.

---

## Commands

### `to-json` — Convert PDF/Word → MHLW standard JSON

```bash
# Single file (Anthropic Claude, default)
export ANTHROPIC_API_KEY=sk-ant-...
sdsforge to-json --input input.pdf --output output.json

# Specify source language
sdsforge to-json --input sds_en.pdf --output output.json --lang en

# Batch mode — process a whole directory
sdsforge to-json --input-dir ./pdfs/ --output-dir ./json/ --lang ja

# OpenAI GPT (defaults to gpt-4o-mini)
sdsforge to-json --input input.pdf --output output.json \
  --provider openai --api-key $OPENAI_API_KEY

# Google Gemini (defaults to gemini-2.0-flash)
sdsforge to-json --input input.pdf --output output.json \
  --provider gemini --api-key $GEMINI_API_KEY

# Local LLM via Ollama (any OpenAI-compatible endpoint)
sdsforge to-json --input input.pdf --output output.json \
  --provider local --base-url http://localhost:11434/v1 \
  --model llama3.2 --api-key dummy

# From pre-extracted text (skip PDF parsing)
sdsforge to-json --input extracted.txt --output output.json --lang ja
```

| Flag | Default | Description |
|---|---|---|
| `--input` | — | Input PDF, DOCX, XLSX, or TXT file |
| `--input-dir` | — | Input directory (batch — processes all `.pdf`/`.docx`/`.xlsx`/`.xls`) |
| `--output` | — | Output JSON file |
| `--output-dir` | — | Output directory (batch — created if absent) |
| `--provider` | `anthropic` | LLM provider: `anthropic`, `openai`, `gemini`, `mistral`, `groq`, `cohere`, `local` |
| `--api-key` | env var | API key (see provider defaults below) |
| `--model` | per-provider | Model name override |
| `--base-url` | — | Custom OpenAI-compatible endpoint (for `--provider local`) |
| `--lang` | auto-detect | Source document language: `ja`, `en`, `zh-cn`, `zh-tw` |
| `--quality` | `medium` | Preset: `low` (fast/cheap), `medium`, `high` (accurate) |
| `--concurrency` | `4` | Max parallel files in batch mode |
| `--suggested-name` | — | Rename output to `SDS_<IssueDate>_<ProductCode>.json` (MHLW §2.1.2 recommended convention) |

**Provider defaults:**

| `--provider` | Default model | Environment variable |
|---|---|---|
| `anthropic` | `claude-haiku-4-5-20251001` (low/medium) · `claude-sonnet-4-6` (high) | `ANTHROPIC_API_KEY` |
| `openai` | `gpt-4o-mini` | `OPENAI_API_KEY` |
| `gemini` | `gemini-2.0-flash` | `GEMINI_API_KEY` |
| `mistral` | `mistral-small-latest` | `MISTRAL_API_KEY` |
| `groq` | `llama-3.3-70b-versatile` | `GROQ_API_KEY` |
| `cohere` | `command-r-plus` | `COHERE_API_KEY` |
| `local` | `llama3` | `LOCAL_LLM_API_KEY` (optional; defaults to `ollama`) |

### `render` — Convert MHLW standard JSON → Word / HTML / PDF

```bash
# Single file (built-in layout)
sdsforge render --input output.json --to docx --output result.docx --lang ja

# Batch mode (built-in layout)
sdsforge render --input-dir ./json/ --output-dir ./docx/ --to docx --lang en

# HTML or PDF instead of DOCX
sdsforge render --input output.json --to html --output result.html --lang ja
sdsforge render --input output.json --to pdf  --output result.pdf  --lang ja

# Fill a Word template with {{Placeholder}} substitution
sdsforge render --input output.json --to docx --output result.docx \
  --template my_template.docx

# Batch mode with template
sdsforge render --input-dir ./json/ --output-dir ./docx/ --to docx \
  --template my_template.docx
```

`to-docx`, `to-html`, and `to-pdf` still work as deprecated aliases for
`render --to docx|html|pdf` (same implementation, a deprecation warning is
printed to stderr) — switch to `render` at your own pace.

#### Word template format

Prepare a `.docx` file with `{{FieldName}}` placeholders where `FieldName` is
a leaf key from the MHLW JSON schema. The full dot-path is also accepted for
disambiguation.

```
{{TradeNameJP}}          → 製品和名
{{CompanyName}}          → 会社名
{{Phone}}                → 電話番号
{{IssueDate}}            → 発行日
{{Identification.SupplierInformation.CompanyName}}  → フルパス指定
```

Placeholders can appear anywhere in the document — paragraphs, table cells,
headers, and footers. Word sometimes splits typed text across internal runs;
the tool automatically merges such splits before substitution.

| Flag | Default | Description |
|---|---|---|
| `--input` | — | Input JSON file |
| `--input-dir` | — | Input directory (batch — processes all `.json`) |
| `--output` | — | Output file |
| `--output-dir` | — | Output directory (batch) |
| `--to` | — | Output format: `docx`, `html`, or `pdf` |
| `--lang` | `ja` | Output language: `ja`, `en`, `zh-cn`, `zh-tw` (without `--template`) |
| `--template` | — | Word template with `{{FieldName}}` placeholders (`--to docx` only) |

### `extract-text` — Extract raw text from PDF/DOCX

Extracts the text that the LLM would receive, without making any API call. Useful for inspecting extraction quality or running the LLM step separately.

```bash
# Save to file
sdsforge extract-text --input input.pdf --output extracted.txt

# Print to stdout
sdsforge extract-text --input input.pdf

# Then feed back into to-json
sdsforge to-json --input extracted.txt --output output.json --lang ja
```

### `validate` — Check a JSON file for structural issues

```bash
# Human-readable output (exits 0 = OK, 1 = warnings found)
sdsforge validate --input output.json

# JSON array output for CI/scripting
sdsforge validate --input output.json --json
```

Checks that key sections (Identification, HazardIdentification, ToxicologicalInformation, etc.) are populated. Exits with code `1` if any issues are found.

---

## Language Support

| Language | `--lang` | Source documents | Output DOCX headings |
|---|---|---|---|
| Japanese | `ja` | JIS Z 7253 compliant SDS | JIS Z 7253 |
| English | `en` | GHS/OSHA HazCom format | GHS Rev.10 / ISO 11014 |
| Simplified Chinese | `zh-cn` | GB/T 16483 format | GB/T 16483-2012 |
| Traditional Chinese | `zh-tw` | CNS 15030 format | CNS 15030 |

---

## Requirements

- Rust 1.75+
- An LLM API key (for `to-json` only) — set the provider's environment variable or pass `--api-key`
  - Anthropic: `ANTHROPIC_API_KEY`
  - OpenAI: `OPENAI_API_KEY`
  - Google Gemini: `GEMINI_API_KEY`
  - Mistral: `MISTRAL_API_KEY`
  - Groq: `GROQ_API_KEY`
  - Cohere: `COHERE_API_KEY`
  - Local LLM (Ollama etc.): use `--provider local --base-url <url>` (no API key required)
- Input files must be **text-based** PDF or DOCX
  - Encrypted PDFs are not supported
  - CID font / Shift-JIS encoded PDFs (common in Japanese documents): handled by `pdftotext` (poppler) fallback
  - Scanned/image-only PDFs: automatically retried via `pdftoppm` + `tesseract` OCR (if installed), or via Claude Vision API (when using `--provider anthropic`)
  - Full 3-tier PDF fallback: `pdf-extract` -> `pdftotext` -> OCR/Vision

---

## Rust Library

The conversion engine is available as a standalone library:

| Crate | crates.io | Description |
|---|---|---|
| `sdsforge-core` | [`sdsforge-core`](https://crates.io/crates/sdsforge-core) | LLM-based extraction, DOCX/HTML generation, MHLW schema |

```toml
[dependencies]
sdsforge-core = "0.4"
```

---

## Changelog

### Completed in 0.3.6 / 0.2.6
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

---

## References

- [MHLW — SDS Standard Data Exchange Format (official page)](https://www.mhlw.go.jp/stf/newpage_56484.html) (Japanese)
- [SDS Data Exchange Format Developer Manual (PDF)](https://www.mhlw.go.jp/content/11305000/001467068.pdf) (Japanese)

---

## License

Licensed under either of:
- Apache License, Version 2.0
- MIT License

at your option.
