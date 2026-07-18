# Migration: sdsconv → sdsforge

**Status: in progress.** Crates, packages, and the CLI have been renamed
(commit #2). The `render` command, the deprecated `to-docx`/`to-html`/
`to-pdf` aliases, the GUI's "Render" tab, the internal `render_docx`/
`render_html`/`render_pdf` renames, and a working `sdsconv` compat binary
that forwards instead of exiting 1 have all landed (commit #3 — see the
CLI-command-mapping table below, stages 1–4). The Python bindings/package
rename (commit #4) has also landed — see "Python API changes" below. The
GUI/CLI config-directory migration (commit #5) has also landed — see
"Config / environment variables" below. The new `generate` CLI command
(CAS/composition → SDS draft — see the render-rollout stage table's
stage 5, a separate numbering from the commit list here), the README
rewrite, and the GitHub repository rename have not shipped yet.

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

| Stage | Change | Status |
|---|---|---|
| 1 | New `render` command ships, backed by the *existing* to-docx/to-html/to-pdf implementation (`--to docx\|html\|pdf`). | Landed (commit #3) |
| 2 | `to-docx`, `to-html`, `to-pdf` become deprecated aliases for `render --to ...` — same implementation, a deprecation notice is printed to **stderr only** (stdout stays machine-readable JSON/output). | Landed (commit #3) |
| 3 | GUI "Generate" tab is relabeled "Render" (no functional change). | Landed (commit #3) |
| 4 | Internal Rust fn/module names (`generate_docx` → `render_docx`, etc.) are cleaned up where it aids clarity — kept out of the same commit as any public API change. | Landed (commit #3) |
| 5 | The new `generate` command ships (CAS/composition → SDS draft), only once "render" is unambiguous everywhere else. | Not started |

The deprecated `sdsconv` binary (commit #3) forwards its argv into the same
CLI implementation (`sdsforge::run_cli_from`) rather than exiting 1 — see the
migration checklist below.

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

**Status: landed** (commit "refactor: rename Python package and bindings to
sdsforge"). `sdsforge_py`'s cdylib, pyo3 module, PyPI project name, and
`python/sdsforge/` package directory all use the new name; `sdsconv` is now
published from a separate pure-Python `sdsconv_py/` package that depends on
`sdsforge` and re-exports it.

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

**Status: landed** (commit "refactor: migrate config directory to sdsforge").
The GUI/CLI-shared config (`AppConfig` in `sdsforge/src/config.rs`) now lives
at `dirs::config_dir()/sdsforge/config.toml`, migrated automatically from the
pre-rename `dirs::config_dir()/sdsconv/config.toml` the first time it's
loaded — GUI and CLI both go through the same `AppConfig::load()`, so the
resolution logic can't drift between them.

Resolution order, checked on every `load()` call:

1. `sdsforge/config.toml` exists → use it. If `sdsconv/config.toml` also
   exists and has different content, a warning naming the two file *paths*
   (never their contents) is logged and the new file still wins.
2. It's missing but `sdsconv/config.toml` exists → read the old file, then
   migrate it to the new path via a write-to-temp-then-rename (atomic,
   0600 permissions on Unix). If migration fails for any reason (directory
   uncreatable, target unwritable, ...), the settings already read from the
   old file are still used — migration failure never blocks loading.
3. Neither file exists → defaults.

The old `sdsconv/config.toml` is never written to or deleted by this process
— users can remove it manually once they've confirmed the new file is
correct. Migration is idempotent: once `sdsforge/config.toml` exists, it's
read directly and never rewritten by `load()`.

Environment variables (`ANTHROPIC_API_KEY`, `SDS_SERVER_TOKEN`, etc.) are
provider/infra names, not product names — unaffected.

## Deprecated APIs & removal timeline

| Deprecated item | Since | Planned removal |
|---|---|---|
| `sdsconv-core` crate (re-export shim) | rename commit | ≥1 minor release after `sdsforge-core` ships |
| `sdsconv` CLI/GUI crate | rename commit | ≥1 minor release after `sdsforge` ships |
| `sdsconv` PyPI package (shim, `sdsconv_py/`, v0.1.8+) | Python rename commit | ≥1 minor release after `sdsforge` PyPI package ships |
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
  `import sdsforge`. Existing `import sdsconv` code keeps working in the
  meantime — the `sdsconv` package now depends on `sdsforge` and re-exports
  it (including `sdsconv.eval` / `sdsconv.causasv_bridge`), emitting a
  `DeprecationWarning` on import.
- CLI: replace `sdsconv` invocations with `sdsforge`; replace `to-docx`/
  `to-html`/`to-pdf` with `render --to docx|html|pdf` at your own pace before
  the deprecation window closes. Existing `sdsconv` scripts keep working
  in the meantime — the binary prints a deprecation warning to stderr and
  forwards its arguments into the same `sdsforge` CLI implementation (or
  launches the same GUI on no args) rather than failing.
- REST clients: only the base binary/service name changes; no request/response
  shape changes.
