# sdsforge

**Python-first, Rust-powered toolkit for converting Safety Data Sheets to Japan MHLW standard JSON — with schema validation, GHS/CAS checks, and corpus-scale quality evaluation.**

[日本語](README_ja.md) | [中文](README_zh.md)

---

## Install

```bash
pip install sdsforge                   # Python bindings
pip install "sdsforge[analysis]"       # + causasv quality analysis
cargo install sdsforge                 # CLI / GUI binary
```

---

## Quick Start — Python

```python
import sdsforge

# Extract raw text (no LLM)
text = sdsforge.extract_text("sample.pdf")

# Convert from URL
data, report = sdsforge.to_json_url_with_report(
    "https://example.com/sds.pdf", lang="ja",
)

# Convert SDS document → MHLW standard JSON
data, report = sdsforge.to_json_with_report(
    "sample.pdf",
    lang="ja",
    strict_mhlw=True,
)

# Validate and get structured findings
findings = sdsforge.validate(data, strict_mhlw=True)

print(f"Sections populated: {len(report['populated_sections'])}")
print(f"Findings: {len(findings)} ({sum(1 for f in findings if f['level']=='HIGH')} HIGH)")

# Save MHLW JSON
sdsforge.write_json(data, "output.json")
```

Corpus-scale evaluation (no manual review needed):

```python
from sdsforge.eval import eval_corpus

df = eval_corpus(
    input_dir="data/sds_raw",
    output_dir="runs/eval_001",
    jobs=8,
)
print(df[["filename", "overall_score", "grade", "high_count"]].head(20))
```

---

## Examples

MHLW official sample SDS — allyl chloride (塩化アリル):

```bash
export ANTHROPIC_API_KEY=sk-ant-...
python examples/mhlw_allyl_chloride/convert.py
```

See [`examples/mhlw_allyl_chloride/`](examples/mhlw_allyl_chloride/) for
`expected.json`, `expected_report.json`, and source attribution.

---

## Why sdsforge

- **MHLW-native**: Converts directly to the Japanese Ministry of Health, Labour and Welfare SDS data exchange format v1.0 (`SDS_Schema_v1.0.json`), validated against the official schema.
- **Evidence-based extraction**: Uses LLM to map free-form SDS text to ~200 nested schema fields. Source-text cross-checks detect hallucinations at the field level.
- **Corpus-scale quality evaluation**: `eval_corpus` processes hundreds of SDS documents and outputs per-rule failure counts, section scores, and `causasv_features.csv` for root-cause analysis — without any human review.
- **No lock-in**: Supports Anthropic Claude, OpenAI GPT, Google Gemini, Mistral, Groq, Cohere, and any OpenAI-compatible local endpoint. Bring your own model.
- **Rust core**: Extraction, schema validation, GHS/CAS checks, and DOCX/HTML generation run in native code. Thin Python bindings on top.

---

## MHLW Compliance

sdsforge targets the MHLW SDS data exchange format v1.0 published 2025-03-31.

| Rule | Behaviour |
|---|---|
| Schema validation | Validates against `SDS_Schema_v1.0.json` |
| Empty-field removal | Removes `""`, `null`, `[]`, `{}` per §3.3 |
| AdditionalInfo | Content outside the official schema is written to `AdditionalInfo.FullText` |
| `--strict-mhlw` | Exits 1 (CLI) / raises `ValueError` (Python) if any HIGH or CRIT finding |
| CRIT/HIGH/MED findings | Structured validation report with rule ID, severity, path, message |

**Validation rules include:** GHS H/P-code validity (GHS Rev.10), CAS format and check-digit, Section 2 GHS completeness (H-codes ↔ pictograms ↔ signal word), Section 3 component row alignment (name/CAS/concentration), UN number completeness, concentration range bounds, duplicate code detection, and more.

Quality baseline (30-file random sample, seed=42):
> CRIT=0 · avg score 89.6 · top issues: `S2-HAZARD-NO-PICTOGRAM`, `S15-ZHCN-NO-GB`, `S14-NO-SHIPPING-NAME`

Full rule catalogue → [docs/quality-check.md](docs/quality-check.md)

---

## Corpus Evaluation

Run without human review:

```python
from sdsforge.eval import eval_corpus

df = eval_corpus("data/sds_raw", "runs/eval_001", jobs=8)
```

Outputs per file:

| File | Contents |
|---|---|
| `generated/<stem>.json` | MHLW standard JSON |
| `reports/<stem>.json` | ConversionReport (language, populated sections, warnings) |
| `findings/<stem>.json` | Structured validation findings |
| `summary.csv` | Per-file scores and grades |
| `failures_by_rule.csv` | Rule frequency and affected file counts |

