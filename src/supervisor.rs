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
use worker_set::{Todo, WorkerSet, WorkerStarted};

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
async fn supervise(
    config: configuration::WorkerConfig,
    mut zombies: Zombies,
    mut proc: Box<dyn ProcessControl>,
) -> Infallible {
    let mut machine = WorkerSet::new(config);
    loop {
        // Process things we need to do now:
        match machine.state().and_then(|s| s.next_todo()) {
            None => {}
            Some(Todo::KillProcess(pid)) => {
                // TODO
                info!("Should kill"; "pid" => pid.as_raw());
            }
            Some(Todo::LaunchProcess) => {
                info!("Need to launch a process");
                match proc.spawn_process("foo").await {
                    Ok(pid) => {
                        info!("ack from process"; "pid" => pid.as_raw());
                        machine = machine.on_worker_started(WorkerStarted(pid));
                    }
                    Err(e) => info!("failed to launch"; "error" => format!("{:?}", e)),
                }
            }
        }
        // Read events off the environment
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
    Ok(supervise(settings.worker, terminations, proc).await)
}
