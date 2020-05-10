use crate::configuration::WorkerConfig;
use machine::*;
use nix::unistd::Pid;
use std::{collections::HashMap, fmt, time::Instant};

#[derive(Debug, PartialEq, Clone)]
pub enum Todo {
    KillProcess(Pid),
    LaunchProcess,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Worker {
    id: String,
    pid: Option<Pid>,
    requested: Option<Instant>,
    launched: Option<Instant>,
    acked: Option<Instant>,
    killed: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Workers {
    by_pid: HashMap<Pid, String>,
    by_id: HashMap<String, Worker>,
}

impl Workers {
    fn register_worker(&mut self, id: String) {
        let w = Worker {
            id: id.to_string(),
            requested: Some(Instant::now()),
            ..Default::default()
        };
        self.by_id.insert(id, w);
    }

    fn launched(&mut self, id: String, pid: Pid) {
        self.by_id.entry(id.to_string()).and_modify(|w| {
            w.launched = Some(Instant::now());
            w.pid = Some(pid);
        });
        self.by_pid.insert(pid, id);
    }

    fn acked(&mut self, id: String) {
        self.by_id.entry(id).and_modify(|w| {
            w.acked = Some(Instant::now());
        });
    }

    #[must_use = "It's important to check that the thing that got reaped is a worker of ours"]
    fn delete_by_pid(&mut self, pid: Pid) -> Option<Worker> {
        if let Some(id) = self.by_pid.get(&pid) {
            self.by_id.remove(id)
        } else {
            None
        }
    }

    fn all(&self) -> impl Iterator<Item = &Worker> {
        self.by_id.values()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    workers: Workers,
    config: WorkerConfig,
}

impl State {
    fn handle_ack<T>(
        mut self,
        id: String,
        self_state: fn(Self) -> T,
        done_state: fn(Self) -> T,
    ) -> T {
        self.workers.acked(id);

        if self.workers.all().filter(|w| w.acked.is_some()).count() >= self.config.count {
            done_state(self)
        } else {
            self_state(self)
        }
    }
}

machine! {
    #[derive(Clone, PartialEq)]
    // not sure why clippy thinks these are different sizes; they're identical.
    #[allow(clippy::large_enum_variant)]
    pub enum WorkerSet {
        Startup { state: State },
        Running { state: State },
        Underprovisioned { state: State },
        Faulted { state: State },
    }
}

impl fmt::Debug for WorkerSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WorkerSet::")?;
        let state = match self {
            WorkerSet::Startup(Startup { state }) => {
                write!(f, "Startup")?;
                state
            }
            WorkerSet::Running(Running { state }) => {
                write!(f, "Running")?;
                state
            }
            WorkerSet::Underprovisioned(Underprovisioned { state }) => {
                write!(f, "Underprovisioned")?;
                state
            }
            WorkerSet::Faulted(Faulted { state }) => {
                write!(f, "Faulted")?;
                state
            }
            WorkerSet::Error => {
                write!(f, "Error")?;
                return Ok(());
            }
        };
        write!(
            f,
            "(acked:{}, launched:{}, requested:{})/{}",
            state.workers.all().filter(|w| w.acked.is_some()).count(),
            state
                .workers
                .all()
                .filter(|w| w.acked.is_none() && w.launched.is_some())
                .count(),
            state
                .workers
                .all()
                .filter(|w| w.acked.is_none() && w.launched.is_none() && w.requested.is_some())
                .count(),
            state.config.count,
        )?;
        Ok(())
    }
}

methods!(WorkerSet, [
    // TODO: Faulted?
    Startup, Underprovisioned => fn required_action(&self) -> Option<Todo>,
    Startup, Running, Underprovisioned => fn working(&self) -> bool
]);

#[derive(Clone, Debug, PartialEq)]
pub struct WorkerDeath(pub Pid);

impl WorkerDeath {
    pub fn new(pid: Pid) -> Self {
        WorkerDeath(pid)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkerRequested {
    id: String,
}

impl WorkerRequested {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkerLaunched {
    id: String,
    pid: Pid,
}

impl WorkerLaunched {
    pub fn new(id: String, pid: Pid) -> Self {
        Self { id, pid }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkerAcked {
    id: String,
}

impl WorkerAcked {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkerLaunchFailure {
    id: Option<String>,
}

impl WorkerLaunchFailure {
    pub fn new(id: Option<String>) -> Self {
        Self { id }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Terminate();

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum MiserableCondition {
    PreloaderDied,
}

transitions!(WorkerSet, [
    (Startup, WorkerRequested) => Startup,
    (Startup, WorkerLaunched) => Startup,
    (Startup, WorkerAcked) => [Running, Startup],
    (Startup, WorkerLaunchFailure) => Faulted,
    (Startup, WorkerDeath) => [Startup, Faulted],
    (Startup, MiserableCondition) => Faulted,

    (Running, WorkerDeath) => [Running, Underprovisioned],
    (Running, WorkerAcked) => Running,
    (Running, MiserableCondition) => Faulted,

    (Underprovisioned, WorkerRequested) => Underprovisioned,
    (Underprovisioned, WorkerLaunched) => Underprovisioned,
    (Underprovisioned, WorkerAcked) => [Running, Underprovisioned],
    (Underprovisioned, WorkerLaunchFailure) => Faulted,
    (Underprovisioned, WorkerDeath) => [Underprovisioned, Faulted],
    (Underprovisioned, MiserableCondition) => Faulted
]);

impl Running {
    fn on_worker_death(self, d: WorkerDeath) -> WorkerSet {
        let mut state = self.state;
        if state.workers.delete_by_pid(d.0).is_some() {
            WorkerSet::underprovisioned(state)
        } else {
            WorkerSet::running(state)
        }
    }

    fn on_worker_acked(self, s: WorkerAcked) -> Running {
        let state = self.state;
        state.handle_ack(s.id, |state| Running { state }, |state| Running { state })
    }

    fn on_miserable_condition(self, _s: MiserableCondition) -> Faulted {
        let state = self.state;
        Faulted { state }
    }

    fn working(&self) -> bool {
        true
    }
}

impl Startup {
    fn on_worker_requested(self, r: WorkerRequested) -> Startup {
        let mut state = self.state;
        state.workers.register_worker(r.id);

        Startup { state }
    }

    fn on_worker_launched(self, r: WorkerLaunched) -> Startup {
        let mut state = self.state;
        state.workers.launched(r.id, r.pid);

        Startup { state }
    }

    fn on_worker_acked(self, s: WorkerAcked) -> WorkerSet {
        let state = self.state;
        state.handle_ack(s.id, WorkerSet::startup, WorkerSet::running)
    }

    fn on_worker_launch_failure(self, _t: WorkerLaunchFailure) -> Faulted {
        // TODO: mark worker as broken
        Faulted { state: self.state }
    }

    fn on_worker_death(self, d: WorkerDeath) -> WorkerSet {
        let mut state = self.state;
        if state.workers.delete_by_pid(d.0).is_some() {
            WorkerSet::faulted(state)
        } else {
            WorkerSet::startup(state)
        }
    }

    fn on_miserable_condition(self, _s: MiserableCondition) -> Faulted {
        let state = self.state;
        Faulted { state }
    }

    fn required_action(&self) -> Option<Todo> {
        if self
            .state
            .workers
            .all()
            .filter(|w| w.killed.is_none())
            .count()
            < self.state.config.count
        {
            Some(Todo::LaunchProcess)
        } else {
            None
        }
    }

    fn working(&self) -> bool {
        true
    }
}

impl Underprovisioned {
    fn on_worker_requested(self, r: WorkerRequested) -> Underprovisioned {
        let mut state = self.state;
        state.workers.register_worker(r.id);

        Underprovisioned { state }
    }

    fn on_worker_launched(self, r: WorkerLaunched) -> Underprovisioned {
        let mut state = self.state;
        state.workers.launched(r.id, r.pid);

        Underprovisioned { state }
    }

    fn on_worker_acked(self, s: WorkerAcked) -> WorkerSet {
        let state = self.state;
        state.handle_ack(s.id, WorkerSet::underprovisioned, WorkerSet::running)
    }

    fn on_worker_launch_failure(self, _t: WorkerLaunchFailure) -> Faulted {
        // TODO: mark worker as broken
        Faulted { state: self.state }
    }

    fn on_worker_death(self, d: WorkerDeath) -> WorkerSet {
        let mut state = self.state;
        if state.workers.delete_by_pid(d.0).is_some() {
            // TODO: treat this better with a circuit breaker (figure out what we want in the first place?)
        }
        WorkerSet::underprovisioned(state)
    }

    fn on_miserable_condition(self, _s: MiserableCondition) -> Faulted {
        let state = self.state;
        Faulted { state }
    }

    fn required_action(&self) -> Option<Todo> {
        if self
            .state
            .workers
            .all()
            .filter(|w| w.killed.is_none())
            .count()
            < self.state.config.count
        {
            Some(Todo::LaunchProcess)
        } else {
            None
        }
    }

    fn working(&self) -> bool {
        true
    }
}

impl WorkerSet {
    pub fn new(config: WorkerConfig) -> WorkerSet {
        let state = State {
            config,
            workers: Default::default(),
        };
        WorkerSet::Startup(Startup { state })
    }
}
