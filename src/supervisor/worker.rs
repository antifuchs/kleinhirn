use crate::configuration::WorkerConfig;
use machine::*;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::{fmt::Debug, sync::Arc};

#[cfg_attr(test, faux::create(self_type = "Arc"))]
pub struct ProcessManager {}

impl ProcessManager {
    pub fn new() -> Arc<Self> {
        Arc::new(ProcessManager {})
    }
}

impl PartialEq for ProcessManager {
    fn eq(&self, _other: &ProcessManager) -> bool {
        true
    }
}

impl Eq for ProcessManager {}

impl Debug for ProcessManager {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

#[cfg_attr(test, faux::methods(self_type = "Arc"))]
impl ProcessManager {
    pub fn kill_process(&self, pid: Pid, signal: Signal) -> Result<(), nix::Error> {
        kill(pid, signal)
    }

    pub fn launch_new(&self, n: usize) { // TODO: probably needs error handling

        // also TODO: talk to preloader
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct State {
    pids: Vec<Pid>,
    config: WorkerConfig,
    process_manager: Arc<ProcessManager>,
}

impl State {
    fn new_worker(&mut self, pid: Pid) {
        self.pids.push(pid);
    }

    fn worker_died(&mut self, dead: Pid) {
        self.pids.retain(|pid| *pid == dead);
    }

    fn kill_all_workers(&self) -> Result<(), nix::Error> {
        for pid in self.pids.iter() {
            self.process_manager.kill_process(*pid, Signal::SIGTERM)?;
        }
        Ok(())
    }
}

machine! {
    #[derive(Clone, Debug, PartialEq)]
    pub enum WorkerSet {
        Startup { state: State },
        Running { state: State },
        Underprovisioned { state: State },
        Overprovisioned { state: State },
        Faulted { state: State },
        Terminating { state: State },
        Terminated {},
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkerDeath(Pid);

#[derive(Clone, Debug, PartialEq)]
pub struct WorkerStarted(Pid);

#[derive(Clone, Debug, PartialEq)]
pub struct Terminate();

transitions!(WorkerSet,
  [
    (Startup, WorkerStarted) => [Running, Startup],
    (Startup, WorkerDeath) => Faulted,
    (Startup, Terminate) => Terminating,

    (Running, WorkerDeath) => Underprovisioned,
    (Running, WorkerStarted) => Running,
    (Running, Terminate) => Terminating,

    (Underprovisioned, WorkerStarted) => [Running, Underprovisioned],
    (Underprovisioned, WorkerDeath) => [Underprovisioned, Faulted],
    (Underprovisioned, Terminate) => Terminating,

    (Terminating, WorkerDeath) => [Terminating, Terminated]
  ]
);

impl Running {
    fn on_worker_death(self, d: WorkerDeath) -> Underprovisioned {
        let mut state = self.state;
        state.worker_died(d.0);

        // TODO: start a new worker somehow
        Underprovisioned { state }
    }

    fn on_worker_started(self, s: WorkerStarted) -> Running {
        let mut state = self.state;
        state.new_worker(s.0);
        Running { state }
    }

    fn on_terminate(self, _: Terminate) -> Terminating {
        let state = self.state;
        state.kill_all_workers().expect("TODO: handle error");
        Terminating { state }
    }
}

impl Startup {
    fn on_worker_started(self, s: WorkerStarted) -> WorkerSet {
        let mut state = self.state;
        state.new_worker(s.0);

        if state.pids.len() >= state.config.count {
            WorkerSet::running(state)
        } else {
            state.process_manager.launch_new(1); // TODO: allow configuring
            WorkerSet::startup(state)
        }
    }

    fn on_worker_death(self, d: WorkerDeath) -> Faulted {
        let mut state = self.state;
        state.worker_died(d.0);
        Faulted { state }
    }

    fn on_terminate(self, _: Terminate) -> Terminating {
        let state = self.state;
        state.kill_all_workers().expect("TODO: handle kill error");
        Terminating { state }
    }
}

impl Underprovisioned {
    fn on_worker_started(self, s: WorkerStarted) -> WorkerSet {
        let mut state = self.state;
        state.new_worker(s.0);

        if state.pids.len() >= state.config.count {
            state.process_manager.launch_new(1); // TODO: allow configuring
            WorkerSet::running(state)
        } else {
            WorkerSet::underprovisioned(state)
        }
    }

    fn on_worker_death(self, d: WorkerDeath) -> WorkerSet {
        let mut state = self.state;
        state.worker_died(d.0);
        state.process_manager.launch_new(1);

        // TODO: allow configuring how many simultaneous deaths make a faulted service.
        WorkerSet::faulted(state)
    }

    fn on_terminate(self, _: Terminate) -> Terminating {
        let state = self.state;
        state.kill_all_workers().expect("TODO: handle kill error");
        Terminating { state }
    }
}

impl Terminating {
    fn on_worker_death(self, d: WorkerDeath) -> WorkerSet {
        let mut state = self.state;
        state.worker_died(d.0);

        if state.pids.len() > 0 {
            WorkerSet::terminating(state)
        } else {
            WorkerSet::terminated()
        }
    }
}

impl WorkerSet {}