Root-cause analysis with [causasv](https://github.com/kent-tokyo/causasv):

```python
from sdsforge.causasv_bridge import print_ranking
print_ranking("runs/eval_001/causasv_features.csv")
```

---

## CLI

```bash
# PDF/DOCX/XLSX/HTML/URL → MHLW JSON
sdsforge to-json --input input.pdf --output output.json --lang ja

# With correction pass and PubChem enrichment
sdsforge to-json --input input.pdf --output output.json --correct --enrich

# MHLW JSON → Word / HTML / PDF
sdsforge render --input output.json --to docx --output result.docx --lang ja
sdsforge render --input output.json --to html --output result.html --lang ja
sdsforge render --input output.json --to pdf  --output result.pdf  --lang ja

# Validate with strict MHLW mode
sdsforge validate --input output.json --strict-mhlw

# Batch: process a directory
sdsforge to-json --input-dir data/ --output-dir out/ --jobs 8

# Corpus evaluation
sdsforge eval-corpus --input-dir data/sds_raw --output-dir runs/eval_001 --jobs 8
```

Full CLI reference → [sdsforge/README.md](./sdsforge/README.md)

---

## REST API

```bash
# Start server (binds to 127.0.0.1:3000 by default)
SDS_SERVER_TOKEN=secret sdsforge-server

# Convert a PDF
curl -X POST http://localhost:3000/api/to-json \
  -H "Authorization: Bearer secret" \
  -F "file=@input.pdf"
```

Endpoints: `POST /api/to-json` · `POST /api/to-docx` · `POST /api/to-html` · `POST /api/validate` · `GET /api/health`

---

## GUI

Run `sdsforge` without arguments to open the graphical interface:

```bash
sdsforge
```

Five tabs: **Convert** · **Render Document** · **Validate** · **Extract Text** · **Settings**

Download the desktop app: [macOS](https://github.com/kent-tokyo/sdsconv/releases/latest/download/sdsconv-macos.zip) · [Windows](https://github.com/kent-tokyo/sdsconv/releases/latest/download/sdsconv-windows-portable.zip) · `brew install --cask sdsconv`

---

## Supported Inputs, Languages, and Backends

**Input formats:** PDF (text, CID/Shift-JIS, scanned) · DOCX · XLSX · TXT · HTML · URL

**Source languages:** `ja` (JIS Z 7253) · `en` (GHS/OSHA HazCom) · `zh-cn` (GB/T 16483) · `zh-tw` (CNS 15030)

**LLM backends:** Anthropic Claude · OpenAI GPT · Google Gemini · Mistral · Groq · Cohere · Local (any OpenAI-compatible endpoint)

---

## For Developers

**Rust library:**

```toml
[dependencies]
sdsforge-core = "0.4"
```

See [sdsforge_core/README.md](./sdsforge_core/README.md) for the Rust API.

**Crates:** [`sdsforge`](https://crates.io/crates/sdsforge) · [`sdsforge-core`](https://crates.io/crates/sdsforge-core)

**Python package:** [`sdsforge`](https://pypi.org/project/sdsforge/) on PyPI — `pip install sdsforge`

---

## Security & Privacy

- **Cloud LLM caution**: When using a cloud LLM backend, SDS document text is sent to the API provider. Avoid sending confidential or trade-secret SDS documents to cloud APIs.
- **Local operation**: Use `--backend local` with any OpenAI-compatible endpoint (e.g. Ollama, LM Studio) for fully offline operation. No data leaves your machine.
- **Raw SDS corpus**: Add `corpus/raw/` and `data/sds_raw/` to `.gitignore`. Only `corpus/manifest.jsonl` (URLs + sha256 hashes) is safe to commit.
- **REST server**: Bearer token authentication with timing-safe comparison, SSRF protection (full IPv6 coverage), redirect-disabled HTTP client, 50 MB upload cap.

---

## Comparison

→ [docs/comparison.md](docs/comparison.md)

---

## References

- [MHLW — SDS Standard Data Exchange Format (official)](https://www.mhlw.go.jp/stf/newpage_56484.html) (Japanese)
- [SDS Data Exchange Format Developer Manual (PDF)](https://www.mhlw.go.jp/content/11305000/001467068.pdf) (Japanese)
- [Quality Check Rule Catalogue — 53 rules by section](docs/quality-check.md) ([日本語](docs/quality-check_ja.md) / [中文](docs/quality-check_zh.md))
- [CHANGELOG](CHANGELOG.md)

---

## License

MIT OR Apache-2.0 — at your option.
