//! Deprecated compat binary. The `sdsconv` package has been renamed to `sdsforge`;
//! this binary forwards its argv into [`sdsforge::run_cli_from`] (or launches the
//! same GUI on no args) so existing scripts, shortcuts, and automation keep working
//! during the migration window instead of hitting a dead end.

fn main() -> anyhow::Result<()> {
    sdsforge::init_process();
    eprintln!("warning: the `sdsconv` command has been renamed to `sdsforge`");

    if std::env::args().len() > 1 {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(sdsforge::run_cli_from(std::env::args_os()))
    } else {
        sdsforge::run_gui()
    }
}
