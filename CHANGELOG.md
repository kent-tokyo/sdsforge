# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-07-19

> Package versions: `sdsforge 0.4.0`, `sdsforge-core 0.5.0`

`sdsforge generate` — the SDS document generation feature — is the headline
addition in this release: drafting a structured SDS from a product
formulation, rather than only converting existing unstructured documents.

### Added

- **`sdsforge generate`**: new CLI command that drafts an SDS from a product
  formulation (`ProductInput`) instead of unstructured document text.
  - JSON/YAML `ProductInput` with unknown-field rejection
    (`#[serde(deny_unknown_fields)]`), so a typo in the input file is a parse
    error, not silently ignored; genuinely-optional collections
    (`other_names`, `measured_properties`, `evidence`) may be omitted.
  - Section 1 and Section 3 draft generation from resolved
    component/supplier data.
  - Measured-property and evidence input for the seven properties Section
    1/3 can never resolve on its own (flash point, boiling point, vapor
    pressure, explosive limits, self-reactivity, oxidizing properties, metal
    corrosivity).
  - Provenance, unresolved fields, evidence summary, and release status
    tracked per field and surfaced in the generation report.
  - Three output artifacts: `official_sds.json` (MHLW-format draft),
    `generation_report.json` (machine-readable findings/provenance/release
    status), `review_report.md` (human-readable summary).
  - Optional PubChem enrichment (`--enrich`) for CAS-based chemical identity
    resolution.
  - Optional chematic-backed chemical structure normalization (SMILES
    canonicalization, formula/charge/fragment consistency checks).
  - `--strict` (exit non-zero when the draft is `Blocked`) and `--force`
    (replace existing output artifacts).
  - Offline-by-default operation — no network access unless `--enrich` is
    passed.

### Safety

- `generate` never returns `Approved` — its output is always an unapproved
  draft requiring human review.
- Missing product-level evidence remains `Unresolved` rather than being
  inferred or estimated.
- Mixture properties (flash point, boiling point, vapor pressure, etc.) are
  never inferred by averaging or otherwise deriving them from
  component-level values.
- Ambiguous CAS resolution (multiple PubChem candidates) and
  chematic-detected formula conflicts block release (`Blocked`) rather than
  silently picking a candidate.
- With `--enrich`, only CAS numbers are ever transmitted to PubChem —
  product name, concentrations, supplier, and evidence data never leave the
  machine.
- Official SDS data (`official_sds.json`) stays fully separate from
  generation diagnostics (findings, provenance, evidence summary) — the
  report is never embedded in the schema output, and vice versa.
- Added whole-document golden generation acceptance tests covering a
  single substance, an evidence-backed mixture, a blocked invalid-CAS
  input, and a salt/multi-fragment structure. The tests verify expected
  official SDS output, findings, unresolved fields, release status,
  provenance levels, and non-fabrication.

### Fixed

