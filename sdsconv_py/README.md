# sdsconv (deprecated)

This package has been renamed to [`sdsforge`](https://pypi.org/project/sdsforge/).

```bash
pip install sdsforge
```

`import sdsconv` still works during the migration window: it emits a
`DeprecationWarning` and re-exports the exact same API from `sdsforge`
(including the `sdsconv.eval` and `sdsconv.causasv_bridge` submodules), so
existing code and scripts keep running while you switch to `import sdsforge`.

```diff
-import sdsconv
+import sdsforge
```

See [`docs/migration-from-sdsconv.md`](https://github.com/kent-tokyo/sdsforge/blob/main/docs/migration-from-sdsconv.md)
for the full migration guide (CLI, Rust, Python, and REST API changes).
