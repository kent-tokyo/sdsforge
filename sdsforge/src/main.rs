fn main() -> anyhow::Result<()> {
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
