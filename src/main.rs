use anyhow::{Context, Result};
use kleinhirn::*;
use slog::{o, Drain, Logger};
use slog_json::Json;
use slog_logfmt::Logfmt;
use slog_scope::info;
use std::io;
use std::{env::current_dir, path::PathBuf};
use structopt::StructOpt;
use tokio::runtime::Runtime;

fn setup_logger(config: &configuration::Config) -> Logger {
    use configuration::LogFormat;
    use configuration::LogOutput::*;

    let output: Box<dyn io::Write + Send + Sync> = match config.log.output {
        Stderr => Box::new(io::stderr()),
        Stdout => Box::new(io::stdout()),
    };
    match config.log.format {
        LogFormat::Json => {
            let drain = Json::new(output)
                .add_default_keys()
                .build()
                .filter_level(config.log.level.into())
                .fuse();
            let drain = slog_async::Async::new(drain).build().fuse();
            Logger::root(drain, o!("logger" => "kleinhirn"))
        }
        LogFormat::Logfmt { print_prefix } => {
            let mut builder = Logfmt::new(output);
            if !print_prefix {
                builder = builder.no_prefix().print_msg(true);
            }
            let drain = builder.build().filter_level(config.log.level.into()).fuse();
            let drain = slog_async::Async::new(drain).build().fuse();
            Logger::root(drain, o!("logger" => "kleinhirn"))
        }
    }
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

    let config_file = opt.config_file.canonicalize()?;
    let mut settings = config::Config::default();
    settings.merge(config::File::from(config_file.as_path()))?;
    let mut settings = settings
        .try_into::<configuration::Config>()
        .context(format!(
            "Could not parse configuration file {:?}",
            &config_file
        ))?;
    let log = setup_logger(&settings);
    let _guard = slog_scope::set_global_logger(log);

    let cwd = current_dir()?;
    settings.base_dir = config_file.parent().map(|p| p.to_owned()).unwrap_or(cwd);
    info!("startup");
    rt.block_on(async {
        kleinhirn::run(settings).await?;

        Ok(())
    })
}