- **PubChem PUG REST property contract restored**: the enrichment lookup
  requested obsolete property names (`CanonicalSMILES`, `IsomericSMILES`)
  that PubChem's current API rejects with HTTP 400. Corrected to
  `SMILES`/`ConnectivitySMILES`:
  - `CanonicalSMILES` → `ConnectivitySMILES`
  - `IsomericSMILES` → `SMILES`
  - replaced the single-step name lookup with two-step CAS → CID → property
    resolution, so a name matching multiple compounds is correctly reported
    as ambiguous rather than silently taking the first result
  - bounded retry/rate-limit handling (HTTP 429/503 retried with backoff,
    400 never retried, PubChem's ~5 req/sec limit respected)
- **Windows/Linux generation-artifact replacement verification**:
  `sdsforge generate --force` reliably replaces all three generation
  artifacts on both Linux and Windows GitHub-hosted runners, confirmed via
  a dedicated CI matrix job exercising the real compiled CLI.

### Fixed (QC r33 + roundtrip test tooling — seed=555 findings)

- **S9-FLASH-POINT-NOT-NUMERIC FP** (`tools/quality_check.py`): Added EN suppression phrases
  `None detected`, `not detected`, `no flash point detected`, `not measurable` to flash-point
  N/A list. en/mgchemicals sds-4120-l had flash point `'None detected'` (no flammable H-codes)
  firing spurious HIGH. Note: `'該当しない'` + H226 in ja/cerij is kept as a real finding.

- **S4-NO-PHYSICIAN FP for zh-cn Simplified** (`tools/quality_check.py`): Added `求医` (求 +
  医 U+533B) and `就诊` (就 + 诊 U+8BCA) to the physician-contact keyword regex. Previous regex
  had Traditional `求醫`/`就診` (醫 U+91AB, 診 U+8A3A) but not the Simplified forms. zh-cn/tci_cn
  had `'求医/就诊'` (seek medical treatment / go to the doctor) that was unmatched.

- **S8-OEL-NO-NUMERIC FP for zh-cn dust-OEL reference** (`tools/quality_check.py`): Added
  `使用粉尘的职业接触限值|在控制.*职业接触限值` to OEL suppression. zh-cn/fpcusa uses the phrase
  "when controlling exposure, use the dust OEL" — a legitimate compliance reference to general
  dust standards. Combined with prior r31/r32 fixes, **S8-OEL-NO-NUMERIC is now 0/208 files**.

- **Roundtrip test timeout robustness** (`tools/roundtrip_diverse50.py`, `tools/roundtrip_random30.py`):
  Replaced `subprocess.run(capture_output=True, timeout=180)` with `subprocess.Popen` +
  `start_new_session=True` + `os.killpg(SIGKILL)` on timeout. On macOS, the old approach could
  hang indefinitely when the Anthropic SDK's internal retry logic accumulated to ~30 minutes
  (ja/cometkato: 1907s with subprocess.run vs expected 300s kill). The process-group kill
  ensures the entire process and all its threads are terminated reliably. Timeout raised to 300s.

### Fixed (converter — zh-cn/cup and zh-tw/herli persistent failures)

- **`max_tokens` raised for medium quality** (`sdsconv/src/tasks.rs`): `Quality::Medium`
  max_tokens increased from 16,384 → 32,768. Complex zh-cn/zh-tw SDSs (dense classification
  tables, verbose Chinese content) exceeded 16k output tokens, causing the API to return
  `stop_reason=max_tokens` with a truncated JSON that failed deserialization (herli:
  `Invalid JSON: expected ',' at line 208 column 5`). herli now converts to **16/16 sections**.

- **N-element array unwrapping for struct-typed sections** (`sdsconv_core/src/converter/llm.rs`):
  Added targeted section-key-based array unwrapping in `lenient_deserialize`. When claude-haiku
  processes certain zh-cn PDFs (e.g. cup/1,2-dichloropropane), it returns every struct-typed
  section as a **7-element array** where element[0] is the correct data and elements[1..6] are
  repeated stacking of the same 7-section block:
  - `"Identification": [{Id}, {HazardId}, {Comp}, ..., {ExposureCtrl}]`  (7 elements)
  - `"HazardIdentification": [{HazardId}, {Comp}, ..., {ExposureCtrl}]`  (7 elements)
  - ...every section gets an array starting with its own correct data
  Fix: for the 15 known struct-typed section keys, take `element[0]` (correct data) and discard
  `[1..6]`. `ToxicologicalInformation` / `EcologicalInformation` (Vec<T>) excluded — handled
  by the existing `vec_section!` macro. cup now converts to **16/16 sections** (was 5/16).

- **`vec_section!` macro for Vec<T> sections** (`sdsconv_core/src/converter/llm.rs`):
  Added a `vec_section!` macro variant used for `ToxicologicalInformation` and
  `EcologicalInformation`. These sections use `Vec<T>` Rust types but some LLMs return them
  as plain objects instead of arrays. The macro wraps bare objects in `vec![obj]` before
  serde deserialization. herli had ToxicologicalInformation/EcologicalInformation as plain
  objects — both sections now deserialize successfully.

### Added (QC r32 — seed=314 diverse 50-file roundtrip test — 53 manufacturers, 48/50 success)

- **QC r32: S3-MAJORITY-CONC-MISSING (HIGH)** (`tools/quality_check.py`): New rule fires when
  a mixture with >3 components has >50% (but NOT all) components lacking numeric concentration
  values. Bridges existing `S3-MIXTURE-ALL-CONC-MISSING` (all-absent HIGH) and per-component
  `S3-CONC-UNIT-NO-VALUE` (MED). Confirmed extraction failure by `extract-text` on ja/eneos PDF:
  source has `3質量%以上5質量%未満` range values that the LLM captures only as unit `質量%`.
  When this fires, per-component `S3-CONC-UNIT-NO-VALUE` MEDs are suppressed (exactly one HIGH
  per file, following r28 dedup precedent). Fires 7/158 files — distinct from all-missing cases.

### Fixed (QC r32)

- **S3-CAS-FORMAT false positives for proprietary ingredients** (`tools/quality_check.py`):
  Added suppression for proprietary/trade-secret CAS placeholders: `Proprietary`, `Trade Secret`,
  `Confidential`, `未特定`, `機密`, `専売品`, `不明`, etc. GHS guidelines allow withholding CAS
  numbers for confidential ingredients — these are not format violations. Confirmed in
  zh-tw/tsrc (`Proprietary`) and ja/shell_lubes_jp (`未特定`). 2 FPs → 0.

- **S8-OEL-NO-NUMERIC false positives (additional phrases)** (`tools/quality_check.py`):
  Added EN `No biological limit`/`no.*biological.*limit`/`not.*allocated` (en/shell),
  zh-cn Simplified `不含有职业接触限值|不含.*职业接触限值` (职 U+804C ≠ Traditional 職 U+8077 —
  r31 added Traditional only, r32 adds Simplified), zh-tw `無資料` (無 U+7121 ≠ 无 U+65E0),
  standalone `\b無\b`/`\b无\b` (zh-tw/tsrc OEL = `["無"]`; word-boundary-safe: confirmed
  does NOT match inside `無資料`, `無機化合物`, `無相關`). Across 158 files: **7 → 2**.

- **S4-NO-PHYSICIAN false positives for zh-tw** (`tools/quality_check.py`):
  Added `送醫`/`送医` (send to hospital/doctor) to the physician-contact keyword list.
  Confirmed FP in sfchem/ttl: both use `立即送醫` (immediately send for medical care)
  which is standard zh-tw emergency guidance but was not matched by existing keywords.

### Notes

- **zh-cn/cup and zh-tw/herli consistently fail PDF→JSON conversion** (exit after 81.4s/39.7s,
  model downgraded to `haiku-4-5` medium quality). Both PDFs have extractable text (confirmed by
  `extract-text`). The failure is in the conversion engine — haiku medium quality fails on these
  particular formats. This is a converter-side issue, not a QC rule issue.

### Added (QC r31 — seed=77 diverse 50-file roundtrip test — 53 manufacturers, 4 languages)

- **QC r31: walk_text NFKC normalization** (`tools/quality_check.py`): Every string value
  processed by `walk_text` is now NFKC-normalized (fullwidth Latin → ASCII: `ｍ→m`, `℃→°C`,
  `３→3`). CJK characters are unaffected. Tested across all 160 on-disk JSONs: zero regressions,
  fixes fullwidth unit failures (e.g. `0.2mg/ｍ3` in ja/nipponkinzoku OEL fields), and newly
  detects flash point fields containing only ` ℃` unit with no numeric value (true positives).

- **QC r31: S2-MULTIPLE-SIGNAL-WORDS (HIGH)** (`tools/quality_check.py`): Detects
  `SignalWord` fields that concatenate two valid GHS signal words (e.g. `危険、警告`, `Danger/Warning`).
  GHS requires exactly one signal word per SDS. Confirmed in ja/pacific_steel. Branches from
  S2-INVALID-SIGNAL-WORD to avoid double-count — same field fires either HIGH (concatenated)
  or MED (unknown word), never both.

### Fixed (QC r31)

- **S9-FLASH-POINT-NOT-NUMERIC bug fix** (`tools/quality_check.py`): Changed `if not fp_val`
  to `if fp_val is None`. Python `not 0.0 == True` caused ExactValue=0.0 (zh-cn/akzonobel)
  to be treated as absent, firing spurious HIGH. 1 FP → 0.

- **S9-FLASH-POINT-NOT-NUMERIC suppression** (`tools/quality_check.py`): Added
  `沸騰するまで引火せず`/`引火しない`/`非引火性`/`不燃` for substances that genuinely don't
  ignite (ja/d_kasei, no flammable H-codes). Substances WITH flammable H-codes + "not applicable"
  still fire correctly (H226 + `該当しない` in ja/cerij remains HIGH).

- **S4-EYE-HCODE-NO-EYE-AID false positives** (`tools/quality_check.py`): Suppressed when
  `ExposureRoute.FirstAidEye` sub-field is populated (6/6 FPs: ja SDSs use `水で…洗う` +
  `コンタクトレンズ` without `目/眼/rinse`). Also added `コンタクトレンズ|まぶた|contact.*lens`
  to keyword list. Akzonobel (eye={}, H314/H319) still fires as true positive.
  6 FPs → 0.

- **S8-OEL-NO-NUMERIC new false positives** (`tools/quality_check.py`): Added suppression
  phrases from diverse manufacturer corpus: `未設定` (ja, not established),
  `無相關|無相關職業接觸限值|無相關職業暴露` (zh-tw, no relevant OEL data),
  `不含.*職業接觸限值|不含.*職業暴露極限値` (zh-tw/zh-cn, no substances with OEL).
  Combined with NFKC fix: `0.2mg/ｍ3` fullwidth unit now correctly detected as numeric.
  S8-OEL-NO-NUMERIC total across 160 files: **51 → 6** (88% reduction).

- **S13-NO-DISPOSAL-KEYWORDS false positives for gas products** (`tools/quality_check.py`):
  Added gas-specific disposal keywords: `dissipat`, `消散` (release/dissipate to atmosphere),
  `气瓶`/`氣瓶` (gas cylinder), `返.*供[应應]商` (return to supplier). Added zh-tw Traditional
  `法規` (U+898F ≠ Simplified 规 U+89C4) and `環保法規`. Covers zh-cn/sandiego, zh-tw/fpcc,
  zh-tw/gs_battery, zh-tw/sh_gas. S13-NO-DISPOSAL-KEYWORDS: **45 → 1** across 160 files.

### Added (QC r30 — seed=202 30-file roundtrip test findings)

- **QC r30: S1-ALL-SUPPLIER-FIELDS-ABSENT (HIGH)** (`tools/quality_check.py`): New rule fires
  when a **hazardous product** (has H-codes) has the `SupplierInformation` key **completely absent**
  from `Identification` — distinguished from `S1-NO-COMPANY-NAME` (which fires when the key exists
  but `CompanyName` is empty). Verified by `sdsconv extract-text` on ichemistry 2933.pdf:
  source document has blank `供应商名称/电话/地址` fields — the entire supplier section was never
  filled in. A hazardous SDS with no supplier contact information is a JIS Z 7253 Section 1
  regulatory violation. Fires for 6/60 files across both test seeds (all ichemistry zh-cn files
  where `SupplierInformation` key is entirely absent).

### Fixed (QC r30)

- **S7-FLAMMABLE-NO-STORAGE-TEMP false positives for EN/zh** (`tools/quality_check.py`):
  Added `keep cool`, `cool place`, `cool.*area`, `cool.*dry` (EN) and `阴凉` (zh-cn) /
  `陰涼` (zh-tw Traditional, U+9670+U+6DBC ≠ Simplified U+9634+U+51C9) to storage-temperature
  suppression. These are legitimate "store in a cool place" instructions without numeric limits.
  Reduced from 8/30 to 0 false positives.

- **S9-CORROSIVE-NO-PH false positives** (`tools/quality_check.py`): Tightened trigger from
  `{H314, H290, H318, H319}` to `{H314, H290}` only. H318 (serious eye damage) and H319 (eye
  irritation) do not imply acidic/basic chemistry — confirmed: acetone oxime (H318), IPA,
  ethanol, 2-propanol (all H319) fired false positives. H314 (skin corrosion) and H290 (corrosive
  to metals) reliably require acid/base chemistry. Reduced 6 of 7 FPs; 2 true positives verified
  (W01W0102-0240JGHEEN H314, W01W0103-0115JGHEJP H314+H290).

- **S2-NON-ENGLISH-SIGNAL-WORD false positives for all-caps EN SDSs** (`tools/quality_check.py`):
  Changed from exact-match `{"Danger","Warning","N/A"}` to case-insensitive comparison.
  mgchemicals EN SDSs write `DANGER`/`WARNING` (all caps) which was not in the valid set.
  2 FPs → 0.

- **S12-NO-LOGP false positive for fujifilm SDSs** (`tools/quality_check.py`):
  Extended search to also check Section 9 (`PhysicalChemicalProperties.OtherPhysicalChemicalProperty`)
  for numeric LogP values. JIS Z 7253 allows partition coefficient in Sec 9 or Sec 12; fujifilm_wako
  SDSs list `n-オクタノール/水分配係数: 2.64` in Section 9. Only suppresses when Section 9 has a
  NUMERIC value (not `"Not specified"`/`"データなし"`), preserving true positives.
  1 FP → 0.

### Added (QC r29 — seed=99 30-file roundtrip test findings)

- **QC r29: CX-JA-TEXT-IN-ZH-SDS (HIGH)** (`tools/quality_check.py`): New cross-field rule
  fires when a zh-cn or zh-tw SDS contains **Japanese hiragana** in `HazardIdentification`
  or `Composition`. Hiragana is exclusive to Japanese and its presence indicates LLM language
  contamination — the model sourced Japanese classification templates for Chinese-language SDSs
  (e.g. Classification values `"分類できない"`, `"該当区分なし"` injected into zh-cn/zh-tw
  Classification objects). Confirmed across 8/30 files (6 zh-cn including all ichemistry files,
  2 zh-tw thu_orglab files). Fires as HIGH to surface this silently wrong extraction to the user.

### Fixed (QC r29)

- **S8-OEL-NO-NUMERIC false positives for zh-tw Traditional Chinese** (`tools/quality_check.py`):
  Added Traditional Chinese "no OEL" phrases `無職業接觸限值`, `未建立職業接觸限值`,
  `無職業接觸限值資料` to the suppression list. The Simplified Chinese equivalents (`无职业接触限值`)
  were already handled but Traditional forms use different Unicode code points
  (U+7121 `無` ≠ U+65E0 `无`, U+81E5 `職` ≠ U+804C `职`) — causing 3 FPs per run → 0.

- **S8-OEL-NO-NUMERIC false positive for zh-cn "不要求"** (`tools/quality_check.py`):
  Added `不要求` (not required) to the OEL suppression list. Strem zh-cn SDSs use this phrase
  when Chinese OEL monitoring is not mandated for the substance. Caused 3-4 FPs per run → 0.

- **S8-OEL-NO-NUMERIC false positive for footnote-annotated values** (`tools/quality_check.py`):
  Fixed numeric detection regex to allow asterisk footnote markers between number and unit
  (e.g. `"0.05* 0.03** mg/m³"` in lead-containing strem SDS). Pattern updated from
  `\d+\.?\d*\s*(mg/m|...)` to `\d+\.?\d*[*＊]*\s*(mg/m|...)`. Was causing 1 FP → 0.

- **S4-H314-NO-REMOVE-CLOTHING false positives for zh-tw** (`tools/quality_check.py`):
  Added Traditional Chinese `脫掉` and `脫去` (U+812B) to the remove-clothing detection regex.
  Simplified Chinese uses `脱` (U+8131) — different Unicode code point — so `脫掉污染的衣物`
  in zh-tw SDSs was not matched. Confirmed in 2 thu_orglab/thu_gclab files → 0 FPs.

- **S8-SKIN-NO-GLOVE-MATERIAL false positive for zh-tw** (`tools/quality_check.py`):
  Added Traditional Chinese `橡膠` (rubber, U+81A0) to the glove material keyword list.
  Simplified Chinese uses `橡胶` (胶 U+80F6) — different Unicode code point — so
  `合成橡膠材質手套` (synthetic rubber gloves) in zh-tw SDSs was not matched → 0 FPs.

### Added (QC r28 — seed=137 30-file roundtrip test findings)

- **QC r28: S3-MIXTURE-ALL-CONC-MISSING (HIGH)** (`tools/quality_check.py`): New rule fires
  when a mixture with >2 components has **zero numeric concentration values** across all
  components. Detects cases where the LLM extracted component structure and unit strings
  (`"% w/w"`) but missed all numeric ranges (confirmed by `extract-text` against Shell diesel
  SDS source which contained `>= 0 - <= 100` ranges). Fires as HIGH. Observed in 3/30 files
  (Shell diesel, ENEOS lubricant, GS Battery).

### Fixed (QC r28)

- **S2 pictogram double-counting** (`tools/quality_check.py`): Specific pictogram rules
  `S2-FLAMMABLE-NO-GHS02`, `S2-CORROSIVE-NO-GHS05`, `S2-ACUTETOX-NO-GHS06`,
  `S2-EXPLOSIVE-NO-GHS01`, `S2-ENV-NO-GHS09` now only fire when the Pictogram list is
  **non-empty** (i.e. some pictograms were extracted but the required one is absent).
  When `S2-HAZARD-NO-PICTOGRAM` fires (all pictograms absent), these rules are suppressed
  to avoid triple-counting. Eliminated 23 redundant MED issues in the 30-file test.

- **S3-CONC-UNIT-NO-VALUE per-component dedup** (`tools/quality_check.py`): When new
  `S3-MIXTURE-ALL-CONC-MISSING` (HIGH) fires, per-component `S3-CONC-UNIT-NO-VALUE` (MED)
  hits for the same file are suppressed (N components × 1 HIGH is cleaner than N+1 issues).

- **S4-NO-PHYSICIAN false positives for zh-tw/zh-cn** (`tools/quality_check.py`): Added
  `就醫` (Traditional Chinese for "seek medical", U+91AB vs Simplified `就医` U+533B), plus
  `醫師`, `醫生`, `醫療`, `就診`, `求醫`, `医生`, `医疗`, `医院` to the physician detection
  regex. Eliminated 9 false positives in the 30-file test (→0).

- **S6-NO-CLEANUP-KEYWORDS false positives for zh-tw/zh-cn** (`tools/quality_check.py`):
  Added Chinese character variants that differ from Japanese Kanji in Unicode: `吸收` (absorb,
  U+6536) vs JA `吸収` (U+53CE), `收集`/`回收` vs JA `収集`/`回収`, `沙` (sand, U+6C99) vs
  JA `砂` (U+7802). Also added `围堵`, `通風` (Traditional), `收容`, `处置`/`處置`.
  Reduced false positives from 7 to 1.

- **S10-NO-STABILITY-KEYWORDS false positives for EN/zh** (`tools/quality_check.py`):
  Added English terms `temperature`, `oxidiz`, `decompos`, `reaction`, `processing` (fujifilm
  EN SDSs use "Extremes of temperature", "Strong oxidizing agents" which were not matched).
  Added Chinese terms `稳定`/`穩定` (stable, zh), `氧化` (oxidation, zh, ≠ JA `酸化`),
  `反应`/`反應` (reaction, zh), `不相容` (incompatible), `避免` (avoid). Eliminated 4 FPs → 0.

- **S13-NO-DISPOSAL-KEYWORDS false positives for zh-tw** (`tools/quality_check.py`):
  Added Traditional Chinese disposal terms: `廢棄物`, `廢棄`, `廢物`, `廢液`, `焚燒` (Traditional
  incineration, ≠ Simplified `焚烧`), `處置`/`处置`, `廢棄`/`废弃`, `丢弃`, `弃置`, plus
  zh-cn regulatory disposal patterns `规章`. Eliminated 11 false positives → 0.

- **S15-NO-LAW-KEYWORDS false positives for zh-cn** (`tools/quality_check.py`):
  Added Chinese regulatory terms: `办法`/`管理办法` (administrative regulations), `规范`/`規範`
  (standards), `名录`/`名錄` (chemical inventories, e.g. IECSC), `管制`, `IECSC`,
  `化学品`, `危险化学`, `安全生产`, `化学品登记`, `环境管理`. Strem zh-cn SDSs reference
  "新化学物质环境管理办法" and "中国现有化学物质名录" which are legitimate regulatory citations.
  Reduced false positives from 6 to 1.

**Net r28 effect on 30-file test:** HIGH 11→14 (+3 new true positives), MED 251→180 (−71 FPs
eliminated, 28% noise reduction). Seven rule categories reach zero false positives.

## [0.2.8] / [0.3.8] — 2026-06-01

### Added

- **QC r27: S2-HAZARD-NO-PICTOGRAM (MED)** (`tools/quality_check.py`): New rule fires when an
  active signal word (Danger/Warning) and H-codes are present but the `Pictogram` list is
  completely empty. Detects PDFs where pictograms are embedded as images and cannot be extracted
  as GHS01–GHS09 text codes by the LLM. Observed in 60% of a 30-file random test sample across
  all four languages.

- **QC r27: S3-CONC-UNIT-NO-VALUE (MED)** (`tools/quality_check.py`): New rule fires for
  mixture components whose `Concentration.NumericRangeWithUnitAndQualifier` contains a `Unit`
  field (e.g. `"%"`) but no `ExactValue`, `LowerValue`, or `UpperValue`. Detects the common
  LLM extraction gap where the unit is identified but the numeric range is missed.

- **`tools/roundtrip_random30.py`**: New balanced random-sample roundtrip test script.
  Selects `n` PDFs with configurable seed across all four language folders (`--seed`, `--n`),
  runs the full PDF → JSON → DOCX → QC pipeline, and produces a per-rule ranking report and
  JSON summary.

### Fixed

- **QC r27: S2-INVALID-SIGNAL-WORD false positives** (`tools/quality_check.py`): Added
  `危險` (Traditional Chinese "Danger", zh-tw) and `Not applicable` (English non-hazardous
  products) to `VALID_SIGNAL_WORDS`. These were incorrectly triggering `S2-INVALID-SIGNAL-WORD`
  (5 false positives in the 30-file test → 0 after fix).

- **QC r27: S14-DG-NO-UN false positives for zh-tw/zh-cn** (`tools/quality_check.py`):
  Extended the UN number detection regex to match Traditional Chinese format
  `聯合國編號(UN No.)：1990` and Simplified Chinese `联合国编号：1990`. Reduced
  `S14-DG-NO-UN` false positives by 3 in the 30-file test.

- **QC r27: S14-NO-PACKING-GROUP false positives for zh-tw** (`tools/quality_check.py`):
  Added `包裝類別` and `包裝等級` (two Traditional Chinese variants for "packing group") and
  Unicode Roman numerals `[ⅠⅡⅢⅣ]` to the packing group detection regex. No cascade false
  positives were introduced.

- **QC r27: S14-NO-SHIPPING-NAME false positives for zh-tw/zh-cn** (`tools/quality_check.py`):
  Added `聯合國運輸名稱`, `运输名称`, and `運輸名稱` to the proper shipping name regex.

### Changed

- **LLM prompt: Section 1 Use fallback** (`llm.rs`): When Section 1.2 exists in the source but no specific use is listed, the source phrase (e.g. `'無相関詳細情報'`, `'无相关详细资料'`, `'no specific use listed'`) is now captured as one entry in the Use array — the Use key is never omitted when Section 1.2 is present in the source.

- **LLM prompt: Section 8 OEL "not required" detection** (`llm.rs`): Added explicit recognition of Chinese and Japanese phrases that indicate no occupational exposure limits are established (`不要求`, `无需监控`, `不适用`, `无职業接触限值`, `no limits established`, `not required`, `no monitoring required`). When any such phrase is present, one entry with `AdditionalInfo.FullText` quoting the source phrase is included instead of omitting the field.

- **LLM prompt: Section 9 Densities and VapourPressure** (`llm.rs`): Added explicit instruction to always extract density, relative density, or specific gravity into the `Densities` array (numeric values via `NumericRangeWithUnitAndQualifier`, text-only values via `AdditionalInfo.FullText`). Also added instruction to extract `VapourPressure` for any flammable or volatile product bearing H224, H225, H226, H330, H331, or H332.

- **LLM prompt: Section 12 PersistenceDegradability** (`llm.rs`): When the source includes a persistence/degradability subsection, `PersistenceDegradability.BiologicalDegradability` is now always populated — using the source text if available, or `'該当データなし'` / `'无相关数据'` if the subsection exists but contains no data. Similarly, bioaccumulation subsections are captured in `AdditionalEcotoxInformation`.

## [0.2.4] / [0.3.4] — 2026-05-26

### Fixed

- **Blank lines lost in extracted text** (`extractor.rs`): Pass 2 deduplication in
  `clean_extracted_text` was counting empty strings (`""`) in the frequency map. When a document
  had 4+ sections (i.e. 4+ blank-line separators), all blank lines after the first were silently
  dropped. Fixed by excluding empty lines from both the frequency count and the deduplication
  check. Added regression test `blank_lines_not_removed_by_dedup`.

- **`FireFightingMeasures` section skipped** (`schema/generated.rs`, `schema/mod.rs`): The LLM
  sometimes returns `FullText` (typed `Option<String>`) as a JSON array (e.g.
  `["有害燃焼副産物：ケイ素酸化物", "炭素酸化物"]`). Serde rejected this with
  "invalid type: sequence, expected a string", causing the entire `FireFightingMeasures` section
  to be skipped. Added `flex_string_opt` custom deserializer (accepts string *or* array-of-strings,
  joining with `\n`) and applied it to **all** `FullText: Option<String>` fields in the schema.
  Added regression test `fire_fighting_full_text_accepts_array`.

- **`StabilityReactivity` section skipped** (`schema/generated.rs`): Same array-vs-string
  mismatch for `HazardousDecompositionProducts.Substance` and `.Condition`. Fixed by applying
  `flex_string_opt` to all `Substance` and `Condition: Option<String>` fields. Added regression
  test `stability_reactivity_substance_accepts_array`.

- **Language misdetection for CID/Shift-JIS PDFs** (`language.rs`): `pdftotext` applied to
  CID/Shift-JIS font PDFs can produce garbled ASCII output (punctuation, special chars, no
  real letters). The previous `detect_language` logic classified this as English because it
  found no CJK or kana characters. Fixed: when no CJK is present, the function now requires
  ≥ 30 % of meaningful chars to be ASCII alphanumeric (a–z, A–Z, 0–9) before returning
  `Language::English`; otherwise it falls back to `Language::Japanese`. Added regression test
  `garbled_cid_font_output_defaults_to_japanese`.

- **pdf-extract panics on unsupported encodings** (`extractor.rs`): `pdf_extract::extract_text`
  panics on PDFs using `90ms-RKSJ-H` or other unsupported font encodings. Wrapped the
  `spawn_blocking` call in `std::panic::catch_unwind(AssertUnwindSafe(...))` so panics are
  caught gracefully, a `tracing::debug!` message is emitted, and extraction continues with the
  `pdftotext`/OCR fallback instead of propagating as a task failure.

- **`TransportInformation` and `Condition` fields lacked flex deserializers** (`schema/generated.rs`):
  `TransportInformationSpecialPrecautionUser.full_text` and
  `TransportInformationSpecialProvisions.full_text` (`Option<Vec<String>>`) were missing the
  `flex_vec_string_opt` deserializer. All `Condition: Option<String>` fields across the schema
  were also missing `flex_string_opt`. Fixed; `tools/generate_schema.py` updated to generate
  these annotations automatically.

### Changed

- **LLM prompt: Section 11 toxicology guidance** (`llm.rs`): Added explicit instructions to
  always extract LD50/LC50/other toxicity values into `ToxicologicalInformation.AcuteToxicity`
  and to never emit empty `Result` arrays (`[{}]`).

- **LLM prompt: Section 12 ecology guidance** (`llm.rs`): Added explicit instructions to
  always extract EC50/LC50/NOEC values into `EcologicalInformation.AquaticAcuteToxicity` /
  `AquaticChronicToxicity` and to never emit empty `Result` arrays.

## [0.2.3] / [0.3.3] — 2026-05-25

### Fixed

- **`CASno.FullText` flex deserialization** (`schema/generated.rs`): `SubstanceIdentifiersSubstanceIdentityCASno.full_text` was typed as `Option<Vec<String>>` but lacked the `flex_vec_string_opt` deserializer. When an LLM returned the CAS number as a bare string (e.g. `"1317-61-9"`) instead of an array, serde rejected it with "expected a sequence", causing the entire `Composition` section to be skipped. Now accepts both bare strings and arrays.

- **`Colour`/`Odour` object→string coercion** (`llm.rs`): For `BasePhysicalChemicalProperties`, the LLM sometimes wraps `Colour` and `Odour` (typed `Option<String>`) in an `{"AdditionalInfo":{"FullText":["..."]}}` object. Added `coerce_obj_to_string` normalization in `normalize_string_fields` (targeting `Colour`, `Odour`, `PhysicalState`) so these are flattened to a plain string before serde deserialization. Fixes `invalid type: map, expected a string` mismatch that caused `PhysicalChemicalProperties` to be skipped.

- **`pdftotext -utf8` flag removed** (`extractor.rs`): poppler ≥ 24 no longer recognises the `-utf8` option and exits with code 99, silently bypassing the pdftotext fallback for CID/Shift-JIS font PDFs. Modern `pdftotext` outputs UTF-8 by default; the flag was redundant. Removing it restores Shift-JIS font PDF support on systems where `poppler-utils` is installed.

### Changed

- **GUI minimum window size** (`app.rs`): Added `with_min_inner_size([640.0, 480.0])` to `ViewportBuilder` so the window cannot be resized below a usable minimum (H1).
- **Busy-poll repaint interval reduced** (`app.rs`): Background task polling interval reduced from 100 ms to 250 ms, cutting CPU usage by ~60% during long conversions while keeping the UI responsive (M8).
- **Log panel height uncapped** (`app.rs`): Removed `.max_height(160.0)` from the log `ScrollArea`; the panel now fills the available vertical space freely (M2).
- **TextEdit fields are now responsive** (`app.rs`): All nine input fields replaced `add_sized([W, 20.0], ...)` with `desired_width(avail_width - offset).max(150.0)`, making them resize with the window (M10).
- **Frame::NONE** (`app.rs`): Replaced deprecated `egui::Frame::none()` with `egui::Frame::NONE` (M1).

### Added

- **Keyboard shortcuts** (`app.rs`): Ctrl+Q quits the application, F1 opens the manual window, Ctrl+O opens the file picker for the current tab (H3).
- **API key show/hide toggle** (`app.rs`): A small button next to the API key field toggles password masking; icon label switches between the locale's "Show" and "Hide" strings (L1).
- **Error modal and About dialog use `egui::Modal`** (`app.rs`): Both dialogs are now true backdrop modals handled by egui — Escape and backdrop click close them automatically via `ModalResponse::should_close()` (H4, M7).
- **Settings saved message auto-clears** (`app.rs`): After clicking Save, `settings_saved_at` records the instant and `ctx.request_repaint_after(3 s + 50 ms)` is called to guarantee a repaint fires even when the user is idle. The `logic()` method clears the message once 3 seconds have elapsed (M9).
- **Drag-and-drop file type validation** (`app.rs`): Dropped files are partitioned into accepted/rejected by extension using `Vec::partition()`. Accepted extensions are tab-aware (e.g. JSON-only on Generate/Validate tabs). Rejected files trigger a localised log warning (L2, L3).
- **Hint text on all input fields** (`app.rs`): Every `TextEdit::singleline` now carries a `.hint_text(...)` placeholder describing the expected input (M4).
- **i18n for file-dialog filter labels** (`app.rs`, `Strings`): All `add_filter("SDS", ...)` / `add_filter("JSON", ...)` etc. now use locale-specific `lbl_filter_*` strings from the `Strings` struct (M5).
- **i18n for OK/Skip/Show/Hide buttons** (`app.rs`, `Strings`): Welcome screen Skip button and modal OK buttons use `s.btn_skip` / `s.btn_ok`; API key toggle uses `s.btn_show_key` / `s.btn_hide_key` (M6, L4).
- **Strings struct extended** (`app.rs`): Added 10 new fields — `btn_ok`, `btn_skip`, `btn_show_key`, `btn_hide_key`, `msg_drop_rejected`, `lbl_filter_sds`, `lbl_filter_json`, `lbl_filter_doc`, `lbl_filter_word`, `lbl_filter_txt`; dead `btn_clear_files` field removed. All three locales (en/ja/zh-cn) updated.

### Fixed

- **Empty-input/output validation now uses error modal** (`app.rs`): `start_convert`, `start_generate`, and `start_validate` previously wrote "no input/output path" as a plain log message. They now set `self.error_modal` so the error surfaces as a blocking dialog (H2).
- **Log clone eliminated** (`app.rs`): Log rendering previously cloned the entire `Vec<String>` before releasing the mutex. The `ScrollArea` now renders directly while holding the `MutexGuard`, saving an allocation on every frame (M3).
- **Redundant "clear files" buttons removed** (`app.rs`): Small "clear files" buttons in the Convert and Validate batch-mode panels were redundant — switching to single mode already clears the list. Both buttons and the dead `btn_clear_files` Strings field have been removed (L7).

### Added (continued from prior unreleased)

- **Automated Windows & macOS release builds** (`.github/workflows/release.yml`): pushing a `v*.*.*` tag now automatically builds `sdsconv-windows-portable.zip` (Windows x86_64) and `sdsconv-macos.zip` (macOS Universal — Apple Silicon + Intel) and attaches them to the GitHub Release. Homebrew Cask auto-update is skipped gracefully when `HOMEBREW_TAP_TOKEN` is not configured.

### Security

- **Timing-safe Bearer token comparison** (`sdsconv_server`): replaced `t == token.as_str()` with `constant_time_eq(t.as_bytes(), token.as_bytes())` to eliminate timing side-channel attacks on the authentication check. New dependency: `constant_time_eq = "0.3"`.
- **HTTP redirect disabled on URL fetch client** (`extractor.rs`): `shared_http_client` now sets `.redirect(Policy::none())`. Previously, an attacker could bypass the SSRF `is_private_host` guard by redirecting through a public URL to a private address.
- **IPv6 SSRF guard extended** (`extractor.rs`): `is_private_host` now blocks `fc00::/7` (ULA unique-local), `fe80::/10` (link-local), and `::ffff:` IPv4-mapped addresses whose embedded IPv4 is loopback/private/link-local. Previously only `::1` and `::` were blocked.
- **Upload size limit reduced to 50 MB** (`sdsconv_server`): `DefaultBodyLimit` lowered from 512 MB to 50 MB — sufficient for any real SDS document. The previous limit allowed DoS via memory exhaustion on large uploads.
- **REST API server now requires authentication** (`sdsconv_server`): Bearer token auth via `SDS_SERVER_TOKEN` env var (auto-generates and prints a random token if not set). Default bind address changed from `0.0.0.0` to `127.0.0.1` (`SDS_SERVER_BIND` override)
- **CORS restricted to localhost** (`sdsconv_server`): replaced `CorsLayer::permissive()` with an allowlist of `http://localhost` and `http://127.0.0.1` only
- **Concurrency cap on REST server** (`sdsconv_server`): `ConcurrencyLimitLayer(10)` prevents resource exhaustion from concurrent requests
- **SSRF protection** (`extractor.rs`): URL fetches now reject private/loopback/link-local/metadata IP addresses and hostnames (`localhost`, `169.254.x.x`, RFC-1918 ranges, `::1`) before issuing the HTTP request
- **Prompt injection mitigation strengthened** (`llm.rs`): `</document>` occurrences in document text are escaped to `</_document>` before insertion into LLM user messages
- **LLM error body no longer forwarded to API clients** (`sdsconv_server`): full provider error responses are logged server-side only; clients receive a sanitized `{"error": "LLM API request failed", "status": N}` response

### Added

- **MHLW §3.3 compliance — empty field pruning** (`tasks.rs`): `prune_empty_strings` post-processes LLM output to remove `""`, `[]`, and `{}` values before writing the JSON file
- **Recommended filename output** (`tasks.rs`, `config.rs`, `app.rs`, `main.rs`): `--suggested-name` CLI flag and GUI Settings checkbox output the file as `SDS_<IssueDate>_<ProductCode>.json` per the MHLW §2.1.2 naming convention. Filename conflicts are resolved atomically
- **`SdsError::display_safe()`** (`error.rs`): new method that returns a sanitized error message safe for external/client display (strips LLM provider error bodies)
- **`ProductNoUser` extraction** (`llm.rs`): added `ProductNoUser` array to the MHLW schema example hint so LLMs extract the field
- **Scanned PDF OCR fallback** (`extractor.rs`): When `pdf_extract` yields fewer than 200 characters
  (image-only / scanned PDF), the extractor automatically shells out to `pdftoppm` (poppler) +
  `tesseract` CLI. Pages are rasterised at 300 dpi, OCR'd with `jpn+eng` (falls back to `eng` if the
  Japanese pack is absent), and the combined text is fed into the normal LLM pipeline. If neither
  tool is installed, a clear install hint is returned as the error message. No new crate dependencies.

