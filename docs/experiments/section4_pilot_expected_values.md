# Section 4 assist v1 pilot — documents and expected values

Input artifact for the real-data pilot of `sdsforge assist` (branch
`feat/assist-v1-section4`, commit `068ef15`). Not a pilot report — this is
the human-reference table to run assist *against*.

## Caveat: this reference was built by an LLM (me), not an independent human

I (Claude, the coding agent) selected these documents and read Section 4
by hand to fill in the expected values below. `assist` also uses an LLM to
extract candidates. That's a real independence gap: correlated blind
spots between "the model reading the document to build the answer key"
and "the model reading the document to propose candidates" are possible,
especially if both happen to be the same model family. This table is a
reasonable starting point for a first pilot, but if the results look
suspiciously perfect, don't take that as strong evidence — have an actual
person independently check a few rows against the source PDFs before
trusting a high precision/recall number.

## Documents

All three are publicly published supplier SDS documents, used here only
for local testing (not redistributed) — SDS documents exist precisely so
downstream handlers can read and rely on their safety content. PDFs
themselves are **not** committed; only source URL + SHA-256 of the exact
file fetched, so the pilot is reproducible without checking a chemical
document into git.

| id | product | manufacturer | language | pages | why chosen | source URL | SHA-256 |
|---|---|---|---|---|---|---|---|
| doc-a | エタノール(99.5) (Ethanol) | 富士フイルム和光純薬 (FUJIFILM Wako Pure Chemical) | Japanese | 7 | Section 4 clearly laid out under plain headings; extractor output has visible inter-character spacing artifacts inside words (e.g. `こ と` instead of `こと`) — a real stress test of whitespace-normalized excerpt matching, not a synthetic one | https://labchem-wako.fujifilm.com/sds/W01W0105-0045JGHEJP.pdf | `78801367ba9d185cb6a1fb621e3a1c5f9127f57703fee3d3a215d0c1fcc80148` |
| doc-b | Acetone (Fluka brand, product 414689) | Sigma-Aldrich | English | 10 | Clean, well-structured numbered subsections (4.1/4.2/4.3); baseline "easy" case | https://dept.harpercollege.edu/chemistry/sds/Acetone.pdf (institutional mirror of the Sigma-Aldrich SDS) | `b68adfa81c4882b843665eab1ab6427d1360c8f135a961afcc97d304eed69da9` |
| doc-c | Xylene | supplier per "GHS-GB" format document (see PDF Section 1 for full identification) | English | 18 | Different SDS house style (European/GHS-GB numbering, longer multi-line general note before the four exposure routes, several mid-sentence line wraps inside a single excerpt) — the "awkward layout" document | https://www.cellpath.com/pdfs/sds/sdsxylenegb00.088.404.pdf | `6f4aad51b263dc4f77009ddb9dcf8ec19a07f795192fa49a30a6aa376f0daf73` |

All three verified as real, text-extractable (non-scanned) PDFs using
`sdsforge extract-text` itself, not just a generic PDF reader — so the
excerpts below are exactly what `excerpt_verifies` will see during the
pilot, not an approximation.

## Expected values

`should_propose` reflects what a careful human reviewer would want
`assist` to surface — not merely "does this text exist under some path."
Boilerplate placeholders ("no data available", "not known to date") are
marked `no`: technically quotable, but proposing them as if they were real
guidance would be actively misleading in an SDS.

