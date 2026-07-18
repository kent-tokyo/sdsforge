# sdsconv (deprecated)

This package has been renamed to [`sdsforge`](https://crates.io/crates/sdsforge).

```bash
cargo install sdsforge
```

The `sdsconv` binary still works during the migration window: it prints a
deprecation warning to stderr, then forwards its arguments to the same CLI
implementation as `sdsforge` (or launches the same GUI if run with no
arguments). Behavior, exit codes, and stdout are otherwise identical to
running `sdsforge` directly. See
[`docs/migration-from-sdsconv.md`](https://github.com/kent-tokyo/sdsconv/blob/main/docs/migration-from-sdsconv.md)
for the full migration guide (CLI, Rust, Python, and REST API changes).
