use crate::configuration::WorkerConfig;
use machine::*;
use nix::unistd::Pid;
use parking_lot::Mutex;
use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    sync::Arc,
    time::Instant,
};

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
        self.by_id.entry(id).and_modify(|w| {
            w.launched = Some(Instant::now());
            w.pid = Some(pid);
        });
    }

    fn acked(&mut self, id: String) {
        self.by_id.entry(id).and_modify(|w| {
            w.acked = Some(Instant::now());
        });
    }

    fn delete_by_pid(&mut self, pid: Pid) {
        if let Some(id) = self.by_pid.get(&pid) {
            self.by_id.remove(id);
        }
    }

    fn all<'a>(&'a self) -> impl Iterator<Item = &'a Worker> {
        self.by_id.values()
    }

    fn len(&self) -> usize {
        self.by_id.len()
    }

    fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct State {
    workers: Workers,
    config: WorkerConfig,

    #[deprecated(
        note = "the state machine should compute this on its own - no need to launch more when we're in Running, for ex."
    )]
    todo: Arc<Mutex<VecDeque<Todo>>>,
}

impl PartialEq for State {
    fn eq(&self, other: &State) -> bool {
        let todo1 = self.todo.lock();
        let todo2 = other.todo.lock();
        self.workers == other.workers && self.config == other.config && *todo1 == *todo2
    }
}

impl State {
    fn kill_all_workers(&mut self) {
        let mut todo = self.todo.lock();

        (*todo).clear();
        for worker in self.workers.all() {
            if let Some(pid) = worker.pid {
                (*todo).push_back(Todo::KillProcess(pid));
            }
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

    pub fn requested_workers(&self) -> usize {
        let todo = self.todo.lock();
        (*todo)
            .iter()
            .filter(|t| *t == &Todo::LaunchProcess)
            .count()
    }

    fn handle_ack<T>(
        mut self,
        id: String,
        self_state: fn(Self) -> T,
        done_state: fn(Self) -> T,
    ) -> T {
        self.workers.acked(id);

        if self.workers.len() >= self.config.count {
            done_state(self)
        } else {
            if self.requested_workers() + self.workers.len() < self.config.count {
                self.start_worker();
            }
            self_state(self)
        }
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
pub struct Terminate();

transitions!(WorkerSet,
  [
    (Startup, WorkerRequested) => Startup,
    (Startup, WorkerLaunched) => Startup,
    (Startup, WorkerAcked) => [Running, Startup],
    (Startup, WorkerDeath) => Faulted,
    (Startup, Terminate) => Terminating,

    (Running, WorkerDeath) => Underprovisioned,
    (Running, WorkerAcked) => Running,
    (Running, Terminate) => Terminating,

    (Underprovisioned, WorkerRequested) => Underprovisioned,
    (Underprovisioned, WorkerLaunched) => Underprovisioned,
    (Underprovisioned, WorkerAcked) => [Running, Underprovisioned],
    (Underprovisioned, WorkerDeath) => [Underprovisioned, Faulted],
    (Underprovisioned, Terminate) => Terminating,

    (Terminating, WorkerDeath) => [Terminating, Terminated]
  ]
);

impl Running {
    fn on_worker_death(self, d: WorkerDeath) -> Underprovisioned {
        let mut state = self.state;
        state.workers.delete_by_pid(d.0);
        state.start_worker();
        Underprovisioned { state }
    }

    fn on_worker_acked(self, s: WorkerAcked) -> Running {
        let state = self.state;
        state.handle_ack(s.id, |state| Running { state }, |state| Running { state })
    }

    fn on_terminate(self, _: Terminate) -> Terminating {
        let mut state = self.state;
        state.kill_all_workers();
        Terminating { state }
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

    fn on_worker_death(self, d: WorkerDeath) -> Faulted {
        let mut state = self.state;
        state.workers.delete_by_pid(d.0);
        Faulted { state }
    }

    fn on_terminate(self, _: Terminate) -> Terminating {
        let mut state = self.state;
        state.kill_all_workers();
        Terminating { state }
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

    fn on_worker_death(self, d: WorkerDeath) -> WorkerSet {
        let mut state = self.state;
        state.workers.delete_by_pid(d.0);
        state.start_worker();
        // TODO: treat this better with a circuit breaker (figure out what we want in the first place?)
        WorkerSet::underprovisioned(state)
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
        state.workers.delete_by_pid(d.0);

        if !state.workers.is_empty() {
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
            workers: Default::default(),
        };
        state.start_worker();
        WorkerSet::Startup(Startup { state })
    }
}