| document | expected_path | expected_excerpt | should_propose | notes |
|---|---|---|---|---|
| doc-a | FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText | 新鮮な空気のある 場所に移すこ と 。 症状が続く 場合には、 医師に連絡する こ と 。 | yes | |
| doc-a | FirstAidMeasures.ExposureRoute.FirstAidSkin.FullText | すぐ に石鹸と 大量の水で洗浄する こ と 。 症状が続く 場合には、 医師に連絡する こ と 。 | yes | |
| doc-a | FirstAidMeasures.ExposureRoute.FirstAidEye.FullText | 眼に入っ た場合、 数分間気を 付けて洗浄する 。 も し コ ン タ ク ト を 装着し ていて、 容易に取り 外せる なら 、 取り 外す。 その 後も 洗浄を 続ける 。 直ち に医師の手当てを 受ける 必要がある 。 | yes | source wraps mid-word across a line break ("その" / "後も", no space in the raw PDF text) — whitespace-normalization turns the line break into a single space, so the *verified* form has a space here that isn't visually obvious in the PDF. Getting this wrong (writing "その後も" with no space) is exactly the kind of transcription slip this note exists to flag — see appendix for the raw multi-line form |
| doc-a | FirstAidMeasures.ExposureRoute.FirstAidIngestion.FullText | 口を すすぐ 。 意識のない人の口には何も 与えないこ と 。 ただち に医師も し く は毒物管理セン タ ーに連絡する こ と 。 医師 の指示がない場合には、 無理に吐かせないこ と 。 | yes | same line-wrap-without-space pattern ("医師" / "の指示が..."); see appendix |
| doc-a | FirstAidMeasures.DescriptionOfFirstAidMeasures.FullText | — | no | doc-a has no general/preamble first-aid text separate from the four route-specific entries |
| doc-a | FirstAidMeasures.InformationToHealthProfessionals.FullText | — | no | no content under this heading in this document |
| doc-a | FirstAidMeasures.MedicalAttentionAndSpecialTreatmentNeeded.FullText | — | no | doc-a has a "protection required for first-aiders" note (個人用保護具を着用すること) instead — it has no matching allowlist path, so the correct behavior is to propose nothing here, not to misfile it under MedicalAttention |
| doc-b | FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText | If breathed in, move person into fresh air. If not breathing, give artificial respiration. Consult a physician. | yes | |
| doc-b | FirstAidMeasures.ExposureRoute.FirstAidSkin.FullText | Wash off with soap and plenty of water. Consult a physician. | yes | |
| doc-b | FirstAidMeasures.ExposureRoute.FirstAidEye.FullText | Rinse thoroughly with plenty of water for at least 15 minutes and consult a physician. | yes | |
| doc-b | FirstAidMeasures.ExposureRoute.FirstAidIngestion.FullText | Do NOT induce vomiting. Never give anything by mouth to an unconscious person. Rinse mouth with water. Consult a physician. | yes | wraps across a line break ("Consult a" / "physician."); see appendix |
| doc-b | FirstAidMeasures.DescriptionOfFirstAidMeasures.FullText | Consult a physician. Show this safety data sheet to the doctor in attendance.Move out of dangerous area. | yes | note: no space between "attendance." and "Move" in the extracted text — a real artifact, not a typo here. A candidate excerpt with a space inserted there will fail exact verification |
| doc-b | FirstAidMeasures.InformationToHealthProfessionals.FullText | The most important known symptoms and effects are described in the labelling (see section 2.2) and/or in section 11 | no | cross-reference only, no substantive content — quotable but not useful; a careful model should skip it. Borderline case, worth a closer look in the actual pilot run |
| doc-b | FirstAidMeasures.MedicalAttentionAndSpecialTreatmentNeeded.FullText | No data available | no | explicit placeholder — must not be proposed as if it were real guidance |
| doc-c | FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText | If breathing is irregular or stopped, immediately seek medical assistance and start first aid actions. In case of respiratory tract irritation, consult a physician. Provide fresh air. | yes | `sdsforge extract-text` reflows this as one line (no hyphenation artifact, unlike some other PDF tools on this same file) |
| doc-c | FirstAidMeasures.ExposureRoute.FirstAidSkin.FullText | Wash with plenty of soap and water. | yes | |
| doc-c | FirstAidMeasures.ExposureRoute.FirstAidEye.FullText | Remove contact lenses, if present and easy to do. Continue rinsing. Irrigate copiously with clean, fresh water for at least 10 minutes, holding the eyelids apart. | yes | wraps across a line break ("at" / "least 10 minutes..."); see appendix |
| doc-c | FirstAidMeasures.ExposureRoute.FirstAidIngestion.FullText | Rinse mouth with water (only if the person is conscious). Do NOT induce vomiting. | yes | |
| doc-c | FirstAidMeasures.DescriptionOfFirstAidMeasures.FullText | Do not leave affected person unattended. Remove victim out of the danger area. Keep affected person warm, still and covered. Take off immediately all contaminated clothing. In all cases of doubt, or when symptoms persist, seek medical advice. In case of unconsciousness place person in the recovery position. Never give anything by mouth. | yes | longest excerpt in the pilot set, wraps across two line breaks; see appendix |
| doc-c | FirstAidMeasures.InformationToHealthProfessionals.FullText | Symptoms and effects are not known to date. | no | same "no real content" pattern as doc-b's 4.2 |
| doc-c | FirstAidMeasures.MedicalAttentionAndSpecialTreatmentNeeded.FullText | none | no | explicit placeholder |

Expected totals: **14 `yes` rows across 3 documents** (4 exposure-route
fields × 3 docs = 12, plus a `DescriptionOfFirstAidMeasures` for doc-b and
doc-c but not doc-a = +2) — this is the recall denominator. Everything marked
`no` exists specifically to check that assist *doesn't* propose it.

## Appendix: raw multi-line source form (exactly as `sdsforge extract-text` produced it)

For the six rows above with a line-wrap note, this is the literal
extracted text (line breaks preserved) that `excerpt_verifies` actually
sees, after whitespace-normalization collapses it to the single-line form
already shown in the table:

```
doc-a eye:
眼に入っ た場合、 数分間気を 付けて洗浄する 。 も し コ ン タ ク ト を 装着し ていて、 容易に取り 外せる なら 、 取り 外す。 その
後も 洗浄を 続ける 。 直ち に医師の手当てを 受ける 必要がある 。

doc-a ingestion:
口を すすぐ 。 意識のない人の口には何も 与えないこ と 。 ただち に医師も し く は毒物管理セン タ ーに連絡する こ と 。 医師
の指示がない場合には、 無理に吐かせないこ と 。

doc-b ingestion:
Do NOT induce vomiting. Never give anything by mouth to an unconscious person. Rinse mouth with water. Consult a
physician.

doc-c eye:
Remove contact lenses, if present and easy to do. Continue rinsing. Irrigate copiously with clean, fresh water for at
least 10 minutes, holding the eyelids apart.

doc-c description:
Do not leave affected person unattended. Remove victim out of the danger area. Keep affected person warm, still
and covered. Take off immediately all contaminated clothing. In all cases of doubt, or when symptoms persist, seek
medical advice. In case of unconsciousness place person in the recovery position. Never give anything by mouth.
```

## How to run the pilot

```bash
sdsforge assist --source <doc-a.pdf> --source-kind supplier-sds --section 4 \
  --output doc-a_assist_proposals.json --provider anthropic --model <model>
# repeat for doc-b, doc-c
```

Then, for each document, diff `proposals[].path` / `.proposed_value` /
`.source_excerpt` against the expected-values table above, and tally:
raw candidates (from `warnings` + `proposals` combined), retained
proposals, unsupported-path rejections, excerpt-verification rejections,
correct proposals, false positives, missed expected fields, latency,
approximate cost. Compute only `precision` and `recall` per the pilot
plan — nothing else yet.