- **GUI application (eframe/egui)**: Hybrid launcher — runs as a GUI window when invoked with no
  arguments; falls back to CLI mode when arguments are present. Cross-platform (macOS, Windows,
  Linux). Five tabs: SDS→JSON Convert, Document Generate, Validate, Extract Text, Settings.

- **Extract Text tab** (`app.rs`, `tasks.rs`): New fifth tab for raw text extraction without
  LLM. Accepts local files (PDF, DOCX, XLSX, TXT, HTML) and URLs (`http://`/`https://`). Result
  is shown inline (capped at 50,000 characters) with an option to save to a file.
  `run_extract_text` function added to `tasks.rs`.

- **Model name and base URL fields** (`app.rs`, `config.rs`): Settings tab now exposes model name
  and base URL text fields. `config.base_url: String` added to `AppConfig`; both values are
  forwarded to `ToJsonParams` at conversion time.

- **DOCX template picker** (`app.rs`): Generate tab shows a template file picker when the DOCX
  output format is selected. The chosen path maps to the existing `ToDocxParams.template` field.

- **Drag & drop input** (`app.rs`): Files dropped anywhere on the window are routed to the
  appropriate tab's input field. A semi-transparent overlay reading "Drop files here" appears on
  hover.

- **Settings persistence** (`config.rs`): App configuration is written to and read from
  `~/.config/sdsconv/config.toml` (created with Unix 0o600 permissions). Includes API keys,
  provider, model name, base URL, output directory, language, and quality preset.

