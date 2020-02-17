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

async fn supervise(mut zombies: Zombies, mut proc: Box<dyn ProcessControl>) -> Result<Infallible> {
    proc.as_mut().initialize().await?;
    loop {
        select! {
            res = zombies.reap().fuse() =>{
                let pid = res?;
                info!("reaped child"; "pid" => pid.as_raw())
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

    let proc: Box<dyn ProcessControl> = match &settings.worker.kind {
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
    supervise(terminations, proc).await
}
