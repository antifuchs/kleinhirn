use anyhow::Result;
use futures::stream::StreamExt;
use futures::Stream;
use kleinhirn::supervisor::reaper::*;
use nix::unistd::Pid;
use rusty_fork::*;
use slog_scope::info;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::time::delay_for;

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
        let mut rt = Runtime::new().expect("failed to setup runtime");
        let pid = fork_child().expect("0th fork");
        rt.block_on(async {
            let mut zombies = setup_child_exit_handler().expect("Should be able to setup");

            let child = zombies.next().await.expect("end of stream").expect("Waiting for child");
            assert_eq!(child, pid);

            let pid = fork_child().expect("first fork");
            delay_for(Duration::from_millis(100)).await; // XXX: not ideal that we're testing by sleep, but ugh.
            let child = zombies.next().await.expect("end of stream").expect("Waiting for child");
            assert_eq!(child, pid);

            let pid = fork_child().expect("2nd fork");
            delay_for(Duration::from_millis(100)).await;
            let child = zombies.next().await.expect("end of stream").expect("Waiting for child");
            assert_eq!(child, pid);
        });
    }
}
