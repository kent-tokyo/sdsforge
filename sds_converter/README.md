# sds-converter

CLI tool for **bidirectional conversion** between Safety Data Sheet (SDS) documents (Word/PDF) and the Japanese Ministry of Health, Labour and Welfare (MHLW) standard JSON format.

Supports **Japanese**, **English**, **Simplified Chinese**, and **Traditional Chinese**.

> **Embedding in your Rust project?** Use [`sds-converter-core`](https://crates.io/crates/sds-converter-core) directly.

---

## Installation

```bash
cargo install sds-converter
```

---

## Commands

### `to-json` — Convert PDF/Word → MHLW standard JSON

```bash
# Single file (Anthropic Claude, default)
export ANTHROPIC_API_KEY=sk-ant-...
sds-converter to-json --input input.pdf --output output.json

# Specify source language
sds-converter to-json --input sds_en.pdf --output output.json --lang en

# Batch mode — process a whole directory
sds-converter to-json --input-dir ./pdfs/ --output-dir ./json/ --lang ja

# OpenAI GPT (defaults to gpt-4o)
sds-converter to-json --input input.pdf --output output.json \
  --provider openai --api-key $OPENAI_API_KEY

# Google Gemini (defaults to gemini-2.0-flash)
sds-converter to-json --input input.pdf --output output.json \
  --provider gemini --api-key $GEMINI_API_KEY

# Local LLM via Ollama (any OpenAI-compatible endpoint)
sds-converter to-json --input input.pdf --output output.json \
  --provider openai --base-url http://localhost:11434/v1 \
  --model llama3.2 --api-key dummy

# From pre-extracted text (skip PDF parsing)
sds-converter to-json --input extracted.txt --output output.json --lang ja
```

| Flag | Default | Description |
|---|---|---|
| `--input` | — | Input PDF, DOCX, or TXT file |
| `--input-dir` | — | Input directory (batch — processes all `.pdf`/`.docx`) |
| `--output` | — | Output JSON file |
| `--output-dir` | — | Output directory (batch — created if absent) |
| `--provider` | `anthropic` | LLM provider: `anthropic`, `openai`, `gemini` |
| `--api-key` | env var | API key (fallback: `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `GEMINI_API_KEY`) |
| `--model` | per-provider | Model name (defaults: `claude-sonnet-4-6` / `gpt-4o` / `gemini-2.0-flash`) |
| `--base-url` | — | Custom OpenAI-compatible endpoint (Ollama, vLLM, etc.) |
| `--lang` | auto-detect | Source document language: `ja`, `en`, `zh-cn`, `zh-tw` |

### `to-docx` — Convert MHLW standard JSON → Word document

```bash
# Single file
sds-converter to-docx --input output.json --output result.docx --lang ja

# Batch mode
sds-converter to-docx --input-dir ./json/ --output-dir ./docx/ --lang en
```

| Flag | Default | Description |
|---|---|---|
| `--input` | — | Input JSON file |
| `--input-dir` | — | Input directory (batch — processes all `.json`) |
| `--output` | — | Output DOCX file |
| `--output-dir` | — | Output directory (batch) |
| `--lang` | `ja` | Output language: `ja`, `en`, `zh-cn`, `zh-tw` |

### `extract-text` — Extract raw text from PDF/DOCX

Extracts the text that the LLM would receive, without making any API call. Useful for inspecting extraction quality or running the LLM step separately.

```bash
# Save to file
sds-converter extract-text --input input.pdf --output extracted.txt

# Print to stdout
sds-converter extract-text --input input.pdf

# Then feed back into to-json
sds-converter to-json --input extracted.txt --output output.json --lang ja
```

### `validate` — Check a JSON file for structural issues

```bash
# Human-readable output (exits 0 = OK, 1 = warnings found)
sds-converter validate --input output.json

# JSON array output for CI/scripting
sds-converter validate --input output.json --json
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
- An LLM API key (for `to-json` only)
  - Anthropic: set `ANTHROPIC_API_KEY` or use `--api-key`
  - OpenAI: set `OPENAI_API_KEY` or use `--api-key`
  - Google Gemini: set `GEMINI_API_KEY` or use `--api-key`
  - Local LLM (Ollama etc.): use `--provider openai --base-url <url> --api-key dummy`
- Input files must be **text-based** PDF or DOCX
  - Encrypted PDFs are not supported
  - Scanned/image-only PDFs are not supported (no text to extract)

---

## License

Licensed under either of:
- Apache License, Version 2.0
- MIT License

at your option.
