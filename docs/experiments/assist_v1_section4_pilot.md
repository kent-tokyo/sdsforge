# Section 4 assist v1 — real-data pilot report

Real run of `sdsforge assist` (branch `feat/assist-v1-section4`, base commit
`068ef15` plus two fixes made during this pilot — see "Fixes made mid-pilot"
below) against the 3 documents and expected values recorded in
[`section4_pilot_expected_values.md`](section4_pilot_expected_values.md).
Read that file's ground-truth-independence caveat before trusting these
numbers — it applies here too.

- **Model / provider:** `claude-sonnet-4-6` via `--provider anthropic`
- **Prompt version:** `section4-v1`
- **Documents:** doc-a (Japanese, FUJIFILM Wako ethanol), doc-b (English,
  Sigma-Aldrich/Fluka acetone), doc-c (English, xylene, GHS-GB layout) —
  full descriptions/URLs/hashes in the expected-values doc. PDFs not
  committed.
- **Latency / cost:** not captured this run — the CLI doesn't print
  per-call timing or token usage by default (`tracing::debug!` logs it,
  but only at `debug` level). Noted as a real gap, not backfilled with a
  second paid run just to fill in this table.

## Fixes made mid-pilot

Two bugs blocked the pilot from producing any output at all against a
real Anthropic response, fixed before continuing (both are plain
correctness fixes, not new features):

1. **Markdown code fences broke every response.** `AnthropicBackend::complete`
   (unlike `OpenAiCompatBackend`) never applied the crate's own
   `strip_code_fences` helper. The real model wrapped its JSON array in a
   ```` ```json ... ``` ```` fence despite the prompt explicitly saying not
   to, and `parse_candidates_json` correctly treated that as malformed
   top-level JSON and refused to write any output — fail-closed working
   exactly as designed, but blocking 100% of runs. Fixed by applying the
   same `strip_code_fences` assist should have been using all along.
2. **Rejection warnings didn't say what was rejected.** `"source_excerpt
   not found in extracted source text"` gave no way to tell *why* without
   re-instrumenting the binary. Added a truncated excerpt to the warning
   message. Purely diagnostic — required to interpret this pilot's own
   results, not scope creep.

## Aggregate results

| document | raw candidates | retained | unsupported-path rejections | excerpt-verification rejections | correct proposals | false positives | missed expected fields |
|---|---|---|---|---|---|---|---|
| doc-a | 5 | 0 | 0 | 5 | 0 | 0 | 4 |
| doc-b | 6 | 6 | 0 | 0 | 5 | 1 | 0 |
| doc-c | 6 | 6 | 0 | 0 | 5 | 1 | 0 |
| **total** | **17** | **12** | **0** | **5** | **10** | **2** | **4** |

- **precision** = 10 correct / 12 emitted = **83%**
- **recall** = 10 correct / 14 expected = **71%**
- **verification rejection rate** = 5 / 17 raw candidates ≈ **29%** — entirely
  concentrated in doc-a; 0% for doc-b/doc-c
- No candidate ever targeted a path outside the Section 4 allowlist.
- No proposal ever carried a non-null `source_page`, a `High` confidence,
  or a `model_estimate` evidence level — all three v1 invariants held for
  every one of the 17 raw candidates across all 3 documents.
- No `official_sds.json`, generation artifact, or `ProductInput` file was
  touched — `assist` wrote only the 3 requested output files.

## Successful proposals (examples)

doc-b, `FirstAidMeasures.ExposureRoute.FirstAidSkin.FullText`:
```
proposed_value: "Wash off with soap and plenty of water. Consult a physician."
source_excerpt: "In case of skin contact\nWash off with soap and plenty of water. Consult a physician."
confidence: medium
```
Exact match to the expected-values table; verbatim quote including the
subsection subheading, verified against the real extracted text.

doc-c, `FirstAidMeasures.DescriptionOfFirstAidMeasures.FullText`:
```
proposed_value: "Do not leave affected person unattended. Remove victim out of the danger area. ..."
```
Full multi-line "General notes" paragraph (three line-wraps in the raw
extracted text), quoted and verified correctly — confirms
whitespace-normalized excerpt verification handles ordinary line-wrap
fine; the doc-a failures below are a different kind of extraction noise.

## Rejected candidates (why they failed) — the doc-a case

All 5 doc-a candidates were rejected, and all 5 for the identical reason.
Example (`FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText`):

