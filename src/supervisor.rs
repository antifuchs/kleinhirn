//! The process supervision part of kleinhirn.
//!
//! This is the "root" process of the kleinhirn process hierarchy. It listens for external
//! commands, spawns the configured number of workers and supervises them.

use anyhow::{Context, Result};
use futures::executor::block_on;
use futures::StreamExt;
use nix::unistd::Pid;
use slog_scope::info;
use std::convert::Infallible;
use std::time::Duration;
use tokio::time::delay_for;

mod control;
mod worker;

// #[cfg(target_os = "linux")]
mod child_processes_linux;

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

fn fork_child() -> Result<()> {
    use nix::unistd::{fork, ForkResult};
    match fork() {
        Ok(ForkResult::Parent { child, .. }) => info!("I'm the parent and that's ok"; "pid" => 
        child.as_raw()),
        Ok(ForkResult::Child) => {
            std::thread::sleep(Duration::from_millis(400));
            info!("I'm a new child process, exiting now!");
            std::process::exit(0);
        }
        Err(e) => {
            return Err(e.into());
        }
    };
    Ok(())
}

pub async fn run() -> Result<()> {
    let terminations = child_processes_linux::setup_child_exit_handler()?;

    // fork a few, so we have something to reap:
    fork_child()?;
    fork_child()?;
    fork_child()?;
    delay_for(Duration::from_secs(1)).await;

    info!("done waiting, let's reap");
    supervise(terminations).await
}
