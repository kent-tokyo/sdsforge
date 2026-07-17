//! Deprecated redirect binary. The `sdsconv` package has been renamed to `sdsforge`;
//! this crate keeps `cargo install sdsconv` from silently installing something stale
//! or failing with a confusing "no such package" error during the migration window.

fn main() {
    eprintln!("sdsconv has been renamed to sdsforge.");
    eprintln!();
    eprintln!("  cargo install sdsforge");
    eprintln!();
    eprintln!(
        "See https://github.com/kent-tokyo/sdsconv/blob/main/docs/migration-from-sdsconv.md \
         for the full migration guide."
    );
    std::process::exit(1);
}
