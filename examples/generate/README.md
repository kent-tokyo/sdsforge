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

`ProductInput` and its nested types have no `#[serde(default)]`, so every
field must be present in the YAML/JSON — even empty ones (`other_names: []`,
`boiling_point: []`, `lower: null`, ...). A missing key is a parse error,
not an implicit default. This is deliberate: the input must state
explicitly what was and wasn't provided.
