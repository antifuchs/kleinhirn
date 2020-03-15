//! The process supervision part of kleinhirn.
//!
//! This is the "root" process of the kleinhirn process hierarchy. It listens for external
//! commands, spawns the configured number of workers and supervises them.

use anyhow::{anyhow, Result};
use fork_exec::ForkExec;
use futures::future::FutureExt;
use health::{HealthIndicator, State};
use nix::unistd::Pid;
use parking_lot::Mutex;
#[cfg(target_os = "linux")]
use preloader::Preloader;
use preloader::PreloaderDied;
use process_control::{Message, ProcessControl};
use reaper::Zombies;
use slog::o;
use slog_scope::{crit, debug, info};
use std::{convert::Infallible, sync::Arc};
use tokio::select;
use worker_set::{
    MiserableCondition, Todo, WorkerAcked, WorkerDeath, WorkerLaunched, WorkerRequested, WorkerSet,
};

mod fork_exec;
mod health;
mod preloader;
mod process_control;

pub mod configuration;
pub mod reaper;
pub mod worker_set;

#[derive(Clone)]
struct Machine(Arc<Mutex<Option<WorkerSet>>>);

impl Machine {
    fn new(set: WorkerSet) -> Self {
        Machine(Arc::new(Mutex::new(Some(set))))
    }
}

impl HealthIndicator for Machine {
    fn health_check(&self) -> health::State {
        let machine = self.0.lock();
        match &*machine {
            Some(WorkerSet::Running(_)) => State::Healthy,
            Some(WorkerSet::Startup(_)) => State::Unhealthy(anyhow!("still starting up").into()),
            Some(state) => {
                State::Unhealthy(anyhow!("Machine in unhealthy state: {:?}", state).into())
            }
            None => unreachable!("We should never have no data here"),
        }
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
    // TODO: uh, I need to make this generic enough for the fork/exec method.
    mut proc: Box<dyn ProcessControl>,
) -> Infallible {
    loop {
        let mut m = machine.0.lock();
        if m.as_ref().and_then(|m| m.working()).is_none() {
            // We're broken. Just reap children & wait quietly for the
            // sweet release of death.
            match zombies.reap().await {
                Ok(pid) => info!("reaped child"; "pid" => ?pid),
                Err(e) => info!("failed to reap"; "error" => ?e),
            }
            continue;
        }

        // Process things we need to do now:
        match m
            .as_ref()
            .and_then(|m| m.required_action())
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
                        *m = m
                            .take()
                            .and_then(|m| Some(m.on_worker_requested(WorkerRequested::new(id))));
                    }
                    Err(e) => info!("failed to launch"; "error" => ?e),
                }
            }
        }
        drop(m);

        // Read events off the environment:
        select! {
            // TODO: check the preloader PID also - could be that the
            // control pipe is held open by a broken child.
            res = zombies.reap().fuse() => {
                let mut m = machine.0.lock();
                match res {
                    Ok(pid) => {
                        info!("reaped child"; "pid" => pid.as_raw());
                        *m = m.take().and_then(|m| Some(m.on_worker_death(WorkerDeath::new(pid))))
                    }
                    Err(e) => info!("failed to reap"; "error" => ?e)
                }
            }
            msg = proc.next_message().fuse() => {
                let mut m = machine.0.lock();
                debug!("received message"; "msg" => ?msg);
                use Message::*;
                match msg {
                    Err(e) if e.is::<PreloaderDied>() => {
                        info!("preloader process is dead");
                        *m = m.take().and_then(|m| Some(m.on_miserable_condition(MiserableCondition::PreloaderDied)));
                    }
                    Err(e) => info!("could not read preloader message"; "error" => ?e),
                    Ok(Launched{id, pid}) => {
                        *m = m.take().and_then(|m| (Some(m.on_worker_launched(WorkerLaunched::new(id, Pid::from_raw(pid as i32))))));
                    }
                    Ok(Ack{id}) => {
                        *m = m.take().and_then(|m| (Some(m.on_worker_acked(WorkerAcked::new(id)))));
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
            Box::new(Preloader::for_ruby(&gemfile, &load, &rb.start_expression)?)
        }
        configuration::WorkerKind::Program(p) => {
            info!("starting fork/exec program";
                  "cmdline" => ?p.cmdline);
            Box::new(ForkExec::for_program(p)?)
        }
    };
    let terminations = reaper::setup_child_exit_handler()?;

    let machine = Machine::new(WorkerSet::new(settings.worker));

    proc.as_mut().initialize().await?;
    let health_server = health::healthcheck_server(machine.clone());
    select! {
        _ = supervise(machine, terminations, proc) => {
            unreachable!("supervise never quits.");
        }
        res = health_server => {
            crit!("healthcheck server terminated"; "result" => ?res);
            unreachable!("the server should never terminate");
        }
    }
}
