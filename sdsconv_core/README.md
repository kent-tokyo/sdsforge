# sdsconv-core (deprecated)

This crate has been renamed to [`sdsforge-core`](https://crates.io/crates/sdsforge-core).

`sdsconv-core` now only re-exports `sdsforge-core` so existing dependents keep
compiling. Update your `Cargo.toml`:

```diff
-sdsconv-core = "0.3"
+sdsforge-core = "0.4"
```

See [`docs/migration-from-sdsconv.md`](https://github.com/kent-tokyo/sdsforge/blob/main/docs/migration-from-sdsconv.md)
for the full migration guide.