- **BusyGuard RAII, error modals, and log panel** (`app.rs`): `BusyGuard` ensures the busy flag
  is always cleared on drop. Recoverable errors surface as modal dialogs. A collapsible log panel
  retains the last 500 lines.

- **Batch mode** (`app.rs`): Convert and Validate tabs support multi-file batch processing.

- **Multi-language UI** (`app.rs`): Interface strings are available in Japanese, English, and
  Simplified Chinese; selected via the Settings tab.

- **Provider API key links and onboarding banner** (`app.rs`): Settings tab displays clickable
  links to the API key page for each provider. First-run onboarding banner guides new users.

### Fixed

- **`repair_json` corrupted string values containing `,}` patterns** (`llm.rs`): the blind `str::replace(",}", "}")` loop rewrote JSON string values such as `"note": "ends here,}"`, producing invalid JSON. Replaced with a byte-level state machine (`remove_trailing_commas`) that tracks `in_string` state (including `\"` escape handling), wrapped in a fixpoint loop. Existing tests still pass; 3 new regression tests added.
- **Silent discard of retry-parse errors** (`llm.rs`): both the text-extraction retry (`llm.rs:660`) and the vision-path retry (`llm.rs:926`) used `if let Ok(...)` on the result of `lenient_deserialize`, silently swallowing parse errors. Replaced with `match` + `Err(e) => tracing::warn!(...)` so failures are always visible in logs.
- **False-positive chemical name matching** (`enrichment.rs`): `names_similar` used substring containment (`a.contains(&b) || b.contains(&a)`), causing short generic words (e.g. `"acid"`) to match unrelated names (e.g. `"hydrochloric acid"`). Replaced with Jaccard word-overlap (intersection/union ≥ 0.5). 5 new unit tests added.
- **`section!` macro schema-mismatch warning lacks context** (`llm.rs`): the `WARN` log only reported the serde error message. Now also logs the first 200 characters of the failing JSON value, making it much easier to diagnose LLM output schema drift.
- **`/api/health` blocked by auth middleware** (`sdsconv_server`): The `require_auth` middleware was applied via `.layer()` to the entire router, causing `GET /api/health` to return 401 for unauthenticated callers (e.g. AWS LWA / load-balancer health checks). Fixed by splitting into a protected router (`.route_layer(require_auth)`) merged with a public router containing only the health route.
- **Japanese CID font PDF panic** (`extractor.rs`): `pdf-extract` panics with `FromUtf8Error` when processing PDFs that use CID fonts (e.g. Shift-JIS encoded Japanese text). The panic was caught by `spawn_blocking` and silently converted to an empty string, causing unnecessary OCR fallback. Added `pdftotext -utf8` (poppler) as a middle tier between `pdf-extract` and OCR: full 3-tier fallback chain is now `pdf-extract` -> `pdftotext` -> tesseract/Vision. `pdftotext` is silently skipped if poppler is not installed.
- URL response body now capped at 50 MB (Content-Length pre-check + streaming byte cap) to prevent OOM on large responses
- CJK text truncation: `out.len()` (byte count) was compared against `max_chars` (character count), causing Japanese text to be cut at 1/3 the intended length. Fixed to use `chars().count()`
- Blocking `std::fs::read` replaced with `tokio::task::spawn_blocking` inside `convert_pdf_to_json_vision` to avoid stalling the Tokio executor during image-only PDF processing
- Log panel now enforces the documented 500-line maximum (was unbounded despite the "max 500" label)
- `start_generate` now validates the output path before spawning the async task, matching the guard in `start_convert`
- Validation result "No issues" message now uses the i18n `Strings` struct instead of a hardcoded Japanese literal
- TOCTOU race in `resolve_unique_suggested_path`: replaced `exists()` check with atomic `OpenOptions::create_new(true)` to prevent concurrent batch runs from overwriting each other's output
- PubChem enrichment no longer silently drops results on HTTP 429: adds 250 ms inter-request delay and retries once with 1 s backoff

