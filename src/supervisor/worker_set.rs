#![allow(dead_code)] // TODO: use this.

use crate::configuration::WorkerConfig;
use machine::*;
use nix::unistd::Pid;
use parking_lot::Mutex;
use std::{collections::VecDeque, fmt::Debug, sync::Arc};

#[derive(Debug, PartialEq, Clone)]
pub enum Todo {
    KillProcess(Pid),
    LaunchProcess,
}

#[derive(Debug, Clone)]
pub struct State {
    pids: Vec<Pid>,
    config: WorkerConfig,
    todo: Arc<Mutex<VecDeque<Todo>>>,
}

impl PartialEq for State {
    fn eq(&self, other: &State) -> bool {
        let todo1 = self.todo.lock();
        let todo2 = other.todo.lock();
        self.pids == other.pids && self.config == other.config && *todo1 == *todo2
    }
}

impl State {
    fn new_worker(&mut self, pid: Pid) {
        self.pids.push(pid);
    }

    fn worker_died(&mut self, dead: Pid) {
        self.pids.retain(|pid| *pid == dead);
    }

    fn kill_all_workers(&mut self) {
        let mut todo = self.todo.lock();

        (*todo).clear();
        for pid in self.pids.iter() {
            (*todo).push_back(Todo::KillProcess(*pid));
        }
    }

    fn start_worker(&mut self) {
        let mut todo = self.todo.lock();
        (*todo).push_back(Todo::LaunchProcess);
    }

    pub fn next_todo(&self) -> Option<Todo> {
        let mut todo = self.todo.lock();
        (*todo).pop_front()
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
        Terminated { state: State },
    }
}

methods!(WorkerSet, [
    Startup, Running, Underprovisioned, Overprovisioned, Faulted, Terminating, Terminated => get state: State
]);

#[derive(Clone, Debug, PartialEq)]
pub struct WorkerDeath(pub Pid);

#[derive(Clone, Debug, PartialEq)]
pub struct WorkerStarted(pub Pid);

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
        state.start_worker();
        Underprovisioned { state }
    }

    fn on_worker_started(self, s: WorkerStarted) -> Running {
        let mut state = self.state;
        state.new_worker(s.0);
        Running { state }
    }

    fn on_terminate(self, _: Terminate) -> Terminating {
        let mut state = self.state;
        state.kill_all_workers();
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
            state.start_worker();
            WorkerSet::startup(state)
        }
    }

    fn on_worker_death(self, d: WorkerDeath) -> Faulted {
        let mut state = self.state;
        state.worker_died(d.0);
        Faulted { state }
    }

    fn on_terminate(self, _: Terminate) -> Terminating {
        let mut state = self.state;
        state.kill_all_workers();
        Terminating { state }
    }
}

impl Underprovisioned {
    fn on_worker_started(self, s: WorkerStarted) -> WorkerSet {
        let mut state = self.state;
        state.new_worker(s.0);

        if state.pids.len() >= state.config.count {
            state.start_worker();
            WorkerSet::running(state)
        } else {
            WorkerSet::underprovisioned(state)
        }
    }

    fn on_worker_death(self, d: WorkerDeath) -> WorkerSet {
        let mut state = self.state;
        state.worker_died(d.0);
        state.start_worker();
        WorkerSet::faulted(state)
    }

    fn on_terminate(self, _: Terminate) -> Terminating {
        let mut state = self.state;
        state.kill_all_workers();
        Terminating { state }
    }
}

impl Terminating {
    fn on_worker_death(self, d: WorkerDeath) -> WorkerSet {
        let mut state = self.state;
        state.worker_died(d.0);

        if !state.pids.is_empty() {
            WorkerSet::terminating(state)
        } else {
            WorkerSet::terminated(state)
        }
    }
}

impl WorkerSet {
    pub fn new(config: WorkerConfig) -> WorkerSet {
        let mut state = State {
            config,
            todo: Arc::new(Mutex::new(Default::default())),
            pids: Default::default(),
        };
        state.start_worker();
        WorkerSet::Startup(Startup { state })
    }
}
