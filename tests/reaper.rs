use anyhow::Result;
use kleinhirn::reaper::*;
use nix::unistd::Pid;
use rusty_fork::*;
use slog_scope::info;
use smol::{self, Timer};
use std::time::Duration;

fn fork_child() -> Result<Pid> {
    use nix::unistd::{fork, ForkResult};
    match fork() {
        Ok(ForkResult::Parent { child, .. }) => {
            info!("I'm the parent and that's ok"; "pid" => child.as_raw());
            return Ok(child);
        }
        Ok(ForkResult::Child) => {
            info!("I'm a new child process, exiting now!");
            std::process::exit(0);
        }
        Err(e) => {
            return Err(e.into());
        }
    };
}

rusty_fork_test! {
    #[test]
    fn returns_all_children() {
        let pid = fork_child().expect("0th fork");
        smol::run(async {
            let mut zombies = setup_child_exit_handler().expect("Should be able to setup");

            let child = zombies.reap().await.expect("end of stream");
            assert_eq!(child, pid);

            let pid = fork_child().expect("first fork");
            Timer::after(Duration::from_millis(100)).await; // XXX: not ideal that we're testing by sleep, but ugh.
            let child = zombies.reap().await.expect("end of stream");
            assert_eq!(child, pid);

            let pid = fork_child().expect("2nd fork");
            Timer::after(Duration::from_millis(100)).await;
            let child = zombies.reap().await.expect("end of stream");
            assert_eq!(child, pid);
        });
    }
}
