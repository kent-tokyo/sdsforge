# Migration: sdsconv → sdsforge

**Status: planning only.** Nothing described below has been executed yet — no
crate, package, CLI command, or file has been renamed. This document specifies
the agreed target state so the rename can proceed in small, reviewable commits.
It will be updated commit-by-commit as the rename actually lands, and the
"planning only" line above will be removed once commit #1 of the rename (not
this docs commit — see `docs/sdsforge-architecture.md` for the commit roadmap)
starts.

## Why

`sdsconv` started as a document converter (Word/PDF ↔ MHLW standard JSON).
`sdsforge` is the same codebase scoped up to five capabilities: **Generate**,
**Convert**, **Translate**, **Validate**, **Render**. The name changes because
"convert" no longer describes what most of the tool does.

## Name mapping

| Area | Old | New |
|---|---|---|
| GitHub repository | `sds-converter` (git remote) / `sdsconv` (Cargo.toml metadata — these already disagree today) | `sdsforge` |
| Core crate | `sdsconv-core` | `sdsforge-core` |
| CLI/GUI crate + binary | `sdsconv` | `sdsforge` |
| Server crate + binary | `sdsconv-server` | `sdsforge-server` |
| Python binding crate | `sdsconv-py` | `sdsforge-py` |
| Python compiled extension module | `_sdsconv` | `_sdsforge` |
| PyPI package | `sdsconv` | `sdsforge` |
| Homebrew tap | `homebrew-sdsconv` | `homebrew-sdsforge` |
| macOS app bundle ID | `com.sdsconv.app` | `com.sdsforge.app` |
| docs.rs | `docs.rs/sdsconv-core`, `docs.rs/sdsconv` | `docs.rs/sdsforge-core`, `docs.rs/sdsforge` |

crates.io and PyPI have no rename primitive — these are **new package
publishes**, not renames of the existing listings. The old listings stay live
and become deprecated compat shims (see "Deprecated APIs" below).

## Terminology (authoritative — do not reintroduce old meanings)

| Term | Meaning |
|---|---|
| `generate` | Create an SDS **draft** from product info, CAS numbers, composition, measured properties, and supporting evidence. New capability — does not exist in `sdsconv` today. |
| `render` | Produce DOCX/HTML/PDF from an existing structured SDS/JSON. This is what today's `to-docx`/`to-html`/`to-pdf` and the GUI "Generate" tab do — they are being renamed to this, not replaced. |
| `convert` | Extract/transform an existing unstructured SDS document into structured JSON. Today's `to-json`. |
| `translate` | Translate human-language fields only. Identifiers, codes, numbers, units, and structure must not change. |
| `validate` | Check a structured SDS against a regulatory profile (today: MHLW only). Unchanged. |

`generate` was ambiguous in earlier drafts of this rename (it collided with the
existing render-out meaning). It is now reserved exclusively for the new
CAS/composition-authoring capability.

## CLI command mapping

The rollout happens in stages so the CLI is never broken mid-migration:

| Stage | Change |
|---|---|
| 1 | New `render` command ships, backed by the *existing* to-docx/to-html/to-pdf implementation (`--to docx\|html\|pdf`). |
| 2 | `to-docx`, `to-html`, `to-pdf` become deprecated aliases for `render --to ...` — same implementation, a deprecation notice is printed to **stderr only** (stdout stays machine-readable JSON/output). |
| 3 | GUI "Generate" tab is relabeled "Render" (no functional change). |
| 4 | Internal Rust fn/module names (`generate_docx` → `render_docx`, etc.) are cleaned up where it aids clarity — kept out of the same commit as any public API change. |
| 5 | The new `generate` command ships (CAS/composition → SDS draft), only once "render" is unambiguous everywhere else. |

Target CLI surface once all stages land:

```bash
sdsforge generate  --input product.yaml   --output draft.json --report report.json
sdsforge render    --input draft.json     --to docx --output draft.docx
sdsforge convert   --input existing.pdf   --output structured.json
sdsforge translate --input structured.json --to en --output translated.json
sdsforge validate  --input draft.json     --profile mhlw-v1
```

