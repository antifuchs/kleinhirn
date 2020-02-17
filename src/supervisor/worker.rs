#![allow(dead_code)] // TODO: use this.

use machine::*;
use nix::unistd::Pid;
use serde::Deserialize;

machine! {
    #[derive(Clone, Debug, PartialEq)]
    pub enum Worker {
        Starting,
        Up,
        Dead,
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Ack {
    id: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Failed {
    id: String,
    message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reaped {
    pid: Pid,
}

transitions!(Worker, [
    (Starting, Failed) => Dead,
    (Starting, Ack) => Up,

    (Up, Reaped) => Dead
]);

impl Starting {
    pub fn on_failed(self, _f: Failed) -> Dead {
        Dead {}
    }

    pub fn on_ack(self, _a: Ack) -> Up {
        Up {}
    }
}

impl Up {
    pub fn on_reaped(self, _r: Reaped) -> Dead {
        Dead {}
    }
}
