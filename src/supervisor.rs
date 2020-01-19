//! The process supervision part of kleinhirn.
//!
//! This is the "root" process of the kleinhirn process hierarchy. It listens for external
//! commands, spawns the configured number of workers and supervises them.

use anyhow::Result;

use futures::StreamExt;
use nix::unistd::Pid;
use slog_scope::info;

use std::time::Duration;

mod control;
mod worker;

pub mod reaper;

enum Event {
    /// A potential worker process (or one of its orphaned children) has exited.
    ChildExited(nix::unistd::Pid),

    /// An out-of-band management command was issued.
    ExternalCommand(control::Command),

    /// A worker has sent a command down our communication channel.
    WorkerAction(worker::ID, worker::Event),
}

/// Supervises child processes and their children.
///
/// This function never exits in the "normal" case.
pub async fn supervise(
    mut zombies: impl futures::Stream<Item = Result<Pid>> + std::marker::Unpin,
) -> Result<()> {
    while let Some(Ok(pid)) = zombies.next().await {
        info!("reaped child"; "pid" => pid.as_raw());
    }
    Ok(())
}

pub async fn run() -> Result<()> {
    let terminations = reaper::setup_child_exit_handler()?;

    info!("done waiting, let's reap");
    supervise(terminations).await
}