```
model's source_excerpt: "吸入した場合\n新鮮な空気のある場所に移すこと。症状が続く場合には、医師に連絡すること。"
our extracted source:   "吸入し た場合\n新鮮な空気のある 場所に移すこ と 。 症状が続く 場合には、 医師に連絡する こ と 。"
```

`sdsforge extract-text`'s output for this PDF has extra spaces inserted
*inside* words (a known artifact of this PDF's CID-keyed Japanese font —
confirmed with both `pdftotext` and the project's own `pdf-extract`-based
extractor; not specific to one tool). The model reads that text, correctly
understands the content, and — very reasonably — quotes it back in
*natural, unspaced* Japanese. `excerpt_verifies`'s whitespace
normalization collapses *runs* of whitespace into one space; it cannot
remove a space that exists mid-word in the source but not in the
excerpt, or vice versa. Every one of doc-a's 5 candidates failed this
exact way — this is not 5 independent failures, it's one systematic
mismatch class.

This is the predicted "conservative false negative" the code's own doc
comment already named (punctuation/width/OCR variants), just realized for
real by a genuine PDF rather than a synthetic test case. The 4 real
first-aid instructions in doc-a were **correct in content** — the model
understood the document fine — but 0 of them survived verification.

## False positives

Both occurrences are the same pattern, one per English document, and
both are explicit boilerplate placeholders rather than hallucinated
content:

- doc-b, `MedicalAttentionAndSpecialTreatmentNeeded.FullText`: proposed
  `"No data available"` — a real, verified quote, but it asserts nothing
  about first aid. Marked `should_propose: no` in the expected table.
- doc-c, same path: proposed `"none"` — same pattern.

Notably, in both documents the model *correctly declined* to propose the
neighboring `InformationToHealthProfessionals` field, whose source text is
similarly low-value (a cross-reference in doc-b, "not known to date" in
doc-c). So the model isn't simply proposing everything under every
heading — it's applying some judgment, just inconsistently between these
two specific boilerplate patterns ("no data available" / "none" → treated
as proposable; "not known to date" / a cross-reference → treated as not
proposable). Neither false positive is a safety risk by itself (nothing
false or invented is asserted), but both are noise a reviewer has to
manually discard.

## False negatives

All 4 in doc-a (see above) — 0 in doc-b or doc-c.

## A structural issue distinct from precision/recall: content duplication

doc-b's `DescriptionOfFirstAidMeasures` proposal is not just the "General
advice" preamble (what the expected-values table anticipated) — it's the
**entire 4.1 subsection verbatim**, including the four route-specific
paragraphs that are *also* separately proposed under their own
`ExposureRoute.*` paths. Both the "description" proposal and the four
"route" proposals are individually correct and verified, but accepting
all five into an authoring input would write the same first-aid
instructions into the SDS twice. doc-c did not have this problem — its
`DescriptionOfFirstAidMeasures` proposal was exactly the general-notes
paragraph, cleanly separated from the four routes, matching the expected
value exactly. The difference tracks the two documents' actual heading
structure (doc-b's flatter "General advice / If inhaled / In case of..."
vs. doc-c's more clearly nested "General notes" / "Following inhalation"
sub-subheadings) rather than being random.

## Limitations discovered

1. **`excerpt_verifies`'s whitespace-run normalization cannot handle
   mid-word extraction noise** — a systematic, document-specific failure
   mode (confirmed on one Japanese PDF with CID-keyed font spacing
   artifacts), not a per-candidate fluke. 100% of that document's content
   was lost to it.
2. **The model doesn't consistently distinguish "no real content"
   boilerplate from real content** — proposes some placeholder phrases
   ("No data available", "none"), correctly skips others (cross-references,
   "not known to date"). Prompt doesn't currently give explicit guidance
   either way.
3. **The model doesn't consistently scope `DescriptionOfFirstAidMeasures`
   the same way across documents** — sometimes the general-notes-only
   content (correct, matches intent), sometimes the entire subsection
   including per-route text that's also proposed separately (duplicative).
4. Latency/cost weren't captured this run (tooling gap, not a pilot
   finding about assist's behavior).

## Recommendation for the next change