## [0.2.0] - 2026-05-23

### Fixed

- **CJK character corruption in template filling** (`template.rs`): `normalize_split_runs` was
  casting individual UTF-8 bytes to `char`, corrupting multi-byte characters such as Japanese,
  Chinese, and Korean text. Rewrote the function to use string-slice–based output, preserving
  full Unicode text outside placeholders.

- **EcologicalInformation data silently dropped** (`llm.rs`): The LLM schema hint used
  `"AquaticToxicity"` (non-existent key) instead of `"AquaticAcuteToxicity"` /
  `"AquaticChronicToxicity"`. serde silently discarded the data on deserialization. Fixed hint to
  match the actual `generated.rs` struct. Similarly corrected `PersistenceDegradability` field
  names (`BiologicalDegradability`, `AbioticDegradation`, `RapidDegradability`).

- **`--quality high` fails with HTTP 400** (`llm.rs`): Anthropic claude-sonnet-4-x and newer
  models reject requests that end with an assistant turn (prefill). Removed the assistant prefill
  message from `AnthropicBackend::complete`; `strip_json_fences` already handles any extra
  wrapping the model may add.

- **Sync I/O blocking the tokio thread pool** (`extractor.rs`): DOCX, TXT, and XLSX extraction
  were calling blocking `std::fs` APIs directly inside `async` functions. Wrapped each format in
  `tokio::task::spawn_blocking`, matching the existing PDF path.

