use crate::configuration::WorkerConfig;
use machine::*;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

#[cfg(test)]
use mockall::{predicate::*, *};
use std::marker::PhantomData;

#[cfg_attr(test, automock)]
pub trait ProcessManager {
    fn kill_process(pid: Pid, signal: Signal) -> Result<(), nix::Error> {
        kill(pid, signal)
    }
}

struct UNIXProcessManager();

// TODO: unforch this doesn't work since you can't parameterize the data inside a state enum variant.
impl ProcessManager for UNIXProcessManager {}

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pids: Vec<Pid>,
    config: WorkerConfig,
}

impl State {
    fn new_worker(&mut self, pid: Pid) {
        self.pids.push(pid);
    }

    fn worker_died(&mut self, dead: Pid) {
        self.pids.retain(|pid| *pid == dead);
    }
}

machine! {
    #[derive(Clone, Debug, PartialEq)]
    pub enum WorkerSet<M: ProcessManager = UnixProcessManager> {
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
        Underprovisioned { state }
    }

    fn on_worker_started(self, s: WorkerStarted) -> Running {
        let mut state = self.state;
        state.new_worker(s.0);
        Running { state }
    }

    fn on_terminate(self, _: Terminate) -> Terminating {
        let state = self.state;
        // TODO: kill the PIDs
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
        // TODO: kill the PIDs
        Terminating { state }
    }
}

impl Underprovisioned {
    fn on_worker_started(self, s: WorkerStarted) -> WorkerSet {
        let mut state = self.state;
        state.new_worker(s.0);

        if state.pids.len() >= state.config.count {
            WorkerSet::running(state)
        } else {
            WorkerSet::underprovisioned(state)
        }
    }

    fn on_worker_death(self, d: WorkerDeath) -> WorkerSet {
        let mut state = self.state;
        state.worker_died(d.0);

        // TODO: allow configuring how many simultaneous deaths make a faulted service.
        WorkerSet::faulted(state)
    }

    fn on_terminate(self, _: Terminate) -> Terminating {
        let state = self.state;
        // TODO: kill the PIDs
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
