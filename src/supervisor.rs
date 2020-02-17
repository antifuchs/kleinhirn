//! The process supervision part of kleinhirn.
//!
//! This is the "root" process of the kleinhirn process hierarchy. It listens for external
//! commands, spawns the configured number of workers and supervises them.

use anyhow::Result;

use crate::configuration;
use futures::future::FutureExt;
use preloader::{ForkExec, Preloader, ProcessControl};
use reaper::Zombies;
use slog::o;
use slog_scope::info;
use std::convert::Infallible;
use tokio::select;

mod control;
mod preloader;
mod worker;
mod worker_set;

pub mod reaper;

// let's try (at least on this function call level) to ensure all
// problematic conditions are handled in a way that doesn't leave this
// loop:
#[forbid(
    clippy::option_unwrap_used,
    clippy::result_unwrap_used,
    clippy::option_expect_used,
    clippy::result_expect_used
)]
async fn supervise(mut zombies: Zombies, _proc: Box<dyn ProcessControl>) -> Infallible {
    loop {
        select! {
            res = zombies.reap().fuse() =>{
                match res {
                    Ok(pid) => info!("reaped child"; "pid" => pid.as_raw()),
                    Err(e) => info!("failed to reap"; "error" => format!("{:?}", e))
                }
            }
        };
    }
}

/// Starts the process supervisor with the configured worker set.
///
/// This function never exits in the "normal" case.
pub async fn run(settings: configuration::Config) -> Result<Infallible> {
    let _g = slog_scope::set_global_logger(
        slog_scope::logger().new(o!("service" => settings.supervisor.name.to_string())),
    );

    let mut proc: Box<dyn ProcessControl> = match &settings.worker.kind {
        configuration::WorkerKind::Ruby(rb) => {
            let gemfile = settings.canonical_path(&rb.gemfile);
            let load = settings.canonical_path(&rb.load);
            info!("loading ruby";
                  "gemfile" => gemfile.to_str().unwrap_or("unprintable"),
                  "load" => load.to_str().unwrap_or("unprintable"),
                  "start_expression" => &rb.start_expression,
            );
            Box::new(Preloader::for_ruby(&gemfile, &load, &rb.start_expression)?)
        }
        configuration::WorkerKind::Program(p) => Box::new(ForkExec::for_program(p)?),
    };
    let terminations = reaper::setup_child_exit_handler()?;

    proc.as_mut().initialize().await?;
    Ok(supervise(terminations, proc).await)
}