- **Batch error counter not incremented on JSON serialize / write failure** (`main.rs`): Errors
  during `serde_json::to_string_pretty` and `std::fs::write` inside `batch_to_json` were silently
  ignored. Both paths now increment `failed` and emit an `[ERROR]` message.

- **`flatten_sds` silently returns empty map on serialization failure** (`template.rs`): Changed
  return type from `HashMap<String, String>` to `Result<HashMap<String, String>, SdsError>` and
  propagated the error with `?`.

- **Separator filter counts bytes instead of characters** (`extractor.rs`): `trimmed.len() >= 3`
  was a byte-length check; replaced with `trimmed.chars().count() >= 3` for correctness with
  multi-byte CJK separator characters.

### Added

- **GHS Rev.10 H/P code validation** (`ghs_codes.rs`, `validator.rs`): New `ghs_codes` module
  with static lookup tables for all valid H-codes (H200–H420) and P-codes (P101–P503).
  `validate` now checks every `HazardStatementCode` and `PrecautionaryStatementCode` against
  the database and emits a warning for any unrecognised code. Compound P-codes like
  `P301+P330+P331` are split on `+` before validation.

- **CAS number format + check-digit validation** (`validator.rs`): `validate` now checks every
  CAS number in `CompositionAndConcentration` for correct `\d{2-7}-\d{2}-\d` format and a
  valid check digit (weighted sum mod 10), emitting a warning for each malformed entry.

