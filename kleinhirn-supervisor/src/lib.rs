//! The process supervision part of kleinhirn.
//!
//! This is the "root" process of the kleinhirn process hierarchy. It listens for external
//! commands, spawns the configured number of workers and supervises them.

use anyhow::Result;

use fork_exec::ForkExec;
use futures::future::FutureExt;
use nix::unistd::Pid;
use preloader::Preloader;
use process_control::{Message, ProcessControl};
use reaper::Zombies;
use slog::o;
use slog_scope::{debug, info};
use std::convert::Infallible;
use tokio::select;
use worker_set::{Todo, WorkerAcked, WorkerDeath, WorkerLaunched, WorkerRequested, WorkerSet};

mod fork_exec;
mod preloader;
mod process_control;

pub mod configuration;
pub mod reaper;
pub mod worker_set;

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
    // TODO: uh, I need to make this generic enough for the fork/exec method.
    mut proc: Box<dyn ProcessControl>,
) -> Infallible {
    let mut machine = WorkerSet::new(config);
    loop {
        // Process things we need to do now:
        match machine.required_action().and_then(|todo| todo) {
            None => {}
            Some(Todo::KillProcess(pid)) => {
                // TODO
                info!("Should kill"; "pid" => pid.as_raw());
            }
            Some(Todo::LaunchProcess) => {
                info!("Need to launch a process");
                match proc.spawn_process().await {
                    Ok(id) => {
                        info!("requested launch"; "id" => &id);
                        machine = machine.on_worker_requested(WorkerRequested::new(id));
                    }
                    Err(e) => info!("failed to launch"; "error" => ?e),
                }
            }
        }

        // Read events off the environment:
        select! {
            res = zombies.reap().fuse() => {
                match res {
                    Ok(pid) => {
                        // TODO: check that the dead PID belongs to a worker
                        // TODO2: figure out what to do about preloader death.
                        info!("reaped child"; "pid" => pid.as_raw());
                        machine = machine.on_worker_death(WorkerDeath::new(pid))
                    }
                    Err(e) => info!("failed to reap"; "error" => ?e)
                }
            }
            msg = proc.next_message().fuse() => {
                debug!("received message"; "msg" => ?msg);
                use Message::*;
                match msg {
                    Err(e) => info!("could not read preloader message"; "error" => ?e),
                    Ok(Launched{id, pid}) => {
                        machine = machine.on_worker_launched(WorkerLaunched::new(id, Pid::from_raw(pid as i32)));
                    }
                    Ok(Ack{id}) => {
                        machine = machine.on_worker_acked(WorkerAcked::new(id));
                    }
                }
            }
        };
        debug!("machine is now"; "machine" => ?machine);
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
                  "gemfile" => ?gemfile,
                  "load" => ?load,
                  "start_expression" => &rb.start_expression,
            );
            Box::new(Preloader::for_ruby(&gemfile, &load, &rb.start_expression)?)
        }
        configuration::WorkerKind::Program(p) => {
            info!("starting fork/exec program";
                  "cmdline" => ?p.cmdline);
            Box::new(ForkExec::for_program(p)?)
        }
    };
    let terminations = reaper::setup_child_exit_handler()?;

    proc.as_mut().initialize().await?;
    Ok(supervise(settings.worker, terminations, proc).await)
}
