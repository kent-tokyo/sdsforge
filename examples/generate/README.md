# Example: `sdsforge generate` input

`example_cleaner.yaml` is a fictional all-purpose cleaner formulation, not
real confidential data. It exercises:

- a two-component formulation with both an exact and a ranged concentration
- one evidence-backed measured property (flash point, `product_test_report`)
- several properties left intentionally unresolved (no evidence supplied),
  so the generated draft stays truthfully incomplete rather than guessing

## Run

```bash
sdsforge generate \
  --input examples/generate/example_cleaner.yaml \
  --output-dir generated
```

Writes `generated/official_sds.json`, `generated/generation_report.json`,
and `generated/review_report.md`. See the repo root README's "Generate"
section for what each file contains.

## Note on completeness

Empty optional collections such as `other_names`, `measured_properties`,
and `evidence` (and any individual property inside `measured_properties`,
e.g. `boiling_point`) may be omitted entirely — omitting one means the same
thing as supplying it empty. `Option` fields (`address`, `phone`, `cas_number`,
`lower`/`upper`, ...) may also be omitted; a missing key deserializes to
`None` exactly like an explicit `null`.

Product name, supplier (`company_name` in particular), components,
concentration, units, and referenced evidence metadata (`evidence_id`,
`id`, `reference`, measurement `conditions`, ...) remain required — a
missing required key is still a parse error with the field name in the
message, and an explicitly-empty value (`trade_name: ""`) is never silently
replaced by a default.

Unknown keys are rejected (`deny_unknown_fields`), so a typo such as
`concentation:` is a parse error instead of being silently ignored.
