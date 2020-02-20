#[cfg(not(target_os = "linux"))]
compile_error!(
    "kleinhirn needs the prctl/PR_SET_CHILD_SUBREAPER syscall, which is only available on linux."
);

use anyhow::{Context, Result};
use kleinhirn_supervisor::*;
use prctl;
use slog::{o, Drain, Logger};
use slog_scope::info;
use std::{env::current_dir, path::PathBuf};
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

    let config_file = opt.config_file.canonicalize()?;
    let mut settings = config::Config::default();
    settings.merge(config::File::from(config_file.as_path()))?;
    let mut settings = settings
        .try_into::<configuration::Config>()
        .context(format!(
            "Could not parse configuration file {:?}",
            &config_file
        ))?;
    let cwd = current_dir()?;
    settings.base_dir = config_file.parent().map(|p| p.to_owned()).unwrap_or(cwd);
    info!("startup");
    prctl::set_child_subreaper(true)
        .map_err(|code| anyhow::anyhow!("Unable to set subreaper status. Status {:?}", code))?;
    rt.block_on(async {
        kleinhirn_supervisor::run(settings).await?;

        Ok(())
    })
}
