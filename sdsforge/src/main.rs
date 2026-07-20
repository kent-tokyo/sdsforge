fn main() -> anyhow::Result<()> {
    // Best-effort: load ANTHROPIC_API_KEY/etc. from a .env file in the current or
    // an ancestor directory into the process environment, if one exists. Silently
    // does nothing when absent -- this is local-dev convenience, not a requirement.
    dotenvy::dotenv().ok();

    sdsforge::init_process();

    if std::env::args().len() > 1 {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(sdsforge::run_cli_from(std::env::args_os()))
    } else {
        sdsforge::run_gui()
    }
}
