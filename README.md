# sds-converter

A Rust workspace for **bidirectional conversion** between Safety Data Sheet (SDS) documents (Word/PDF) and the Japanese Ministry of Health, Labour and Welfare (MHLW) standard JSON format.

Supports documents in **Japanese**, **English**, **Simplified Chinese**, and **Traditional Chinese**.

---

## Crates

| Crate | Description |
|---|---|
| [`sds-converter-core`](./sds_converter_core/) | Rust library — LLM-based extraction, DOCX generation, MHLW schema |
| [`sds-converter`](./sds_converter/) | CLI binary — `to-json` and `to-docx` subcommands |

---

## Features

- **SDS document → JSON**: Extracts text from PDF/DOCX and converts it to the MHLW SDS data exchange format v1.0 via LLM API.
- **JSON → DOCX**: Generates a JIS Z 7253-compliant 16-section Word document from the standard JSON, with localized section headings.
- **Multilingual**: Handles source documents in `ja` / `en` / `zh-CN` / `zh-TW`.
- **Extensible LLM backend**: Ships with Anthropic Claude, OpenAI GPT, and Google Gemini backends. Bring your own by implementing `LlmBackend`.
- **Library + CLI**: Use as a Rust library or as a standalone command-line tool.

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

# Convert JSON → Word document
sds-converter to-docx --input output.json --output result.docx --lang ja
```

See the [`sds-converter` README](./sds_converter/README.md) for full CLI reference and [`sds-converter-core` README](./sds_converter_core/README.md) for library API.

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

| Tool | Language | AI/LLM | MHLW JSON | Bidirectional | Multilingual |
|---|---|---|---|---|---|
| **sds-converter** (this) | Rust | Yes (pluggable) | Yes | Yes (↔ DOCX) | ja / en / zh-CN / zh-TW |
| [sds_parser](https://github.com/astepe/sds_parser) | Python | No (regex) | No | No | Limited |
| [tungsten](https://github.com/CrucibleSDS/tungsten) | Python | No (rule-based) | No | No | English only |

### Commercial products (Japan)

| Product | Provider | AI | MHLW JSON | Source PDF → JSON | Open-source |
|---|---|---|---|---|---|
| **sds-converter** (this) | — | Yes (your API key) | Yes | Yes | Yes (MIT/Apache-2.0) |
| [SDS Meister](https://www.kcs.co.jp/ja/service/ind/general/chemical/sds.html) | さくらケーシーエス | No | Yes | No (authoring only) | No |
| [SmartSDS](https://smartsds.jp/) | テクノヒル | Yes (translation) | Yes | Partial (JP only) | No |
| [Dr.EHS Chemical](https://www.iad.co.jp/drehs/chemical2/) | アイアンドディー | AI-OCR | Yes | Yes | No |

### Commercial products (Global)

| Product | AI/LLM | Input | Output | Open-source |
|---|---|---|---|---|
| **sds-converter** (this) | Pluggable LLM | PDF / DOCX | MHLW JSON + DOCX | Yes |
| [Affinda](https://www.affinda.com/documents/material-safety-data-sheet) | LLM (adaptive) | PDF / Word | Custom JSON | No |
| [SDS Manager API](https://sdsmanager.com/) | NLP/ML | PDF | JSON / XML | No |
| [safetydatasheetapi.com](https://safetydatasheetapi.com/) | ML + OCR | PDF (incl. scanned) | JSON / XML / CSV | No |
| [EcoOnline Smart Extraction](https://www.ecoonline.com/) | AI/NLP | PDF | Internal only | No |

**Key advantages:** the only open-source solution that supports the MHLW standard JSON, bidirectional conversion (JSON → DOCX), local execution without cloud subscriptions, and a pluggable LLM backend.

---

## License

Licensed under either of:
- Apache License, Version 2.0
- MIT License

at your option.