Per the pilot's own decision rule, pick exactly one. The doc-a failure is
the largest single effect observed: it didn't degrade one document's
results, it zeroed out an entire document (0/4 expected fields, 5/5
candidates rejected) despite the model reading the source correctly. The
other two documents already work well (precision 83%, recall 100% each,
no allowlist/page/confidence-level violations). **Recommend: improve
excerpt normalization** — specifically, add a fallback comparison that
strips *all* whitespace (not just collapses runs) on both sides when the
current whitespace-normalized check fails, so a source with
extraction-inserted mid-word spaces can still verify a naturally-spaced
quote. This is a narrower, targeted fix for the exact failure mode
observed, not general fuzzy matching (still no punctuation/OCR/width
normalization) — that stays deferred until it's actually needed.

Not recommended yet, despite being real findings: the boilerplate/dedup
issues are real but cost only 1 point of precision per document and don't
zero anything out — worth a future prompt refinement, not urgent enough
to be the one change this round.

## Follow-up: CJK inter-character spacing fix, implemented and rerun

Implemented a narrower version of the recommendation above: rather than
stripping *all* whitespace, `excerpt_verifies` now also removes a
whitespace character when both of its immediate neighbors are Hiragana,
Katakana, Han/Kanji, or the Japanese `。`/`、` marks (see
`remove_cjk_intercharacter_whitespace` in `sdsforge_core/src/assist.rs`).
Ordinary word/number/CAS-style spacing (`"15 minutes"`, `"fresh air"`,
`"CAS 64-17-5"`) is untouched.

**The `。`/`、` marks were not in the original plan** — an offline replay
of the initial Han/Kanji/Hiragana/Katakana-only version against the real
cached doc-a text (before spending more API budget on a live rerun, per
plan) showed it *still* rejected both candidates checked. The real
extraction artifact inserts a space on both sides of these two
punctuation marks too, not just between ideographs. Widened the fix to
include them once the offline replay made that concrete, then re-verified
offline again before doing the live rerun. Worth flagging: the original
scope (three named scripts only) would not have fixed the actual pilot
failure it was written for — treat "supports at least X, Y, Z" as a floor
to empirically verify against real data, not a checklist to stop at.

### Before / after

| document | raw | retained before | retained after | correct | false positives | misses | excerpt-verification rejections |
|---|---|---|---|---|---|---|---|
| doc-a | 5 | 0 | 5 | 4 | 1 | 0 | 0 |
| doc-b | 6 | 6 | 6 | 5 | 1 | 0 | 0 |
| doc-c | 6 | 6 | 6 | 5 | 1 | 0 | 0 |
| **total** | **17** | **12** | **17** | **14** | **3** | **0** | **0** |

- **precision** = 14 / 17 ≈ **82%** (was 83%)
- **recall** = 14 / 14 = **100%** (was 71%)

Exactly the predicted shape: recall jumped (doc-a's 4 real citations are
no longer lost to an extraction artifact), precision stayed essentially
flat. doc-a's 5th candidate (`InformationToHealthProfessionals` ←
"個人用保護具を着用すること。", the first-aiders'-own-protective-equipment
note) is a new false positive by count, but it isn't a new *kind* of
problem — before this fix it happened to be rejected for the same
excerpt-mismatch reason as the four real citations, not because anything
caught its semantic mismatch. It's the same "boilerplate/misfiled content
proposed as if valid" category already named above, not evidence this fix
did anything wrong. It should be picked up by that already-identified,
not-yet-implemented follow-up (deterministic placeholder/semantic
filtering), per the acceptance criteria for this round explicitly
allowing it to remain for now.

Confirmed for this round: all four expected doc-a fields retained (doc-a
is no longer a zero-output document); no candidate targeted a path
outside the Section 4 allowlist; every proposal is still exactly
`confidence: medium`, `source_evidence_level: supplier_sds`,
`source_page: null`; doc-b/doc-c's proposals are byte-for-byte the same
shape as before (same 6 paths, same content) -- the fix has zero effect
on non-CJK documents, confirmed, not just assumed.

### Next recommended change (unchanged from before, now with a live green light)

The false positives left are concentrated in exactly the pattern already
named: boilerplate placeholders (`"No data available"`, `"none"`) and one
misfiled note (doc-a's protective-equipment line) being proposed as if
they were real first-aid content. Recommend a deterministic
semantic-filtering pass for these specific placeholder patterns next, in
its own single commit, per the established one-change-at-a-time rule —
not fuzzy matching, not a prompt change, not Section 5.
