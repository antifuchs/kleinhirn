use anyhow::{Context, Result};
use kleinhirn::*;
use slog::{o, Drain, Logger};
use slog_scope::info;
use std::path::PathBuf;
use structopt::StructOpt;
use tokio::runtime::Runtime;

fn create_logger() -> Logger {
    // This should do for now, but
    // TODO: Use json logging
    let decorator = slog_term::PlainSyncDecorator::new(std::io::stderr());
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    Logger::root(drain, o!("logger" => "kleinhirn"))
}

/// A prefork process supervisor that keeps worker processes alive, with pre-loading.
#[derive(StructOpt, Debug)]
#[structopt(name = "kleinhirn")]
struct Opt {
    /// Path to the configuration file to use for the service.
    #[structopt(short = "f", long, default_value = "./kleinhirn.toml")]
    config_file: PathBuf,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let mut rt = Runtime::new()?;

    let log = create_logger();
    let _guard = slog_scope::set_global_logger(log);

    let mut settings = config::Config::default();
    settings.merge(config::File::from(opt.config_file.as_path()))?;
    let settings = settings
        .try_into::<configuration::Config>()
        .context(format!(
            "Could not parse configuration file {:?}",
            &opt.config_file
        ))?;
    info!("startup");
    rt.block_on(async {
        supervisor::run(settings).await?;

        Ok(())
    })
}
