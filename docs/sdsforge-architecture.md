# sdsforge: architecture & design decisions

Internal design reference for the `sdsconv` → `sdsforge` expansion. Pairs with
`docs/migration-from-sdsconv.md` (user-facing naming/compat doc). Nothing in
this document is implemented yet — it is the agreed spec for the commits that
follow this one.

## Product scope

Five capabilities, one crate family:

1. **Generate** — SDS draft from product info + CAS + composition + measured
   properties + evidence. New.
2. **Convert** — unstructured SDS document → structured JSON. Existing
   (`convert_to_json` et al. in `sdsconv_core`).
3. **Translate** — human-language fields only, structure/codes/units frozen.
   Existing at a basic level; needs the field-classification split described
   in the original project brief (translatable vs. frozen fields) — not yet
   implemented, out of scope for this doc's commit.
4. **Validate** — structured SDS vs. regulatory profile. Existing
   (`validate`/`validate_typed`/`Finding` in `sdsconv_core::converter::validator`).
5. **Render** — structured SDS → DOCX/HTML/PDF. Existing, currently
   misnamed "generate" (see terminology table in the migration doc).

## Safety design priority (verbatim, governs every ambiguous call below)

```
人の安全
> 誤情報の防止
> 根拠の追跡可能性
> 規制上の正確性
> 不確実性の明示
> 後方互換性
> 利便性
> 出力項目数
```

In practice: a missing/uncertain value is always reported as `Unresolved`,
never filled with a plausible guess. An incomplete draft is an acceptable
output; a draft that *looks* complete but is wrong is not.

## The three-name problem

Today the project answers to three different names depending on where you
look:

- git remote: `sds-converter`
- `Cargo.toml` `repository`/`homepage` fields: `sdsconv`
- target product name: `sdsforge`

The rename commits need to also decide whether the GitHub repository itself
gets renamed to `sdsforge` (with GitHub's automatic redirect from the old
name) or kept as `sds-converter` with only the Cargo metadata updated to
match. Recommendation: rename the GitHub repo to `sdsforge` — GitHub redirects
old clone/HTTPS URLs automatically, so this is low-risk, and it removes one of
the three names instead of adding a fourth.

## Versioning

Continue the existing version sequence rather than resetting to 0.1.0 for the
renamed packages — the codebase is not new, and resetting would misrepresent
its maturity (works against the 根拠の追跡可能性 priority above):

| Crate | Last `sdsconv*` version | First `sdsforge*` version |
|---|---|---|
| core | `sdsconv-core` 0.3.9 | `sdsforge-core` 0.4.0 |
| CLI/GUI | `sdsconv` 0.2.9 | `sdsforge` 0.3.0 |
| Python bindings | `sdsconv-py` (PyPI `sdsconv`) 0.1.7 | `sdsforge-py` (PyPI `sdsforge`) 0.2.0 |
| server | `sdsconv-server` 0.1.0 (unpublished) | `sdsforge-server` 0.1.0 (unpublished, unchanged) |

CHANGELOG.md gets an explicit old→new version cross-reference table when the
rename commit lands.

## Generation architecture

### Why a new module, not an extension of `converter`

`sdsconv_core::converter` is entirely oriented around unstructured-text → LLM
→ `SdsRoot` (`extract_sds_from_text` in `llm.rs` is the only path that
produces an `SdsRoot`). There is no existing "compose `SdsRoot` from
structured input" path to extend — this is confirmed by reading
`converter/generator.rs`, which despite its name only renders `SdsRoot` →
DOCX/PDF, never the reverse. The new capability is genuinely new architecture:
a sibling module, e.g. `sdsforge_core::generation`, not a refactor of
`converter`.

### Regulatory profile layering

`schema/generated.rs` hardcodes MHLW JSON key names directly onto the domain
structs (`#[serde(rename = "...")]` throughout, `SdsRoot` included) — there is
currently no separation between "the data" and "the MHLW serialization of the
data." Introducing:

```
DomainSds  →  RegulatoryProfile  →  MhlwV1Serializer
```

is new architecture, not a refactor of an existing partial abstraction. The
only prior art is `SourceCountry`/`compliance.rs`, which is a *validation
overlay* (gap report against the single hardcoded MHLW model), not an
alternate serialization target — it does not need to change for `generate` to
work, but it is the closest existing pattern for "regulatory-jurisdiction
awareness" and is worth reading before designing `RegulatoryProfile`.