- **PubChem CAS enrichment** (`enrichment.rs`): New `enrichment` module with `lookup_cas` /
  `enrich_composition` API. The CLI `to-json` command now accepts an `--enrich` flag that
  looks up each CAS number in PubChem and reports mismatches between the PubChem IUPAC name
  and the SDS substance name.

- **HTML/URL input support** (`extractor.rs`): `to-json` and `extract-text` now accept a URL
  (`http://` / `https://`) as `--input` in addition to local files. HTML files (`.html`/`.htm`)
  are also supported. Text is extracted with `scraper` (table cells tab-separated, noise elements
  skipped). New dependency: `scraper = "0.21"`.

- **`to-html` subcommand** (`html.rs`, `main.rs`): Generates a self-contained UTF-8 HTML5
  document from MHLW standard JSON with inline CSS (including `@media print`). Object fields
  are rendered as key-value tables; `CompositionAndConcentration` and other object arrays are
  rendered as column tables. Batch mode (`--input-dir`) supported.

- **`to-pdf` subcommand** (`main.rs`): Converts MHLW standard JSON to PDF via
  `soffice --headless --convert-to pdf` (LibreOffice). Requires `soffice` in PATH; fails with
  a clear error message if not found. Batch mode (`--input-dir`) supported.

- **Input file size limits**: Input files are now rejected before reading if they exceed limits —
  500 MB for PDF / DOCX / XLSX, 100 MB for TXT / JSON — preventing OOM on oversized uploads.

