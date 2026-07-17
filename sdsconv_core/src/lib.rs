//! **Deprecated**: this crate has been renamed to [`sdsforge-core`](https://docs.rs/sdsforge-core).
//!
//! `sdsconv-core` re-exports `sdsforge-core` unchanged so existing dependents keep compiling
//! during the migration window. Update your `Cargo.toml` to depend on `sdsforge-core` directly;
//! see `docs/migration-from-sdsconv.md` in the repository for the full migration guide.

pub use sdsforge_core::*;
