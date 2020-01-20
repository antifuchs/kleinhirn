use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq)]
pub struct Config {
    pub supervisor: SupervisorConfig,
    pub worker: WorkerConfig,
    // TODO: binding sockets
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq)]
pub struct SupervisorConfig {
    /// Name of the supervised service. Determines logging fields and defaults for the socket
    /// name.   
    pub name: String,

    /// UNIX domain socket on which to listen for commands. Defaults to "/tmp/kleinhirn-<name>.sock".
    /// Also creates a lock file next to the socket, in order to prevent multiple uncontrollable
    /// copies from being created.
    pub socket: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq)]
pub struct WorkerConfig {
    /// Number of workers to spawn. Default: 1
    #[serde(default = "default_count")]
    pub count: usize,

    #[serde(flatten)]
    pub kind: WorkerKind,
}

fn default_count() -> usize {
    1
}

#[derive(Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq)]
pub enum WorkerKind {
    /// Supervise a program that gets forked & exec'ed [`WorkerConfig.count`] times. This does
    /// not support any variable substitution
    /// or shell expansion.
    Program {
        /// The commandline as an array of arguments.
        cmdline: Vec<String>,

        /// Environment variables that should be set before the program spawns.
        #[serde(default)]
        env: HashMap<String, String>,

        /// The directory to chdir into before running the above commandline.
        cwd: Option<PathBuf>,
    },
    // TODO: Ruby, Python and the rest
}
