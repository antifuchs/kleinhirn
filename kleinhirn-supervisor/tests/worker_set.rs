use kleinhirn_supervisor::configuration;
use kleinhirn_supervisor::worker_set::{
    Todo, WorkerAcked, WorkerDeath, WorkerLaunched, WorkerRequested, WorkerSet,
};
use matches::assert_matches;
use nix::unistd::Pid;

#[must_use]
fn ack_n_workers(mut machine: WorkerSet, from: usize, n: usize) -> WorkerSet {
    for i in dbg!(from)..=from + n - 1 {
        let id = format!("i:{}", i);
        let pid = dbg!(i);
        assert_eq!(
            Some(Todo::LaunchProcess),
            machine.required_action().and_then(|todo| todo),
            "i: {:?} machine {:?}",
            i,
            machine
        );
        machine = machine.on_worker_requested(WorkerRequested::new(id.to_string()));
        machine = machine.on_worker_launched(WorkerLaunched::new(
            id.to_string(),
            Pid::from_raw(pid as i32),
        ));
        machine = machine.on_worker_acked(WorkerAcked::new(id.to_string()));
    }
    machine
}

#[test]
fn starts_workers_until_done() {
    let config = configuration::WorkerConfig {
        count: 3,
        kind: configuration::WorkerKind::Program(configuration::Program {
            cmdline: vec!["/bin/true".to_string()],
            ..Default::default()
        }),
    };
    let mut machine = WorkerSet::new(config);
    assert_matches!(&machine, &WorkerSet::Startup(_));
    assert_eq!(
        Some(Todo::LaunchProcess),
        machine.required_action().and_then(|todo| todo)
    );
    machine = ack_n_workers(machine, 1, 2);
    assert_matches!(&machine, &WorkerSet::Startup(_));
    machine = ack_n_workers(machine, 3, 1);

    assert_matches!(&machine, &WorkerSet::Running(_));
    assert_eq!(None, machine.required_action().and_then(|todo| todo));
}

#[test]
fn keeps_them_running() {
    let config = configuration::WorkerConfig {
        count: 3,
        kind: configuration::WorkerKind::Program(configuration::Program {
            cmdline: vec!["/bin/true".to_string()],
            ..Default::default()
        }),
    };
    let mut machine = WorkerSet::new(config);
    machine = ack_n_workers(machine, 1, 3);
    // kill the second worker:
    machine = machine.on_worker_death(WorkerDeath::new(Pid::from_raw(2)));
    assert_matches!(&machine, &WorkerSet::Underprovisioned(_));

    // start one up again:
    machine = ack_n_workers(machine, 4, 1);

    assert_matches!(&machine, &WorkerSet::Running(_));
    assert_eq!(None, machine.required_action().and_then(|todo| todo));
}

#[test]
fn no_problems_with_unrelated_pids() {
    let config = configuration::WorkerConfig {
        count: 3,
        kind: configuration::WorkerKind::Program(configuration::Program {
            cmdline: vec!["/bin/true".to_string()],
            ..Default::default()
        }),
    };
    let mut machine = WorkerSet::new(config);
    machine = ack_n_workers(machine, 1, 3);
    // kill the second worker:
    machine = machine.on_worker_death(WorkerDeath::new(Pid::from_raw(90)));
    assert_matches!(&machine, &WorkerSet::Running(_));
}