- **ZIP bomb protection in template filling** (`template.rs`): Template files are checked against
  a 50 MB limit before opening; individual ZIP entries are read through `Read::take(100 MB)`.

- **HTTP timeouts on LLM backends** (`llm.rs`): Both `AnthropicBackend` and
  `OpenAiCompatBackend` now set a 120 s total timeout and a 10 s connect timeout via
  `reqwest::Client::builder`.

- **Prompt injection mitigation** (`llm.rs`): Document text is wrapped in
  `<document>…</document>` tags and the system prompt instructs the model to treat that content
  as data only.

- **Explicit schema mismatch warnings** (`llm.rs`): When a section's JSON value cannot be
  deserialized into its target struct, a `WARN` log is emitted rather than silently substituting
  an empty default.

- **Retry failure logging** (`llm.rs`): If the retry LLM call also fails, the error is now
  logged at `WARN` level instead of being silently ignored.

- **API key exposure warning** (`main.rs`): Passing `--api-key` on the command line now prints a
  warning recommending the use of environment variables instead.

- **`bounds check` in DOCX generator** (`generator.rs`): An `assert_eq!` at the top of
  `generate_docx` verifies that `SECTION_NAMES` and `SECTION_KEYS` have equal length, catching
  accidental mismatches at startup.

- **`check_json_file_size` helper** (`main.rs`): Validates JSON input size before reading in
  `validate`, `to-docx` single-file, and `batch_to_docx` modes.

### Changed

- **O(N²) → O(N) placeholder substitution** (`template.rs`): `apply_substitutions` previously
  called `String::replace` once per key (hundreds of iterations × document size). Replaced with a
  single forward scan using `str::find("{{")`.

- **`repair_json` skipped for already-valid JSON** (`llm.rs`): `lenient_deserialize` now tries
  `serde_json::from_str` first and only falls back to `repair_json` when the initial parse fails,
  avoiding an unnecessary allocation on every successful response.

- **Eliminated `clone()` in section deserialization** (`llm.rs`): `lenient_deserialize` now uses
  `map.remove(key)` to take ownership of each section value, removing the `v.clone()` call before
  `serde_json::from_value`.

- **`tracing::debug!` → `tracing::trace!` for LLM output** (`llm.rs`): The full JSON response
  body is now logged only at `TRACE` level, keeping document content out of normal `DEBUG` logs.

- **Shared `send_with_retry` helper** (`llm.rs`): Extracted duplicate retry-and-backoff logic
  from `AnthropicBackend` and `OpenAiCompatBackend` into a single `send_with_retry` function.

- **`collect_files` helper** (`main.rs`): Extracted duplicate directory-traversal logic from
  `batch_to_json` and `batch_to_docx` into a shared helper.

- **I/O errors annotated with file paths** (`main.rs`): `std::fs::read_to_string` and
  `std::fs::write` calls in single-file modes now attach the file path via `anyhow::Context`.

- **`escape_xml` handles quotes** (`template.rs`): Added `"` → `&quot;` and `'` → `&apos;`
  escaping, which was previously missing.

- **`is_content_xml` simplified** (`template.rs`): Removed redundant explicit matches for
  `header1/2/3.xml` and `footer1/2/3.xml`; the `starts_with` predicates already cover them.

- **`unwrap()` on URL construction removed** (`llm.rs`): `OpenAiCompatBackend::openai()` and
  `::gemini()` now embed URL literals directly instead of calling `.unwrap()` on
  `openai_compat_url()`.

- **Empty file-stem guard in batch modes** (`main.rs`): Files with an empty or missing stem are
  now skipped with a `[SKIP]` message and counted as failures instead of silently producing
  dot-files.

## [0.1.2] - 2025-05-21

### Added

- Multi-language README files (ja, en, zh-CN, zh-TW) with MHLW reference links
- Cargo.toml SEO metadata improvements (keywords, categories, documentation links)

## [0.1.1] - 2025-05-15

### Added

- Word template filling (`to-docx --template`): replaces `{{FieldName}}` placeholders in `.docx`
  templates with values extracted from MHLW standard JSON
- Full-path placeholder support (`{{Section.SubSection.Field}}`)
- Run-split normalization: handles Word's tendency to split typed words across `<w:r>` runs
- XML-safe value escaping (`&`, `<`, `>`)

## [0.1.0] - 2025-05-01

### Added

- Initial release: bidirectional conversion between SDS documents and MHLW standard JSON
- LLM-based extraction from PDF, DOCX, TXT, and XLSX inputs
- DOCX generation from JSON (JIS Z 7253 16-section format, 4 languages)
- Parallel batch conversion with configurable concurrency
- Multi-provider LLM support: Anthropic, OpenAI, Gemini, Mistral, Groq, Cohere, Ollama
- Quality presets (low / medium / high) with per-preset model and token-limit defaults
- `validate` subcommand for structural JSON checking
- `extract-text` subcommand for raw text extraction without LLM
- Exponential backoff retry on HTTP 429 / 529 responses
