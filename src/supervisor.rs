//! The process supervision part of kleinhirn.
//!
//! This is the "root" process of the kleinhirn process hierarchy. It listens for external
//! commands, spawns the configured number of workers and supervises them.

use anyhow::Result;

use crate::configuration;
use reaper::Zombies;
use slog::o;
use slog_scope::info;
use std::convert::Infallible;

mod control;
mod worker;

pub mod reaper;

/// Supervises child processes and their children.
///
/// This function never exits in the "normal" case.
pub async fn supervise(mut zombies: Zombies) -> Result<Infallible> {
    loop {
        let pid = zombies.reap().await?;
        info!("reaped child"; "pid" => pid.as_raw());
    }
}

pub async fn run(settings: configuration::Config) -> Result<Infallible> {
    let _g = slog_scope::set_global_logger(
        slog_scope::logger().new(o!("service" => settings.supervisor.name.to_string())),
    );

    let terminations = reaper::setup_child_exit_handler()?;
    supervise(terminations).await
}