`extract-text`, `detect-lang`, and `eval-corpus` are unaffected by this rename
(no naming collision) and carry over as-is under the `sdsforge` binary name.

CLI unifies rendering under `render --to <format>`; the **Rust library API
keeps separate per-format functions** (`render_docx()`, `render_html()`,
`render_pdf()`) rather than one polymorphic function — the unification is a
CLI ergonomics choice, not a forced Rust abstraction.

## Rust API changes

Old:
```rust
use sdsconv_core::{convert_to_json, SdsRoot, Finding};
```

New:
```rust
use sdsforge_core::{convert_to_json, SdsRoot, Finding};
```

The `sdsconv-core` crate becomes a thin deprecated shim:
```rust
#[deprecated(note = "sdsconv-core has been renamed to sdsforge-core")]
pub use sdsforge_core::*;
```

`generator::generate_docx` / `pdf::generate_pdf` are renamed to
`render_docx` / `render_pdf` per stage 4 above; the old names remain as
`#[deprecated]` re-exports for at least one release.

## Python API changes

Old:
```python
import sdsconv
data, report = sdsconv.to_json_with_report("sample.pdf", lang="ja")
```

New:
```python
import sdsforge
data, report = sdsforge.to_json_with_report("sample.pdf", lang="ja")
```

The `sdsconv` PyPI package becomes a shim that re-exports from `sdsforge` and
raises a `DeprecationWarning` on import. Function names inside the package are
unaffected by this rename except where the CLI rename above changes an
underlying Rust name — the Python-facing signatures (`to_json`, `validate`,
`extract_text`, etc.) stay the same names; only the render-family functions
(none currently exposed to Python) would follow stage 4 if/when they are.

## REST API changes

No route paths or JSON shapes change — investigation confirmed no `sdsconv`
literal appears in any route path or response body. Only doc comments,
startup log lines, and the crate/binary name change (`sdsconv-server` →
`sdsforge-server`).

## Config / environment variables

No change in this round. `sdsconv`'s GUI config currently lives at
`dirs::config_dir()/sdsconv/config.toml`. When the rename lands, this needs an
explicit **read-old-if-new-missing** migration on first launch (not a silent
directory rename) so users don't lose saved API keys/settings — this is a
rename-commit concern, not a docs concern, and is called out here so it isn't
missed. Environment variables (`ANTHROPIC_API_KEY`, `SDS_SERVER_TOKEN`, etc.)
are provider/infra names, not product names — unaffected.

## Deprecated APIs & removal timeline

| Deprecated item | Since | Planned removal |
|---|---|---|
| `sdsconv-core` crate (re-export shim) | rename commit | ≥1 minor release after `sdsforge-core` ships |
| `sdsconv` CLI/GUI crate | rename commit | ≥1 minor release after `sdsforge` ships |
| `sdsconv` PyPI package (shim) | rename commit | ≥1 minor release after `sdsforge` ships |
| `to-docx` / `to-html` / `to-pdf` CLI subcommands | render-rollout stage 2 | ≥1 minor release after `render` ships |
| `generate_docx` / `generate_html` / `generate_pdf` Rust fns | render-rollout stage 4 | ≥1 minor release after `render_*` ships |

Exact version numbers are filled in once the corresponding commit actually
ships (see `docs/sdsforge-architecture.md` for the version-numbering
decision).

## Migration checklist for downstream users

- Rust: change `Cargo.toml` dependency from `sdsconv-core` to `sdsforge-core`
  (or keep the old dependency during the deprecation window — it will keep
  working, just with a compiler warning).
- Python: `pip install sdsforge` and change `import sdsconv` to
  `import sdsforge`.
- CLI: replace `sdsconv` invocations with `sdsforge`; replace `to-docx`/
  `to-html`/`to-pdf` with `render --to docx|html|pdf` at your own pace before
  the deprecation window closes.
- REST clients: only the base binary/service name changes; no request/response
  shape changes.
