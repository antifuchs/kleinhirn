use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Config {
    pub supervisor: SupervisorConfig,
    pub worker: WorkerConfig,

    #[serde(skip)]
    pub base_dir: PathBuf,
}

impl Config {
    pub(crate) fn canonical_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.base_dir.join(path)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone)]
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
#[derive(Debug, PartialEq, Eq, Clone)]
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
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Program {
    pub cmdline: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Ruby {
    /// The pathname identifying the Gemfile for the bundled ruby
    /// application.
    pub gemfile: PathBuf,

    /// A ruby file that can be `load`ed.
    pub load: PathBuf,

    /// A ruby expression that each worker runs in order to start.
    pub start_expression: String,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum WorkerKind {
    /// Supervise a program that gets forked & exec'ed [`WorkerConfig.count`] times. This does
    /// not support any variable substitution
    /// or shell expansion.
    Program(Program),

    /// Supervise a bundled ruby program that can be preloaded. That
    /// bundle is expected to include the `kleinhirn_loader` gem,
    /// which is then launched via the following expression:
    ///
    /// ```sh
    /// bundle exec --gemfile=<gemfile_path> --keep-file-descriptors \
    ///             kleinhirn_loader -- <load> <start_expression>
    /// ```
    Ruby(Ruby),
}