This layering is listed here for completeness; only the `mhlw-v1` profile
(today's existing behavior, reorganized) needs to exist for this feature to
ship. `jis-z7253-2019`, `jis-z7253-2025`, `un-ghs`, `osha-hcs`, `eu-clp` are
future profiles, not part of this round.

### Provenance & evidence type system

```rust
/// How a field's value was determined. `Calculated`/`Estimated`/`Literature`
/// must never auto-promote to `Confirmed` — promotion requires new evidence,
/// not a higher-confidence label on the same evidence.
pub enum FieldStatus<T> {
    Confirmed(T),      // finished-product or same-batch test report
    Supplied(T),       // user/manufacturer/supplier explicitly provided
    Literature(T),      // public database, literature, existing SDS
    Calculated(T),      // deterministic calculation (e.g. molecular weight)
    Estimated(T),        // model/analogy/structure/empirical-rule estimate
    Unresolved(UnresolvedField),
    NotApplicable(NotApplicableReason),
}

/// Priority-ordered; not every level is valid for every field — see
/// FieldPolicy::allowed_evidence.
pub enum EvidenceLevel {
    ProductTestReport,
    EquivalentBatchTestReport,
    SupplierSpecification,
    SupplierSds,
    RegulatoryDatabase,
    PeerReviewedLiterature,
    ReferenceDatabase,
    DeterministicCalculation,
    ModelEstimate,
    UnverifiedUserInput,
    None,
}

pub struct FieldProvenance {
    pub path: String,                 // e.g. "PhysicalAndChemicalProperties.FlashPoint"
    pub source_type: EvidenceLevel,
    pub source_reference: Option<String>,
    pub source_value: Option<String>,
    pub method: String,
    pub sample_id: Option<String>,
    pub batch_id: Option<String>,
    pub test_method: Option<String>,
    pub conditions: Option<MeasurementConditions>,
    pub retrieved_at: Option<String>,
    pub confidence: ConfidenceLevel,
    pub warnings: Vec<String>,
}

pub struct UnresolvedField {
    pub path: String,
    pub title: String,
    pub reason: UnresolvedReason,
    pub required_inputs: Vec<RequiredInput>,
    pub acceptable_evidence: Vec<EvidenceLevel>,
    pub safety_impact: SafetyImpact,
    pub regulatory_impact: RegulatoryImpact,
    pub recommended_action: String,
    pub blocks_release: bool,
}

pub enum UnresolvedReason {
    MissingInput,
    ProductTestRequired,
    AmbiguousChemicalIdentity,
    ConflictingSources,
    UnsupportedCalculation,
    InsufficientMeasurementConditions,
    MixtureCannotBeDerivedFromComponents,
    RegulatoryJudgementRequired,
    HumanReviewRequired,
}

/// Per-field policy — what evidence is even eligible for this field.
/// e.g. MolecularWeight allows DeterministicCalculation; FlashPoint does not
/// (mixture flash points are not derivable from component values — see the
/// "no averaging" rule below).
pub struct FieldPolicy {
    pub path: &'static str,
    pub allowed_evidence: &'static [EvidenceLevel],
    pub product_test_required: bool,
    pub calculation_allowed: bool,
    pub estimation_allowed: bool,
    pub blocks_release_if_missing: bool,
}

pub struct GenerationResult {
    pub sds: DomainSds,
    pub findings: Vec<Finding>,            // reuses sdsforge_core::converter::validator::Finding
    pub unresolved: Vec<UnresolvedField>,
    pub provenance: Vec<FieldProvenance>,
    pub evidence_summary: EvidenceSummary,
    pub release_status: ReleaseStatus,
}

/// `Approved` is never set by generation code — only by an explicit
/// human-approval record (approver, timestamp, target version) applied
/// afterward. Automated output is always `Draft` or `ReviewRequired`, or
/// `Blocked` if a release-gating field is unresolved.
pub enum ReleaseStatus { Draft, ReviewRequired, Blocked, Approved }

pub struct ReleaseGateResult {
    pub status: ReleaseStatus,
    pub blocking_findings: Vec<Finding>,
    pub required_actions: Vec<String>,
}
```

`Finding` (existing, `sdsconv_core::converter::validator::Finding` —
`{level, rule, message}`) is reused as-is for the `findings` field above; it
already has the severity axis (`CRIT`/`HIGH`/`MED`/`LOW`/`WARN`) this needs.
`FieldProvenance`/`UnresolvedField` are the net-new types — nothing comparable

**Implementation note (commit #10):** `GenerationResult.sds` is `SdsRoot`,
not `DomainSds` as sketched above. The `DomainSds → RegulatoryProfile →
MhlwV1Serializer` layering described earlier in this document is real future
work, but it doesn't exist yet, and commit #9 already committed to returning
the current MHLW-backed `SdsRoot` from its Section 1/3 draft generator.
Introducing a `DomainSds` type alias or thin wrapper now, only to satisfy
this document's original sketch, would be a speculative abstraction with no
behavior behind it — `sds: SdsRoot` is documented in code as today's
`mhlw-v1` representation, and this note exists so a future commit that adds
real profile-layering doesn't mistake the current shape for a settled
regulatory-profile design.
`FieldProvenance`/`UnresolvedField` are the net-new types — nothing comparable
exists in `sdsconv_core` today (confirmed by grep for "provenance"/
"confidence").

### Properties that require product-level evidence — never derived from CAS + composition alone

| Property | Minimum required input |
|---|---|
| Flash point | Product (or same-batch) closed/open-cup test: value, unit, method, sample/batch ID |
| Initial boiling point / boiling range | Value(s), pressure, method, decomposition-before-boiling flag |
| Vapor pressure | Value(s) **with measurement temperature**, method, basis (measured vs. calculated) |
| Explosive (flammability) limits (LEL/UEL) | Values, atmosphere/O₂%, temperature, pressure, method |
| Self-reactivity | UN test series A–H result or SADT, DSC screening alone is not sufficient |
| Self-accelerating decomposition temp (SADT) | Package mass/type, test result |
| Oxidizing properties | Physical-state-appropriate UN test (O.1/O.2/O.3), not structure alone |
| Metal corrosivity | Steel/aluminium corrosion-rate test at specified temperature — pH alone is insufficient |
| Dust explosivity | Particle size, moisture, Kst, min. ignition energy |
| Decomposition temperature | Test result |
| Viscosity / density | Product measurement, temperature |
| Autoignition temperature | Test result |
| Transport classification test values | UN test data, not CAS-number inference |
| Product-specific toxicity / ecotoxicity | Product or read-across test data, not component data alone |

Mixture properties are **never** derived by averaging component values —
mixing non-idealities, azeotropes, and test-method sensitivity make an
averaged value plausible-looking but wrong. Every one of the properties above
resolves to `Unresolved` (with `required_inputs` populated) rather than a
computed/estimated placeholder when the finished-product evidence is absent.

### chematic integration boundary

```
CAS resolver                    (existing: enrichment::lookup_cas → PubChem)
    ↓  chemical identity candidate (IUPAC name, formula, PubChem CID, canonical SMILES)
chematic normalization/checks    (SMILES → canonical form, consistency check,
                                   UVCB/salt/solvate/isomer ambiguity detection,
                                   structural alerts — PAINS/Brenk as screening only)
    ↓
sdsforge domain model
```

`enrichment::lookup_cas` (`sdsconv_core/src/enrichment.rs:40`) already does
the CAS→PubChem step and is the reuse target — it is not rebuilt. chematic is
never the source of CAS→identity resolution; it only normalizes/validates a
structure PubChem already resolved, and flags ambiguity (multiple structure
candidates, UVCB, polymer, mixture CAS, salt, solvate) as a warning or
`Unresolved`, never a silent pick of one candidate.

chematic (v0.4.30, 18+ sub-crates: fingerprints, conformers, force fields, ADMET
screening, etc.) is a much larger surface than this integration needs. Only
its SMILES parsing/canonicalization, structural-alert screening, and
ambiguity/consistency checks are in scope for this feature. Per chematic's own
published limitations, nothing it returns is treated as guaranteed-exact —
e.g. its canonical SMILES has a documented ~5.5% E/Z-normalization
instability and ~96.3% aromaticity/CIP parity with RDKit — these numbers stay
below `DeterministicCalculation`/`ModelEstimate` evidence level, never
`ProductTestReport`/`Confirmed`.

### Output separation

```
official_sds.json      — MHLW schema only, nothing else. This is the artifact
                          downstream systems may treat as authoritative.
generation_report.json — unresolved + provenance + findings + evidence_summary
                          + release gate. Never merged into official_sds.json.
review_report.md       — human-readable rendering of generation_report.json.
```

No field outside the official MHLW schema is ever written into
`official_sds.json`, even as an extra/unofficial key — this was an explicit
requirement (厚生労働省の正式スキーマに存在しない情報を、正式フィールドへ勝手に混入させない).

### LLM role limits (generation feature only)

LLM use is limited to: extracting candidate values from source documents,
free-text → structured conversion, translation-candidate generation, writing
explanatory text for unresolved items and human-readable reports. LLM output
is never used directly for: guessing missing physical properties, finalizing
a GHS classification, inventing test values/report numbers/sources, resolving
CAS-to-structure ambiguity, or determining transport classification. Every
LLM-sourced value goes through original-text cross-check, type/unit/range/code
validation before use; anything that fails validation becomes `Unresolved` or
`UnverifiedUserInput`, never a silently-accepted value.

## Commit roadmap

Adapted from the original 12-commit sequence to the actual repo layout found
during investigation. Each commit keeps `cargo build`/`cargo test` green
before moving to the next.

1. **docs: define sdsforge scope and migration plan** — *this commit.*
2. **refactor: rename Rust workspace packages** — `sdsconv_core` → `sdsforge_core`
   (dir + `Cargo.toml` `name`), same for `sdsconv`/`sdsconv_server`/`sdsconv_py`;
   root `Cargo.toml` `members`; add `#[deprecated]` re-export shim crates at
   the old names/paths.
3. **feat: introduce `render` command + deprecate to-docx/to-html/to-pdf**
   (render-rollout stages 1–2 from the migration doc) — new command, old
   commands become aliases with stderr-only deprecation notices.
4. **refactor: rename internal generate_* fns to render_*, GUI tab rename**
   (render-rollout stages 3–4) — `sdsconv/src/app.rs` `Tab` enum
   (`app.rs:519`) label + `sdsconv_core/src/converter/generator.rs` /
   `pdf.rs` fn renames, kept separate from commit 3's public CLI change.
5. **refactor: rename Python package and bindings** — the 5 coordinated
   points identified in investigation: `sdsconv_py/Cargo.toml` crate name,
   `[lib] name`, `#[pyo3(name=...)]` in `src/lib.rs`, `pyproject.toml`
   `module-name`/`[project] name`, and the `from ._sdsconv import` line in
   `python/sdsconv/__init__.py` — all move together or the import breaks.
6. **refactor: update GUI and server branding** — remaining ~10 user-visible
   "sdsconv" strings in `app.rs` (About dialog, window title, manual text ×3
   languages), `sdsconv_server/src/main.rs` doc comment + startup log line.
7. **docs: update README, examples, badges, and links** (3 languages: en/ja/zh)
   — crates.io/PyPI/docs.rs links, install examples, CLI examples,
   `docs/comparison.md` product name, undocumented commands (`to-html`,
   `to-pdf`, `detect-lang`, `eval-corpus`) get documented under their final
   names.
8. **feat: add formulation input domain model** — `ProductInput`,
   `ComponentInput`, `SupplierInput`, `ConcentrationRange` per the original
   brief, in the new `sdsforge_core::generation` module.
9. **feat: generate Section 1 and Section 3 draft data** — wraps
   `enrichment::lookup_cas`/`enrich_composition`, produces `DomainSds`
   fragments for product identification + composition only (the sections
   derivable from CAS + composition + supplied data without product testing).
10. **feat: add unresolved fields and provenance** — the type system above
    (`FieldStatus`, `EvidenceLevel`, `FieldProvenance`, `UnresolvedField`,
    `FieldPolicy`, `GenerationResult`, `ReleaseStatus`/`ReleaseGateResult`),
    wired through commit 9's Section 1/3 generator as the first real caller.
11. **feat: integrate chematic chemical normalization** — SMILES
    canonicalization/consistency/ambiguity-flagging step between
    `lookup_cas` and the domain model, per the boundary diagram above.
12. **test: add generation and migration regression tests** — the full test
    list from the safety-design spec (no-fabrication tests, provenance tests,
    unresolved tests, release-gate tests) plus the rename regression suite
    (CLI name, `--help` has no old names, README install examples, Python
    import, crate names, GUI About text).
13. **ci: update package and release workflows** — `release.yml` bundle ID
    (`com.sdsconv.app` → `com.sdsforge.app`), artifact names
    (`sdsconv-macos.zip`→`sdsforge-macos.zip`, `sdsconv-windows-portable.zip`),
    Homebrew tap target (`homebrew-sdsconv`→`homebrew-sdsforge`),
    `python-wheels.yml` wheel-name matching, PyPI trusted-publishing project
    name.

Commits 8–11 (the actual generation feature) are the largest and most
safety-sensitive; each should land with its own tests before the next, not as
one combined commit.

## Baseline snapshot (captured before any code change, this session)

```
cargo fmt --all -- --check                                    → FAIL (pre-existing, 549 diff hunks)
cargo build --workspace --all-features                        → FAIL on sdsconv-py only
                                                                  (macOS cdylib link error — known pyo3
                                                                  extension-module issue with plain
                                                                  `cargo build`; not CI-relevant since
                                                                  python-wheels.yml already uses maturin,
                                                                  which sets the required link flags)
cargo clippy --workspace --all-targets --all-features -- -D warnings → FAIL (pre-existing, 20 errors,
                                                                  e.g. converter/mod.rs:399 manual char
                                                                  comparison)
cargo test --workspace --all-features                          → PASS, 84/84
                                                                  (sdsconv: 6, sdsconv_core: 74,
                                                                   sdsconv_py: 0, sdsconv_server: 0,
                                                                   + 4 doctests)
```

None of these are introduced by this docs commit. They're recorded here so
later sessions can tell a real regression from pre-existing debt. `fmt`/
`clippy` cleanup is not part of the rename's scope unless a rename commit
happens to touch a file that's already in violation — fixing the other 545+
hunks unrelated to the rename is a separate task.
