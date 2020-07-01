//! The process supervision part of kleinhirn.
//!
//! This is the "root" process of the kleinhirn process hierarchy. It listens for external
//! commands, spawns the configured number of workers and supervises them.

#![recursion_limit = "2048"] // select! needs a higher recursion limit /:

use anyhow::{anyhow, Context, Result};
use fork_exec::ForkExec;
use futures::select;
use futures::{future::FutureExt, Stream, StreamExt};
use health::{HealthIndicator, State};
use nix::unistd::Pid;
use parking_lot::Mutex;
#[cfg(target_os = "linux")]
use preloader::Preloader;
use preloader::PreloaderDied;
use process_control::{Message, ProcessControl};
use reaper::Zombies;
use slog::o;
use slog_scope::{crit, debug, info, warn};
use std::{convert::Infallible, sync::Arc, time::Instant};
use worker_set::{
    MiserableCondition, Tick, Todo, WorkerAcked, WorkerDeath, WorkerLaunchFailure, WorkerLaunched,
    WorkerRequested, WorkerSet,
};

mod fork_exec;
mod health;
mod preloader;
mod process_control;

pub mod configuration;
pub mod reaper;
pub mod worker_ack;
pub mod worker_set;

#[derive(Clone)]
struct Machine(Arc<Mutex<Option<WorkerSet>>>);

impl Machine {
    fn new(set: WorkerSet) -> Self {
        Machine(Arc::new(Mutex::new(Some(set))))
    }

    fn interrogate<T>(&self, with: fn(&WorkerSet) -> T) -> T {
        self.0.lock().as_ref().map(with).unwrap()
    }

    fn update(&self, with: impl Fn(WorkerSet) -> WorkerSet) {
        let mut guard = self.0.lock();
        let new_machine = guard.take().map(with);
        *guard = new_machine;
    }
}

impl HealthIndicator for Machine {
    fn health_check(&self) -> health::State {
        self.interrogate(|machine| match machine {
            WorkerSet::Running(_) => State::Healthy,
            WorkerSet::Startup(_) => State::Unhealthy(anyhow!("still starting up").into()),
            state => State::Unhealthy(anyhow!("Machine in unhealthy state: {:?}", state).into()),
        })
    }
}

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
    machine: Machine,
    mut zombies: Zombies,
    mut proc: Box<dyn ProcessControl>,
    ticker: Box<dyn Stream<Item = Instant> + std::marker::Unpin>,
) -> Infallible {
    let mut known_broken = false;
    let mut ticker = ticker.fuse();

    loop {
        if machine.interrogate(|m| m.working()).is_none() {
            // We're broken. Just reap children & wait quietly for the
            // sweet release of death.
            if !known_broken {
                warn!("The workers are in a faulty state! Marking self as unhealthy & reaping any workers that exit.");
                known_broken = true;
            }
            match zombies.reap().await {
                Ok(pid) => info!("reaped child"; "pid" => ?pid),
                Err(e) => info!("failed to reap"; "error" => ?e),
            }
            continue;
        }

        // Process things we need to do now:
        match machine
            .interrogate(|m| m.required_action())
            .and_then(|todo| todo)
        {
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
                        machine.update(move |m| {
                            m.on_worker_requested(WorkerRequested::new(id.clone()))
                        });
                    }
                    Err(e) => {
                        machine.update(move |m| {
                            m.on_worker_launch_failure(WorkerLaunchFailure::new(None))
                        });
                        warn!("failed to launch"; "error" => ?e);
                    }
                }
            }
        }

        // Read events off the environment:
        select! {
            tick = ticker.next() => {
                if let Some(tick) = tick {
                    machine.update(|m| m.on_tick(Tick::new(tick)));
                }
            }
            // TODO: check the preloader PID also - could be that the
            // control pipe is held open by a broken child.
            res = zombies.reap().fuse() => {
                match res {
                    Ok(pid) => {
                        info!("reaped child"; "pid" => pid.as_raw());
                        machine.update(|m| m.on_worker_death(WorkerDeath::new(pid)));
                    }
                    Err(e) => info!("failed to reap"; "error" => ?e)
                }
            }
            msg = proc.next_message().fuse() => {
                debug!("received message"; "msg" => ?msg);
                use Message::*;
                match msg {
                    Err(e) if e.is::<PreloaderDied>() => {
                        info!("preloader process is dead");
                        machine.update(|m| m.on_miserable_condition(MiserableCondition::PreloaderDied));
                    }
                    Err(e) => info!("could not read preloader message"; "error" => ?e),
                    Ok(Launched{id, pid}) => {
                        machine.update(move |m| m.on_worker_launched(WorkerLaunched::new(id.clone(), Pid::from_raw(pid as i32))))
                    }
                    Ok(Ack{id}) => {
                        machine.update(move |m| m.on_worker_acked(WorkerAcked::new(id.clone())))
                    }
                    Ok(LaunchError{id, pid, error}) => {
                        warn!("error launching worker";
                              "worker_id" => ?id,
                              "pid" => ?pid,
                              "error" => ?error,
                        );
                        machine.update(move |m| {
                            m.on_worker_launch_failure(WorkerLaunchFailure::new(Some(id.clone())))
                        });
                    }
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

    let terminations =
        reaper::setup_child_exit_handler().context("Could not set up child exit handler")?;

    let ticker = settings.worker.ack_ticker();
    let mut proc: Box<dyn ProcessControl> = match &settings.worker.kind {
        #[cfg(target_os = "linux")]
        configuration::WorkerKind::Ruby(rb) => {
            let gemfile = settings.canonical_path(&rb.gemfile);
            let load = settings.canonical_path(&rb.load);
            info!("loading ruby";
                  "gemfile" => ?gemfile,
                  "load" => ?load,
                  "start_expression" => &rb.start_expression,
            );
            Box::new(
                Preloader::for_ruby(&gemfile, &load, &rb.start_expression)
                    .context("Failed to spawn the preloader")?,
            )
        }
        configuration::WorkerKind::Program(p) => {
            info!("starting fork/exec program";
                  "cwd" => ?p.cwd,
                  "cmdline" => ?p.cmdline,
            );
            Box::new(ForkExec::for_program(p).context("Failed to spawn the program")?)
        }
    };

    let machine = Machine::new(WorkerSet::new(settings.worker));

    proc.as_mut().initialize().await?;
    let health_server = health::healthcheck_server(settings.health_check, machine.clone());
    select! {
        _ = supervise(machine, terminations, proc, ticker).fuse() => {
            unreachable!("supervise never quits.");
        }
        res = health_server.fuse() => {
            crit!("healthcheck server terminated"; "result" => ?res);
            unreachable!("the server should never terminate");
        }
    }
}
